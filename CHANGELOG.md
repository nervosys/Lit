# Changelog

All notable changes to Lit will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Security

- **The agent client now verifies the agent before sending it anything** — 1.5.0 sent the request, passphrase included, to whatever was listening on the port named in `~/.lit/agent.json`. A port outlives the process that held it: if the agent was killed, crashed, or was lost to a reboot that left the endpoint file behind, that port is free for anything else to bind — on Windows, including a process belonging to another user. The next `lit agent unlock` would have handed that process the passphrase, defeating the cross-user boundary the agent is for. The client now sends a nonce and requires a MAC over it under the token before sending anything else. A peer that answers wrongly, answers with something that is not a proof, or does not answer at all gets nothing
- The refusal says what actually happened and what to do — a failed handshake is not a wrong passphrase, and suggesting the user check their passphrase would send them to the one thing that is not the problem

## [1.5.0] - 2026-08-06

### Added

- **`lit agent` — a passphrase agent, so commands can stop asking twice** — the in-process cache cannot help the command line, because every `lit` command is a new process that starts with an empty one. The agent is a single long-lived process that holds the passphrase; `lit agent unlock` gives it one and later commands find it with no prompt and no environment variable. `start`, `unlock`, `lock [--all]`, `status` and `stop`. Entries expire after a configurable idle period rather than at a fixed age, so a repository in active use does not start prompting in the middle of the work it is being used for
- The agent is **off unless started**: nothing listens on a port, writes a token, or holds a secret until `lit agent start` is run
- **What it protects, stated plainly.** It listens on loopback and authenticates with a token in a file only its owner can read, which keeps out other users on the machine. It keeps out nothing running *as you* — such a process can read the token file and ask the agent for the passphrase. A Unix socket or named pipe restricted to the owner would grant exactly the same set of processes, so this is not a transport shortcoming. Against a same-user attacker the agent is no stronger than `LIT_PASSPHRASE`; it is better only in that the secret is not in an environment block, where it appears in process listings and is inherited by every child, and in that it expires

## [1.4.1] - 2026-08-06

### Security

- **The brute-force throttle now survives between commands** — the failed-attempt counter lived in a process `static`, and every `lit` command is a new process, so each one started from zero. The exponential backoff never grew past its first step and the five-attempt lockout could not be reached at all by a script that reran the binary, which is precisely the case it was written for. The count is now kept next to the key file as `<key_file>.throttle`, restricted to its owner, and read back on each attempt. This raises the cost of guessing rather than ending it: an attacker who can delete the state file is already inside the same directory as the key, and PBKDF2 at 600,000 iterations remains the defence that does not depend on that assumption. Corrupt or unreadable state is treated as a clean slate, so a damaged file cannot lock you out of your own repository

### Fixed

- **`docs/ENCRYPTION.md` no longer documents passphrase caching that cannot happen** — it showed `lit commit` followed by `lit status` answering `✓ Using cached passphrase`. The cache is process-local, and those are two processes, so the second command always prompted. `cache_timeout_secs` has effect where one process performs several operations — the GUI, the daemon, an embedder using the library — and none at all for ordinary command-line use. The page now says so, and points at `LIT_PASSPHRASE_FILE` for the case the example was reaching for

## [1.4.0] - 2026-08-06

Renumbering only — identical in content to 1.3.3, which was published as a
patch by mistake. `crypto::fips::ensure_self_tests()` is an addition to the
public API, and an addition is a MINOR bump. 1.3.3 works and is not yanked;
anything resolving `^1.3` moves here on its own.

## [1.3.3] - 2026-08-06

### Security

- **FIPS power-on self-tests now precede cryptography for every consumer, not just the CLI** — closes finding I-2. `main()` ran the known-answer tests at startup, which covered the `lit` binary and nothing else. The Tauri GUI links the library directly and has no startup path of its own, so every repository operation it performed ran with no power-on test having executed — FIPS 140-3 §4.9.1 asks that the tests precede cryptographic use, not merely that they exist. The guarantee moved onto the crypto entry point: `crypto::fips::ensure_self_tests()` runs the KATs once per process behind a `OnceLock`, and `EncryptionEngine::new` gates on it. Every AES-GCM operation in the crate goes through an engine, so no consumer can skip the tests by forgetting to ask for them. Later calls read the stored result, so the cost is one run per process

## [1.3.2] - 2026-08-06

### Security

- **Owner-only permissions on Windows, and on files that already exist** — closes finding I-1. The Windows path used the read-only attribute, which stops writes and does nothing about reads: the HMAC signing key that makes the audit log tamper-evident was readable by any local account, and anyone who could read it could forge entries. `restrict_to_owner` now builds a DACL granting only the current user's SID and applies it with `SetNamedSecurityInfoW`, marked `PROTECTED_DACL_SECURITY_INFORMATION` so the parent directory's inherited entries are dropped rather than surviving alongside it. SYSTEM and Administrators lose access too; Administrators can still take ownership, so this is not a barrier to them, but software running as SYSTEM will no longer be able to read these files
- **The restriction is applied on load, not only at creation** — restricting at creation is no help to a key already sitting on disk from an earlier version, which keeps its original permissions for the life of the installation. On the machine this was developed on, `~/.lit/audit.key` dated from October 2025 and `~/.lit/encryption.key` from March, both still carrying the directory's inherited ACL. The load paths now re-apply the restriction, so an upgrade corrects them
- **The airgap transport log is restricted too** — `~/.lit/airgap_audit.log` had no permission handling at any point and had grown to 162 KB world-readable. Every line is a filesystem path the user moved data through, which is a record of what they have and where they keep it

## [1.3.1] - 2026-08-05

### Security

- **The encryption key file is restricted to its owner** — it had no permission restriction on any platform and was written world-readable (0644), while `audit.rs` and the debug log already set 0600. It stores no key material, only the PBKDF2 salt and a verification hash, but that hash turns the 49-byte file into an offline oracle: anyone who can read it can test passphrase guesses without touching the repository. Now 0600 on Unix, applied to the temporary file before the rename so the key is never briefly readable under its real name. On Windows it is the read-only attribute, which stops writes and not reads — restricting reads there needs an explicit DACL, still open as finding I-1 in `docs/SECURITY_AUDIT.md`

### Added

- **CI runs the ignored test** — one test is ignored for runtime rather than correctness: it exercises the passphrase throttle, and each attempt that reaches verification costs a 600,000-iteration PBKDF2. Skipping it everywhere meant a real test that nothing ever ran. CI now runs `cargo test -- --ignored` on one platform, buying the coverage without paying for it three times

## [1.3.0] - 2026-08-05

### Security

- **Branch and tag names are no longer exposed on disk** — an encrypted repository keeps all refs in a single encrypted index (`.lit/refs.enc`) instead of one file per ref. A ref name is a filename, so the directory leaked every branch and tag name however well the contents were encrypted. The cost is granularity: refs become read-modify-write as a unit, so two processes updating different branches can race where separate files could not — which is why this applies only when encryption is on, leaving an unencrypted repository its directory and its concurrency
- **Refs and HEAD are now encrypted** — `write_ref` and `update_head` stored them in clear text, so every commit hash a branch or tag pointed at was readable off disk even when the objects themselves were encrypted: the whole commit graph was exposed. Both sides now go through the same cipher as the object store and index. Reads stay tolerant of un-migrated refs, so a repository part-way through conversion still works, and `migrate-encryption` converts them. A branch *name* is a filename and remains visible — encrypting contents cannot hide directory entries, and hiding names would mean one encrypted index in place of a file per ref

### Fixed

- **The GUI had not compiled for two months** — `gui/src-tauri/Cargo.toml` asked for `lit = { path = "../.." }`, but that package was renamed to `litvc` on 2026-06-01. Nothing noticed because CI built the Rust crate and nothing else

### Added

- **CI builds the GUI backend and the website** — 62 tracked files across `gui/` and `website/` had no verification at all, which is how the GUI stayed broken. The Tauri backend is checked on Windows so it needs no system libraries; the website runs `npm ci && npm run build`

## [1.2.1] - 2026-08-05

### Fixed

- **`lit --version` did not exist** — the clap derive carried no `version` attribute, so `--version`, `-V` and `version` were all rejected as unknown arguments, and a released binary could not report which release it was. Found by installing the published crate from crates.io and smoke-testing that, rather than testing the local build

## [1.2.0] - 2026-08-05

### Added

- **`lit migrate-encryption`** — encrypts a repository created before encryption was switched on. Previously this could not be done at all: the existing index and objects carry no encryption header, so setting `enabled = true` left every command failing with no way forward. The command encrypts the loose objects and the index in place, and expands any pack into encrypted loose objects for `gc` to re-pack. It is idempotent, so an interrupted run is finished by running it again rather than leaving the repository half-converted

### Fixed

- **Cross-pack delta bases depended on directory order** — a `REF_DELTA` names its base by hash, and a thin pack leaves that base outside the file, usually in a sibling pack. Packs were resolved one at a time and folded into the discovered set afterwards, so a base in another pack was only found when `read_dir` happened to return that pack first: the same repository could import or fail depending on filesystem order. All packs now resolve together, keyed by pack and offset, so a base is found wherever it lives. A thin pack whose bases are absent from the source entirely is still reported — there is nothing to rebuild from

### Documentation

- **Refs and HEAD are not encrypted** — `write_ref` and `update_head` store them in clear text, so branch and tag names and the commit hashes they point at stay visible on disk even in an encrypted repository. Object contents, commit messages and the index are encrypted; the shape of the history is not. The README now says so rather than leaving it to be discovered

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
