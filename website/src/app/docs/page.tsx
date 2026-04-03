import Link from "next/link";

const SECTIONS = [
  {
    title: "Getting Started",
    description: "Install Lit, learn the basics, and understand the design philosophy.",
    docs: [
      { slug: "QUICKSTART", label: "Quick Start Guide", desc: "Installation and first steps" },
      { slug: "EXAMPLES", label: "Examples", desc: "Common workflows and command usage" },
      { slug: "DESIGN", label: "Design", desc: "Design philosophy and architecture decisions" },
    ],
  },
  {
    title: "Architecture",
    description: "Deep dive into Lit's internal architecture and type system.",
    docs: [
      { slug: "ARCHITECTURE", label: "Architecture", desc: "System architecture and module organization" },
      { slug: "ONTOLOGY", label: "Ontology", desc: "Type graph and semantic relationships" },
      { slug: "ROADMAP", label: "Roadmap", desc: "Planned features and milestones" },
    ],
  },
  {
    title: "Decentralized",
    description: "DID identity, UCAN delegation, local-first issues & PRs, and peer-to-peer federation.",
    docs: [
      { slug: "FEDERATION", label: "Identity, Issues & Federation", desc: "DID, UCAN, trust, issues, PRs, events, delegation, and peer sync" },
    ],
  },
  {
    title: "Security",
    description: "Cryptographic primitives, encryption, key management, and audit reports.",
    docs: [
      { slug: "SECURITY", label: "Security", desc: "Security policy and vulnerability reporting" },
      { slug: "SECURITY_AUDIT", label: "Security Audit", desc: "CVE/ATT&CK/FIPS/CMMC audit report" },
      { slug: "CRYPTOGRAPHY", label: "Cryptography", desc: "Post-quantum algorithms and hashing" },
      { slug: "ENCRYPTION", label: "Encryption", desc: "AES-256-GCM at-rest encryption" },
      { slug: "ENCRYPTION_ENHANCEMENTS", label: "Encryption Enhancements", desc: "Advanced encryption features" },
      { slug: "KEY_DISTRIBUTION", label: "Key Distribution", desc: "Key exchange and management" },
    ],
  },
  {
    title: "Compliance",
    description: "NIST FIPS 140-3 compliance, airgap mode, and regulatory frameworks.",
    docs: [
      { slug: "FIPS_140-3_COMPLIANCE", label: "FIPS 140-3 Compliance", desc: "NIST FIPS 140-3 Level 1 compliance report" },
      { slug: "FIPS_140-2", label: "FIPS 140-2 (Superseded)", desc: "Legacy FIPS 140-2 documentation" },
      { slug: "AIRGAP", label: "Airgap Mode", desc: "Fully offline operation and transport controls" },
    ],
  },
  {
    title: "Operations",
    description: "Deployment, testing, and project overview.",
    docs: [
      { slug: "DEPLOYMENT", label: "Deployment", desc: "Build targets and deployment strategies" },
      { slug: "TESTING", label: "Testing", desc: "Test framework and security test suite" },
      { slug: "PROJECT_SUMMARY", label: "Project Summary", desc: "Project metrics and overview" },
    ],
  },
];

export default function DocsIndex() {
  return (
    <div className="max-w-3xl">
      <h1 className="text-3xl font-bold tracking-tight mb-2">Documentation</h1>
      <p className="text-[var(--muted)] mb-10">
        Welcome to the Lit VCS documentation. Choose a topic below or use the
        sidebar to navigate.
      </p>
      {SECTIONS.map((section) => (
        <section key={section.title} className="mb-10">
          <h2 className="text-xl font-semibold mb-1">{section.title}</h2>
          <p className="text-sm text-[var(--muted)] mb-4">{section.description}</p>
          <div className="grid gap-2">
            {section.docs.map((doc) => (
              <Link
                key={doc.slug}
                href={`/docs/${doc.slug}`}
                className="flex items-baseline gap-3 rounded-lg border border-[var(--border)] px-4 py-3 hover:border-brand-400 transition-colors group"
              >
                <span className="font-medium text-sm group-hover:text-brand-600 dark:group-hover:text-brand-400 transition-colors">
                  {doc.label}
                </span>
                <span className="text-xs text-[var(--muted)]">{doc.desc}</span>
              </Link>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
