import { useState } from "react";
import { useParams } from "react-router-dom";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { useSpacesQuery } from "../spaces/use-spaces";
import { useOrgMembersQuery } from "./members-queries";
import {
  useAddOrgMemberMutation,
  useSetOrgMemberRoleMutation,
  useRemoveOrgMemberMutation,
} from "./members-mutations";

export function OrgMembersPage() {
  const { orgId = "" } = useParams();
  const spaces = useSpacesQuery();
  const org = spaces.data?.orgs.find((entry) => entry.id === orgId);
  const isAdmin = org?.role === "org_admin";

  const members = useOrgMembersQuery(orgId);
  const addMember = useAddOrgMemberMutation(orgId);
  const setRole = useSetOrgMemberRoleMutation(orgId);
  const removeMember = useRemoveOrgMemberMutation(orgId);

  const [username, setUsername] = useState("");
  const [role, setRoleValue] = useState<"org_admin" | "org_member">("org_member");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  if (spaces.isLoading) return <p>Loading…</p>;
  if (!org) return <p>Access denied or organization not found.</p>;

  const add = async () => {
    setErrorMessage(null);
    try {
      await addMember.mutateAsync({ usernameOrEmail: username.trim(), role });
      setUsername("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to add member");
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-lg font-semibold">{org.name} members</h1>

      <table>
        <thead>
          <tr>
            <th>Username</th>
            <th>Role</th>
            {isAdmin ? <th>Actions</th> : null}
          </tr>
        </thead>
        <tbody>
          {members.data?.members.map((member) => (
            <tr key={member.userId}>
              <td>{member.username}</td>
              <td>
                {isAdmin ? (
                  <select
                    aria-label={`Role for ${member.username}`}
                    value={member.role}
                    onChange={(event) =>
                      setRole.mutateAsync({
                        userId: member.userId,
                        role: event.target.value as "org_admin" | "org_member",
                      })
                    }
                  >
                    <option value="org_admin">org_admin</option>
                    <option value="org_member">org_member</option>
                  </select>
                ) : (
                  member.role
                )}
              </td>
              {isAdmin ? (
                <td>
                  <Button
                    type="button"
                    variant="outline"
                    aria-label={`Remove ${member.username}`}
                    onClick={() => removeMember.mutateAsync({ userId: member.userId })}
                  >
                    Remove
                  </Button>
                </td>
              ) : null}
            </tr>
          ))}
        </tbody>
      </table>

      {isAdmin ? (
        <div className="flex items-end gap-2">
          <Input
            placeholder="Username"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
          />
          <select
            aria-label="New member role"
            value={role}
            onChange={(event) =>
              setRoleValue(event.target.value as "org_admin" | "org_member")
            }
          >
            <option value="org_member">org_member</option>
            <option value="org_admin">org_admin</option>
          </select>
          <Button type="button" onClick={add} disabled={addMember.isPending}>
            Add member
          </Button>
        </div>
      ) : null}

      {errorMessage ? (
        <p className="text-sm text-destructive">{errorMessage}</p>
      ) : null}
    </div>
  );
}
