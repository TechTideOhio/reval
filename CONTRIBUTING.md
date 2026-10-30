# Contributing to reval

Thank you for your interest in contributing to `reval`!

`reval` is a core toolchain component of **TideOS** by [TechTide AI](https://techtideai.io). We prioritize deterministic evaluation, zero external C/FFI dependencies, and sub-second CLI execution.

---

## Design Principles

1. **Pure Rust / Zero-FFI**: Every dependency must compile without `-sys` C bindings or native toolchain drift. It must cross-compile cleanly across Windows, Linux, and macOS.
2. **Deterministic & Reproducible**: No non-deterministic LLM judges in gate paths. All checks and error analysis must produce identical, byte-for-byte SHA-256 verifiable proofs for identical inputs.
3. **Fail-Closed Security**: Blocker violations (credential leaks, secret patterns, prohibited paths) immediately result in `score = 0.0`.
4. **Minimalistic**: Follow YAGNI strictly. Prefer small, composable CLI commands over complex configuration layers.

---

## Local Development Workflow

### Prerequisites

- Rust 1.82+ (2024 edition compatible)
- Node.js 18+ (for running the quiet test runner)

### Running Checks

Before submitting a Pull Request, all four gates must pass:

```bash
# 1. Check formatting
cargo fmt --all -- --check

# 2. Strict Clippy lints (warnings treated as errors)
cargo clippy --all-targets -- -D warnings

# 3. Unit, property, and integration test suite
cargo test --all-targets

# 4. Release compilation
cargo build --release
```

If you are developing inside the TechTide workspace, run all checks through the quiet runner:

```bash
node Tools/agent-harness/q.mjs -- cargo fmt --manifest-path Tools/reval/Cargo.toml --all -- --check
node Tools/agent-harness/q.mjs -- cargo clippy --manifest-path Tools/reval/Cargo.toml --all-targets -- -D warnings
node Tools/agent-harness/q.mjs -- cargo test --manifest-path Tools/reval/Cargo.toml --all-targets
node Tools/agent-harness/q.mjs -- cargo build --manifest-path Tools/reval/Cargo.toml --release
```

---

## Pull Request Guidelines

1. **Keep PRs focused**: One bug fix or feature per PR.
2. **Add tests**: Every new check, parser, or coding rule must include corresponding unit tests and property tests.
3. **Preserve documentation**: Keep doc comments and README synchronized with CLI changes.
4. **No `.unwrap()` in production code**: Use `RevalError` and `?` operator or provide fallback defaults.
