import Link from "next/link";

const ALL_DOCS = [
  { slug: "QUICKSTART", title: "Quick Start" },
  { slug: "EXAMPLES", title: "Usage Examples" },
  { slug: "DESIGN", title: "Design Philosophy" },
  { slug: "ARCHITECTURE", title: "Architecture" },
  { slug: "ONTOLOGY", title: "Ontology & Type Graph" },
  { slug: "ROADMAP", title: "Roadmap" },
  { slug: "SECURITY", title: "Security Policy" },
  { slug: "SECURITY_AUDIT", title: "Security Audit" },
  { slug: "CRYPTOGRAPHY", title: "Cryptography" },
  { slug: "ENCRYPTION", title: "Encryption" },
  { slug: "ENCRYPTION_ENHANCEMENTS", title: "Encryption Enhancements" },
  { slug: "KEY_DISTRIBUTION", title: "Key Distribution" },
  { slug: "FIPS_140-3_COMPLIANCE", title: "FIPS 140-3 Compliance" },
  { slug: "FIPS_140-2", title: "FIPS 140-2 (Superseded)" },
  { slug: "AIRGAP", title: "Airgap Mode" },
  { slug: "DEPLOYMENT", title: "Deployment" },
  { slug: "TESTING", title: "Testing" },
  { slug: "PROJECT_SUMMARY", title: "Project Summary" },
];

export function DocPagination({ slug }: { slug: string }) {
  const idx = ALL_DOCS.findIndex((d) => d.slug === slug);
  if (idx === -1) return null;

  const prev = idx > 0 ? ALL_DOCS[idx - 1] : null;
  const next = idx < ALL_DOCS.length - 1 ? ALL_DOCS[idx + 1] : null;

  return (
    <nav
      className="mt-12 pt-6 flex justify-between gap-4"
      style={{ borderTop: "1px solid var(--border)" }}
      aria-label="Pagination"
    >
      {prev ? (
        <Link
          href={`/docs/${prev.slug}/`}
          className="group flex flex-col items-start text-sm hover:text-brand-600 dark:hover:text-brand-400 transition-colors"
        >
          <span className="text-xs text-[var(--muted)] mb-0.5">← Previous</span>
          <span className="font-medium">{prev.title}</span>
        </Link>
      ) : (
        <div />
      )}
      {next ? (
        <Link
          href={`/docs/${next.slug}/`}
          className="group flex flex-col items-end text-sm hover:text-brand-600 dark:hover:text-brand-400 transition-colors"
        >
          <span className="text-xs text-[var(--muted)] mb-0.5">Next →</span>
          <span className="font-medium">{next.title}</span>
        </Link>
      ) : (
        <div />
      )}
    </nav>
  );
}
