# Decentralized Features

Lit v1.1 introduces decentralized identity, capability delegation, local-first collaboration tools, and peer-to-peer federation — enabling fully autonomous agent workflows without centralized servers.

## DID Identity

Lit uses [Decentralized Identifiers (DIDs)](https://www.w3.org/TR/did-core/) to establish cryptographic identity for agents and users.

### Generate Identity

```bash
lit did generate                          # Ed25519 key by default
lit did generate --method ml-dsa-87       # Post-quantum key
```

Generates a `did:key` identifier backed by either Ed25519 or ML-DSA-87 (NIST FIPS 204) keys. The identity is stored in `.lit/identity/`.

### Show & Resolve

```bash
lit did show                              # Display your current DID document
lit did resolve did:key:z6Mk...          # Resolve any DID to its document
```

## UCAN Capability Delegation

[UCANs (User Controlled Authorization Networks)](https://ucan.xyz/) provide fine-grained, cryptographically signed permission tokens that can be delegated without a central authority.

### Issue Tokens

```bash
lit ucan issue <audience-did> --resource repo --action push
lit ucan issue <audience-did> --resource "branch:main" --action merge --expiry 3600
```

### Manage Tokens

```bash
lit ucan list                             # List all tokens
lit ucan list <audience-did>              # Filter by audience
lit ucan revoke <cid>                     # Revoke a specific token
```

## Agent Trust Scoring

Lit tracks agent reputation using an event-driven trust model with five levels: unknown, low, medium, high, and trusted.

```bash
lit trust show <did>                      # Current score and level
lit trust list                            # All tracked agents
lit trust history <did>                   # Full event log
```

Trust scores update based on agent behavior: successful operations increase trust, while failures or violations decrease it. Scores decay over time if an agent becomes inactive.

## Local-First Issues

Issues are stored as git refs under `refs/issues/` — no server required.

```bash
lit issue create "Bug in merge logic" --body "Steps to reproduce..." --label bug
lit issue list --state open
lit issue show 1
lit issue comment 1 "Fixed in commit abc123"
lit issue close 1
```

## Local-First Pull Requests

Pull requests are stored as git refs under `refs/prs/` — works fully offline.

```bash
lit pr create "Add federation support" --head feature/federation --base main
lit pr list --state open
lit pr show 1
lit pr comment 1 "LGTM, ready to merge"
lit pr merge 1
lit pr close 2
```

## Event Subscriptions

Subscribe to repository events and read them from the event log.

```bash
lit subscribe add commit merge --branch main
lit subscribe list
lit subscribe events --event-type commit --limit 10
lit subscribe remove <subscription-id>
```

Supported event types: `commit`, `branch`, `merge`, `tag`, `push`.

## Agent Task Delegation

Delegate tasks between agents using a structured protocol with priority and status tracking.

```bash
lit delegate create <agent-did> "Review PR #3" --priority high
lit delegate accept <task-id>
lit delegate complete <task-id> "Reviewed, approved with comments"
lit delegate list --status pending
lit delegate show <task-id>
```

Task statuses: `pending`, `accepted`, `in-progress`, `completed`, `rejected`.

## Peer-to-Peer Federation

Federate repositories across peers using content-addressed sync with want-list negotiation.

```bash
lit peer add <did> --endpoint https://peer.example.com --public-key <hex>
lit peer list
lit peer show <did>
lit peer sync <did>                       # Sync objects with peer
lit peer remove <did>
```

Peers are identified by DID and authenticated with their public key. The sync protocol uses want-list negotiation to transfer only the objects each peer needs.
