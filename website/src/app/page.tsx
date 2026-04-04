import Link from "next/link";

const FEATURES = [
  { icon: "🔐", title: "Post-Quantum Cryptography", desc: "ML-DSA-87 (FIPS 204), AES-256-GCM, SHA3-512 + BLAKE3 hashing" },
  { icon: "⚡", title: "52 CLI Commands", desc: "Full Git-compatible command set plus identity, federation, issues, and agent delegation" },
  { icon: "🤖", title: "30 MCP Tools", desc: "First-class AI agent integration via Model Context Protocol" },
  { icon: "🆔", title: "DID Identity & UCAN", desc: "Decentralized identifiers with fine-grained capability delegation tokens" },
  { icon: "🎫", title: "Local-First Issues & PRs", desc: "Issue tracker and pull requests stored as git refs — no server required" },
  { icon: "🌍", title: "Peer-to-Peer Federation", desc: "Content-addressed repository sync with want-list negotiation" },
  { icon: "📦", title: "Sandboxed Execution", desc: "Process-isolated environments for untrusted operations" },
  { icon: "✈️", title: "Airgap Mode", desc: "Fully offline operation with transport permission controls" },
  { icon: "🌐", title: "4 Transport Protocols", desc: "HTTPS, SSH, lit://, and stdio for maximum flexibility" },
  { icon: "🎯", title: "Intent → Commit → Converge", desc: "Agentic workflow replacing branch/PR with scoped intents, commit attachment, and trust-gated convergence" },
];

const COMPARISON = [
  { feature: "Post-quantum signatures", lit: true, git: false },
  { feature: "At-rest encryption (AES-256-GCM)", lit: true, git: false },
  { feature: "AI agent support (MCP)", lit: true, git: false },
  { feature: "DID-based identity", lit: true, git: false },
  { feature: "UCAN capability delegation", lit: true, git: false },
  { feature: "Local-first issues & PRs", lit: true, git: false },
  { feature: "Content-addressed federation", lit: true, git: false },
  { feature: "Agent trust scoring", lit: true, git: false },
  { feature: "Sandboxed execution", lit: true, git: false },
  { feature: "Airgap mode", lit: true, git: false },
  { feature: "Intent → Converge workflow", lit: true, git: false },
  { feature: "FIPS 140-3 compliance", lit: true, git: false },
  { feature: "HMAC-signed audit logs", lit: true, git: false },
  { feature: "Brute-force protection", lit: true, git: false },
  { feature: "Git-compatible CLI", lit: true, git: true },
  { feature: "Distributed version control", lit: true, git: true },
];

const SECURITY_HIGHLIGHTS = [
  { label: "FIPS 140-3 Level 1", detail: "All cryptographic modules use NIST-approved algorithms" },
  { label: "CMMC 2.0 Level 2", detail: "22 of 26 assessed practices met for DoD workflows" },
  { label: "Zero Critical CVEs", detail: "Full security audit with 9 findings, all remediated" },
  { label: "NIST CAVP Vectors", detail: "Known Answer Tests from NIST Cryptographic Algorithm Validation Program" },
];

export default function Home() {
  return (
    <main className="max-w-7xl mx-auto px-6 py-16">
      {/* Hero */}
      <section className="text-center mb-20">
        <div className="inline-block mb-4 px-3 py-1 text-xs font-medium rounded-full bg-brand-100 dark:bg-brand-900 text-brand-700 dark:text-brand-300">
          v1.0.0 — AGPL-3.0-or-later
        </div>
        <h1 className="text-5xl font-bold tracking-tight mb-4">
          <span className="text-brand-600 dark:text-brand-400">Lit</span> Version Control
        </h1>
        <p className="text-xl text-[var(--muted)] max-w-2xl mx-auto mb-8">
          The first agentic-first distributed version control system. Built in
          Rust with post-quantum cryptography, sandbox isolation, and native AI
          agent support.
        </p>
        <div className="flex gap-4 justify-center mb-10">
          <Link
            href="/docs/QUICKSTART"
            className="px-6 py-3 bg-brand-600 text-white rounded-lg font-medium hover:bg-brand-700 transition-colors"
          >
            Get Started
          </Link>
          <Link
            href="/docs"
            className="px-6 py-3 border border-[var(--border)] rounded-lg font-medium hover:bg-[var(--sidebar-bg)] transition-colors"
          >
            Documentation
          </Link>
        </div>

        {/* Install command */}
        <div className="max-w-lg mx-auto">
          <div className="relative rounded-lg border border-[var(--border)] bg-[var(--code-bg)] px-4 py-3 text-sm font-mono text-center">
            <span className="text-[var(--muted)] select-none">$ </span>
            <span>cargo install lit-vcs</span>
          </div>
        </div>
      </section>

      {/* Feature cards */}
      <section className="mb-20">
        <h2 className="text-2xl font-bold text-center mb-8">Features</h2>
        <div className="grid md:grid-cols-2 lg:grid-cols-3 gap-6">
          {FEATURES.map((f) => (
            <div
              key={f.title}
              className="border border-[var(--border)] rounded-lg p-6 hover:border-brand-400 transition-colors"
            >
              <div className="text-2xl mb-3">{f.icon}</div>
              <h3 className="font-semibold text-lg mb-2">{f.title}</h3>
              <p className="text-sm text-[var(--muted)]">{f.desc}</p>
            </div>
          ))}
        </div>
      </section>

      {/* Comparison table */}
      <section className="mb-20">
        <h2 className="text-2xl font-bold text-center mb-2">Lit vs Git</h2>
        <p className="text-center text-[var(--muted)] mb-8">
          Everything Git does, plus security and AI-native capabilities.
        </p>
        <div className="overflow-x-auto">
          <table className="w-full text-sm border border-[var(--border)] rounded-lg overflow-hidden">
            <thead>
              <tr className="bg-[var(--sidebar-bg)]">
                <th className="text-left px-4 py-3 font-semibold">Feature</th>
                <th className="text-center px-4 py-3 font-semibold text-brand-600 dark:text-brand-400">Lit</th>
                <th className="text-center px-4 py-3 font-semibold">Git</th>
              </tr>
            </thead>
            <tbody>
              {COMPARISON.map((row) => (
                <tr key={row.feature} className="border-t border-[var(--border)]">
                  <td className="px-4 py-2.5">{row.feature}</td>
                  <td className="px-4 py-2.5 text-center">{row.lit ? "✅" : "—"}</td>
                  <td className="px-4 py-2.5 text-center">{row.git ? "✅" : "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {/* Security highlights */}
      <section className="mb-20">
        <h2 className="text-2xl font-bold text-center mb-2">Security & Compliance</h2>
        <p className="text-center text-[var(--muted)] mb-8">
          Audited against CVE, MITRE ATT&CK v15, NIST FIPS 140-3, and CMMC 2.0 Level 2.
        </p>
        <div className="grid sm:grid-cols-2 gap-4">
          {SECURITY_HIGHLIGHTS.map((s) => (
            <div
              key={s.label}
              className="border border-[var(--border)] rounded-lg p-5 bg-[var(--sidebar-bg)]"
            >
              <div className="font-semibold text-brand-600 dark:text-brand-400 mb-1">{s.label}</div>
              <div className="text-sm text-[var(--muted)]">{s.detail}</div>
            </div>
          ))}
        </div>
        <div className="text-center mt-6">
          <Link
            href="/docs/SECURITY_AUDIT"
            className="text-sm text-[var(--link)] hover:underline"
          >
            Read the full security audit report →
          </Link>
        </div>
      </section>
    </main>
  );
}
