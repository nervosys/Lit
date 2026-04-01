"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { ThemeToggle } from "@/components/ThemeProvider";

export function Header() {
  const pathname = usePathname();
  const isDocsActive = pathname.startsWith("/docs");

  return (
    <header className="sticky top-0 z-50 border-b backdrop-blur-md"
      style={{ borderColor: "var(--border)", backgroundColor: "color-mix(in srgb, var(--bg) 80%, transparent)" }}>
      <div className="mx-auto flex h-14 max-w-7xl items-center justify-between px-4 sm:px-6">
        <Link href="/" className="flex items-center gap-2 font-bold text-lg tracking-tight">
          <span className="text-brand-600 dark:text-brand-400">Lit</span>
          <span className="hidden sm:inline text-[var(--muted)] font-normal text-sm">
            Agentic VCS
          </span>
        </Link>

        <nav className="flex items-center gap-6 text-sm">
          <Link
            href="/docs/"
            className={`transition-colors ${
              isDocsActive
                ? "text-brand-600 dark:text-brand-400 font-medium"
                : "text-[var(--muted)] hover:text-[var(--fg)]"
            }`}
          >
            Docs
          </Link>
          <a
            href="https://github.com/nervosys/Lit"
            target="_blank"
            rel="noopener noreferrer"
            className="text-[var(--muted)] hover:text-[var(--fg)] transition-colors"
          >
            GitHub
          </a>
          <ThemeToggle />
        </nav>
      </div>
    </header>
  );
}
