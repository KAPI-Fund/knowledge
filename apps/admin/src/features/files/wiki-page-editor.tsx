import { useEffect, useState } from "react";

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
  onDeleted: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(content);
  const [feedback, setFeedback] = useState("");
  const save = useSaveFileContentMutation(projectId);
  const remove = useDeleteWikiPagesMutation(projectId);

  useEffect(() => {
    setEditing(false);
    setDraft(content);
    setFeedback("");
  }, [path, content]);

  if (!isEditableWikiPath(path)) {
    return null;
  }

  async function handleSave() {
    try {
      await save.mutateAsync({ path, content: draft });
      setEditing(false);
      setFeedback("Saved.");
    } catch (error) {
      setFeedback(normalizeAppError(error).message);
    }
  }

  async function handleDelete() {
    if (!window.confirm(`Delete ${path} and clean up references to it?`)) {
      return;
    }
    try {
      const result = await remove.mutateAsync({ paths: [path] });
      setFeedback(
        `Deleted ${result.deletedPaths.length} page(s), rewrote ${result.rewrittenFiles} file(s).`,
      );
      onDeleted();
    } catch (error) {
      setFeedback(normalizeAppError(error).message);
    }
  }

  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap gap-2">
        {editing ? (
          <>
            <Button disabled={save.isPending} onClick={handleSave}>
              Save
            </Button>
            <Button
              onClick={() => {
                setEditing(false);
                setDraft(content);
              }}
              variant="ghost"
            >
              Cancel
            </Button>
          </>
        ) : (
          <Button onClick={() => setEditing(true)} variant="secondary">
            Edit
          </Button>
        )}
        <Button disabled={remove.isPending} onClick={handleDelete} variant="destructive">
          Delete
        </Button>
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
    </div>
  );
}
