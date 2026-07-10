import { LibraryBig } from "lucide-react";
import type { ReactNode } from "react";

export function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <main className="grid min-h-svh lg:grid-cols-2">
      <div className="relative hidden flex-col justify-between bg-sidebar p-10 lg:flex">
        <div className="flex items-center gap-2">
          <div className="flex size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <LibraryBig className="size-4" />
          </div>
          <span className="text-lg font-semibold tracking-tight">Knowledge</span>
        </div>
        <div className="grid gap-2">
          <p className="text-lg font-medium leading-snug">
            Turn scattered sources into a living, searchable knowledge base.
          </p>
          <p className="text-sm text-muted-foreground">
            Import, review, deduplicate, and explore your team's knowledge — from raw documents to
            a connected graph.
          </p>
        </div>
      </div>
      <div className="flex items-center justify-center p-6 lg:p-10">
        <div className="grid w-full max-w-sm gap-6">{children}</div>
      </div>
    </main>
  );
}
