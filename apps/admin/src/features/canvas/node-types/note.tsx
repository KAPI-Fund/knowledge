import { StickyNote } from "lucide-react";

import { CompositionTextarea } from "@/components/ui/composition-input";

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
      <CompositionTextarea
        value={data.markdown ?? ""}
        onValueChange={onChange}
        placeholder="Write a note in markdown..."
        className="nodrag h-full resize-none border-none bg-transparent p-0 text-sm shadow-none focus-visible:ring-0"
      />
    </NodeShell>
  );
}
