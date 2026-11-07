// Runs cargo and test binaries. No VS Code dependencies, so it stays easy to test.

import { spawn } from "node:child_process";
import * as path from "node:path";

export interface ProcessResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export interface TestBinary {
  targetName: string;
  executable: string;
  packageDirectory: string;
}

/** Called with a function that stops the process, so callers can wire up cancellation. */
export type CancelHook = (kill: () => void) => void;

export const noCancel: CancelHook = () => undefined;

/** The parts of cargo's JSON messages this extension reads. */
interface CargoMessage {
  reason?: string;
  executable?: string | null;
  profile?: { test?: boolean };
  target?: { name?: string };
  manifest_path?: string;
  message?: { level?: string; rendered?: string | null; message?: string };
  workspace_root?: string;
}

function parseCargoMessages(cargoOutput: string): CargoMessage[] {
  return cargoOutput
    .split(/\r?\n/)
    .filter((line) => line.startsWith("{"))
    .map((line) => JSON.parse(line) as CargoMessage);
}

export function runProcess(
  command: string,
  args: string[],
  workingDirectory: string,
  extraEnvironment: Record<string, string>,
  onCancel: CancelHook,
): Promise<ProcessResult> {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: workingDirectory,
      env: { ...process.env, ...extraEnvironment },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString();
    });
    child.on("error", (error) => {
      resolve({ exitCode: -1, stdout, stderr: stderr + error.message });
    });
    child.on("close", (code) => {
      resolve({ exitCode: code ?? -1, stdout, stderr });
    });
    onCancel(() => child.kill());
  });
}

/** Picks test executables out of `cargo test --no-run --message-format=json` output. */
export function parseTestBinaries(cargoOutput: string): TestBinary[] {
  const binaries: TestBinary[] = [];
  for (const message of parseCargoMessages(cargoOutput)) {
    const isTestArtifact = message.reason === "compiler-artifact" && message.profile?.test === true;
    const { executable, target, manifest_path: manifestPath } = message;
    if (isTestArtifact && typeof executable === "string" && target?.name !== undefined && manifestPath !== undefined) {
      binaries.push({ targetName: target.name, executable, packageDirectory: path.dirname(manifestPath) });
    }
  }
  return binaries;
}

/** Rendered compiler errors from cargo's JSON output, for showing a failed build. */
export function parseBuildErrors(cargoOutput: string): string[] {
  return parseCargoMessages(cargoOutput)
    .filter((message) => message.reason === "compiler-message" && message.message?.level === "error")
    .map((message) => message.message?.rendered ?? message.message?.message ?? "unknown compiler error");
}

export async function buildTestBinaries(
  workspaceDirectory: string,
  onCancel: CancelHook,
): Promise<{ binaries: TestBinary[]; result: ProcessResult }> {
  const result = await runProcess("cargo", ["test", "--no-run", "--message-format=json"], workspaceDirectory, {}, onCancel);
  return { binaries: parseTestBinaries(result.stdout), result };
}

export async function findWorkspaceRoot(directory: string): Promise<string> {
  const result = await runProcess("cargo", ["metadata", "--format-version", "1", "--no-deps"], directory, {}, noCancel);
  if (result.exitCode !== 0) {
    return directory;
  }
  const metadata = JSON.parse(result.stdout) as CargoMessage;
  return metadata.workspace_root ?? directory;
}

/** Environment a test binary expects when cargo is not the one starting it. */
export function binaryEnvironment(binary: TestBinary): Record<string, string> {
  return { CARGO_MANIFEST_DIR: binary.packageDirectory };
}
