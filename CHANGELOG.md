# Changelog

All notable changes to Lit will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-03-31

### Added

- **42 CLI commands** — full Git-equivalent workflow plus agent-native extensions (init, add, commit, status, log, diff, show, branch, checkout, merge, resolve, tag, stash, reset, revert, cherry-pick, rebase, blame, bisect, reflog, remote, push, pull, clone, fetch, batch, serve, mcp-serve, tx, snapshot, search, watch, ontology, schema, lfs, verify, gc, import-git, export-git, rotate-key, config, sandbox, swarm)
- **30 MCP tools** — LLM agents interact via Model Context Protocol tool calls
- **4 transport protocols** — HTTPS, SSH, `lit://` (custom TCP), stdio pipe
- **Structured JSON output** — all commands emit machine-readable JSON by default
- **Post-quantum signatures** — ML-DSA-87 (NIST FIPS 204, Security Level 5)
- **FIPS 140-3 compliance** — AES-256-GCM, PBKDF2-HMAC-SHA512 (600K iterations), automatic Known Answer Tests at startup
- **Sandboxed execution** — process isolation with filesystem, environment, and network fences; symlink protection
- **Swarm coordination** — multi-agent registration, file leasing, and conflict-free concurrent access
- **Airgap mode** — complete network isolation for classified environments (USB, file shares only)
- **Atomic transactions** — begin/commit/rollback for multi-operation sequences
- **Large File Storage** — LFS tracking and migration for binary assets
- **Git interop** — bidirectional import/export with existing Git repositories
- **Command ontology** — machine-readable type graph for agent discovery and SDK generation
- **JSON Schema** — auto-generated draft 2020-12 schemas for all command inputs/outputs
- **Per-IP rate limiting** — sliding window throttle (100 req/60s) on all server endpoints
- **Tamper-evident audit logs** — HMAC-SHA256 signed, append-only operation logs
- **3-way merge** with structured conflict objects and programmatic resolution
- **REST API server** with bearer token authentication
- **MCP server** with stdio and HTTP transports
- **`lit://` TCP daemon** for LAN deployments
- **428 tests** — unit, command, integration, performance, adversarial, concurrency, and network suites
- **CI/CD** — GitHub Actions for testing (Linux/macOS/Windows), nightly security, and release builds (5 targets)
- **Cross-platform** — Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)
- **Documentation website** — Next.js 14 static site with 18 documentation pages, dark mode
- **Install scripts** — `install.sh` (Linux/macOS) and `install.ps1` (Windows)

[1.0.0]: https://github.com/nervosys/Lit/releases/tag/v1.0.0
