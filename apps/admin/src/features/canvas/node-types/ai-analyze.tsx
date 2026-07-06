import { Loader2, Play, Sparkles } from "lucide-react";

import { MarkdownMessage } from "@/components/shared/markdown-message";
import { Button } from "@/components/ui/button";
import { CompositionTextarea } from "@/components/ui/composition-input";

import { ModelTag } from "./model-tag";
import { NodeError } from "./node-error";
import { NodeShell } from "./node-shell";
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
  nodeId: string;
  index?: number;
  selected?: boolean;
  model?: string | null;
  onRerun: () => void;
  onVersionChange: (id: string) => void;
  onPromptChange?: (prompt: string) => void;
}

export function AiAnalyzeNode({
  data,
  nodeId,
  index,
  selected,
  model,
  onRerun,
  onVersionChange,
  onPromptChange,
}: AiAnalyzeNodeProps) {
  const versions = data.versions ?? [];
  const active = versions.find((v) => v.id === data.activeVersionId) ?? versions[versions.length - 1];
  const status = data.status ?? "idle";
  const running = status === "running";
  const isError = status === "error";
  const actionLabel = isError ? "Retry" : versions.length > 0 ? "Rerun" : "Run";

  return (
    <NodeShell
      icon={<Sparkles className="size-3.5" />}
      label="AI · ANALYZE"
      nodeId={nodeId}
      index={index}
      selected={selected}
      status={status}
      headerRight={
        <>
          {versions.length > 0 && active ? (
            <VersionSwitcher versions={versions} activeId={active.id} onChange={onVersionChange} />
          ) : null}
          <Button
            type="button"
            size="xs"
            variant={isError ? "destructive" : "default"}
            aria-label="rerun analysis"
            onClick={onRerun}
            disabled={running}
          >
            {running ? <Loader2 className="size-3 animate-spin" /> : <Play className="size-3" />}
            {actionLabel}
          </Button>
        </>
      }
    >
      <ModelTag model={model} />
      <CompositionTextarea
        value={data.prompt ?? ""}
        onValueChange={onPromptChange ?? (() => {})}
        readOnly={!onPromptChange}
        placeholder="Describe what to analyze..."
        className="nodrag h-14 resize-none text-xs"
      />
      {isError ? (
        <NodeError title="Analysis failed" message={data.error ?? "Analysis failed"} />
      ) : null}
      <div className="min-h-0 flex-1 overflow-auto text-sm">
        {active ? (
          <MarkdownMessage content={active.content} />
        ) : (
          <div className="flex h-full items-center justify-center text-[11px] uppercase tracking-wider text-muted-foreground">
            Empty · click run
          </div>
        )}
      </div>
    </NodeShell>
  );
}
