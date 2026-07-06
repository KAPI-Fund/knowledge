import { Loader2, Search } from "lucide-react";

import { MarkdownMessage } from "@/components/shared/markdown-message";
import { Button } from "@/components/ui/button";
import { CompositionInput } from "@/components/ui/composition-input";

import { NodeError } from "./node-error";
import { NodeShell } from "./node-shell";

export type SearchNodeStatus = "idle" | "running" | "error";

export interface SearchNodeData {
  query?: string;
  markdown?: string;
  status?: SearchNodeStatus;
  error?: string | null;
}

interface SearchNodeProps {
  data: SearchNodeData;
  nodeId: string;
  index?: number;
  selected?: boolean;
  onQueryChange: (query: string) => void;
  onRun: () => void;
}

export function SearchNode({ data, nodeId, index, selected, onQueryChange, onRun }: SearchNodeProps) {
  const status = data.status ?? "idle";
  const running = status === "running";
  const isError = status === "error";
  return (
    <NodeShell
      icon={<Search className="size-3.5" />}
      label="SEARCH · WEB"
      nodeId={nodeId}
      index={index}
      selected={selected}
      status={status}
      headerRight={
        <Button
          type="button"
          size="xs"
          variant={isError ? "destructive" : "default"}
          onClick={onRun}
          disabled={running}
        >
          {running ? (
            <Loader2 className="size-3 animate-spin" />
          ) : (
            <Search className="size-3" />
          )}
          {isError ? "Retry" : "Search"}
        </Button>
      }
    >
      <CompositionInput
        value={data.query ?? ""}
        onValueChange={onQueryChange}
        placeholder="Search the web..."
        className="nodrag h-8 text-xs"
      />
      {isError ? (
        <NodeError title="Search failed" message={data.error ?? "Search failed"} />
      ) : null}
      {data.markdown ? (
        <div className="min-h-0 flex-1 overflow-auto text-sm">
          <MarkdownMessage content={data.markdown} />
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 items-center justify-center text-[11px] uppercase tracking-wider text-muted-foreground">
          Empty · click search
        </div>
      )}
    </NodeShell>
  );
}
