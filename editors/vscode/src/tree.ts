// Pure test-tree logic: turns listings into nodes, with no VS Code dependency.

import * as path from "node:path";
import { TestBinary } from "./cargo";
import { TestCaseResult } from "./junit";
import { Listing } from "./listing";

export type NodeKind = "binary" | "suite" | "test";

export interface TestNode {
  id: string;
  label: string;
  kind: NodeKind;
  binary: TestBinary;
  suite: string;
  test: string;
  file: string;
  line: number;
  error: string;
  children: TestNode[];
}

export interface DiscoveredBinary {
  binary: TestBinary;
  listing: Listing;
}

export function binaryId(binary: TestBinary): string {
  return `${binary.packageDirectory}|${binary.targetName}`;
}

export function suiteId(binary: TestBinary, suite: string): string {
  return `${binaryId(binary)}/${suite}`;
}

export function testId(binary: TestBinary, suite: string, test: string): string {
  return `${suiteId(binary, suite)}/${test}`;
}

export function absoluteFile(workspaceRoot: string, file: string): string {
  return path.isAbsolute(file) ? file : path.join(workspaceRoot, file);
}

/** One node per TestFerrum binary, with its suites and tests below it. */
export function buildTree(workspaceRoot: string, discovered: DiscoveredBinary[]): TestNode[] {
  return discovered
    .filter((entry) => entry.listing.isTestFerrum)
    .map(({ binary, listing }) => ({
      id: binaryId(binary),
      label: binary.targetName,
      kind: "binary" as const,
      binary,
      suite: "",
      test: "",
      file: "",
      line: 0,
      error: "",
      children: listing.suites.map((suite) => ({
        id: suiteId(binary, suite.name),
        label: suite.name,
        kind: "suite" as const,
        binary,
        suite: suite.name,
        test: "",
        file: absoluteFile(workspaceRoot, suite.file),
        line: suite.line,
        error: suite.errors.join("\n"),
        children: suite.tests.map((test) => ({
          id: testId(binary, suite.name, test.name),
          label: test.name,
          kind: "test" as const,
          binary,
          suite: suite.name,
          test: test.name,
          file: absoluteFile(workspaceRoot, test.file),
          line: test.line,
          error: "",
          children: [],
        })),
      })),
    }));
}

/** Every node by id, collected without recursion. */
export function indexNodes(roots: TestNode[]): Map<string, TestNode> {
  const index = new Map<string, TestNode>();
  const pending = [...roots];
  for (let node = pending.pop(); node !== undefined; node = pending.pop()) {
    index.set(node.id, node);
    pending.push(...node.children);
  }
  return index;
}

/** The test nodes under a node, or the node itself if it is a test. */
export function leafNodes(node: TestNode): TestNode[] {
  const leaves: TestNode[] = [];
  const pending = [node];
  for (let next = pending.pop(); next !== undefined; next = pending.pop()) {
    if (next.children.length === 0) {
      leaves.push(next);
    } else {
      pending.push(...next.children);
    }
  }
  return leaves;
}

export function filterArguments(node: TestNode): string[] {
  const filters: Record<NodeKind, string[]> = {
    binary: [],
    suite: ["--suite", node.suite],
    test: ["--test", node.suite, node.test],
  };
  return filters[node.kind];
}

/** Results slower than the threshold. A threshold of 0 or less turns the check off. */
export function slowResults(results: TestCaseResult[], thresholdMs: number): TestCaseResult[] {
  if (thresholdMs <= 0) {
    return [];
  }
  return results.filter((result) => result.status !== "skipped" && result.durationMs > thresholdMs);
}
