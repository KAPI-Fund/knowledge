import {
  BookOpenText,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  FilePlus2,
  FileText,
  Loader2,
  Wrench,
} from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";

import type { AgentEvent, AgentReference } from "./agent-types";

interface AgentToolStep {
  id: string;
  tool: string;
  input?: string | null;
  output?: string | null;
  status: "running" | "done";
}

interface AgentFileChange {
  id: string;
  path: string;
  tool: string;
  existedBefore: boolean;
}

interface AgentActivitySummary {
  steps: AgentToolStep[];
  files: AgentFileChange[];
  references: AgentReference[];
  errors: string[];
}

function summarizeEvents(events: AgentEvent[]): AgentActivitySummary {
  const steps: AgentToolStep[] = [];
  const files: AgentFileChange[] = [];
  const references: AgentReference[] = [];
  const errors: string[] = [];
  let counter = 0;
  for (const event of events) {
    if (event.type === "toolStart") {
      counter += 1;
      steps.push({
        id: `step-${counter}`,
        tool: event.tool,
        input: event.input,
        status: "running",
      });
    } else if (event.type === "toolEnd") {
      const open = [...steps]
        .reverse()
        .find((step) => step.tool === event.tool && step.status === "running");
      if (open) {
        open.status = "done";
        open.output = event.output;
      }
    } else if (event.type === "fileChanged") {
      counter += 1;
      files.push({
        id: `file-${counter}`,
        path: event.path,
        tool: event.tool,
        existedBefore: event.existedBefore,
      });
    } else if (event.type === "referenceAdded") {
      references.push(event.reference);
    } else if (event.type === "error") {
      errors.push(event.message);
    }
  }
  return { steps, files, references, errors };
}

export function AgentActivity({ events, live = false }: { events: AgentEvent[]; live?: boolean }) {
  const [expandedSteps, setExpandedSteps] = useState<Record<string, boolean>>({});
  const summary = useMemo(() => summarizeEvents(events), [events]);

  if (
    summary.steps.length === 0 &&
    summary.files.length === 0 &&
    summary.references.length === 0 &&
    summary.errors.length === 0
  ) {
    return null;
  }

  return (
    <section
      aria-label="Agent activity"
      className="mb-2 rounded-md border border-border/60 bg-background/60 text-xs"
    >
      {summary.steps.length > 0 && (
        <ul>
          {summary.steps.map((step) => {
            const expanded = Boolean(expandedSteps[step.id]);
            const hasDetail = Boolean(step.input) || Boolean(step.output);
            return (
              <li className="border-b border-border/40 last:border-b-0" key={step.id}>
                <button
                  aria-expanded={expanded}
                  className="flex w-full min-w-0 items-center gap-1.5 px-2 py-1.5 text-left hover:bg-accent/40 disabled:cursor-default"
                  disabled={!hasDetail}
                  onClick={() =>
                    setExpandedSteps((current) => ({ ...current, [step.id]: !expanded }))
                  }
                  type="button"
                >
                  {hasDetail ? (
                    expanded ? (
                      <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" />
                    ) : (
                      <ChevronRight className="size-3.5 shrink-0 text-muted-foreground" />
                    )
                  ) : (
                    <span className="size-3.5 shrink-0" />
                  )}
                  {step.status === "running" && live ? (
                    <Loader2 className="size-3.5 shrink-0 animate-spin text-muted-foreground" />
                  ) : (
                    <Wrench className="size-3.5 shrink-0 text-muted-foreground" />
                  )}
                  <Badge className="font-mono text-[10px]" variant="secondary">
                    {step.tool}
                  </Badge>
                  <span className="min-w-0 flex-1 truncate text-muted-foreground">
                    {step.input ?? ""}
                  </span>
                </button>
                {expanded && (
                  <div className="space-y-1 border-t border-border/30 bg-muted/30 px-3 py-2">
                    {step.input ? (
                      <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words font-mono text-[10px] leading-4">
                        {step.input}
                      </pre>
                    ) : null}
                    {step.output ? (
                      <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words border-t border-border/30 pt-1 font-mono text-[10px] leading-4 text-muted-foreground">
                        {step.output}
                      </pre>
                    ) : null}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}

      {summary.files.length > 0 && (
        <ul className="border-t border-border/40">
          {summary.files.map((file) => {
            const FileIcon = file.existedBefore ? FileText : FilePlus2;
            return (
              <li
                className="flex min-w-0 items-center gap-1.5 border-b border-border/30 px-2 py-1.5 last:border-b-0"
                key={file.id}
              >
                <FileIcon className="size-3.5 shrink-0 text-muted-foreground" />
                <span className="min-w-0 flex-1 truncate font-mono" title={file.path}>
                  {file.path}
                </span>
                <Badge className="text-[10px]" variant="outline">
                  {file.existedBefore ? "modified" : "created"}
                </Badge>
                <span className="shrink-0 text-[10px] text-muted-foreground">{file.tool}</span>
              </li>
            );
          })}
        </ul>
      )}

      {summary.references.length > 0 && (
        <div className="flex flex-wrap items-center gap-1 border-t border-border/40 px-2 py-1.5">
          <BookOpenText className="size-3.5 shrink-0 text-muted-foreground" />
          {summary.references.map((reference) => (
            <Badge
              className="max-w-56 truncate text-[10px]"
              key={`${reference.kind}:${reference.path}`}
              title={reference.path}
              variant="outline"
            >
              {reference.title || reference.path}
            </Badge>
          ))}
        </div>
      )}

      {summary.errors.map((message, index) => (
        <p
          className="flex items-center gap-1.5 border-t border-destructive/20 px-2 py-1.5 text-[11px] text-destructive"
          key={`error-${index}`}
        >
          <CircleAlert className="size-3.5 shrink-0" />
          {message}
        </p>
      ))}
    </section>
  );
}
