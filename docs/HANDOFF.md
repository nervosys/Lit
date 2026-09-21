# Handoff

State of the repository as of **2026-08-06**, after a run of security work that
took it from 1.0.2 to 1.5.1, and a follow-up pass on the three items in §6.
Written for whoever picks this up next, including a future me who has forgotten
all of it.

§0 was added **2026-09-17** and is about work that came after all of that.

---

## 0. Self-hosting (2026-09-17)

### What has been verified, and what has not

| | |
| --- | --- |
| `cargo test --lib` | **all pass** (181 tests) |
| `cargo test --test command_tests -- server::` | **all pass** (21 tests, real server on a real socket) |
| `cargo clippy --all-targets` | **clean**, no warnings |
| `cargo fmt --check` | clean |
| The manual walkthrough, `docs/SELF_HOSTING.md` §6 | not run by hand — but §6's substance is covered by the integration tests |

The security surface is exercised end to end: the banner before authentication,
forged and expired tokens, role enforcement, routes that fail closed, lockout,
the account lifecycle, revocation by both routes it can travel, and the audit
log's attribution. Those tests are in `tests/commands/server.rs`, and they are
the thing to keep honest — they are the only place a route wired to the wrong
check would be caught.

Repository routes are covered as of the `execute_at` work: the commands now
take an explicit repository root, so the server serves the one it was handed
rather than whichever the process's working directory resolves to. The test
holding that is `a_repository_route_reads_the_repository_it_was_given`, which
writes a marker into the served repository and asserts the response reflects
it — so the old behaviour fails rather than passing quietly.

Two environment traps cost most of a day here and will cost it again:

- **`Access is denied. (os error 5)` across unrelated modules, and `LNK1104:
  cannot open file 'C:\...\Temp\lnk{...}.tmp'` from the linker.** Both are one
  cause: a sandboxed shell that can create directories under `%TEMP%` but not
  files. Point `TMP` and `TEMP` at a writable directory and both go away —
  `cargo test --lib` went from 44 failures to zero on that change alone. Read
  the *full* linker message before concluding anything; `LNK1104` was misread
  here as a held file lock, and build processes were killed for nothing.
- **`Blocking waiting for file lock on build directory`.** rust-analyzer runs
  `cargo check` continuously against the same target directory. Retry, or run in
  the background. Do not kill its processes; a separate `CARGO_TARGET_DIR` does
  not help either, because a cold build needs MSVC `cl.exe` for the C build
  scripts in `ring` and `pqcrypto-internals`.

If this suite starts failing in a broad permission-shaped pattern again,
suspect the environment before the code,
and re-run with `--test-threads=1` to tell them apart.

### What was added

`src/server/` (`auth`, `session`, `banner`, `audit`, `tls`) and
`src/commands/server.rs`, behind a new `lit server` CLI. `lit serve` is
untouched and still the loopback development server. The control mapping is
`docs/NIST_800-171.md`; deployment is `docs/SELF_HOSTING.md`.

Six defects were found and fixed along the way, the first three in code that was
harmless while `serve` bound loopback only and stopped being harmless the moment
it could be hosted:

1. The body size cap was bypassable with chunked transfer encoding, before
   authentication.
2. The rate limiter's client table grew without bound.
3. The account store was read once at startup and written back on every login,
   so a CLI `disable` against a running server reported success, did nothing,
   and was then erased.
4. Even once (3) was fixed, a session still carried the role it was minted with
   and was never re-checked, so a revoked account kept its open session for up
   to eight hours. Every request now re-checks the account behind its session.
5. `UserStore::save` used a fixed `users.json.tmp`. Two processes administering
   one store — a case (3) deliberately added support for — would overwrite each
   other's half-written file and race on the rename. The name now carries the pid.
6. `UserStore::save` never called `allow_replacement` before its rename, though
   that helper exists in `crypto::encryption` for exactly this and
   `EncryptionKey::save` calls it: Windows refuses to rename onto a file it
   considers read-only, which is what the previous save's restriction leaves.

**§6's lesson held up four separate times, which is the real finding here.**
That lesson was: when a class of bug is fixed, grep for the pattern rather than
fixing the instance.

- (3): the store had a careful write-temp-restrict-rename `save` and no notion
  that anyone else might write the same file.
- (4): fixing (3) made revocation *recorded*; it took a second look to notice
  that recorded is not *enforced*.
- (5) and (6) are the sharpest, because the pattern was **already written down
  in this repository**. `EncryptionKey::save` does the atomic-replace dance
  correctly and calls `allow_replacement`; `crypto::agent` was fixed in 1.6.0 to
  write-restrict-rename for the same reason. A new `save` was then written from
  scratch, next to both of them, repeating the bug they had each already fixed.

The generalisation for whoever is next: when you write a function whose shape
matches one that already exists here — anything that writes a secret to disk,
restricts it, and renames it into place — go and read the existing one first.
There are at least three now, and they did not converge on their own.

### What is known-open

- `disable`, `enable`, `role`, `password`, `unlock` have no API route. The CLI
  covers them and works against a running server.
- Lit's TLS is `rustls`, **not FIPS-validated**. The at-rest cryptography is.
  Both serve `https://`, so the difference is invisible from outside — which is
  why it is stated in `src/server/tls.rs` as well as in the docs.
- No MFA, no CUI marking. Both are documented as customer responsibility.
- A residual lost-update race between two processes administering one store.

### Decided: the per-request `stat` stays

Re-checking the account store on every request costs one `fs::metadata` call,
and an earlier draft left open whether that wanted a cache under load. It does
not, and the reasoning is worth keeping so it is not reopened by instinct.

Every authorized request already performs a SHA3-512 digest for the session
lookup, JSON parsing, and an audit append — file open, write, HMAC-SHA256,
close. A warm `stat` on NTFS is a single syscall in the low microseconds; the
audit append on that same request costs strictly more. Caching would therefore
trade immediate revocation, which is the entire point of the check, for less
time than it takes to write the log line recording the request.

Revisit only with a profile showing otherwise, and then with an explicit bound
and a stated worst-case revocation delay — not an unbounded cache.

### The next things

Nothing outstanding in this area. What remains is inherently the deploying
organization's: MFA, CUI marking, and FIPS-validated TLS in transit, each
documented in `docs/NIST_800-171.md` §5 with the proxy pattern that addresses
it.

And the standing one, per §6: **look for the next instance of an old pattern**
rather than the next new bug. That has now paid out four times in this section
alone.

---

## 1. Read this first: CI has not validated any of this

Every claim below was verified **locally**. GitHub Actions is not running:

- 17 workflow runs are **queued** and never start.
- The ones that started earlier failed on `Failed to resolve action download info: Service Unavailable` — GitHub could not fetch `actions/checkout` and friends.

**This was diagnosed, and it is not ours.** GitHub declared a *major outage* of
Actions beginning 2026-08-06 15:22 UTC — "workflow runs are failing or delayed
in starting, queued jobs may time out, requests to the Actions API are returning
errors" — affecting Actions and Pages. Every run in this repository that stopped
executing stopped after that timestamp; the last run to complete was at 15:42.
Repository Actions permissions are `enabled: true, allowed_actions: all`, so
there is nothing to change here.

What to do: wait for the incident to clear, then re-run the queued workflows
(`gh run rerun <id>`) and read the results. If they are still queued long after
GitHub reports recovery, the next thing to check is the organisation's Actions
policy and spending limit, which needs `admin:org` and was not readable from
this account.

**Do not read a green or red badge as a statement about this code** until runs
actually execute.

The local equivalent of the full CI matrix:

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test                      # 440 tests, 1 ignored
cargo test -- --ignored         # the throttle test; costs a 600k-iteration PBKDF2
cargo check --manifest-path gui/src-tauri/Cargo.toml
cd website && npm ci && npm run build
```

All of the above pass on `master` at `v1.6.0`: 445 tests, 1 ignored, plus the
ignored throttle test run separately. Still one person, one platform, Windows.

---

## 2. What shipped, and why

Nine releases, all published to crates.io as `litvc` and each smoke-tested by
installing **from the registry** rather than trusting the local build.

| Version | The problem it fixed |
| --- | --- |
| 1.1.0 | Git import/export: pack delta resolution, dependency ordering, annotated tags |
| 1.2.0 | Pack reader — packed objects were written and then unreadable |
| 1.2.1 | `lit --version` did not exist; clap carried no `version` attribute |
| 1.3.0 | Refs and HEAD stored in clear text; ref *names* leaked as filenames. GUI had not compiled in two months |
| 1.3.1 | Encryption key file was world-readable (0644) |
| 1.3.2 | Windows used the read-only attribute, which blocks writes and not reads. Restriction applied only at creation, so old files stayed permissive forever |
| 1.3.3 / 1.4.0 | FIPS power-on self-tests ran in `main()` only, so the GUI did crypto with no self-test. (1.4.0 renumbers 1.3.3, which was mis-published as a patch) |
| 1.4.1 | The brute-force throttle lived in a process `static`, so it reset on every command |
| 1.5.0 | `lit agent` — passphrase reuse across commands |
| 1.5.1 | The agent client sent the passphrase to whatever answered on the recorded port |
| 1.6.0 | `rotate-key` had never worked. One key file was shared by every repository. The agent's token file was restricted only after being written |

### The root cause behind most of it

`.github/workflows/ci.yml` ran `cargo test -- --test-threads=1 --verbose`.
`--verbose` went to libtest, which does not have that option, so the step exited
`Unrecognized option: 'verbose'` with code 101 **before running a single test**.
That step had failed on every push for as long as it existed. Formatting drift,
three cross-platform clippy errors, disabled tests, and tests writing into
`~/.lit` all accumulated behind it unseen. Fixed in 1.3.1.

If something here looks like it was never checked, that is why.

---

## 3. Known-open, with honest severity

**Nothing below is a secret defect — each is a documented limit.** They are
listed so nobody has to rediscover them.

### The agent does not defend against processes running as you

`lit agent` keeps out *other users* on the machine: it listens on loopback and
authenticates with a token in an owner-only file. It keeps out nothing running
as you, because such a process can read the token file and ask.

This is **not** fixable by changing transport. A Unix socket or a named pipe
restricted to the owner grants exactly the same set of processes. On an ordinary
operating system, "another program running as me" is inside the trust boundary.
Against that attacker the agent is no stronger than `LIT_PASSPHRASE` — better
only in that the secret is not in an environment block, where it appears in
process listings and is inherited by every child, and in that it expires.

Documented in `docs/ENCRYPTION.md` and the module header of `src/crypto/agent.rs`.

### The throttle state file can be deleted

`<key_file>.throttle` holds the failed-attempt count. An attacker who can delete
it resets the throttle — but they are already in the same directory as the key
file, so they are inside the boundary the throttle assumes. PBKDF2 at 600,000
iterations is the defence that does not depend on that assumption.

Corrupt or unreadable state is deliberately treated as a clean slate: a damaged
file must not become a way of locking the owner out of their own repository.

### The agent serves one connection at a time

`serve()` handles connections in a single loop. Read and write timeouts are 5s
and a connection is capped at two messages, so a stalled client costs at most
five seconds — but a same-user process can still make the agent unresponsive.
Given that a same-user process can simply *ask the agent for the passphrase*,
this is not the weakest link, and threading it was not worth the concurrency
surface. Revisit if the agent ever grows a stronger same-user story.

### ~~One encryption key file is shared across repositories~~ — decided

Resolved. `key_file` left unset now resolves to `~/.lit/keys/<repo>-<digest>.key`
and the chosen path is written back into `.lit/encryption.toml`, so a moved
repository keeps its key. Key files stay outside the working tree: they carry
the salt and verification hash that make offline guessing possible, and those
should not travel with the data they protect. Existing configs name their key
file explicitly and are untouched — including ones pointing at the old shared
path, where the "Invalid passphrase" now explains that the file is shared.

Two things fell out of that decision and are worth knowing:

- A missing key file used to be replaced with a new one, silently. That is right
  the first time and wrong every time after. A repository that already holds
  encrypted content now refuses and says the key file is gone.
- `.lit/HEAD` is written in the clear by `lit init` and stays that way until
  something moves HEAD, so an encrypted repository legitimately contains one
  plaintext file naming the current branch. Rotation now re-writes it encrypted,
  but a repository that has never rotated still has it.

### `rotate-key` cannot change the passphrase without a terminal

Found while smoke-testing 1.6.0 from crates.io. Both prompts — the current
passphrase and the new one — read `LIT_PASSPHRASE` first, and there is only one
such variable, so a non-interactive run rotates to the passphrase it already
had. That is still a real re-key, since the salt changes, but it is not what a
script asking for a rotation means. Changing the passphrase needs a TTY.

Fixing it needs a second source for the new passphrase — `LIT_NEW_PASSPHRASE`,
or flags — which is a small design decision nobody has made. The library call
`rotate_with_passphrases(old, new)` takes both and has no such limit.

### Thin packs with absent bases

A thin pack whose delta bases are not present in the source cannot be resolved.
This is not a bug to fix — the information is not there. Import rejects them
rather than fabricating a base.

---

## 4. Traps that cost real time

**Cargo fingerprint staleness.** Three separate times, `cargo` served a stale
binary against newer sources, making working changes look broken. If behaviour
contradicts the source you are reading, check the binary's mtime before
debugging the code:

```bash
ls -la target/release/lit.exe && ls -la src/crypto/encryption.rs
cargo clean -p litvc   # the fix
```

**A running agent locks `lit.exe` on Windows.** `cargo build --release` fails
with `Access is denied. (os error 5)`. Run `lit agent stop` first.

**Multi-line `perl`/`sed` on CRLF files silently does nothing.** Several edits
appeared to succeed and changed nothing. Use the editor, not stream tools, for
anything spanning lines.

**Verify against the registry, not the local build.** `lit --version` being
entirely absent survived 423 passing tests and a three-platform CI matrix. It
was caught by `cargo install litvc && lit --version`. Do that before calling a
release good.

**Test rigs can be silently invalid.** While demonstrating the throttle bug at
the CLI, the test repository's configured `key_file` did not exist, so
`EncryptionKey::load` was never reached — the run proved nothing, in either
direction, and was briefly reported as proof. Before trusting a negative result,
confirm the code path you think you are exercising actually runs.

---

## 5. Where the security posture is written down

- `docs/SECURITY_AUDIT.md` — all 14 findings, each with what was actually done. I-1 and I-2 were both marked FIXED while still broken; both now carry the specifics of what changed and why the earlier fix was insufficient.
- `docs/ENCRYPTION.md` — passphrase handling, the agent, and what the cache genuinely does. It previously documented cross-command caching that could not happen.
- `CHANGELOG.md` — every entry states the defect, not just the fix.

**A pattern worth keeping.** Twice, a finding marked FIXED was verified and found
still broken — the fix existed but only covered the CLI, or only covered files
created after it. When an audit says a thing is fixed, check the claim against a
running system rather than against the source that was supposed to implement it.

---

## 6. The three next things, and what came of them

1. **Get CI running** — diagnosed, not ours: a GitHub Actions major outage from 15:22 UTC. See §1 for what to do when it clears. Still true that everything rests on local verification on Windows, and that the Unix branches of `restrict_to_owner` and `restrict_dir_to_owner` have never executed anywhere.
2. **Decide the shared-key-file question** — decided and implemented; see §3.
3. **Attack the agent again** — done. Four findings, in `docs/SECURITY_AUDIT.md` §13. None is a break of the protocol: the handshake added in 1.5.1 holds up. The one worth naming is that `~/.lit/agent.json` was written and *then* restricted, leaving the token briefly readable at a known path — the same defect I-1 and I-3 were about for the key file, in a file written after those were fixed. **The lesson generalises: when a class of bug is fixed, grep for the pattern rather than fixing the instance.**

While reading that code, `lit rotate-key` turned out never to have worked — it
derived the new key by reading the key file that still described the old
passphrase, and stopped at "Invalid passphrase". Its one test covered the
encryption-disabled early return, so the path that does the work had never
executed. It also ignored `.lit/packs` and `refs.enc` entirely, which would have
destroyed every packed object and the whole ref namespace had it got that far.
Fixed, with an end-to-end test.

### What is actually next

1. **Still: get CI running**, then re-run everything and believe nothing until it is green. The Actions outage was still unresolved when 1.6.0 shipped, so 1.6.0 has never been built on Linux or macOS by anything.
2. **`.lit/HEAD` in the clear.** See §3. It names the current branch. Either encrypt it at `init` when encryption is configured, or accept it and say so in `docs/ENCRYPTION.md`.
3. **Give `rotate-key` a non-interactive path.** See §3 — it currently cannot change a passphrase without a TTY.
4. **Look for the next instance of an old pattern**, rather than the next new bug. Two of the last three findings were repeats of something already fixed elsewhere.

**1.6.0 is published.** Tagged: no — `v1.6.0` does not exist, so the Release
workflow has not built binaries or cut a GitHub release for it. That is the one
piece of the usual release sequence still outstanding.

The registry build was smoke-tested per §4: `cargo install litvc` → 1.6.0, an
encrypted repository resolved its own key file under `~/.lit/keys/`, and
`lit rotate-key` completed — 3 objects, 2 refs, both pack files expanded, the
repository readable afterwards and still refusing the wrong passphrase. That is
the first time that command has ever run to completion.
