import fs from "fs";
import path from "path";

/** Root of the docs directory (relative to workspace root, one level up from website/) */
const DOCS_DIR = path.join(process.cwd(), "..", "docs");

/** Read a single markdown doc by slug (filename without extension) */
export function getDoc(slug: string): { slug: string; title: string; content: string } {
  const filePath = path.join(DOCS_DIR, `${slug}.md`);
  const raw = fs.readFileSync(filePath, "utf-8");

  // Extract title from first # heading
  const titleMatch = raw.match(/^#\s+(.+)$/m);
  const title = titleMatch ? titleMatch[1] : slug;

  return { slug, title, content: raw };
}

/** List all doc slugs */
export function getAllDocSlugs(): string[] {
  return fs
    .readdirSync(DOCS_DIR)
    .filter((f) => f.endsWith(".md"))
    .map((f) => f.replace(/\.md$/, ""));
}
