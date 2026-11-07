// Parses the `--list` output of a TestFerrum binary (format version 1).

export const LIST_HEADER = "testferrum-list 1";

export interface ListedTest {
  name: string;
  file: string;
  line: number;
}

export interface ListedSuite {
  name: string;
  file: string;
  line: number;
  tests: ListedTest[];
  errors: string[];
}

export interface Listing {
  isTestFerrum: boolean;
  suites: ListedSuite[];
}

const ESCAPES: Record<string, string> = { "\\": "\\", t: "\t", n: "\n", r: "\r" };

/** Reverses the escaping TestFerrum applies to list fields. */
export function unescapeField(text: string): string {
  let result = "";
  let index = 0;
  while (index < text.length) {
    const character = text.charAt(index);
    const next = text.charAt(index + 1);
    if (character === "\\" && next !== "") {
      result += ESCAPES[next] ?? next;
      index += 2;
    } else {
      result += character;
      index += 1;
    }
  }
  return result;
}

/** Binaries that do not print the header (such as libtest binaries) are not TestFerrum. */
export function parseListing(output: string): Listing {
  const lines = output.split(/\r?\n/).filter((line) => line.length > 0);
  if (lines[0] !== LIST_HEADER) {
    return { isTestFerrum: false, suites: [] };
  }

  const suites: ListedSuite[] = [];
  for (const line of lines.slice(1)) {
    const fields = line.split("\t").map(unescapeField);
    const [kind = "", first = "", second = "", third = "", fourth = ""] = fields;
    const current = suites.at(-1);
    if (kind === "suite" && fields.length >= 4) {
      suites.push({ name: first, file: second, line: Number(third), tests: [], errors: [] });
    } else if (kind === "test" && fields.length >= 5 && current !== undefined) {
      current.tests.push({ name: second, file: third, line: Number(fourth) });
    } else if (kind === "error" && fields.length >= 3) {
      const target = suites.find((suite) => suite.name === first) ?? current;
      target?.errors.push(second);
    }
  }
  return { isTestFerrum: true, suites };
}
