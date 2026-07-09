import { Trash2 } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select } from "@/components/ui/select";
import {
  useGrantCandidatesQuery,
  useProjectGranteesQuery,
} from "./manage-access-queries";
import {
  useRemoveGrantMutation,
  useUpsertGrantMutation,
} from "./manage-access-mutations";

const ROLE_VARIANT: Record<string, "default" | "secondary" | "outline"> = {
  owner: "default",
  editor: "secondary",
  viewer: "outline",
};

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

        <div className="flex flex-col gap-4">
          <div className="rounded-lg border border-border">
            {grantees.data?.members.length ? (
              <ScrollArea className="max-h-64">
                <ul className="divide-y divide-border">
                  {grantees.data.members.map((member) => (
                    <li
                      key={member.userId}
                      className="flex items-center justify-between gap-2 px-3 py-2"
                    >
                      <div className="flex min-w-0 items-center gap-2">
                        <span className="truncate text-sm font-medium">{member.username}</span>
                        <Badge variant={ROLE_VARIANT[member.role] ?? "outline"}>
                          {member.role}
                        </Badge>
                      </div>
                      {member.role === "owner" ? null : (
                        <Button
                          type="button"
                          size="icon-sm"
                          variant="ghost"
                          aria-label={`Remove ${member.username}`}
                          onClick={() => removeGrantee(member.userId)}
                          className="text-muted-foreground hover:text-destructive"
                        >
                          <Trash2 className="size-4" />
                        </Button>
                      )}
                    </li>
                  ))}
                </ul>
              </ScrollArea>
            ) : (
              <p className="px-3 py-6 text-center text-sm text-muted-foreground">
                No members have access yet.
              </p>
            )}
          </div>

          <div className="flex items-end gap-2">
            <label className="grid flex-1 gap-1.5 text-sm font-medium">
              Add user
              <Select
                aria-label="Add user"
                value={selectedUser}
                onChange={(event) => setSelectedUser(event.target.value)}
              >
                <option value="">Select a member...</option>
                {filteredCandidates?.map((candidate) => (
                  <option key={candidate.userId} value={candidate.userId}>
                    {candidate.username}
                  </option>
                ))}
              </Select>
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              Grant role
              <Select
                aria-label="Grant role"
                className="w-32"
                value={selectedRole}
                onChange={(event) =>
                  setSelectedRole(event.target.value as "editor" | "viewer")
                }
              >
                <option value="editor">editor</option>
                <option value="viewer">viewer</option>
              </Select>
            </label>
            <Button
              type="button"
              onClick={addGrant}
              disabled={!selectedUser || upsert.isPending}
            >
              Add grant
            </Button>
          </div>

          {errorMessage ? (
            <p className="text-sm text-destructive">{errorMessage}</p>
          ) : null}
        </div>

        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
