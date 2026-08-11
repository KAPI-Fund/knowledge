// Ported from upstream_llm_wiki/src/components/editor/file-history-panel.tsx:
// entry list + prefix/suffix diff against the current content + restore. The
// upstream inline overlay becomes a shadcn Sheet; entry content is fetched
// lazily by id (the server list endpoint returns metadata only).
import { History, RotateCcw } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import { normalizeAppError } from "@/lib/app-error";

import { diffFileContent } from "./file-diff";
import {
  useFileHistoryEntryQuery,
  useFileHistoryQuery,
  useRestoreFileHistoryMutation,
} from "./queries";

export function isRestorableHistoryPath(path: string) {
  return (path.startsWith("wiki/") && path.endsWith(".md")) || path.startsWith("agent-workspace/");
}

export function FileHistoryPanel({
  projectId,
  path,
  currentContent,
}: {
  projectId: string;
  path: string;
  currentContent: string | null;
}) {
  const [open, setOpen] = useState(false);
  const [selectedId, setSelectedId] = useState("");
  const [confirmRestore, setConfirmRestore] = useState(false);
  const history = useFileHistoryQuery(projectId, path, open);
  const selectedEntry = useFileHistoryEntryQuery(projectId, open ? selectedId : "");
  const restore = useRestoreFileHistoryMutation(projectId);

  function handleOpenChange(nextOpen: boolean) {
    setOpen(nextOpen);
    if (!nextOpen) {
      setSelectedId("");
    }
  }

  const diff =
    selectedEntry.data && currentContent !== null
      ? diffFileContent(selectedEntry.data.content, currentContent).diff
      : null;

  return (
    <Sheet onOpenChange={handleOpenChange} open={open}>
      <SheetTrigger asChild>
        <Button size="sm" variant="outline">
          <History />
          History
        </Button>
      </SheetTrigger>
      <SheetContent className="flex w-full flex-col gap-0 sm:max-w-2xl">
        <SheetHeader>
          <SheetTitle>File history</SheetTitle>
          <SheetDescription className="truncate">{path}</SheetDescription>
        </SheetHeader>
        <div className="grid min-h-0 flex-1 grid-rows-[minmax(0,220px)_auto_minmax(0,1fr)] gap-3 px-4 pb-4">
          <ScrollArea className="rounded-lg border border-border/70">
            <div className="grid gap-1 p-2">
              {history.isLoading ? (
                <>
                  <Skeleton className="h-12 w-full" />
                  <Skeleton className="h-12 w-full" />
                </>
              ) : null}
              {history.error ? (
                <p className="p-2 text-sm text-destructive">
                  {normalizeAppError(history.error).message}
                </p>
              ) : null}
              {history.data && history.data.length === 0 ? (
                <p className="p-2 text-sm text-muted-foreground">No history recorded yet.</p>
              ) : null}
              {history.data?.map((entry) => (
                <Button
                  className="h-auto justify-start py-2"
                  key={entry.id}
                  onClick={() => setSelectedId(entry.id)}
                  variant={selectedId === entry.id ? "secondary" : "ghost"}
                >
                  <span className="grid min-w-0 gap-0.5 text-left">
                    <span className="flex items-center gap-2">
                      <span className="truncate text-sm font-medium">{entry.author}</span>
                      <Badge variant="outline">{entry.tool}</Badge>
                    </span>
                    <span className="text-xs text-muted-foreground">
                      {new Date(entry.createdAt).toLocaleString()}
                    </span>
                  </span>
                </Button>
              ))}
            </div>
          </ScrollArea>
          <Separator />
          {selectedId ? (
            <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-2">
              <div className="flex items-center justify-end">
                <Button
                  disabled={!selectedEntry.data || restore.isPending || !isRestorableHistoryPath(path)}
                  onClick={() => setConfirmRestore(true)}
                  size="sm"
                  variant="outline"
                >
                  <RotateCcw />
                  Restore this version
                </Button>
              </div>
              <ScrollArea className="rounded-lg border border-border/70 bg-muted/30">
                {selectedEntry.isLoading ? (
                  <div className="grid gap-2 p-3">
                    <Skeleton className="h-4 w-3/4" />
                    <Skeleton className="h-4 w-1/2" />
                  </div>
                ) : (
                  <pre className="whitespace-pre-wrap break-words p-3 font-mono text-xs">
                    {diff ?? selectedEntry.data?.content ?? ""}
                  </pre>
                )}
              </ScrollArea>
            </div>
          ) : (
            <div className="grid place-items-center rounded-lg border border-dashed border-border/70 text-sm text-muted-foreground">
              Select an entry to compare it with the current content.
            </div>
          )}
        </div>
        <AlertDialog onOpenChange={setConfirmRestore} open={confirmRestore}>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Restore this version?</AlertDialogTitle>
              <AlertDialogDescription>
                {path} will be overwritten with the selected version. The current content stays
                available in history.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction
                onClick={() => {
                  restore.mutate(
                    { entryId: selectedId, path },
                    {
                      onError: (error) => toast.error(normalizeAppError(error).message),
                      onSuccess: (result) => {
                        toast.success(`Restored ${result.path} from history.`);
                        handleOpenChange(false);
                      },
                    },
                  );
                }}
              >
                Restore
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </SheetContent>
    </Sheet>
  );
}
