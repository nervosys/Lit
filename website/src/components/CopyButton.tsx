"use client";

import { useState, useEffect, useCallback } from "react";

export function CopyCodeBlocks() {
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const handleClick = useCallback(async (e: MouseEvent) => {
    const button = (e.target as HTMLElement).closest("[data-copy-btn]");
    if (!button) return;
    const pre = button.closest(".code-block-wrapper")?.querySelector("pre");
    if (!pre) return;
    const text = pre.textContent || "";
    try {
      await navigator.clipboard.writeText(text);
      const id = button.getAttribute("data-copy-btn")!;
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch {
      // clipboard API not available
    }
  }, []);

  useEffect(() => {
    // Wrap all pre elements in prose with copy buttons
    const proseEl = document.querySelector(".prose");
    if (!proseEl) return;

    const pres = proseEl.querySelectorAll("pre");
    pres.forEach((pre, i) => {
      if (pre.parentElement?.classList.contains("code-block-wrapper")) return;
      const wrapper = document.createElement("div");
      wrapper.className = "code-block-wrapper relative group";
      pre.parentElement?.insertBefore(wrapper, pre);
      wrapper.appendChild(pre);

      const btn = document.createElement("button");
      btn.setAttribute("data-copy-btn", `cb-${i}`);
      btn.className =
        "absolute top-2 right-2 p-1.5 rounded-md text-xs opacity-0 group-hover:opacity-100 transition-opacity bg-[var(--border)] hover:bg-[var(--muted)] text-[var(--fg)]";
      btn.setAttribute("aria-label", "Copy code");
      btn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;
      wrapper.appendChild(btn);
    });

    document.addEventListener("click", handleClick);
    return () => document.removeEventListener("click", handleClick);
  }, [handleClick]);

  useEffect(() => {
    if (!copiedId) return;
    const btn = document.querySelector(`[data-copy-btn="${copiedId}"]`);
    if (btn) {
      btn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>`;
      const timeout = setTimeout(() => {
        btn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;
      }, 2000);
      return () => clearTimeout(timeout);
    }
  }, [copiedId]);

  return null;
}
