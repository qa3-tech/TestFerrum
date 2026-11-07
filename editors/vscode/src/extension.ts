// VS Code edge: effects and UI only. Tree logic lives in tree.ts, parsing in
// listing.ts and junit.ts, process handling in cargo.ts.

import * as vscode from "vscode";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import {
  binaryEnvironment,
  buildTestBinaries,
  CancelHook,
  findWorkspaceRoot,
  noCancel,
  parseBuildErrors,
  runProcess,
} from "./cargo";
import { parseJunit, TestCaseResult } from "./junit";
import { parseListing } from "./listing";
import {
  binaryId,
  buildTree,
  DiscoveredBinary,
  filterArguments,
  indexNodes,
  leafNodes,
  slowResults,
  testId,
  TestNode,
} from "./tree";

/** Everything discovery produces. Replaced as a whole after each discovery. */
interface ExtensionState {
  nodes: Map<string, TestNode>;
  items: Map<string, vscode.TestItem>;
  folders: Map<string, vscode.WorkspaceFolder>;
}

const emptyState = (): ExtensionState => ({ nodes: new Map(), items: new Map(), folders: new Map() });

function cancelWith(token: vscode.CancellationToken | undefined): CancelHook {
  if (token === undefined) {
    return noCancel;
  }
  return (kill) => {
    token.onCancellationRequested(kill);
  };
}

// ---- Discovery: effects ----

async function buildFolder(directory: string, output: vscode.OutputChannel, onCancel: CancelHook) {
  const { binaries, result } = await buildTestBinaries(directory, onCancel);
  if (result.exitCode !== 0) {
    output.appendLine(`Build failed in ${directory}:`);
    parseBuildErrors(result.stdout).forEach((error) => {
      output.appendLine(error);
    });
    output.appendLine(result.stderr);
  }
  return binaries;
}

async function listFolder(directory: string, output: vscode.OutputChannel, onCancel: CancelHook): Promise<DiscoveredBinary[]> {
  const binaries = await buildFolder(directory, output, onCancel);
  const discovered: DiscoveredBinary[] = [];
  for (const binary of binaries) {
    const listed = await runProcess(binary.executable, ["--list"], binary.packageDirectory, binaryEnvironment(binary), onCancel);
    discovered.push({ binary, listing: parseListing(listed.stdout) });
  }
  return discovered;
}

// ---- Discovery: VS Code items ----

function toTestItem(controller: vscode.TestController, node: TestNode): vscode.TestItem {
  const uri = node.file.length > 0 ? vscode.Uri.file(node.file) : undefined;
  const item = controller.createTestItem(node.id, node.label, uri);
  if (node.line > 0) {
    item.range = new vscode.Range(node.line - 1, 0, node.line - 1, 0);
  }
  if (node.error.length > 0) {
    item.error = node.error;
  }
  return item;
}

/** Creates items for the whole tree, parents before children, without recursion. */
function toTestItems(controller: vscode.TestController, roots: TestNode[]): Map<string, vscode.TestItem> {
  const items = new Map<string, vscode.TestItem>();
  const pending: { node: TestNode; parent: vscode.TestItem | undefined }[] = roots.map((node) => ({ node, parent: undefined }));
  for (let next = pending.pop(); next !== undefined; next = pending.pop()) {
    const { node, parent } = next;
    const item = toTestItem(controller, node);
    parent?.children.add(item);
    items.set(node.id, item);
    pending.push(...node.children.map((child) => ({ node: child, parent: item })));
  }
  return items;
}

async function discover(
  controller: vscode.TestController,
  output: vscode.OutputChannel,
  token: vscode.CancellationToken | undefined,
): Promise<ExtensionState> {
  const roots: TestNode[] = [];
  const folders = new Map<string, vscode.WorkspaceFolder>();
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    const directory = folder.uri.fsPath;
    if (!fs.existsSync(path.join(directory, "Cargo.toml"))) {
      continue;
    }
    const discovered = await listFolder(directory, output, cancelWith(token));
    const folderRoots = buildTree(await findWorkspaceRoot(directory), discovered);
    folderRoots.forEach((root) => {
      folders.set(root.id, folder);
    });
    roots.push(...folderRoots);
  }
  const items = toTestItems(controller, roots);
  controller.items.replace(roots.flatMap((root) => items.get(root.id) ?? []));
  return { nodes: indexNodes(roots), items, folders };
}

// ---- Running ----

function debugBinary(folder: vscode.WorkspaceFolder | undefined, node: TestNode, args: string[]): Promise<void> {
  const sessionName = `TestFerrum: ${node.label} ${String(Date.now())}`;
  return new Promise((resolve) => {
    const listener = vscode.debug.onDidTerminateDebugSession((session) => {
      if (session.configuration.name === sessionName) {
        listener.dispose();
        resolve();
      }
    });
    const configuration: vscode.DebugConfiguration = {
      type: "lldb",
      request: "launch",
      name: sessionName,
      program: node.binary.executable,
      args,
      cwd: node.binary.packageDirectory,
      env: binaryEnvironment(node.binary),
    };
    void vscode.debug.startDebugging(folder, configuration).then((started) => {
      if (!started) {
        listener.dispose();
        resolve();
      }
    });
  });
}

function report(run: vscode.TestRun, state: ExtensionState, node: TestNode, result: TestCaseResult): void {
  const item = state.items.get(testId(node.binary, result.suite, result.name));
  if (item === undefined) {
    return;
  }
  if (result.status === "passed") {
    run.passed(item, result.durationMs);
  } else if (result.status === "skipped") {
    run.skipped(item);
  } else {
    const message = new vscode.TestMessage(result.details.length > 0 ? result.details : result.message);
    if (item.uri !== undefined && item.range !== undefined) {
      message.location = new vscode.Location(item.uri, item.range);
    }
    run.failed(item, message, result.durationMs);
  }
}

function slowThresholdMs(): number {
  return vscode.workspace.getConfiguration("testferrum").get<number>("slowThresholdMs", 0);
}

async function runNode(
  run: vscode.TestRun,
  state: ExtensionState,
  node: TestNode,
  shouldDebug: boolean,
  token: vscode.CancellationToken,
): Promise<void> {
  const leaves = leafNodes(node).flatMap((leaf) => state.items.get(leaf.id) ?? []);
  leaves.forEach((leaf) => {
    run.started(leaf);
  });

  const junitPath = path.join(os.tmpdir(), `testferrum-${String(process.pid)}-${String(Date.now())}.xml`);
  const args = [...filterArguments(node), "--junit", junitPath];

  let failureText = "";
  if (shouldDebug) {
    await debugBinary(state.folders.get(binaryId(node.binary)), node, args);
  } else {
    const result = await runProcess(node.binary.executable, args, node.binary.packageDirectory, binaryEnvironment(node.binary), cancelWith(token));
    run.appendOutput((result.stdout + result.stderr).replace(/\r?\n/g, "\r\n"));
    failureText = result.stderr;
  }

  if (!fs.existsSync(junitPath)) {
    const message = new vscode.TestMessage(failureText.length > 0 ? failureText : "The test binary stopped before writing results.");
    leaves.forEach((leaf) => {
      run.errored(leaf, message);
    });
    return;
  }
  const results = parseJunit(fs.readFileSync(junitPath, "utf8"));
  fs.rmSync(junitPath, { force: true });
  results.forEach((result) => {
    report(run, state, node, result);
  });

  const threshold = slowThresholdMs();
  slowResults(results, threshold).forEach((result) => {
    run.appendOutput(`Slow: ${result.suite} › ${result.name} took ${result.durationMs.toFixed(0)} ms (> ${String(threshold)} ms)\r\n`);
  });
}

// ---- Activation: the single mutable state cell lives here ----

export function activate(context: vscode.ExtensionContext): void {
  const controller = vscode.tests.createTestController("testferrum", "TestFerrum");
  const output = vscode.window.createOutputChannel("TestFerrum");
  context.subscriptions.push(controller, output);

  let state = emptyState();
  const refresh = async (token: vscode.CancellationToken | undefined): Promise<ExtensionState> => {
    state = await discover(controller, output, token);
    return state;
  };

  const runHandler = async (shouldDebug: boolean, request: vscode.TestRunRequest, token: vscode.CancellationToken) => {
    const requestedIds: string[] = [];
    if (request.include === undefined) {
      controller.items.forEach((item) => {
        requestedIds.push(item.id);
      });
    } else {
      request.include.forEach((item) => {
        requestedIds.push(item.id);
      });
    }
    const excludedIds = new Set((request.exclude ?? []).map((item) => item.id));

    // Rebuild and relist so runs always use current code.
    const current = await refresh(token);
    const run = controller.createTestRun(request);
    for (const id of requestedIds) {
      const node = current.nodes.get(id);
      if (token.isCancellationRequested || node === undefined || excludedIds.has(id)) {
        continue;
      }
      await runNode(run, current, node, shouldDebug, token);
    }
    run.end();
  };

  controller.resolveHandler = async (item) => {
    if (item === undefined) {
      await refresh(undefined);
    }
  };
  controller.refreshHandler = async (token) => {
    await refresh(token);
  };
  controller.createRunProfile("Run", vscode.TestRunProfileKind.Run, (request, token) => runHandler(false, request, token), true);
  controller.createRunProfile("Debug", vscode.TestRunProfileKind.Debug, (request, token) => runHandler(true, request, token), true);

  let pending: NodeJS.Timeout | undefined;
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (document.languageId !== "rust") {
        return;
      }
      clearTimeout(pending);
      pending = setTimeout(() => {
        void refresh(undefined);
      }, 1000);
    }),
  );
}

export function deactivate(): void {
  // Nothing to clean up: subscriptions are disposed by VS Code.
}
