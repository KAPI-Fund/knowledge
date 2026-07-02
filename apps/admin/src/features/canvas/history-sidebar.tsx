import { Plus } from "lucide-react";
import { useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

import { useCanvasList, useCreateCanvas } from "./queries";

interface HistorySidebarProps {
  activeId?: string;
}

export function HistorySidebar({ activeId }: HistorySidebarProps) {
  const navigate = useNavigate();
  const list = useCanvasList();
  const createCanvas = useCreateCanvas();

  const onCreate = () => {
    createCanvas.mutate(undefined, {
      onSuccess: (canvas) => navigate(`/canvas/${canvas.id}`),
    });
  };

  return (
    <aside className="flex h-full w-64 shrink-0 flex-col border-r bg-sidebar text-sidebar-foreground">
      <div className="flex items-center justify-between gap-2 p-3">
        <span className="text-sm font-semibold">Canvases</span>
        <Button
          type="button"
          size="sm"
          onClick={onCreate}
          disabled={createCanvas.isPending}
        >
          <Plus className="size-3" />
          New canvas
        </Button>
      </div>
      <ScrollArea className="min-h-0 flex-1 px-2 pb-2">
        {list.isLoading ? (
          <p className="px-2 py-4 text-xs text-muted-foreground">Loading...</p>
        ) : (list.data ?? []).length === 0 ? (
          <p className="px-2 py-4 text-xs text-muted-foreground">No canvases yet - click "New canvas".</p>
        ) : (
          <ul className="space-y-1">
            {(list.data ?? []).map((canvas) => (
              <li key={canvas.id}>
                <button
                  type="button"
                  onClick={() => navigate(`/canvas/${canvas.id}`)}
                  className={cn(
                    "w-full truncate rounded-md px-2 py-1.5 text-left text-sm hover:bg-sidebar-accent",
                    canvas.id === activeId && "bg-sidebar-accent font-medium",
                  )}
                >
                  {canvas.title || "Untitled canvas"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </ScrollArea>
    </aside>
  );
}
