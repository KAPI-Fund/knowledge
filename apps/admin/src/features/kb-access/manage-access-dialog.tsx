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
import {
  useProjectGranteesQuery,
  useGrantCandidatesQuery,
} from "./manage-access-queries";
import {
  useUpsertGrantMutation,
  useRemoveGrantMutation,
} from "./manage-access-mutations";

export function ManageAccessDialog({
  projectId,
  spaceKind,
  orgId,
  teamId,
  open,
  onOpenChange,
}: {
  projectId: string;
  spaceKind: "org" | "team";
  orgId: string;
  teamId: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const grantees = useProjectGranteesQuery(projectId);
  const candidates = useGrantCandidatesQuery({ spaceKind, orgId, teamId });
  const upsert = useUpsertGrantMutation(projectId);
  const remove = useRemoveGrantMutation(projectId);

  const [selectedUser, setSelectedUser] = useState("");
  const [selectedRole, setSelectedRole] = useState<"editor" | "viewer">("editor");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const granteeIds = new Set(grantees.data?.members.map((m) => m.userId) ?? []);
  const filteredCandidates = candidates.data?.filter(
    (c) => !granteeIds.has(c.userId),
  );

  const addGrant = async () => {
    if (!selectedUser) return;
    setErrorMessage(null);
    try {
      await upsert.mutateAsync({ userId: selectedUser, role: selectedRole });
      setSelectedUser("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to add grant");
    }
  };

  const removeGrantee = async (userId: string) => {
    setErrorMessage(null);
    try {
      await remove.mutateAsync({ userId });
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to remove grant");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Manage access</DialogTitle>
          <DialogDescription>
            Add or remove member access to this project.
          </DialogDescription>
        </DialogHeader>

        <ul className="flex flex-col gap-1">
          {grantees.data?.members.map((member) => (
            <li key={member.userId} className="flex items-center justify-between">
              <span>
                <span>{member.username}</span> ({member.role})
              </span>
              {member.role === "owner" ? null : (
                <Button
                  type="button"
                  variant="outline"
                  aria-label={`Remove ${member.username}`}
                  onClick={() => removeGrantee(member.userId)}
                >
                  Remove
                </Button>
              )}
            </li>
          ))}
        </ul>

        <div className="flex items-end gap-2">
          <label className="flex flex-col text-sm">
            Add user
            <select
              aria-label="Add user"
              value={selectedUser}
              onChange={(event) => setSelectedUser(event.target.value)}
            >
              <option value="">Select...</option>
              {filteredCandidates?.map((candidate) => (
                <option key={candidate.userId} value={candidate.userId}>
                  {candidate.username}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col text-sm">
            Grant role
            <select
              aria-label="Grant role"
              value={selectedRole}
              onChange={(event) =>
                setSelectedRole(event.target.value as "editor" | "viewer")
              }
            >
              <option value="editor">editor</option>
              <option value="viewer">viewer</option>
            </select>
          </label>
          <Button type="button" onClick={addGrant} disabled={upsert.isPending}>
            Add grant
          </Button>
        </div>

        {errorMessage ? (
          <p className="text-sm text-destructive">{errorMessage}</p>
        ) : null}

        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
