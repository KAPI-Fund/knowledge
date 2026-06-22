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
import { useCreateTeamMutation } from "./workspace-mutations";

export function CreateTeamDialog({
  orgId,
  open,
  onOpenChange,
}: {
  orgId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const mutation = useCreateTeamMutation(orgId);

  const submit = async () => {
    setErrorMessage(null);
    try {
      await mutation.mutateAsync({ name: name.trim(), slug: slug.trim() });
      onOpenChange(false);
      setName("");
      setSlug("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create team");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New team</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <Input
            placeholder="Name"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
          <Input
            placeholder="Slug"
            value={slug}
            onChange={(event) => setSlug(event.target.value)}
          />
        </div>
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
