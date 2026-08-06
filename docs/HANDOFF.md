# Handoff

State of the repository as of **2026-08-06**, after a run of security work that
took it from 1.0.2 to 1.5.1. Written for whoever picks this up next, including a
future me who has forgotten all of it.

---

## 1. Read this first: CI has not validated any of this

Every claim below was verified **locally**. GitHub Actions is not running:

- 14 workflow runs are **queued** and never start.
- The ones that do start fail on `Failed to resolve action download info: Service Unavailable` — GitHub cannot fetch `actions/checkout` and friends.

That is infrastructure, not code. The website job passes when it runs; the Rust
jobs never get far enough to compile anything. **Do not read a green or red
badge as a statement about this code** until runs actually execute. First job
for the next person with repository admin: find out why Actions is stalled.

The local equivalent of the full CI matrix:

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test                      # 440 tests, 1 ignored
cargo test -- --ignored         # the throttle test; costs a 600k-iteration PBKDF2
cargo check --manifest-path gui/src-tauri/Cargo.toml
cd website && npm ci && npm run build
```

All of the above pass on `master` at `v1.5.1`.

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

### One encryption key file is shared across repositories

`encryption.key` defaults to `~/.lit/encryption.key` — one file, not one per
repository. Initialising encryption in a second repository fails if the first
one's key exists under a different passphrase. This surfaced while testing:
`migrate-encryption` succeeded in one repo and then failed in a fresh one for no
obvious reason. Worth a design decision, not just a better error message.

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

## 6. If you want the next thing to do

In rough order of value:

1. **Get CI running.** Everything here rests on local verification by one person on one platform (Windows). The Unix permission paths in `restrict_to_owner` have never executed anywhere.
2. **Decide the shared-key-file question** in §3 — it is the one item that is a design gap rather than a documented limit.
3. **Attack the agent again.** The 1.5.1 hole was found by reviewing 1.5.0 an hour after publishing it, and every unit test written for 1.5.0 passed against the vulnerable code: they tested whether the *server* authenticated the client, and never asked whether the *client* authenticated the server. Assume there is another one.
