# TestFerrum for VS Code

Run and debug TestFerrum suites from the Test Explorer.

## Requirements

- [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb), installed automatically as a dependency
- `cargo` on your `PATH`

## How it works

1. **Discover:** the extension builds your test binaries with `cargo test --no-run`, then runs each one with `--list`. Binaries that answer with the TestFerrum header appear in the Test Explorer, with run and debug buttons at the line where each suite and test is declared.
2. **Run:** it starts the binary with `--suite` or `--test` filters and `--junit`, then reads the report to mark each test passed, failed or skipped.
3. **Debug:** it launches the same binary and filters under CodeLLDB, so breakpoints work as in any Rust program.

Tests refresh when you save a Rust file. Use the refresh button in the Test Explorer to force a rebuild.

Duplicate suite or test names show as errors on the suite.

## Settings

| Setting | Default | Effect |
|---|---|---|
| `testferrum.slowThresholdMs` | `0` (off) | Lists tests slower than this many milliseconds in the test output |

## Build locally

```bash
npm ci
npm run lint      # ESLint, strict type-checked rules
npm test          # compile and run parser checks
npm run package   # produces testferrum-<version>.vsix
code --install-extension testferrum-<version>.vsix
```
