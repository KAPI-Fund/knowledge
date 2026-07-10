import { Pencil, Save, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
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
  const [feedback, setFeedback] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const save = useSaveFileContentMutation(projectId);
  const remove = useDeleteWikiPagesMutation(projectId);

  useEffect(() => {
    setEditing(false);
    setDraft(content);
  }, [path, content]);

  useEffect(() => {
    setFeedback("");
  }, [path]);

  if (!isEditableWikiPath(path)) {
    return null;
  }

  const isDirty = editing && draft !== content;

  async function handleSave() {
    try {
      await save.mutateAsync({ path, content: draft });
      setEditing(false);
      setFeedback("Saved.");
      toast.success(`Saved ${path}.`);
    } catch (error) {
      toast.error(normalizeAppError(error).message);
    }
  }

  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-center gap-2">
        {editing ? (
          <>
            <Button disabled={save.isPending} onClick={handleSave}>
              <Save />
              Save
            </Button>
            <Button
              onClick={() => {
                setEditing(false);
                setDraft(content);
              }}
              variant="ghost"
            >
              <X />
              Cancel
            </Button>
          </>
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
        {isDirty ? <Badge variant="outline">Unsaved changes</Badge> : null}
      </div>
      {editing ? (
        <Textarea
          aria-label="Page content"
          className="min-h-[320px] font-mono text-sm"
          onChange={(event) => setDraft(event.target.value)}
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
