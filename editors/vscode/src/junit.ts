// Reads the JUnit XML that TestFerrum writes. TestFerrum controls the exact shape,
// so a small reader covers it without an XML dependency.

export type TestStatus = "passed" | "failed" | "skipped";

export interface TestCaseResult {
  suite: string;
  name: string;
  status: TestStatus;
  durationMs: number;
  message: string;
  details: string;
}

export function unescapeXml(text: string): string {
  return text
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, "&");
}

/** A capture group, or "" when the group did not match. */
function group(match: RegExpMatchArray | RegExpExecArray, index: number): string {
  return match[index] ?? "";
}

function parseTestCase(suite: string, name: string, seconds: string, body: string): TestCaseResult {
  const failure = /<failure message="([^"]*)"[^>]*>([\s\S]*?)<\/failure>/.exec(body);
  const skipped = /<skipped message="([^"]*)"\/>/.exec(body);
  const durationMs = Number(seconds) * 1000;
  if (failure !== null) {
    return {
      suite,
      name,
      status: "failed",
      durationMs,
      message: unescapeXml(group(failure, 1)),
      details: unescapeXml(group(failure, 2)),
    };
  }
  if (skipped !== null) {
    return { suite, name, status: "skipped", durationMs, message: unescapeXml(group(skipped, 1)), details: "" };
  }
  return { suite, name, status: "passed", durationMs, message: "", details: "" };
}

export function parseJunit(xml: string): TestCaseResult[] {
  const results: TestCaseResult[] = [];
  const suitePattern = /<testsuite name="([^"]*)"[^>]*>([\s\S]*?)<\/testsuite>/g;
  const casePattern = /<testcase name="([^"]*)" time="([^"]*)"(?:\/>|>([\s\S]*?)<\/testcase>)/g;
  for (const suiteMatch of xml.matchAll(suitePattern)) {
    const suite = unescapeXml(group(suiteMatch, 1));
    for (const caseMatch of group(suiteMatch, 2).matchAll(casePattern)) {
      results.push(parseTestCase(suite, unescapeXml(group(caseMatch, 1)), group(caseMatch, 2), group(caseMatch, 3)));
    }
  }
  return results;
}
