import { PanelLeftClose, PanelLeftOpen, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

import { useCanvasList, useCreateCanvas, useDeleteCanvas } from "./queries";
import type { CanvasSummary } from "./types";

interface HistorySidebarProps {
  activeId?: string;
  collapsed?: boolean;
  onToggle?: () => void;
}

export function HistorySidebar({ activeId, collapsed = false, onToggle }: HistorySidebarProps) {
  const navigate = useNavigate();
  const list = useCanvasList();
  const createCanvas = useCreateCanvas();
  const deleteCanvas = useDeleteCanvas();
  const [deleteTarget, setDeleteTarget] = useState<CanvasSummary | null>(null);

  const onCreate = () => {
    createCanvas.mutate(undefined, {
      onSuccess: (canvas) => navigate(`/canvas/${canvas.id}`),
    });
  };

  const confirmDelete = () => {
    if (!deleteTarget) {
      return;
    }
    const target = deleteTarget;
    deleteCanvas.mutate(target.id, {
      onSuccess: () => {
        setDeleteTarget(null);
        // Leaving the deleted canvas mounted would keep loading a now-missing
        // id, so drop back to the index route when the open canvas is removed.
        if (target.id === activeId) {
          navigate("/canvas");
        }
      },
    });
  };

  return (
    <aside
      className={cn(
        "flex h-full shrink-0 flex-col overflow-hidden border-r bg-sidebar text-sidebar-foreground transition-[width] duration-200 ease-in-out",
        collapsed ? "w-12" : "w-64",
      )}
    >
      {collapsed ? (
        <div className="flex w-12 flex-col items-center gap-1 py-3">
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            onClick={onToggle}
            aria-label="Expand canvas list"
          >
            <PanelLeftOpen className="size-4" />
          </Button>
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            onClick={onCreate}
            disabled={createCanvas.isPending}
            aria-label="New canvas"
          >
            <Plus className="size-4" />
          </Button>
        </div>
      ) : (
        <div className="flex h-full w-64 flex-col">
          <div className="flex items-center justify-between gap-2 p-3">
            <div className="flex min-w-0 items-center gap-1">
              <Button
                type="button"
                size="icon-sm"
                variant="ghost"
                onClick={onToggle}
                aria-label="Collapse canvas list"
              >
                <PanelLeftClose className="size-4" />
              </Button>
              <span className="truncate text-sm font-semibold">Canvases</span>
            </div>
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
                  <li key={canvas.id} className="group relative flex items-center">
                    <button
                      type="button"
                      onClick={() => navigate(`/canvas/${canvas.id}`)}
                      className={cn(
                        "min-w-0 flex-1 truncate rounded-md py-1.5 pl-2 pr-8 text-left text-sm hover:bg-sidebar-accent",
                        canvas.id === activeId && "bg-sidebar-accent font-medium",
                      )}
                    >
                      {canvas.title || "Untitled canvas"}
                    </button>
                    <Button
                      type="button"
                      size="icon-xs"
                      variant="ghost"
                      onClick={() => setDeleteTarget(canvas)}
                      aria-label={`Delete ${canvas.title || "Untitled canvas"}`}
                      className="absolute right-1 text-muted-foreground opacity-0 transition-opacity hover:text-destructive focus-visible:opacity-100 group-hover:opacity-100"
                    >
                      <Trash2 className="size-3.5" />
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </ScrollArea>
        </div>
      )}

      <Dialog open={deleteTarget !== null} onOpenChange={(open) => !open && setDeleteTarget(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Delete canvas</DialogTitle>
            <DialogDescription>
              {`Delete "${deleteTarget?.title || "Untitled canvas"}"? This can't be undone.`}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => setDeleteTarget(null)}>
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleteCanvas.isPending}
            >
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </aside>
  );
}
