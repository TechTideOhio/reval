# AGENTS.md for Tools/reval

Instructions for AI coding agents operating on this repository:

- Pure Rust (2024 edition, MSRV 1.82). Zero FFI or C dependencies.
- Never introduce `.unwrap()` in production code under `src/`.
- Maintain `#![forbid(unsafe_code)]` in `lib.rs` and `main.rs`.
- Zero em-dashes (`\u2014`) and zero en-dashes (`\u2013`) across code, comments, and documentation.
- Verify with `cargo test` and run clippy with `-D warnings`.
