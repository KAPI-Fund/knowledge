import { useMutation, useQueryClient } from "@tanstack/react-query";
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
import { Input } from "@/components/ui/input";
import { createOrg } from "@/features/shared/tenancy-api";

function slugify(value: string) {
  return value
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

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
  const [slugEdited, setSlugEdited] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => createOrg({ name: name.trim(), slug: (slugEdited ? slug : slugify(name)).trim() }),
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
      setErrorMessage(error instanceof Error ? error.message : "Failed to create organization");
    }
  };

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New organization</DialogTitle>
          <DialogDescription>
            Create a shared workspace for teams and public knowledge bases.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-3">
          <label className="grid gap-1.5 text-sm font-medium">
            Name
            <Input
              onChange={(event) => setName(event.target.value)}
              placeholder="Acme Research"
              value={name}
            />
          </label>
          <label className="grid gap-1.5 text-sm font-medium">
            Slug
            <Input
              onChange={(event) => {
                setSlugEdited(true);
                setSlug(event.target.value);
              }}
              placeholder={slugify(name) || "acme-research"}
              value={slugEdited ? slug : slugify(name)}
            />
          </label>
          {errorMessage ? <p className="text-sm text-destructive">{errorMessage}</p> : null}
        </div>
        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button disabled={!name.trim() || mutation.isPending} onClick={submit} type="button">
            Create organization
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
