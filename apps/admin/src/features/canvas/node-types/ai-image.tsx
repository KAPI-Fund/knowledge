import { ImageIcon, Loader2, Play } from "lucide-react";

import { Button } from "@/components/ui/button";
import { CompositionTextarea } from "@/components/ui/composition-input";

import { ModelTag } from "./model-tag";
import { NodeError } from "./node-error";
import { NodeShell } from "./node-shell";
import { VersionSwitcher } from "./version-switcher";

export interface ImageVersion {
  id: string;
  url: string;
  createdAt?: string;
}

export type AiNodeStatus = "idle" | "running" | "error";

export interface AiImageNodeData {
  prompt?: string;
  versions?: ImageVersion[];
  activeVersionId?: string | null;
  status?: AiNodeStatus;
  error?: string | null;
}

interface AiImageNodeProps {
  data: AiImageNodeData;
  nodeId: string;
  index?: number;
  selected?: boolean;
  model?: string | null;
  onRegenerate: () => void;
  onVersionChange: (id: string) => void;
  onPromptChange?: (prompt: string) => void;
}

export function AiImageNode({
  data,
  nodeId,
  index,
  selected,
  model,
  onRegenerate,
  onVersionChange,
  onPromptChange,
}: AiImageNodeProps) {
  const versions = data.versions ?? [];
  const active = versions.find((v) => v.id === data.activeVersionId) ?? versions[versions.length - 1];
  const status = data.status ?? "idle";
  const running = status === "running";
  const isError = status === "error";
  const actionLabel = isError ? "Retry" : versions.length > 0 ? "Regenerate" : "Run";

  return (
    <NodeShell
      icon={<ImageIcon className="size-3.5" />}
      label="AI · IMAGE"
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
            aria-label="regenerate image"
            onClick={onRegenerate}
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
        placeholder="Describe the image to generate..."
        className="nodrag h-14 resize-none text-xs"
      />
      {isError ? (
        <NodeError title="Image generation failed" message={data.error ?? "Generation failed"} />
      ) : null}
      <div className="flex min-h-0 flex-1 items-center justify-center overflow-hidden rounded-md bg-muted/30">
        {active ? (
          <img
            src={active.url}
            alt={data.prompt ?? "Generated image"}
            className="max-h-full max-w-full rounded-md object-contain"
          />
        ) : (
          <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
            Empty · click run
          </span>
        )}
      </div>
    </NodeShell>
  );
}
