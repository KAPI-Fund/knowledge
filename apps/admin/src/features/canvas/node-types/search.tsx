import { Loader2, Search } from "lucide-react";

import { MarkdownMessage } from "@/components/shared/markdown-message";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

import { NodeError } from "./node-error";
import { NodeShell } from "./node-shell";

export type SearchNodeStatus = "idle" | "loading" | "error";

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
  onSearch: () => void;
}

export function SearchNode({ data, nodeId, index, selected, onQueryChange, onSearch }: SearchNodeProps) {
  const status = data.status ?? "idle";
  const loading = status === "loading";
  const isError = status === "error";
  return (
    <NodeShell
      icon={<Search className="size-3.5" />}
      label="SEARCH · WEB"
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
          onClick={onSearch}
          disabled={loading || !data.query}
        >
          {loading ? (
            <Loader2 className="size-3 animate-spin" />
          ) : (
            <Search className="size-3" />
          )}
          {isError ? "Retry" : "Search"}
        </Button>
      }
    >
      <Input
        value={data.query ?? ""}
        onChange={(event) => onQueryChange(event.target.value)}
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
