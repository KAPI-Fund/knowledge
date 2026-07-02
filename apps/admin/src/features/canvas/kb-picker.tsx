import { Button } from "@/components/ui/button";

import { useProjectsQuery } from "../projects/queries";

interface KbProjectPickerProps {
  onPick: (project: { id: string; name: string }) => void;
}

export function KbProjectPicker({ onPick }: KbProjectPickerProps) {
  const projects = useProjectsQuery();
  const items = projects.data ?? [];

  if (projects.isLoading) {
    return <p className="text-sm text-muted-foreground">Loading projects...</p>;
  }
  if (items.length === 0) {
    return <p className="text-sm text-muted-foreground">No knowledge bases available.</p>;
  }
  return (
    <div className="space-y-2">
      <ul className="max-h-64 space-y-1 overflow-auto">
        {items.map((project) => (
          <li key={project.id}>
            <Button
              type="button"
              variant="ghost"
              className="w-full justify-start"
              onClick={() => onPick(project)}
            >
              {project.name}
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}
