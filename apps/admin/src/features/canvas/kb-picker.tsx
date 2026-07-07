import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";

import { groupAndFilter } from "./kb-grouping";
import { useAccessibleKnowledgeBases } from "./use-accessible-kbs";

interface KbProjectPickerProps {
  onPick: (project: { id: string; name: string }) => void;
}

const ROLE_LABEL: Record<"owner" | "editor" | "viewer", string> = {
  owner: "Owner",
  editor: "Editor",
  viewer: "Viewer",
};

export function KbProjectPicker({ onPick }: KbProjectPickerProps) {
  const { items, isLoading, isError } = useAccessibleKnowledgeBases();
  const [query, setQuery] = useState("");
  const groups = useMemo(() => groupAndFilter(items, query), [items, query]);

  return (
    <div className="flex flex-col gap-3">
      <Input
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        placeholder="Search knowledge bases..."
        aria-label="Search knowledge bases"
      />
      <ScrollArea className="max-h-[50vh] pr-1">
        {isLoading ? (
          <div className="space-y-1.5">
            {[0, 1, 2, 3].map((row) => (
              <Skeleton key={row} className="h-9 w-full" />
            ))}
          </div>
        ) : isError ? (
          <p className="py-10 text-center text-sm text-destructive">
            Failed to load knowledge bases.
          </p>
        ) : groups.length === 0 ? (
          <p className="py-10 text-center text-sm text-muted-foreground">
            {items.length === 0 ? "No knowledge bases available." : "No matches."}
          </p>
        ) : (
          <div className="space-y-4">
            {groups.map((group) => (
              <div key={group.key} className="space-y-1">
                <p className="px-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">
                  {group.label}
                </p>
                <ul className="space-y-0.5">
                  {group.items.map((kb) => (
                    <li key={kb.id}>
                      <Button
                        type="button"
                        variant="ghost"
                        className="h-auto w-full justify-between gap-2 px-2 py-2 font-normal"
                        onClick={() => onPick({ id: kb.id, name: kb.name })}
                      >
                        <span className="truncate">{kb.name}</span>
                        <Badge variant="outline" className="shrink-0">
                          {ROLE_LABEL[kb.role]}
                        </Badge>
                      </Button>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        )}
      </ScrollArea>
    </div>
  );
}
