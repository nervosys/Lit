import { Sidebar, MobileSidebar } from "@/components/Sidebar";
import { CopyCodeBlocks } from "@/components/CopyButton";

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="max-w-[100rem] mx-auto flex flex-col lg:flex-row">
      <Sidebar />
      <div className="flex-1 min-w-0 px-4 sm:px-6 lg:px-8 py-8">
        <MobileSidebar />
        {children}
        <CopyCodeBlocks />
      </div>
    </div>
  );
}
