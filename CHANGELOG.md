# Changelog

All notable changes to Lit will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2026-08-05

### Security

- **AES-GCM nonces could repeat across processes** — the nonce was an engine-local counter in its top 8 bytes plus 4 random bytes, documented as guaranteeing uniqueness. The counter restarted at zero for every engine, meaning every process and every store or index opened within one, so the first encryption after each start reused counter 0 and only 32 random bits separated two nonces — a birthday collision over roughly 65,000 encryptions. A repeated nonce under one AES-GCM key leaks the XOR of the plaintexts and exposes the GHASH key, permitting forgery. The nonce is now 96 random bits (NIST SP 800-38D §8.2.2). Data written under the old scheme still decrypts, since the nonce travels with the ciphertext. A regression test simulates 64 process starts and fails if the leading bytes repeat — under the old construction all 64 began with eight zero bytes

### Added

- **At-rest encryption is connected to the commands** — AES-256-GCM object encryption was implemented and unit-tested but unreachable: every command built its store with `ObjectStore::new`, which read `.lit/encryption.toml` and never supplied a passphrase, and `Index::load`/`save` did the same. Setting `enabled = true` made `add` and `commit` fail rather than encrypt. The passphrase is now taken from `LIT_PASSPHRASE`, `LIT_PASSPHRASE_FILE` or the cache — never by prompting — and `gc` packs through the same cipher, so packing an encrypted repository does not put its contents on disk in the clear

### Fixed

- **`gc` no longer makes a repository unreadable** — it packed loose objects, deleted the loose copies and reported success, but nothing could read a pack back: `ObjectStore::read` looked only at the loose path, `list()` walked only the loose shards, and no pack reader was wired in. After a `gc`, `show`, `diff` and `checkout` failed and `export-git` reported "Exported 0 objects" with an ok status. `ObjectStore` now falls back to the pack indexes for reads, existence checks and listing
- **`checkout` corrupted the index in any repository with subdirectories** — the index was rebuilt from the root tree's entries alone, keyed by entry name, so a subdirectory was recorded as though it were a file and the blobs beneath it got no entry. `status` then called `fs::read` on a directory and failed outright. The index is now filled by the same recursion that writes the files
- **`lit show HEAD` reported the object as missing** — `HEAD` had no case in the resolver, so it fell through and was treated as a literal hash. The same resolver gated its hash branch on a length of 64 while a Lit hash is 192 characters, so that arm never ran and hashes worked only by falling through unchanged
- **Enabling encryption on an existing repository now explains itself** — it surfaced as `Unsupported encryption version: 123` internally (123 being the `{` of the plaintext index) and a bare "Operation failed" to the user, since rendered messages are deliberately sanitized. Both this and a locked repository now carry suggestions saying what to do
- **`.gitignore` re-included secret file types under `docs/`** — the secrets block ended with `!docs/**`, and a negation re-includes what it matches, so `docs/anything.key` was committable. Narrowed to markdown

### Changed

- **The encryption key is derived once per process** — a command opens several stores and each one ran PBKDF2's 600,000 iterations, so one `lit status` paid the cost several times over. Successful derivations are now held in memory for the life of the process, keyed by key file and passphrase together: a second initialization drops from 9.15s to 64µs in a debug build. The cache never touches disk, and because only successful derivations are stored a wrong passphrase still meets the rate limiter
- `cargo test` runs **420 tests** (plus 2 ignored), the six added here covering the pack reader, the checkout index, `show HEAD`, the encrypted lifecycle, the migration failure and AES-GCM nonce uniqueness

## [1.0.2] - 2026-08-04

### Added

- **Git pack delta resolution** — `import-git` reconstructs `OFS_DELTA` and `REF_DELTA` pack entries instead of discarding them, so packed repositories import in full. Delta chains resolve to any depth, and resolution is order-independent
- **Annotated tag import** — `import-git` converts Git annotated tags into Lit tag objects, keeping their tagger, message and own identity; previously they were stored as opaque blobs and could not be re-exported
- **Hash-preserving Git round-trip** — a repository taken through `import-git` and back out through `export-git` now reproduces every object under its original SHA-1 — blobs, trees, commits and annotated tags alike — and the result passes `git fsck --strict`. Verified by tests that build delta-compressed and tag-carrying repositories with the Git CLI and compare object sets
- Referential-integrity tests for both directions of Git interop, walking the converted graph from every ref and asserting that each referenced object resolves
- Unit tests for the delta decoder, covering copy and insert instructions, the 64K zero-size encoding, and each malformed-input rejection

### Fixed

- **Git interop no longer writes unresolvable hashes** — `export-git` and `import-git` converted objects in filesystem order, so a tree could be written before the blobs it names. Both sides filled the gap with a fabricated hash (the first 40 characters of the Lit hash on export, a zero-padded Git hash on import), silently producing repositories whose trees and commits pointed at objects that were never stored. Both now convert the object graph in dependency order and report an incomplete graph instead of encoding one
- **`export-git` could hang while serializing a tree** — the SHA-1 padding loop in `serialize_git_tree` never terminated for a hash shorter than 20 bytes, growing the output buffer until memory was exhausted
- **Commits survive a Git round-trip unchanged** — `import-git` dropped the author's timezone offset and the message's trailing newline, so `export-git` rewrote every commit with `+0000` and no final newline, changing its hash. The timezone is now retained and the message is preserved byte for byte
- **A truncated pack no longer panics** — the entry loop computed `data.len() - 20` on a pack under 20 bytes, underflowing the subtraction
- **Encryption tests wrote to the operator's key file** — `test_rate_limiting`, `test_encryption_manager_with_cache` and `attack_brute_force_passphrase` read, wrote and finally deleted real paths under `~/.lit`. All three carried `#[ignore]`, so a plain `cargo test` was unaffected, but `cargo test -- --ignored` destroyed whatever key the machine had. Each now uses a per-test path in the temp directory
- **A security test that had never run** — `attack_brute_force_passphrase` asserted a brute-force attack takes 10+ seconds, but the throttle refuses rather than sleeps, and its guesses were dictionary words the complexity rule rejects before the throttle is ever consulted. It now exercises both gates in order and is enabled
- **CI never ran the test suite** — the workflow invoked `cargo test -- --test-threads=1 --verbose`, which hands `--verbose` to the libtest harness rather than to cargo. The harness has no such option, so it exited with `Unrecognized option: 'verbose'` before running a single test, and the step failed on every push. Corrected to `cargo test --verbose -- --test-threads=1`. This is why formatting drift and three cross-platform clippy errors had accumulated on `master` unnoticed
- **Three clippy errors that only fire off Windows** — an `unused_variables` and a `dead_code` in `airgap.rs`, both from bindings and a stub that only the Windows-gated paths use, and a `suspicious_open_options` in `errors.rs` where the debug log was opened with `create(true)` and no truncate behaviour. The log now uses `create_new(true)`, which also stops a file appearing between the existence check and the open from being truncated
- **Flaky `identity::trust` unit tests** — the module's tests shared a single on-disk scratch directory, so trust scores persisted by one test leaked into another's assertions
- **Flaky performance benchmarks** — wall-clock budgets written for an optimized build were asserted verbatim under `cargo test`, which builds unoptimized and runs the suite in parallel, so failures tracked machine load rather than any regression. The budgets are now enforced only in release builds, where they are meaningful; an unoptimized run still executes every benchmark and prints its timing. `LIT_BENCH_SCALE` widens the budgets for slow release runners

### Changed

- `Commit` and `Tag` gain an optional `timezone` field, carrying the offset of an object imported from Git. It is skipped when absent, so objects Lit created itself serialize and hash exactly as before
- `lib.rs` now exports `identity`, `federation`, `events` and `api`, which previously existed only inside the binary — so DID identity, UCAN delegation, trust scoring, federation and event subscriptions are reachable from the published crate, not just the CLI. Purely additive; no existing path changes
- **The CLI consumes the library instead of recompiling it** — `main.rs` declared the same modules `lib.rs` does, so every one was compiled a second time into the binary and its unit tests ran twice. It now imports from `lit::`, converting the four clap subcommand enums to their library counterparts at the call sites. This also retires the blanket `#![allow(dead_code)]` the duplication required, so dead code in the CLI is visible again
- `cargo test` now runs **414 tests** (plus 2 ignored), up from 382 at 1.0.0, and all 414 are distinct — the same suite previously reported 508 by running 94 unit tests in both targets. Two suites that had been disabled rather than fixed are running again

## [1.0.1] - 2026-06-02

### Added

- **Universal content-type registry expanded to ~100 built-in types** — Lit is now a one-stop VCS for modern engineering, versioning CAD, EDA, CAM, 3D modeling, simulation, and AI/ML model files alongside source code, each with domain-appropriate diff, merge, and storage strategies
- **New content domains** — `cam`, `simulation`, and `ml-model` join the existing software/cad/eda/manuscript/database/scientific/media/geospatial/legal/financial/config domains
- **CAD & 3D** — Siemens NX, Siemens Solid Edge, SolidWorks, CATIA, Autodesk Inventor, Fusion 360, PTC Creo/Pro-E, Rhino 3DM, SketchUp, FreeCAD, OpenSCAD, Blender, AutoCAD DWG/DXF, Parasolid, ACIS, JT, OBJ, FBX, glTF/GLB, COLLADA, USD/USDZ, PLY, and Alembic
- **EDA** — KiCad (PCB & schematic), Altium (schematic/PCB/project), EAGLE, OrCAD, Gerber, Excellon drill, GDSII, OASIS, IPC-2581, Touchstone, LEF/DEF, SPEF, SPICE, Verilog, and VHDL
- **CAM** — G-code, STEP-NC, APT, and Mastercam
- **Simulation / FEA / CFD** — Nastran (bulk & OP2), Abaqus (input & ODB), ANSYS (CDB & DB), LS-DYNA, OpenFOAM, COMSOL, Gmsh, VTK, CGNS, Exodus, Modelica, Simulink, and FMU
- **AI/ML models** — ONNX, SafeTensors, PyTorch, TensorFlow, Keras, GGUF, TensorRT, Core ML, TFLite, NumPy, pickle, checkpoint, joblib

## [1.0.0] - 2026-04-01

### Security

- **Security audit** — comprehensive multi-framework audit (CVE, MITRE ATT&CK, NIST FIPS 140-3, CMMC 2.0 Level 2) with 9 findings, all remediated
- Gate test-passphrase bypass with `#[cfg(test)]` — compiled out of release builds
- Sanitize error messages in daemon, MCP, and HTTP error handlers (`user_message()` vs `internal_message()`)
- Add 1 MB line length limit to `lit://` daemon and stdio readers to prevent memory exhaustion
- Document lease TOCTOU race condition and Windows DACL limitation with security advisories

### Added

- **50 CLI commands** — full Git-equivalent workflow plus agent-native extensions (init, add, commit, status, log, diff, show, branch, checkout, merge, resolve, tag, stash, reset, revert, cherry-pick, rebase, blame, bisect, reflog, remote, push, pull, clone, fetch, batch, serve, mcp-serve, tx, snapshot, search, watch, ontology, schema, lfs, verify, gc, import-git, export-git, rotate-key, config, sandbox, swarm, did, ucan, trust, issue, pr, subscribe, delegate, peer)
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
- **382 tests** — unit, command, integration, performance, adversarial, concurrency, and network suites
- **CI/CD** — GitHub Actions for testing (Linux/macOS/Windows), nightly security, and release builds (5 targets)
- **Cross-platform** — Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64)
- **Documentation website** — Next.js 14 static site with 18 documentation pages, dark mode
- **Install scripts** — `install.sh` (Linux/macOS) and `install.ps1` (Windows)

[1.0.2]: https://github.com/nervosys/Lit/releases/tag/v1.0.2
[1.0.1]: https://github.com/nervosys/Lit/releases/tag/v1.0.1
[1.0.0]: https://github.com/nervosys/Lit/releases/tag/v1.0.0
