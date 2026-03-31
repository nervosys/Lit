import Link from "next/link";

const FEATURES = [
  { title: "Post-Quantum Cryptography", desc: "ML-DSA-87 (FIPS 204), AES-256-GCM, SHA3-512 + BLAKE3 hashing" },
  { title: "42 CLI Commands", desc: "Full Git-compatible command set plus sandboxes, swarms, and ontology" },
  { title: "30 MCP Tools", desc: "First-class AI agent integration via Model Context Protocol" },
  { title: "Sandboxed Execution", desc: "Process-isolated environments for untrusted operations" },
  { title: "Airgap Mode", desc: "Fully offline operation with transport permission controls" },
  { title: "4 Transport Protocols", desc: "HTTPS, SSH, lit://, and stdio for maximum flexibility" },
];

export default function Home() {
  return (
    <main className="max-w-5xl mx-auto px-6 py-16">
      <section className="text-center mb-20">
        <h1 className="text-5xl font-bold tracking-tight mb-4">
          <span className="text-brand-600">Lit</span> Version Control
        </h1>
        <p className="text-xl text-[var(--muted)] max-w-2xl mx-auto mb-8">
          The first agentic-first distributed version control system. Built in
          Rust with post-quantum cryptography, sandbox isolation, and native AI
          agent support.
        </p>
        <div className="flex gap-4 justify-center">
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
      </section>

      <section className="grid md:grid-cols-2 lg:grid-cols-3 gap-6">
        {FEATURES.map((f) => (
          <div
            key={f.title}
            className="border border-[var(--border)] rounded-lg p-6 hover:border-brand-400 transition-colors"
          >
            <h3 className="font-semibold text-lg mb-2">{f.title}</h3>
            <p className="text-sm text-[var(--muted)]">{f.desc}</p>
          </div>
        ))}
      </section>
    </main>
  );
}
