import { Cpu } from "lucide-react";

interface ModelTagProps {
  model?: string | null;
}

export function ModelTag({ model }: ModelTagProps) {
  return (
    <div className="flex min-w-0 items-center gap-1.5 text-[10px] text-muted-foreground">
      <span className="shrink-0 font-semibold uppercase tracking-wider">Model</span>
      <span className="flex min-w-0 items-center gap-1 rounded bg-muted px-1.5 py-0.5 font-mono">
        <Cpu className="size-3 shrink-0" />
        <span className="truncate">{model || "not configured"}</span>
      </span>
    </div>
  );
}
