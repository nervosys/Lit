import { Sidebar, MobileSidebar } from "@/components/Sidebar";

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="max-w-7xl mx-auto px-4 sm:px-6">
      <MobileSidebar />
      <div className="flex gap-8">
        <aside className="hidden md:block w-64 shrink-0 py-8">
          <Sidebar />
        </aside>
        <main className="min-w-0 flex-1 py-8">{children}</main>
      </div>
    </div>
  );
}
