import { Check, Pencil, Trash2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/shared/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { normalizeAppError } from "@/lib/app-error";

import { useDeleteWikiPagesMutation, useSaveFileContentMutation } from "./queries";

export function isEditableWikiPath(path: string) {
  return path.startsWith("wiki/") && path.endsWith(".md");
}

// Auto-save semantics ported from upstream_llm_wiki
// src/components/layout/preview-panel.tsx handleSave L68-86 (1000ms debounce +
// immediate flush) and src/components/editor/wiki-editor.tsx (Ctrl+S L49-55,
// Edit/Done L57-74, lastLoadedRef no-op suppression).
export function WikiPageEditor({
  projectId,
  path,
  content,
  onDeleted,
}: {
  projectId: string;
  path: string;
  content: string;
  onDeleted: (summary: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(content);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved">("idle");
  const [feedback, setFeedback] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const save = useSaveFileContentMutation(projectId);
  const remove = useDeleteWikiPagesMutation(projectId);
  // Last content written to (or loaded from) the server; saving is a no-op
  // while the draft matches it, so refetch echoes never trigger writes.
  const lastSavedRef = useRef(content);
  const editingRef = useRef(false);
  editingRef.current = editing;

  useEffect(() => {
    setEditing(false);
    setDraft(content);
    setSaveState("idle");
    setFeedback("");
    lastSavedRef.current = content;
    // Reset only when switching files; content refreshes are handled below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path]);

  useEffect(() => {
    // Adopt external content changes (restore, agent writes) unless the user
    // is mid-edit, in which case the local draft wins.
    if (!editingRef.current) {
      setDraft(content);
      lastSavedRef.current = content;
    }
  }, [content]);

  const flushSave = useCallback(
    async (value: string) => {
      if (value === lastSavedRef.current) {
        return;
      }
      setSaveState("saving");
      try {
        await save.mutateAsync({ path, content: value });
        lastSavedRef.current = value;
        setSaveState("saved");
      } catch (error) {
        setSaveState("idle");
        toast.error(normalizeAppError(error).message);
      }
    },
    [path, save],
  );

  // Upstream preview-panel L68-86: debounce writes while typing.
  useEffect(() => {
    if (!editing || draft === lastSavedRef.current) {
      return;
    }
    const timer = setTimeout(() => {
      void flushSave(draft);
    }, 1000);
    return () => clearTimeout(timer);
  }, [draft, editing, flushSave]);

  if (!isEditableWikiPath(path)) {
    return null;
  }

  const isDirty = editing && draft !== lastSavedRef.current;

  async function handleDone() {
    await flushSave(draft);
    setEditing(false);
  }

  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-center gap-2">
        {editing ? (
          <Button disabled={save.isPending} onClick={() => void handleDone()}>
            <Check />
            Done
          </Button>
        ) : (
          <Button onClick={() => setEditing(true)} variant="secondary">
            <Pencil />
            Edit
          </Button>
        )}
        <Button disabled={remove.isPending} onClick={() => setConfirmDelete(true)} variant="destructive">
          <Trash2 />
          Delete
        </Button>
        {saveState === "saving" ? <Badge variant="outline">Saving…</Badge> : null}
        {saveState === "saved" && !isDirty ? <Badge variant="outline">Saved</Badge> : null}
        {isDirty && saveState !== "saving" ? <Badge variant="outline">Unsaved changes</Badge> : null}
      </div>
      {editing ? (
        <Textarea
          aria-label="Page content"
          className="min-h-[320px] font-mono text-sm"
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            // Upstream wiki-editor L49-55: Ctrl/Cmd+S saves immediately.
            if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
              event.preventDefault();
              void flushSave(draft);
            }
          }}
          value={draft}
        />
      ) : null}
      {feedback ? <p className="text-sm text-muted-foreground">{feedback}</p> : null}
      <ConfirmDialog
        confirmLabel="Delete page"
        description={`${path} will be deleted and references to it will be cleaned up.`}
        destructive
        isPending={remove.isPending}
        onConfirm={async () => {
          try {
            const result = await remove.mutateAsync({ paths: [path] });
            const summary = `Deleted ${result.deletedPaths.length} page(s), rewrote ${result.rewrittenFiles} file(s).`;
            setFeedback(summary);
            toast.success(summary);
            onDeleted(summary);
          } catch (error) {
            toast.error(normalizeAppError(error).message);
          }
        }}
        onOpenChange={setConfirmDelete}
        open={confirmDelete}
        title="Delete this page?"
      />
    </div>
  );
}
