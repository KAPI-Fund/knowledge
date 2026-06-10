export function TopBar() {
  return (
    <header className="rounded-2xl border border-border/70 bg-card/95 px-5 py-4 shadow-[0_18px_50px_-30px_rgba(15,23,42,0.35)] backdrop-blur">
      <div className="flex items-center justify-between gap-4">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
            System
          </p>
          <p className="mt-1 text-base font-semibold text-foreground">
            Route-centered operations workbench
          </p>
        </div>
      </div>
    </header>
  );
}
