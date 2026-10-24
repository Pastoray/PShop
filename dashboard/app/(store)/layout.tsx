import { EnginePanel } from "@/components/engine/engine-panel";
import { EngineProvider } from "@/components/engine/engine-provider";
import { SiteFooter } from "@/components/store/site-footer";
import { SiteHeader } from "@/components/store/site-header";

export default function StoreLayout({ children }: { children: React.ReactNode }) {
  return (
    <EngineProvider>
      <div className="bg-paper text-ink flex min-h-screen flex-col">
        <a href="#main" className="bg-ink sr-only z-50 rounded px-3 py-2 text-white focus:not-sr-only focus:absolute focus:top-2 focus:left-2">
          Skip to content
        </a>
        <SiteHeader />
        <main id="main" className="mx-auto w-full max-w-6xl flex-1 px-4 pt-8 pb-20 sm:px-6">
          {children}
        </main>
        <SiteFooter />
      </div>
      <EnginePanel />
    </EngineProvider>
  );
}
