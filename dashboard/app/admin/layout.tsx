import type { Metadata } from "next";

export const metadata: Metadata = { title: "Operations" };

// The ops console is dark: scope the shadcn `.dark` tokens to this subtree only.
export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return <div className="dark bg-background text-foreground min-h-screen">{children}</div>;
}
