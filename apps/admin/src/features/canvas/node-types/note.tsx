import { StickyNote } from "lucide-react";

import { Textarea } from "@/components/ui/textarea";

import { NodeShell } from "./node-shell";

export interface NoteNodeData {
  markdown?: string;
}

interface NoteNodeProps {
  data: NoteNodeData;
  nodeId: string;
  index?: number;
  selected?: boolean;
  onChange: (markdown: string) => void;
}

export function NoteNode({ data, nodeId, index, selected, onChange }: NoteNodeProps) {
  return (
    <NodeShell
      icon={<StickyNote className="size-3.5" />}
      label="NOTE"
      nodeId={nodeId}
      index={index}
      selected={selected}
    >
      <Textarea
        value={data.markdown ?? ""}
        onChange={(event) => onChange(event.target.value)}
        placeholder="Write a note in markdown..."
        className="nodrag h-full resize-none border-none bg-transparent p-0 text-sm shadow-none focus-visible:ring-0"
      />
    </NodeShell>
  );
}
