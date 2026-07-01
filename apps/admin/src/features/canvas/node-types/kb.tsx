import { Database } from "lucide-react";

import { Badge } from "@/components/ui/badge";

export interface KbNodeData {
  projectId?: string;
  projectName?: string;
  noAccess?: boolean;
}

interface KbNodeProps {
  data: KbNodeData;
}

export function KbNode({ data }: KbNodeProps) {
  return (
    <div className="flex h-full flex-col gap-2 rounded-md border bg-card p-3 text-card-foreground shadow-sm">
      <div className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
        <Database className="size-3" />
        Knowledge base
      </div>
      <div className="text-sm font-medium">{data.projectName ?? data.projectId ?? "Untitled"}</div>
      {data.noAccess ? (
        <Badge variant="destructive" className="w-fit">
          No access
        </Badge>
      ) : null}
    </div>
  );
}
