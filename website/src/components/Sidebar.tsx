"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

export interface DocEntry {
  slug: string;
  title: string;
  category: string;
}

const DOC_NAV: { category: string; items: { slug: string; title: string }[] }[] = [
  {
    category: "Getting Started",
    items: [
      { slug: "QUICKSTART", title: "Quick Start" },
      { slug: "EXAMPLES", title: "Usage Examples" },
      { slug: "DESIGN", title: "Design Philosophy" },
    ],
  },
  {
    category: "Architecture",
    items: [
      { slug: "ARCHITECTURE", title: "Architecture" },
      { slug: "ONTOLOGY", title: "Ontology & Type Graph" },
      { slug: "ROADMAP", title: "Roadmap" },
    ],
  },
  {
    category: "Security",
    items: [
      { slug: "SECURITY", title: "Security Policy" },
      { slug: "SECURITY_AUDIT", title: "Security Audit" },
      { slug: "CRYPTOGRAPHY", title: "Cryptography" },
      { slug: "ENCRYPTION", title: "Encryption" },
      { slug: "ENCRYPTION_ENHANCEMENTS", title: "Encryption Enhancements" },
      { slug: "KEY_DISTRIBUTION", title: "Key Distribution" },
    ],
  },
  {
    category: "Compliance",
    items: [
      { slug: "FIPS_140-3_COMPLIANCE", title: "FIPS 140-3 Compliance" },
      { slug: "FIPS_140-2", title: "FIPS 140-2 (Superseded)" },
      { slug: "AIRGAP", title: "Airgap Mode" },
    ],
  },
  {
    category: "Operations",
    items: [
      { slug: "DEPLOYMENT", title: "Deployment" },
      { slug: "TESTING", title: "Testing" },
      { slug: "PROJECT_SUMMARY", title: "Project Summary" },
    ],
  },
];

export function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="w-64 shrink-0 border-r overflow-y-auto h-[calc(100vh-3.5rem)] sticky top-14 hidden lg:block"
      style={{ borderColor: "var(--border)", background: "var(--sidebar-bg)" }}>
      <nav className="p-4 text-sm">
        {DOC_NAV.map((section) => (
          <div key={section.category} className="mb-5">
            <h3 className="font-semibold text-xs uppercase tracking-wider text-[var(--muted)] mb-2">
              {section.category}
            </h3>
            <ul className="space-y-1">
              {section.items.map((item) => {
                const href = `/docs/${item.slug}/`;
                const active = pathname === href || pathname === `/docs/${item.slug}`;
                return (
                  <li key={item.slug}>
                    <Link
                      href={href}
                      className={`block rounded-md px-2 py-1.5 transition-colors ${
                        active
                          ? "bg-brand-50 dark:bg-brand-950 text-brand-700 dark:text-brand-300 font-medium"
                          : "text-[var(--fg)] hover:bg-[var(--border)]/50"
                      }`}
                    >
                      {item.title}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </nav>
    </aside>
  );
}

export function MobileSidebar() {
  const pathname = usePathname();

  return (
    <details className="lg:hidden mb-4 border rounded-lg" style={{ borderColor: "var(--border)" }}>
      <summary className="cursor-pointer px-4 py-2 text-sm font-medium">
        Navigation
      </summary>
      <nav className="px-4 pb-3 text-sm">
        {DOC_NAV.map((section) => (
          <div key={section.category} className="mb-3">
            <h3 className="font-semibold text-xs uppercase tracking-wider text-[var(--muted)] mb-1">
              {section.category}
            </h3>
            <ul className="space-y-0.5">
              {section.items.map((item) => {
                const href = `/docs/${item.slug}/`;
                const active = pathname === href || pathname === `/docs/${item.slug}`;
                return (
                  <li key={item.slug}>
                    <Link
                      href={href}
                      className={`block rounded px-2 py-1 ${
                        active
                          ? "text-brand-600 dark:text-brand-400 font-medium"
                          : "text-[var(--fg)]"
                      }`}
                    >
                      {item.title}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </nav>
    </details>
  );
}
