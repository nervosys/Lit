import type { Metadata } from "next";
import "./globals.css";
import { Header } from "@/components/Header";

export const metadata: Metadata = {
  title: "Lit — Agentic-First Distributed Version Control",
  description:
    "The world's first version control system designed for AI agents first and humans second. Post-quantum cryptography, sandboxed execution, and 30 MCP tools.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="antialiased">
        <Header />
        {children}
      </body>
    </html>
  );
}
