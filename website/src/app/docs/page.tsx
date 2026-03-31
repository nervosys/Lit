import Link from "next/link";

const SECTIONS = [
  {
    title: "Getting Started",
    docs: [
      { slug: "QUICKSTART", label: "Quick Start Guide" },
      { slug: "EXAMPLES", label: "Examples" },
      { slug: "DESIGN", label: "Design" },
    ],
  },
  {
    title: "Architecture",
    docs: [
      { slug: "ARCHITECTURE", label: "Architecture" },
      { slug: "ONTOLOGY", label: "Ontology" },
      { slug: "ROADMAP", label: "Roadmap" },
    ],
  },
  {
    title: "Security",
    docs: [
      { slug: "SECURITY", label: "Security" },
      { slug: "SECURITY_AUDIT", label: "Security Audit" },
      { slug: "CRYPTOGRAPHY", label: "Cryptography" },
      { slug: "ENCRYPTION", label: "Encryption" },
      { slug: "ENCRYPTION_ENHANCEMENTS", label: "Encryption Enhancements" },
      { slug: "KEY_DISTRIBUTION", label: "Key Distribution" },
    ],
  },
  {
    title: "Compliance",
    docs: [
      { slug: "FIPS_140-3_COMPLIANCE", label: "FIPS 140-3 Compliance" },
      { slug: "FIPS_140-2", label: "FIPS 140-2 (Superseded)" },
      { slug: "AIRGAP", label: "Airgap Mode" },
    ],
  },
  {
    title: "Operations",
    docs: [
      { slug: "DEPLOYMENT", label: "Deployment" },
      { slug: "TESTING", label: "Testing" },
      { slug: "PROJECT_SUMMARY", label: "Project Summary" },
    ],
  },
];

export default function DocsIndex() {
  return (
    <div className="prose">
      <h1>Documentation</h1>
      <p>
        Welcome to the Lit VCS documentation. Choose a topic below or use the
        sidebar to navigate.
      </p>
      {SECTIONS.map((section) => (
        <section key={section.title} className="mb-8">
          <h2>{section.title}</h2>
          <ul>
            {section.docs.map((doc) => (
              <li key={doc.slug}>
                <Link href={`/docs/${doc.slug}`}>{doc.label}</Link>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
