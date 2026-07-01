import { Textarea } from "@/components/ui/textarea";

export interface NoteNodeData {
  markdown?: string;
}

interface NoteNodeProps {
  data: NoteNodeData;
  onChange: (markdown: string) => void;
}

export function NoteNode({ data, onChange }: NoteNodeProps) {
  return (
    <div className="flex h-full flex-col gap-2 rounded-md border bg-card p-3 text-card-foreground shadow-sm">
      <div className="text-xs font-medium text-muted-foreground">Note</div>
      <Textarea
        value={data.markdown ?? ""}
        onChange={(event) => onChange(event.target.value)}
        placeholder="Write a note in markdown…"
        className="h-full resize-none border-none bg-transparent p-0 text-sm shadow-none focus-visible:ring-0"
      />
    </div>
  );
}
