# Contributing to Lit

Thank you for your interest in contributing to Lit. This document covers the process for contributing to this project.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/<you>/Lit.git`
3. Create a branch: `git checkout -b my-feature`
4. Make your changes
5. Submit a pull request

## Development Setup

```bash
# Build
cargo build

# Run tests
cargo test --lib -- --test-threads=1

# Check formatting
cargo fmt -- --check

# Run linter
cargo clippy -- -D warnings

# Generate docs
cargo doc --no-deps
```

## Requirements

All contributions must:

- **Pass CI** — `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` must all pass
- **Include tests** — new features need unit tests; bug fixes need regression tests
- **Maintain security** — this is a security-focused tool; follow Rust security best practices
- **Add audit logging** — operations that modify repository state must emit audit log entries
- **Preserve structured output** — all commands must return valid JSON by default

## Code Style

- Run `cargo fmt` before committing
- No clippy warnings (`-D warnings`)
- Keep functions focused and small
- Use typed errors (`LitError`), not `unwrap()` or `expect()` in library code

## Security

If you discover a security vulnerability, **do not** open a public issue. See [SECURITY.md](SECURITY.md) for responsible disclosure instructions.

## License

By contributing, you agree that your contributions will be licensed under the [AGPL-3.0-or-later](LICENSE) license.
