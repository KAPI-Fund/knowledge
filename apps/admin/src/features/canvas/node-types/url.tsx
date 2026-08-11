import { Download, Globe, Loader2 } from "lucide-react";

import { MarkdownMessage } from "@/components/shared/markdown-message";
import { Button } from "@/components/ui/button";
import { CompositionInput } from "@/components/ui/composition-input";

import { NodeError } from "./node-error";
import { NodeShell } from "./node-shell";

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
  nodeId: string;
  index?: number;
  selected?: boolean;
  onUrlChange: (url: string) => void;
  onFetch: () => void;
}

export function UrlNode({ data, nodeId, index, selected, onUrlChange, onFetch }: UrlNodeProps) {
  const status = data.status ?? "idle";
  const loading = status === "loading";
  const isError = status === "error";
  return (
    <NodeShell
      icon={<Globe className="size-3.5" />}
      label="URL · WEB"
      nodeId={nodeId}
      index={index}
      selected={selected}
      status={status}
      targetHandle={false}
      headerRight={
        <Button
          type="button"
          size="xs"
          variant={isError ? "destructive" : "default"}
          onClick={onFetch}
          disabled={loading || !data.url}
        >
          {loading ? (
            <Loader2 className="size-3 animate-spin" />
          ) : (
            <Download className="size-3" />
          )}
          {isError ? "Retry" : "Fetch"}
        </Button>
      }
    >
      <CompositionInput
        value={data.url ?? ""}
        onValueChange={onUrlChange}
        placeholder="https://..."
        className="nodrag h-8 font-mono text-xs"
      />
      {isError ? (
        <NodeError title="Fetch failed" message={data.error ?? "Failed to fetch"} />
      ) : null}
      {data.title ? <div className="text-sm font-medium">{data.title}</div> : null}
      {data.markdown ? (
        <div className="min-h-0 flex-1 overflow-auto text-sm">
          <MarkdownMessage content={data.markdown} />
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 items-center justify-center text-[11px] uppercase tracking-wider text-muted-foreground">
          Empty · click fetch
        </div>
      )}
    </NodeShell>
  );
}
