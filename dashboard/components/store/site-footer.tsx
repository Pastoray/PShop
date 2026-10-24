import Link from "next/link";

export function SiteFooter() {
  return (
    <footer className="border-line bg-ink border-t text-white/75">
      <div className="mx-auto flex max-w-6xl flex-wrap items-end justify-between gap-6 px-4 py-10 text-sm sm:px-6">
        <div>
          <p className="display text-4xl text-white">PShop<span className="text-tag">.</span></p>
          <p className="mt-3 max-w-xs leading-snug">Free returns for 30 days. Orders ship within two working days.</p>
        </div>
        <div className="max-w-sm leading-snug">
          <p>This is a simulation. Orders are real to the system, but no money moves and nothing ships.</p>
          <p className="mt-3">
            <Link href="/admin" className="underline underline-offset-4 hover:text-white">Operations dashboard</Link>
          </p>
        </div>
      </div>
    </footer>
  );
}
