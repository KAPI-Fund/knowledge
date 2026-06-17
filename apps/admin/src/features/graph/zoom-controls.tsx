import { useSigma } from "@react-sigma/core";
import { Maximize, ZoomIn, ZoomOut } from "lucide-react";
import { Button } from "@/components/ui/button";

export function ZoomControls() {
  const sigma = useSigma();

  return (
    <div className="absolute top-3 right-3 flex flex-col gap-1">
      <Button
        variant="outline"
        size="icon"
        className="h-7 w-7 bg-background/80 backdrop-blur-sm"
        aria-label="Zoom in"
        onClick={() => sigma.getCamera().animatedZoom({ duration: 200 })}
      >
        <ZoomIn className="h-3.5 w-3.5" />
      </Button>
      <Button
        variant="outline"
        size="icon"
        className="h-7 w-7 bg-background/80 backdrop-blur-sm"
        aria-label="Zoom out"
        onClick={() => sigma.getCamera().animatedUnzoom({ duration: 200 })}
      >
        <ZoomOut className="h-3.5 w-3.5" />
      </Button>
      <Button
        variant="outline"
        size="icon"
        className="h-7 w-7 bg-background/80 backdrop-blur-sm"
        aria-label="Reset view"
        onClick={() => sigma.getCamera().animatedReset({ duration: 300 })}
      >
        <Maximize className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}
