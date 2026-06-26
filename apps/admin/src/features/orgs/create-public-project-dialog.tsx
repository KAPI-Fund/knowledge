import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useCreateSpaceProjectMutation } from "./workspace-mutations";

export function CreatePublicProjectDialog({
  targetSpaceId,
  listSpaceId,
  title,
  open,
  onOpenChange,
}: {
  targetSpaceId: string;
  listSpaceId: string;
  title: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [name, setName] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const mutation = useCreateSpaceProjectMutation(listSpaceId);

  const submit = async () => {
    setErrorMessage(null);
    try {
      await mutation.mutateAsync({ name: name.trim(), spaceId: targetSpaceId });
      onOpenChange(false);
      setName("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create project");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>
            Give the project a name to get started.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-3">
          <label className="grid gap-1.5 text-sm font-medium">
            Name
            <Input
              placeholder="My Project"
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
          </label>
          {errorMessage ? (
            <p className="text-sm text-destructive">{errorMessage}</p>
          ) : null}
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            type="button"
            onClick={submit}
            disabled={!name.trim() || mutation.isPending}
          >
            Create
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
