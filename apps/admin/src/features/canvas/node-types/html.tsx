import { useEffect, useState } from "react";

import { Loader2, Presentation, RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";

import { NodeError } from "./node-error";
import { NodeShell } from "./node-shell";

export type HtmlNodeStatus = "running" | "done" | "error";

export interface HtmlNodeData {
  status?: HtmlNodeStatus;
  jobId?: string;
  assetId?: string;
  url?: string;
  title?: string;
  error?: string | null;
  progressStage?: string | null;
  progressMessage?: string | null;
}

const STAGE_LABELS: Record<string, string> = {
  queued: "排队中",
  create: "准备沙箱",
  codex: "生成中",
};

function formatElapsed(seconds: number) {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function RunningProgress({ stage, message }: { stage?: string | null; message?: string | null }) {
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setElapsed((v) => v + 1), 1000);
    return () => clearInterval(t);
  }, []);

  return (
    <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-2 px-3">
      <div className="flex items-center gap-2">
        <Loader2 className="size-3.5 animate-spin text-muted-foreground" />
        <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
          {(stage && STAGE_LABELS[stage]) ?? "正在生成幻灯片"}
        </span>
        <span className="text-[11px] tabular-nums text-muted-foreground">
          已进行 {formatElapsed(elapsed)}
        </span>
      </div>
      {message ? (
        <p className="line-clamp-2 max-w-full break-all text-center font-mono text-[10px] text-muted-foreground">
          {message}
        </p>
      ) : null}
    </div>
  );
}

interface HtmlNodeProps {
  data: HtmlNodeData;
  nodeId: string;
  index?: number;
  selected?: boolean;
  onRetry?: () => void;
}

export function HtmlNode({ data, nodeId, index, selected, onRetry }: HtmlNodeProps) {
  const status = data.status ?? "running";
  const shellStatus = status === "running" ? "running" : status === "error" ? "error" : "ok";

  return (
    <NodeShell
      icon={<Presentation className="size-3.5" />}
      label="SKILL · PPT"
      nodeId={nodeId}
      index={index}
      selected={selected}
      status={shellStatus}
    >
      {status === "done" && data.url ? (
        <div className="flex min-h-0 flex-1 flex-col gap-2">
          {/* allow-scripts without allow-same-origin: deck JS (paging, wheel,
              animations) runs in an opaque origin, so untrusted model HTML still
              cannot reach the admin session's cookies or storage. */}
          <iframe
            sandbox="allow-scripts"
            allowFullScreen
            src={data.url}
            title={data.title ?? "deck"}
            className="min-h-0 w-full flex-1 rounded-md border border-border bg-white"
          />
          <div className="flex shrink-0 items-center justify-between gap-2 text-xs text-muted-foreground">
            <span>点击幻灯片后可用 ←/→ 翻页</span>
            <a href={data.url} target="_blank" rel="noreferrer" className="underline">
              在新标签打开
            </a>
          </div>
        </div>
      ) : status === "error" ? (
        <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto">
          <NodeError title="幻灯片生成失败" message={data.error ?? "生成失败"} />
          {onRetry && data.jobId ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="self-start"
              onClick={onRetry}
            >
              <RotateCcw className="size-3.5" />
              重试
            </Button>
          ) : null}
        </div>
      ) : (
        <RunningProgress stage={data.progressStage} message={data.progressMessage} />
      )}
    </NodeShell>
  );
}
