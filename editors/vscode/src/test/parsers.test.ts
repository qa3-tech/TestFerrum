import { test } from "node:test";
import * as assert from "node:assert/strict";
import { parseListing, unescapeField } from "../listing";
import { parseJunit } from "../junit";
import { parseTestBinaries } from "../cargo";
import { buildTree, filterArguments, indexNodes, leafNodes, slowResults } from "../tree";

/** Fails the test when a value is missing, and narrows its type otherwise. */
function present<T>(value: T | undefined): T {
  assert.ok(value !== undefined, "expected a value");
  return value;
}

void test("parses a listing with escapes and errors", () => {
  const listing = parseListing(
    "testferrum-list 1\nsuite\tMath\ttests/math.rs\t8\ntest\tMath\ttab\\there\ttests/math.rs\t9\nerror\tMath\tduplicate test name: adds\n",
  );
  assert.equal(listing.isTestFerrum, true);
  assert.deepEqual(listing.suites, [
    {
      name: "Math",
      file: "tests/math.rs",
      line: 8,
      tests: [{ name: "tab\there", file: "tests/math.rs", line: 9 }],
      errors: ["duplicate test name: adds"],
    },
  ]);
});

void test("rejects output without the header", () => {
  assert.equal(parseListing("adds: test\n").isTestFerrum, false);
});

void test("unescapes backslashes", () => {
  assert.equal(unescapeField("a\\\\b\\nc"), "a\\b\nc");
});

void test("reads junit results", () => {
  const xml = `<testsuites>
  <testsuite name="Math &amp; More" tests="3">
    <testcase name="adds" time="0.001"/>
    <testcase name="later" time="0">
      <skipped message="not &quot;ready&quot;"/>
    </testcase>
    <testcase name="breaks" time="0.002">
      <failure message="sum" type="AssertionError">  sum
    Expected: 4</failure>
    </testcase>
  </testsuite>
</testsuites>`;
  const results = parseJunit(xml);
  assert.deepEqual(
    results.map((result) => [result.suite, result.name, result.status, result.message]),
    [
      ["Math & More", "adds", "passed", ""],
      ["Math & More", "later", "skipped", 'not "ready"'],
      ["Math & More", "breaks", "failed", "sum"],
    ],
  );
  assert.match(present(results[2]).details, /Expected: 4/);
});

void test("finds test executables in cargo output", () => {
  const output = [
    '{"reason":"compiler-artifact","profile":{"test":true},"executable":"/t/math-1","target":{"name":"math"},"manifest_path":"/p/Cargo.toml"}',
    '{"reason":"compiler-artifact","profile":{"test":false},"executable":null,"target":{"name":"lib"},"manifest_path":"/p/Cargo.toml"}',
    '{"reason":"build-finished","success":true}',
  ].join("\n");
  assert.deepEqual(parseTestBinaries(output), [{ targetName: "math", executable: "/t/math-1", packageDirectory: "/p" }]);
});

const binary = { targetName: "math", executable: "/t/math-1", packageDirectory: "/p" };
const listing = parseListing(
  "testferrum-list 1\nsuite\tMath\ttests/math.rs\t8\ntest\tMath\tadds\ttests/math.rs\t9\ntest\tMath\tsubtracts\t/abs/other.rs\t3\n",
);

void test("builds a tree with absolute files and skips non-TestFerrum binaries", () => {
  const roots = buildTree("/workspace", [
    { binary, listing },
    { binary: { ...binary, targetName: "unit" }, listing: { isTestFerrum: false, suites: [] } },
  ]);
  assert.equal(roots.length, 1);
  const root = present(roots[0]);
  const suite = present(root.children[0]);
  const [adds, subtracts] = suite.children;
  assert.equal(suite.file, "/workspace/tests/math.rs");
  assert.equal(present(subtracts).file, "/abs/other.rs");
  assert.deepEqual(filterArguments(root), []);
  assert.deepEqual(filterArguments(suite), ["--suite", "Math"]);
  assert.deepEqual(filterArguments(present(adds)), ["--test", "Math", "adds"]);
  assert.equal(indexNodes(roots).size, 4);
  assert.deepEqual(leafNodes(root).map((node) => node.label).sort(), ["adds", "subtracts"]);
});

void test("reports slow results only when a threshold is set", () => {
  const results = parseJunit(
    `<testsuite name="Math"><testcase name="adds" time="0.600"/><testcase name="later" time="0"><skipped message="x"/></testcase></testsuite>`,
  );
  assert.equal(slowResults(results, 0).length, 0);
  assert.deepEqual(slowResults(results, 500).map((result) => result.name), ["adds"]);
});
