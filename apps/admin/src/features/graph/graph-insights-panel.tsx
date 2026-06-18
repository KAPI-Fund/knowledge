import { AlertTriangle, Lightbulb, Link2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { knowledgeGapKey } from "./graph-insights";
import type { KnowledgeGap, SurprisingConnection } from "./types";

interface GraphInsightsPanelProps {
  surprising: SurprisingConnection[];
  gaps: KnowledgeGap[];
  highlightedNodes: Set<string>;
  onHighlight: (nodeIds: Set<string>) => void;
  onDismiss: (key: string, ids?: Set<string>) => void;
  onClose: () => void;
}

export function GraphInsightsPanel({
  surprising,
  gaps,
  highlightedNodes,
  onHighlight,
  onDismiss,
  onClose,
}: GraphInsightsPanelProps) {
  const visibleSurprising = surprising;
  const visibleGaps = gaps;

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0">
        <CardTitle className="flex items-center gap-2 text-sm">
          <Lightbulb className="h-4 w-4 text-amber-500" />
          Insights
        </CardTitle>
        <Button aria-label="Close insights" onClick={onClose} size="icon" variant="ghost" className="h-7 w-7">
          <X className="h-3.5 w-3.5" />
        </Button>
      </CardHeader>
      <CardContent className="grid gap-4 text-sm">
        <section className="grid gap-2">
          <h3 className="flex items-center gap-2 font-medium">
            <Link2 className="h-3.5 w-3.5 text-sky-500" />
            Surprising connections
          </h3>
          {visibleSurprising.length === 0 ? (
            <p className="text-xs text-muted-foreground">No surprising connections in the current view.</p>
          ) : (
            <ul className="grid gap-2">
              {visibleSurprising.map((item) => {
                const active = highlightedNodes.has(item.source.id) && highlightedNodes.has(item.target.id);
                return (
                  <li key={item.key}>
                    <div
                      className={`rounded-md border p-2 ${active ? "border-sky-400 bg-sky-50" : ""}`}
                    >
                      <div className="flex items-start justify-between gap-2">
                        <button
                          className="text-left font-medium hover:underline"
                          onClick={() => onHighlight(new Set([item.source.id, item.target.id]))}
                          type="button"
                        >
                          {item.source.label} → {item.target.label}
                        </button>
                        <button
                          aria-label="Dismiss insight"
                          className="text-muted-foreground hover:text-foreground"
                          onClick={() => onDismiss(item.key, new Set([item.source.id, item.target.id]))}
                          type="button"
                        >
                          <X className="h-3 w-3" />
                        </button>
                      </div>
                      <p className="mt-1 text-xs text-muted-foreground">{item.reasons.join("; ")}</p>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <section className="grid gap-2">
          <h3 className="flex items-center gap-2 font-medium">
            <AlertTriangle className="h-3.5 w-3.5 text-amber-500" />
            Knowledge gaps
          </h3>
          {visibleGaps.length === 0 ? (
            <p className="text-xs text-muted-foreground">No knowledge gaps detected in the current view.</p>
          ) : (
            <ul className="grid gap-2">
              {visibleGaps.map((gap) => {
                const key = knowledgeGapKey(gap);
                return (
                  <li key={key}>
                    <div className="rounded-md border p-2">
                      <div className="flex items-start justify-between gap-2">
                        <button
                          className="text-left font-medium hover:underline"
                          onClick={() => onHighlight(new Set(gap.nodeIds))}
                          type="button"
                        >
                          {gap.title}
                        </button>
                        <button
                          aria-label="Dismiss insight"
                          className="text-muted-foreground hover:text-foreground"
                          onClick={() => onDismiss(key, new Set(gap.nodeIds))}
                          type="button"
                        >
                          <X className="h-3 w-3" />
                        </button>
                      </div>
                      <p className="mt-1 text-xs text-muted-foreground">{gap.description}</p>
                      <p className="mt-1 text-xs text-muted-foreground/80">{gap.suggestion}</p>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </section>
      </CardContent>
    </Card>
  );
}
