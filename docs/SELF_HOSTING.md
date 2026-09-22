# Self-Hosting a Lit Server

How to run `lit server serve` as a hosted service, with the controls a
CUI environment needs actually switched on.

For the requirement-by-requirement mapping — including what Lit does *not*
satisfy — see `docs/NIST_800-171.md`. Read §5 of that document before you deploy
anything that handles CUI; the TLS caveat in particular changes the architecture.

---

## 1. Which server am I running?

Lit has two HTTP servers, and picking the wrong one is the most likely way to
get this wrong.

| | `lit serve` | `lit server serve` |
| --- | --- | --- |
| Intended for | A developer's own machine | A hosted deployment |
| Binds | `127.0.0.1` only, always | Whatever you tell it |
| Auth | Optional shared bearer token | Named accounts with roles |
| Sessions | None | Expiring, revocable |
| Audit | None | Every decision |

`lit serve` is not a hardened server with weaker defaults — it has no account
system at all. Do not put it behind a proxy and call it hosted.

---

## 2. First run

### 2.1 Create the first administrator

The server refuses to start with no accounts, and tells you this:

```
No accounts exist in C:\Users\you\.lit\server\users.json.
Create the first administrator with:  lit server user add <name> --role admin
```

There is deliberately no `--password` flag — a command-line argument is visible
in the process table to every other account on the host. Supply the password
one of three ways:

```bash
# From a file (delete it afterwards)
lit server user add alice --role admin --password-file ./secret.txt

# From stdin
printf '%s' "$PASSWORD" | lit server user add alice --role admin --password-stdin

# From the environment
LIT_SERVER_PASSWORD="..." lit server user add alice --role admin
```

Passwords must be at least 15 characters, must not contain the username, and
must not be a short repeated sequence. They are stored as PBKDF2-HMAC-SHA512 at
600,000 iterations with a 128-bit random salt.

### 2.2 Create working accounts with the least role that works

```bash
lit server user add ci-bot     --role reader       --password-stdin < ci.key
lit server user add dev-agent  --role contributor  --password-stdin < agent.key
lit server user add release    --role maintainer   --password-stdin < rel.key
```

| Role | Can |
| --- | --- |
| `reader` | status, log, diff, show, search, tags, config, verify, ontology |
| `contributor` | the above, plus `add`, `commit`, `snapshot` |
| `maintainer` | the above, plus `checkout`, `merge`, `branch` |
| `admin` | everything, plus `/api/v1/admin/*` |

Keep your administrator account separate from your day-to-day account
(`03.01.06`). Lit does not enforce this.

### 2.3 Write your system use notification

Lit ships a placeholder that announces itself as one. Replace it:

```bash
cat > /etc/lit/banner.txt <<'EOF'
AUTHORIZED USE ONLY
This system processes Controlled Unclassified Information...
EOF
```

The server warns on stderr at every startup while the placeholder is in use, and
records `banner=default_placeholder` in the audit log, so this is visible in an
assessment rather than quietly missing.

---

## 3. Running it

### 3.1 Loopback, for a single-host deployment

```bash
lit server serve --bind 127.0.0.1 --port 3000 --banner /etc/lit/banner.txt
```

Plaintext on loopback is allowed: the traffic never becomes a transmission.

### 3.2 With native TLS

```bash
lit server serve \
  --bind 0.0.0.0 --port 8443 \
  --tls-cert /etc/lit/tls/fullchain.pem \
  --tls-key  /etc/lit/tls/privkey.pem \
  --banner   /etc/lit/banner.txt \
  --audit-log /var/log/lit/audit.log
```

The private key must be an unencrypted PEM; Lit cannot prompt for a passphrase,
because Lit never prompts for anything. Lit tightens the key file to owner-only
before reading it and refuses to start if it cannot.

**Binding a routable address without TLS is refused:**

```
Refusing to serve plaintext HTTP on 0.0.0.0:8443, which is not a loopback address.
Session tokens and passwords would cross the network unprotected
(NIST SP 800-171r3 03.13.08).
Supply --tls-cert and --tls-key, put a TLS-terminating proxy in front and bind
loopback, or pass --allow-plaintext if this network is protected by other means.
```

`--allow-plaintext` exists for the deployment where something else genuinely
provides the protection. Using it is recorded in the audit log at startup.

### 3.3 Tuning the session and lockout policy

```bash
lit server serve ... \
  --idle-timeout 600 \      # terminate after 10 minutes idle
  --max-lifetime 14400 \    # and after 4 hours regardless
  --max-attempts 3 \        # lock after 3 consecutive failures
  --lockout-secs 0          # locked until an admin runs `user unlock`
```

---

## 4. The FIPS-validated deployment

**Read this if your system must demonstrate FIPS-validated cryptography
protecting CUI in transit (`03.13.11`).**

Lit's own TLS uses `rustls`, which is not FIPS-validated. Lit's *at-rest*
cryptography is a different story — it goes through the FIPS module in
`crypto::fips`, with power-on self-tests — but the TLS is not. Both
arrangements serve `https://`, so this is not observable from outside; it has to
be an architectural decision.

The supported arrangement is to terminate TLS in a validated module:

```
                    ┌──────────────────────────┐
  client ──TLS──►   │ nginx (OpenSSL FIPS)     │
                    │ or HAProxy, or stunnel   │
                    └───────────┬──────────────┘
                                │ loopback, plaintext
                                ▼
                    ┌──────────────────────────┐
                    │ lit server serve         │
                    │ --bind 127.0.0.1         │
                    └──────────────────────────┘
```

```bash
lit server serve --bind 127.0.0.1 --port 3000 --banner /etc/lit/banner.txt
```

Lit still authenticates, authorizes, expires sessions and audits. Only the TLS
moves — but two things behind a proxy do not work the way they look.

### What a proxy costs you, and what to do about it

Lit identifies a client by the peer address of the TCP connection. Behind a
proxy, every connection comes from the proxy, so:

1. **Rate limiting collapses into one bucket.** The limit is 100 requests per
   minute *per address*, and behind a proxy there is only one address. All your
   users share it, and one noisy client starves everyone else.
2. **The audit `source` field records the proxy, not the client.** For
   `03.03.02` — a record that says where the request came from — that is a
   material loss. Your failed-login trail will name accounts correctly and
   locations uselessly.

Both are fixed by telling Lit which addresses are proxies:

```bash
lit server serve --bind 127.0.0.1 --port 3000 \
  --trusted-proxy 127.0.0.1 \
  --banner /etc/lit/banner.txt
```

With that set, a request arriving from `127.0.0.1` has its `X-Forwarded-For`
believed, and the client address it names is what the rate limiter buckets on
and what the audit log records. Repeat `--trusted-proxy` for each proxy address.

**Without it, the header is ignored entirely**, and that is the right default.
`X-Forwarded-For` is caller-supplied: a server that believes it from anyone lets
every client choose what its audit records say and which rate-limit bucket it
lands in, which is worse than the problem it solves. Lit therefore reads the
header only on connections from an address you have named.

Make sure your proxy actually sets it, and **overwrites** rather than appends to
any header the client sent:

```nginx
proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
```

Lit walks the chain from the right, stepping over addresses that are themselves
trusted proxies and taking the first that is not — so a client that prepends a
chain of its own invention cannot get those entries used. But that only holds if
the proxy appends the address it actually received from, which is what
`$proxy_add_x_forwarded_for` does.

Worth doing anyway, whether or not you set `--trusted-proxy`:

- **Rate limit at the proxy** too, per real client address. `limit_req` in
  nginx, `stick-table` in HAProxy. Lit's limiter is a backstop, not your only
  control.
- **Log the real client address at the proxy**, so you have a second source if
  the correlation is ever questioned.

This is also where **multi-factor authentication** goes (`03.05.03`). Lit does
not implement MFA. An authenticating proxy doing OIDC/SAML with MFA in front of
a loopback-bound Lit satisfies it; Lit's own accounts then become the second,
service-level authorization layer rather than the primary one.

---

## 5. Operating it

### 5.1 Day-to-day administration

The CLI works against a running server — the account store is re-read whenever
the file changes underneath it:

```bash
lit server user list                    # who exists, and their status
lit server user add erin --role reader --password-stdin < erin.key
lit server user role alice maintainer   # change a role
lit server user disable bob             # suspend without deleting
lit server user unlock carol            # clear a lockout
lit server user remove dave             # delete
```

The same operations, minus a few, are on the API to an `admin` session:

```
GET    /api/v1/admin/users
POST   /api/v1/admin/users            {"username","password","role"}
DELETE /api/v1/admin/users/<name>     # also terminates that account's sessions
GET    /api/v1/admin/sessions
```

`disable`, `enable`, `role`, `password`, and `unlock` have no API equivalent
yet — they are CLI-only.

**Revocation takes effect immediately, including on open sessions.** Every
request re-checks the account behind its session, so an account you disable or
remove loses its existing session on its very next request — it does not linger
until the idle timeout. A demotion applies the same way: the session drops to
the account's current role rather than the one it was issued with.

Verify this yourself rather than taking it on trust; §6 step 11 below is the
check, and it is the one an assessor runs for personnel termination.

### 5.2 The audit log

Records are single-line and machine-parseable:

```
2026-09-17T14:02:11Z | AUTH_SUCCESS | {"subject":"alice","source":"10.0.0.7","object":"/api/v1/auth/login","outcome":"success"} | 9f86d081...
```

`timestamp | event | json | hmac`. The HMAC makes modification of an existing
record detectable — check it with `lit verify`. It does **not** make deletion
detectable, so forward the log off the host:

```bash
lit server serve ... --audit-log /var/log/lit/audit.log
# then ship /var/log/lit/audit.log with your usual collector
```

A `SERVER_STOP` record is written when the serve loop exits, which means a
server that is killed rather than stopped — `kill -9`, `Stop-Process -Force`, an
OOM kill — leaves none. A missing stop record is therefore not by itself
evidence of tampering, and an alert that treats it that way will cry wolf every
time a host reboots ungracefully. Pair it with the next `SERVER_START` to tell
an unclean shutdown from a gap.

If the log cannot be written, Lit prints `AUDIT FAILURE:` on stderr naming the
event that was lost, and keeps serving. If your policy requires it to stop
instead, run it under a supervisor that watches for that string.

### 5.3 Health of the deployment

`lit server serve` handles one request at a time. That is fine for agent traffic
and a small team, and is not fine as a shared service under load. Put a proxy in
front, and size accordingly.

---

## 6. Verifying the controls end to end

A ten-minute walkthrough that exercises each mechanism and leaves evidence in
the audit log.

```bash
# 0. Set up
lit server user add admin1 --role admin      --password-stdin <<< "a very long admin password"
lit server user add read1  --role reader     --password-stdin <<< "a very long reader password"
lit server serve --bind 127.0.0.1 --port 3000 --banner /etc/lit/banner.txt &

# 1. The banner is served before authentication          (03.01.09)
curl -s localhost:3000/api/v1/banner

# 2. An unauthenticated request is refused               (03.05.01)
curl -s -o /dev/null -w '%{http_code}\n' localhost:3000/api/v1/status
# 401

# 3. A bad password fails, and is logged                 (03.03.01)
curl -s -X POST localhost:3000/api/v1/auth/login \
     -d '{"username":"read1","password":"wrong"}'

# 4. A good password yields a session                    (03.05.01)
TOKEN=$(curl -s -X POST localhost:3000/api/v1/auth/login \
        -d '{"username":"read1","password":"a very long reader password"}' \
        | python -c 'import json,sys; print(json.load(sys.stdin)["token"])')

# 5. The session can read                                (03.01.02)
curl -s -H "Authorization: Bearer $TOKEN" localhost:3000/api/v1/status

# 6. ...but a reader cannot commit                       (03.01.05)
curl -s -o /dev/null -w '%{http_code}\n' -X POST \
     -H "Authorization: Bearer $TOKEN" \
     -d '{"message":"nope"}' localhost:3000/api/v1/commit
# 403

# 7. ...and cannot administer                            (03.01.07)
curl -s -o /dev/null -w '%{http_code}\n' \
     -H "Authorization: Bearer $TOKEN" localhost:3000/api/v1/admin/users
# 403

# 8. Logging out invalidates the token                   (03.01.11)
curl -s -X POST -H "Authorization: Bearer $TOKEN" localhost:3000/api/v1/auth/logout
curl -s -o /dev/null -w '%{http_code}\n' \
     -H "Authorization: Bearer $TOKEN" localhost:3000/api/v1/status
# 401

# 9. Five bad passwords lock the account                 (03.01.08)
for i in 1 2 3 4 5; do
  curl -s -o /dev/null -X POST localhost:3000/api/v1/auth/login \
       -d '{"username":"read1","password":"wrong"}'
done
curl -s -X POST localhost:3000/api/v1/auth/login \
     -d '{"username":"read1","password":"a very long reader password"}'
# 423 Account locked

# 10. Every one of those decisions is in the log, attributed
tail -20 ~/.lit/audit.log
lit verify

# 11. Revocation reaches a session that is already open   (03.01.01, 03.09.02)
lit server user unlock read1
TOKEN=$(curl -s -X POST localhost:3000/api/v1/auth/login \
        -d '{"username":"read1","password":"a very long reader password"}' \
        | python -c 'import json,sys; print(json.load(sys.stdin)["token"])')
curl -s -o /dev/null -w 'before revocation: %{http_code}\n' \
     -H "Authorization: Bearer $TOKEN" localhost:3000/api/v1/status
# 200

lit server user disable read1          # CLI, against the running server

curl -s -o /dev/null -w 'after revocation:  %{http_code}\n' \
     -H "Authorization: Bearer $TOKEN" localhost:3000/api/v1/status
# 403 — the open session is terminated on its next request, not at expiry
```

Step 10 is the one that matters. Each of the refusals above should appear with a
named subject, a source address, the route, and an outcome — that is
`03.03.02`, and it is the reason the account system had to exist before the
audit log was worth anything.

---

## 7. Deployment checklist

Before a server handles CUI:

- [ ] TLS terminated in a FIPS-validated module (§4), or documented as accepted risk
- [ ] MFA in front of Lit (§4), or documented as accepted risk
- [ ] `--banner` set to your approved wording, and the startup warning gone
- [ ] Administrator accounts separate from day-to-day accounts
- [ ] Audit log forwarded off-host and reviewed
- [ ] Session idle and lifetime timeouts set to your policy
- [ ] Lockout threshold set to your policy
- [ ] `~/.lit/server/users.json`, the audit log, and the TLS key confirmed owner-only
- [ ] Repository encryption configured, and `.lit/HEAD` disclosure (§5.4 of the mapping) accepted or mitigated
- [ ] `lit verify` runs clean and is scheduled
- [ ] Personnel-termination runbook uses `DELETE /api/v1/admin/users/<name>`, or restarts the server after a CLI revocation — a CLI change alone leaves open sessions alive (`docs/NIST_800-171.md` §5.0)
- [ ] Revocation actually tested: revoke an account, then confirm both that it cannot authenticate *and* that its existing session stopped working

---

## 8. See also

| Document | Covers |
| --- | --- |
| `docs/NIST_800-171.md` | Requirement-by-requirement mapping and the gap list |
| `docs/DEPLOYMENT.md` | Installing Lit, workstation configuration |
| `docs/FIPS_140-3_COMPLIANCE.md` | The at-rest cryptographic module |
| `docs/ENCRYPTION.md` | At-rest encryption and its boundaries |
| `docs/AIRGAP.md` | Isolated-environment operation |
| `docs/SECURITY_AUDIT.md` | Lit's findings against itself |
