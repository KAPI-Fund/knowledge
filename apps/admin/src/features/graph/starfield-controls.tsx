import { Globe, Torus, Tornado } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

import type { StarfieldLayout } from "./starfield-forces";

const LAYOUTS: Array<{ value: StarfieldLayout; label: string; icon: typeof Globe }> = [
  { value: "sphere", label: "Sphere layout", icon: Globe },
  { value: "ring", label: "Ring layout", icon: Torus },
  { value: "tornado", label: "Tornado layout", icon: Tornado },
];

export function StarfieldControls({
  layout,
  onLayoutChange,
}: {
  layout: StarfieldLayout;
  onLayoutChange: (layout: StarfieldLayout) => void;
}) {
  return (
    <div className="absolute top-3 left-3 z-10 flex gap-1 rounded-lg border bg-background/80 p-1 backdrop-blur-sm">
      <TooltipProvider delayDuration={300}>
      {LAYOUTS.map(({ value, label, icon: Icon }) => (
        <Tooltip key={value}>
          <TooltipTrigger asChild>
            <Button
              aria-label={label}
              aria-pressed={layout === value}
              className="h-7 w-7"
              onClick={() => onLayoutChange(value)}
              size="icon"
              variant={layout === value ? "default" : "ghost"}
            >
              <Icon className="h-3.5 w-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">{label}</TooltipContent>
        </Tooltip>
      ))}
      </TooltipProvider>
    </div>
  );
}
