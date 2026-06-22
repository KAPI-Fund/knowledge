import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "../../components/ui/dialog";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
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
        </DialogHeader>
        <Input
          placeholder="Name"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        {errorMessage ? (
          <p className="text-sm text-destructive">{errorMessage}</p>
        ) : null}
        <DialogFooter>
          <Button type="button" onClick={submit} disabled={mutation.isPending}>
            Create
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
