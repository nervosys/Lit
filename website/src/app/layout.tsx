import type { Metadata } from "next";
import "./globals.css";
import { Header } from "@/components/Header";
import { Footer } from "@/components/Footer";
import { ThemeProvider } from "@/components/ThemeProvider";

export const metadata: Metadata = {
  title: "Lit — Agentic-First Distributed Version Control",
  description:
    "The world's first version control system designed for AI agents first and humans second. Post-quantum cryptography, sandboxed execution, and 30 MCP tools.",
  keywords: [
    "version control",
    "VCS",
    "Git alternative",
    "post-quantum cryptography",
    "AI agents",
    "MCP",
    "Model Context Protocol",
    "Rust",
    "FIPS 140-3",
    "airgap",
  ],
  authors: [{ name: "Nervosys" }],
  openGraph: {
    title: "Lit — Agentic-First Distributed Version Control",
    description:
      "Built in Rust with ML-DSA-87 post-quantum signatures, AES-256-GCM encryption, 42 CLI commands, and 30 MCP tools for AI agent integration.",
    url: "https://github.com/nervosys/Lit",
    siteName: "Lit VCS",
    type: "website",
    locale: "en_US",
  },
  twitter: {
    card: "summary_large_image",
    title: "Lit — Agentic-First Distributed Version Control",
    description:
      "Post-quantum cryptography, sandboxed execution, and native AI agent support. Built in Rust.",
  },
  robots: { index: true, follow: true },
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="antialiased flex flex-col min-h-screen">
        <ThemeProvider>
          <Header />
          <div className="flex-1">{children}</div>
          <Footer />
        </ThemeProvider>
      </body>
    </html>
  );
}
