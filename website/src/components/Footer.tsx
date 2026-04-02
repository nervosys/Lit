export function Footer() {
  return (
    <footer
      className="border-t text-sm text-[var(--muted)] py-8 mt-16"
      style={{ borderColor: "var(--border)" }}
    >
      <div className="max-w-7xl mx-auto px-4 sm:px-6 flex flex-col sm:flex-row items-center justify-between gap-4">
        <p>&copy; {new Date().getFullYear()} Nervosys. All rights reserved.</p>
        <div className="flex items-center gap-6">
          <a href="/docs/" className="hover:text-[var(--fg)] transition-colors">
            Docs
          </a>
          <a href="/docs/SECURITY/" className="hover:text-[var(--fg)] transition-colors">
            Security
          </a>
          <a
            href="https://github.com/nervosys/Lit"
            target="_blank"
            rel="noopener noreferrer"
            className="hover:text-[var(--fg)] transition-colors"
          >
            GitHub
          </a>
          <a
            href="mailto:licensing@nervosys.ai"
            className="hover:text-[var(--fg)] transition-colors"
          >
            Commercial Licensing
          </a>
        </div>
      </div>
    </footer>
  );
}
