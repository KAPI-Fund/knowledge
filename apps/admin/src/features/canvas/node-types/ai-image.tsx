import { Download, ImageIcon, Loader2, Play } from "lucide-react";

import { Button } from "@/components/ui/button";
import { CompositionTextarea } from "@/components/ui/composition-input";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

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
          <ImageViewer url={active.url} versionId={active.id} prompt={data.prompt} />
        ) : (
          <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
            Empty · click run
          </span>
        )}
      </div>
    </NodeShell>
  );
}

// The in-node preview opens a lightbox with the image at full size plus a
// download link. `nodrag` keeps React Flow from starting a node drag on click.
function ImageViewer({ url, versionId, prompt }: { url: string; versionId: string; prompt?: string }) {
  const alt = prompt ?? "Generated image";
  return (
    <Dialog>
      <DialogTrigger asChild>
        <button
          type="button"
          aria-label="View full image"
          className="nodrag flex max-h-full max-w-full cursor-zoom-in items-center justify-center"
        >
          <img src={url} alt={alt} className="max-h-full max-w-full rounded-md object-contain" />
        </button>
      </DialogTrigger>
      <DialogContent
        aria-describedby={undefined}
        className="w-auto max-w-[92vw] gap-3 p-4"
      >
        <DialogTitle className="sr-only">Image preview</DialogTitle>
        <img src={url} alt={alt} className="max-h-[80vh] max-w-full rounded-md object-contain" />
        <DialogFooter className="sm:items-center sm:justify-between">
          <span className="min-w-0 truncate text-xs text-muted-foreground">{prompt}</span>
          <Button asChild size="sm" variant="outline">
            <a href={url} download={`ai-image-${versionId}.png`}>
              <Download className="size-3.5" />
              Download
            </a>
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
