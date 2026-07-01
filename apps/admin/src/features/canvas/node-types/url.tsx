import { Loader2 } from "lucide-react";

import { MarkdownMessage } from "@/components/shared/markdown-message";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export type UrlNodeStatus = "idle" | "loading" | "error";

export interface UrlNodeData {
  url?: string;
  title?: string;
  markdown?: string;
  status?: UrlNodeStatus;
  error?: string | null;
}

interface UrlNodeProps {
  data: UrlNodeData;
  onUrlChange: (url: string) => void;
  onFetch: () => void;
}

export function UrlNode({ data, onUrlChange, onFetch }: UrlNodeProps) {
  const loading = data.status === "loading";
  return (
    <div className="flex h-full flex-col gap-2 rounded-md border bg-card p-3 text-card-foreground shadow-sm">
      <div className="flex items-center gap-2">
        <Input
          value={data.url ?? ""}
          onChange={(event) => onUrlChange(event.target.value)}
          placeholder="https://…"
          className="h-8 text-sm"
        />
        <Button type="button" size="sm" onClick={onFetch} disabled={loading || !data.url}>
          {loading ? <Loader2 className="size-3 animate-spin" /> : "Fetch"}
        </Button>
      </div>
      {data.status === "error" ? (
        <div className="flex items-center justify-between gap-2 rounded-md bg-destructive/10 px-2 py-1 text-xs text-destructive">
          <span>{data.error ?? "Failed to fetch"}</span>
          <Button type="button" size="xs" variant="ghost" onClick={onFetch}>
            Retry
          </Button>
        </div>
      ) : null}
      {data.title ? <div className="text-sm font-medium">{data.title}</div> : null}
      {data.markdown ? (
        <div className="min-h-0 flex-1 overflow-auto text-sm">
          <MarkdownMessage content={data.markdown} />
        </div>
      ) : null}
    </div>
  );
}
