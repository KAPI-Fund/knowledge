import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { createOrg } from "../shared/tenancy-api";

export function CreateOrgDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => createOrg({ name: name.trim(), slug: slug.trim() }),
    onSuccess: async (org) => {
      await queryClient.invalidateQueries({ queryKey: ["spaces"] });
      onOpenChange(false);
      navigate(`/orgs/${org.id}`);
    },
  });

  const submit = async () => {
    setErrorMessage(null);
    try {
      await mutation.mutateAsync();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create org");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New org</DialogTitle>
          <DialogDescription>Create a new organization.</DialogDescription>
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
          {errorMessage ? (
            <p className="text-sm text-destructive">{errorMessage}</p>
          ) : null}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} type="button">
            Cancel
          </Button>
          <Button onClick={submit} disabled={mutation.isPending} type="button">
            Create
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
