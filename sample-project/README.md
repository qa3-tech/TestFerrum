# Sample Project

A small library tested with TestFerrum.

```bash
cargo test                                         # unit tests + TestFerrum target
cargo test --test calculator                       # TestFerrum target only
cargo test --test calculator -- --parallel
cargo test --test calculator -- --junit results.xml
```

The test file is its own binary: `harness = false` in `Cargo.toml` hands `main` to TestFerrum.
