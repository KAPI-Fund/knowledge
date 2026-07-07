import { Presentation } from "lucide-react";

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
}

interface HtmlNodeProps {
  data: HtmlNodeData;
  nodeId: string;
  index?: number;
  selected?: boolean;
}

export function HtmlNode({ data, nodeId, index, selected }: HtmlNodeProps) {
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
          {/* sandbox="" strips all privileges: the generated deck runs with no
              script/same-origin access, so untrusted model HTML cannot touch the
              admin session. */}
          <iframe
            sandbox=""
            src={data.url}
            title={data.title ?? "deck"}
            className="min-h-0 w-full flex-1 rounded-md border border-border bg-white"
          />
          <a
            href={data.url}
            target="_blank"
            rel="noreferrer"
            className="shrink-0 text-xs text-muted-foreground underline"
          >
            在新标签打开
          </a>
        </div>
      ) : status === "error" ? (
        <NodeError title="幻灯片生成失败" message={data.error ?? "生成失败"} />
      ) : (
        <div className="flex min-h-0 flex-1 items-center justify-center">
          <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
            正在生成幻灯片…
          </span>
        </div>
      )}
    </NodeShell>
  );
}
