import { Button } from "@/components/ui/button";

import { EmptyState } from "./empty-state";

export function ProjectlessState({ onCreateProject }: { onCreateProject?: () => void }) {
  return (
    <EmptyState
      action={
        onCreateProject ? (
          <Button onClick={onCreateProject}>Create Project</Button>
        ) : undefined
      }
      description="Create your first knowledge workspace to start importing sources and running workflows."
      title="No projects yet"
    />
  );
}
