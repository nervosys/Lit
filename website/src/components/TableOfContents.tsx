"use client";

import { useEffect, useState } from "react";

interface TocItem {
  id: string;
  text: string;
  level: number;
}

export function TableOfContents() {
  const [items, setItems] = useState<TocItem[]>([]);
  const [activeId, setActiveId] = useState<string>("");

  useEffect(() => {
    const article = document.querySelector("article.prose");
    if (!article) return;

    const headings = article.querySelectorAll("h2, h3");
    const toc: TocItem[] = [];
    headings.forEach((h) => {
      if (h.id) {
        toc.push({
          id: h.id,
          text: h.textContent?.replace(/ #$/, "") || "",
          level: h.tagName === "H2" ? 2 : 3,
        });
      }
    });
    setItems(toc);
  }, []);

  useEffect(() => {
    if (items.length === 0) return;

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setActiveId(entry.target.id);
            break;
          }
        }
      },
      { rootMargin: "-80px 0px -60% 0px", threshold: 0.1 }
    );

    items.forEach((item) => {
      const el = document.getElementById(item.id);
      if (el) observer.observe(el);
    });

    return () => observer.disconnect();
  }, [items]);

  if (items.length < 3) return null;

  return (
    <aside className="hidden xl:block w-56 shrink-0 ml-8">
      <div className="sticky top-20">
        <h4 className="text-xs font-semibold uppercase tracking-wider text-[var(--muted)] mb-3">
          On this page
        </h4>
        <nav className="text-sm space-y-1 max-h-[calc(100vh-8rem)] overflow-y-auto">
          {items.map((item) => (
            <a
              key={item.id}
              href={`#${item.id}`}
              className={`block py-0.5 transition-colors ${
                item.level === 3 ? "pl-3" : ""
              } ${
                activeId === item.id
                  ? "text-brand-600 dark:text-brand-400 font-medium"
                  : "text-[var(--muted)] hover:text-[var(--fg)]"
              }`}
            >
              {item.text}
            </a>
          ))}
        </nav>
      </div>
    </aside>
  );
}
