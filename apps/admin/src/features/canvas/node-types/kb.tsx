import { Database } from "lucide-react";

import { NodeShell } from "./node-shell";

export interface KbNodeData {
  projectId?: string;
  projectName?: string;
  noAccess?: boolean;
}

interface KbNodeProps {
  data: KbNodeData;
  nodeId: string;
  index?: number;
  selected?: boolean;
}

export function KbNode({ data, nodeId, index, selected }: KbNodeProps) {
  return (
    <NodeShell
      icon={<Database className="size-3.5" />}
      label="KNOWLEDGE BASE"
      nodeId={nodeId}
      index={index}
      selected={selected}
      status={data.noAccess ? "no-access" : "idle"}
    >
      <div className="text-sm font-medium">
        {data.projectName ?? data.projectId ?? "Untitled"}
      </div>
    </NodeShell>
  );
}
