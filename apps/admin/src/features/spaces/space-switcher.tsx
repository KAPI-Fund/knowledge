import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSpacesQuery } from "./use-spaces";
import { CreateOrgDialog } from "./create-org-dialog";

export function SpaceSwitcher() {
  const navigate = useNavigate();
  const { data, isLoading } = useSpacesQuery();
  const [dialogOpen, setDialogOpen] = useState(false);

  if (isLoading || !data) {
    return null;
  }

  return (
    <div className="flex items-center gap-2">
      <button type="button" onClick={() => navigate("/projects")}>
        Personal
      </button>
      {data.orgs.map((org) => (
        <button key={org.id} type="button" onClick={() => navigate(`/orgs/${org.id}`)}>
          {org.name}
        </button>
      ))}
      <button type="button" onClick={() => setDialogOpen(true)}>
        New org
      </button>
      <CreateOrgDialog open={dialogOpen} onOpenChange={setDialogOpen} />
    </div>
  );
}
