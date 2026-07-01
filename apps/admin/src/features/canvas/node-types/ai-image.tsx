import { ImageIcon, Loader2, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";

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
  onRegenerate: () => void;
  onVersionChange: (id: string) => void;
}

export function AiImageNode({ data, onRegenerate, onVersionChange }: AiImageNodeProps) {
  const versions = data.versions ?? [];
  const active = versions.find((v) => v.id === data.activeVersionId) ?? versions[versions.length - 1];
  const running = data.status === "running";
  return (
    <div className="flex h-full flex-col gap-2 rounded-md border bg-card p-3 text-card-foreground shadow-sm">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1 text-xs font-medium text-muted-foreground">
          <ImageIcon className="size-3" />
          AI · Image
        </div>
        <div className="flex items-center gap-1">
          {versions.length > 0 && active ? (
            <VersionSwitcher versions={versions} activeId={active.id} onChange={onVersionChange} />
          ) : null}
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            aria-label="regenerate image"
            onClick={onRegenerate}
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
          {data.error ?? "Generation failed"}
        </div>
      ) : null}
      <div className="flex min-h-0 flex-1 items-center justify-center overflow-hidden">
        {active ? (
          <img
            src={active.url}
            alt={data.prompt ?? "Generated image"}
            className="max-h-full max-w-full rounded-md object-contain"
          />
        ) : (
          <span className="text-xs text-muted-foreground">{data.prompt ?? "No image yet."}</span>
        )}
      </div>
    </div>
  );
}
