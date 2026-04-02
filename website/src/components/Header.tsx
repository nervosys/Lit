"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { ThemeToggle } from "@/components/ThemeProvider";

export function Header() {
  const pathname = usePathname();
  const isDocsActive = pathname.startsWith("/docs");

  return (
    <header className="sticky top-0 z-50 border-b backdrop-blur-xl"
      style={{
        borderColor: "var(--border)",
        backgroundColor: "color-mix(in srgb, var(--bg) 85%, transparent)",
      }}>
      {/* Top accent line */}
      <div className="h-px w-full bg-gradient-to-r from-transparent via-[var(--accent)] to-transparent opacity-50" />
      <div className="mx-auto flex h-14 max-w-7xl items-center justify-between px-4 sm:px-6">
        <Link href="/" className="flex items-center gap-2.5 group">
          {/* Logo mark */}
          <div className="relative flex items-center justify-center w-8 h-8">
            <div className="absolute inset-0 rounded bg-[var(--accent)] opacity-10 group-hover:opacity-20 transition-opacity" />
            <span className="font-mono font-bold text-lg text-[var(--accent)]">L</span>
          </div>
          <div className="flex items-baseline gap-2">
            <span className="font-bold text-lg tracking-tight text-[var(--fg)]">Lit</span>
            <span className="hidden sm:inline font-mono text-[10px] tracking-widest uppercase text-[var(--muted)]">
              v1.0.0
            </span>
          </div>
        </Link>

        <nav className="flex items-center gap-1 text-sm">
          <Link
            href="/docs/"
            className={`px-3 py-1.5 rounded transition-all font-mono text-xs uppercase tracking-wider ${
              isDocsActive
                ? "text-[var(--accent)] bg-[var(--accent)]/10"
                : "text-[var(--muted)] hover:text-[var(--fg)] hover:bg-[var(--border)]/30"
            }`}
          >
            Docs
          </Link>
          <a
            href="https://github.com/nervosys/Lit"
            target="_blank"
            rel="noopener noreferrer"
            className="px-3 py-1.5 rounded text-[var(--muted)] hover:text-[var(--fg)] hover:bg-[var(--border)]/30 transition-all font-mono text-xs uppercase tracking-wider"
          >
            GitHub
          </a>
          <div className="w-px h-5 bg-[var(--border)] mx-1" />
          <ThemeToggle />
        </nav>
      </div>
    </header>
  );
}
