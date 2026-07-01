import { ChevronLeft, ChevronRight } from "lucide-react";

import { Button } from "@/components/ui/button";

interface Version {
  id: string;
}

interface VersionSwitcherProps {
  versions: Version[];
  activeId: string;
  onChange: (id: string) => void;
}

export function VersionSwitcher({ versions, activeId, onChange }: VersionSwitcherProps) {
  const index = Math.max(
    0,
    versions.findIndex((v) => v.id === activeId),
  );
  const atStart = index <= 0;
  const atEnd = index >= versions.length - 1;
  return (
    <div className="flex items-center gap-1 font-mono text-xs">
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        aria-label="previous version"
        disabled={atStart}
        onClick={() => onChange(versions[index - 1].id)}
      >
        <ChevronLeft className="size-3" />
      </Button>
      <span>{`${index + 1}/${versions.length}`}</span>
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        aria-label="next version"
        disabled={atEnd}
        onClick={() => onChange(versions[index + 1].id)}
      >
        <ChevronRight className="size-3" />
      </Button>
    </div>
  );
}
