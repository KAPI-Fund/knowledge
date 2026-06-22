import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { useSpacesQuery } from "../spaces/use-spaces";
import { useTeamMembersQuery, useTeamProjectsQuery } from "./team-queries";
import {
  useAddTeamMemberMutation,
  useRemoveTeamMemberMutation,
  useCreateTeamKbMutation,
} from "./team-mutations";
import { ManageAccessDialog } from "../kb-access/manage-access-dialog";

export function TeamPage() {
  const { orgId = "", teamId = "" } = useParams();
  const spaces = useSpacesQuery();
  const org = spaces.data?.orgs.find((entry) => entry.id === orgId);
  const team = spaces.data?.teams.find((entry) => entry.id === teamId);
  const teamSpaceId = team?.spaceId ?? "";

  const isAdmin = org?.role === "org_admin";
  const isLeader = team?.role === "leader";
  const canManage = Boolean(isAdmin || isLeader);

  const members = useTeamMembersQuery(orgId, teamId);
  const projects = useTeamProjectsQuery(teamSpaceId, teamId);
  const addMember = useAddTeamMemberMutation(orgId, teamId);
  const removeMember = useRemoveTeamMemberMutation(orgId, teamId);
  const createKb = useCreateTeamKbMutation(teamSpaceId);

  const [username, setUsername] = useState("");
  const [kbName, setKbName] = useState("");
  const [manageProjectId, setManageProjectId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  if (spaces.isLoading) return <p>Loading…</p>;
  if (!team) return <p>Access denied or team not found.</p>;

  const add = async () => {
    setErrorMessage(null);
    try {
      await addMember.mutateAsync({ usernameOrEmail: username.trim() });
      setUsername("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to add member");
    }
  };

  const remove = async (userId: string) => {
    setErrorMessage(null);
    try {
      await removeMember.mutateAsync({ userId });
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to remove member");
    }
  };

  const addKb = async () => {
    setErrorMessage(null);
    try {
      await createKb.mutateAsync({ name: kbName.trim() });
      setKbName("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create KB");
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <header className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">Team · {team.name}</h1>
        <Link to={`/orgs/${orgId}`}>Back to workspace</Link>
      </header>

      <section className="flex flex-col gap-2">
        <h2 className="text-sm font-medium">Members</h2>
        <table>
          <thead>
            <tr>
              <th>Username</th>
              <th>Role</th>
              {canManage ? <th>Actions</th> : null}
            </tr>
          </thead>
          <tbody>
            {members.data?.members.map((member) => (
              <tr key={member.userId}>
                <td>{member.username}</td>
                <td>{member.role}</td>
                {canManage ? (
                  <td>
                    {member.role === "leader" ? null : (
                      <Button
                        type="button"
                        variant="outline"
                        aria-label={`Remove ${member.username}`}
                        onClick={() => remove(member.userId)}
                      >
                        Remove
                      </Button>
                    )}
                  </td>
                ) : null}
              </tr>
            ))}
          </tbody>
        </table>
        {canManage ? (
          <div className="flex items-end gap-2">
            <Input
              placeholder="Username"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
            />
            <Button type="button" onClick={add} disabled={addMember.isPending}>
              Add member
            </Button>
          </div>
        ) : null}
      </section>

      <section className="flex flex-col gap-2">
        <h2 className="text-sm font-medium">Team KBs</h2>
        {(projects.data ?? []).length === 0 ? (
          <p className="text-sm text-muted-foreground">No team KBs yet.</p>
        ) : (
          <ul className="flex flex-col gap-1">
            {projects.data?.map((project) => (
              <li key={project.id} className="flex items-center justify-between">
                <Link to={`/projects/${project.id}`}>{project.name}</Link>
                {canManage ? (
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => setManageProjectId(project.id)}
                  >
                    Manage access
                  </Button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
        {canManage ? (
          <div className="flex items-end gap-2">
            <Input
              placeholder="New team KB name"
              value={kbName}
              onChange={(event) => setKbName(event.target.value)}
            />
            <Button type="button" onClick={addKb} disabled={createKb.isPending}>
              New team KB
            </Button>
          </div>
        ) : null}
      </section>

      {errorMessage ? (
        <p className="text-sm text-destructive">{errorMessage}</p>
      ) : null}

      {manageProjectId ? (
        <ManageAccessDialog
          projectId={manageProjectId}
          spaceKind="team"
          orgId={orgId}
          teamId={teamId}
          open={Boolean(manageProjectId)}
          onOpenChange={(open) => {
            if (!open) setManageProjectId(null);
          }}
        />
      ) : null}
    </div>
  );
}
