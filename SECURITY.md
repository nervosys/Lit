# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in Lit, please report it responsibly.

**Do not open a public GitHub issue for security vulnerabilities.**

### How to Report

Email **security@nervosys.ai** with:

1. A description of the vulnerability
2. Steps to reproduce (if applicable)
3. The affected version(s)
4. Any potential impact assessment

### What to Expect

- **Acknowledgement** within 48 hours
- **Assessment** within 7 business days
- **Fix timeline** communicated after assessment — critical issues are prioritized
- **Credit** in the advisory (unless you prefer to remain anonymous)

### Scope

The following are in scope:

- Cryptographic weaknesses (key derivation, signatures, encryption)
- Authentication or authorization bypasses
- Sandbox escapes or isolation failures
- Path traversal or symlink attacks
- Memory safety issues
- Information disclosure (passphrase, keys, internal paths in error messages)
- Audit log tampering or bypass
- Denial of service via malformed input

### Out of Scope

- Physical coercion attacks (see [SECURITY.md § Known Limitations](docs/SECURITY.md))
- Attacks requiring pre-existing privileged access to the host
- Social engineering

## Security Documentation

For detailed security architecture and threat model, see:

- [docs/SECURITY.md](docs/SECURITY.md) — Security model and threat mitigation
- [docs/SECURITY_AUDIT.md](docs/SECURITY_AUDIT.md) — DoD-standard security audit (14 findings, all remediated)
- [docs/CRYPTOGRAPHY.md](docs/CRYPTOGRAPHY.md) — Cryptographic design
- [docs/ENCRYPTION.md](docs/ENCRYPTION.md) — Encryption system
- [docs/FIPS_140-3_COMPLIANCE.md](docs/FIPS_140-3_COMPLIANCE.md) — FIPS 140-3 compliance
