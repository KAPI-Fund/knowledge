import { useMemo, useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { COMMUNITY_COLORS, nodeColor } from "./graph-colors";
import type { CommunityInfo, GraphFilterState, GraphNode } from "./types";

interface GraphLegendProps {
  nodes: GraphNode[];
  communities: CommunityInfo[];
  colorMode: "type" | "community";
  hiddenTypes: GraphFilterState["hiddenTypes"];
  onToggleType: (type: string) => void;
  onShowAllTypes: () => void;
}

export function GraphLegend({
  nodes,
  communities,
  colorMode,
  hiddenTypes,
  onToggleType,
  onShowAllTypes,
}: GraphLegendProps) {
  const [collapsed, setCollapsed] = useState(false);

  const typeCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const node of nodes) {
      counts.set(node.type, (counts.get(node.type) ?? 0) + 1);
    }
    return [...counts.entries()].sort((a, b) => b[1] - a[1]);
  }, [nodes]);

  return (
    <div className="absolute bottom-3 left-3 w-56 rounded-md border bg-background/85 p-3 text-xs shadow-sm backdrop-blur-sm">
      <div className="flex items-center justify-between">
        <span className="font-semibold">
          {colorMode === "community" ? "Communities" : "Node Types"}
        </span>
        <div className="flex items-center gap-1">
          {colorMode === "type" && hiddenTypes.size > 0 ? (
            <button
              className="text-[11px] text-muted-foreground hover:text-foreground"
              onClick={onShowAllTypes}
              type="button"
            >
              show all
            </button>
          ) : null}
          <button
            aria-label={collapsed ? "Expand legend" : "Collapse legend"}
            className="text-muted-foreground hover:text-foreground"
            onClick={() => setCollapsed((value) => !value)}
            type="button"
          >
            {collapsed ? <ChevronUp className="h-3.5 w-3.5" /> : <ChevronDown className="h-3.5 w-3.5" />}
          </button>
        </div>
      </div>

      {collapsed ? null : (
        <ul className="mt-2 grid max-h-48 gap-1 overflow-y-auto pr-1">
          {colorMode === "community"
            ? communities.map((community) => {
                const sparse = community.cohesion < 0.15 && community.nodeCount >= 3;
                return (
                  <li key={community.id} className="flex items-center gap-2">
                    <span
                      className="inline-block h-3 w-3 shrink-0 rounded-full"
                      style={{ backgroundColor: COMMUNITY_COLORS[community.id % COMMUNITY_COLORS.length] }}
                    />
                    <span className="truncate">{community.topNodes[0] ?? `Community ${community.id}`}</span>
                    <span className="ml-auto text-muted-foreground">{community.nodeCount}</span>
                    {sparse ? (
                      <span className="text-amber-500" title="Low cohesion cluster">!</span>
                    ) : null}
                  </li>
                );
              })
            : typeCounts.map(([type, count]) => {
                const hidden = hiddenTypes.has(type);
                return (
                  <li key={type}>
                    <button
                      aria-pressed={!hidden}
                      className={`flex w-full items-center gap-2 rounded px-1 py-0.5 text-left hover:bg-muted ${hidden ? "opacity-40" : ""}`}
                      onClick={() => onToggleType(type)}
                      type="button"
                    >
                      <span
                        className="inline-block h-3 w-3 shrink-0 rounded-full"
                        style={{ backgroundColor: nodeColor(type) }}
                      />
                      <span className={`truncate ${hidden ? "line-through" : ""}`}>{type}</span>
                      <span className="ml-auto text-muted-foreground">{count}</span>
                    </button>
                  </li>
                );
              })}
        </ul>
      )}
    </div>
  );
}
