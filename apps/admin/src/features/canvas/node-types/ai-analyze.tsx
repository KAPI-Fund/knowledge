import { Loader2, RefreshCw, Sparkles } from "lucide-react";

import { MarkdownMessage } from "@/components/shared/markdown-message";
import { Button } from "@/components/ui/button";

import { VersionSwitcher } from "./version-switcher";

export interface AnalyzeVersion {
  id: string;
  content: string;
  createdAt?: string;
}

export type AiNodeStatus = "idle" | "running" | "error";

export interface AiAnalyzeNodeData {
  prompt?: string;
  versions?: AnalyzeVersion[];
  activeVersionId?: string | null;
  status?: AiNodeStatus;
  error?: string | null;
}

interface AiAnalyzeNodeProps {
  data: AiAnalyzeNodeData;
  onRerun: () => void;
  onVersionChange: (id: string) => void;
}

export function AiAnalyzeNode({ data, onRerun, onVersionChange }: AiAnalyzeNodeProps) {
  const versions = data.versions ?? [];
  const active = versions.find((v) => v.id === data.activeVersionId) ?? versions[versions.length - 1];
  const running = data.status === "running";
  return (
    <div className="flex h-full flex-col gap-2 rounded-md border bg-card p-3 text-card-foreground shadow-sm">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          <Sparkles className="size-3" />
          AI - Analyze
        </div>
        <div className="flex items-center gap-1">
          {versions.length > 0 && active ? (
            <VersionSwitcher versions={versions} activeId={active.id} onChange={onVersionChange} />
          ) : null}
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            aria-label="rerun analysis"
            onClick={onRerun}
            disabled={running}
          >
            {running ? (
              <Loader2 className="size-3 animate-spin" />
            ) : (
              <RefreshCw className="size-3" />
            )}
          </Button>
        </div>
      </div>
      {data.status === "error" ? (
        <div className="rounded-md bg-destructive/10 px-2 py-1 text-xs text-destructive">
          {data.error ?? "Analysis failed"}
        </div>
      ) : null}
      <div className="min-h-0 flex-1 overflow-auto text-sm">
        {active ? (
          <MarkdownMessage content={active.content} />
        ) : (
          <span className="text-muted-foreground">{data.prompt ?? "No analysis yet."}</span>
        )}
      </div>
    </div>
  );
}
