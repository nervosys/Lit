# Handoff

State of the repository as of **2026-08-06**, after a run of security work that
took it from 1.0.2 to 1.5.1, and a follow-up pass on the three items in §6.
Written for whoever picks this up next, including a future me who has forgotten
all of it.

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
