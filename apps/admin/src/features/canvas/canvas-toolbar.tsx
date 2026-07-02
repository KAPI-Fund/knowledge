import { Database, FileText, Globe } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";

import type { SkillNodePayload } from "./chat-panel";
import { KbProjectPicker } from "./kb-picker";

interface CanvasToolbarProps {
  onAdd: (payload: SkillNodePayload) => void;
}

export function CanvasToolbar({ onAdd }: CanvasToolbarProps) {
  const [kbOpen, setKbOpen] = useState(false);
  return (
    <div className="flex items-center gap-1">
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={() => onAdd({ node: { type: "note", data: {} }, x: 0, y: 0 })}
      >
        <FileText className="size-3" />
        Note
      </Button>
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={() => onAdd({ node: { type: "url", data: {} }, x: 0, y: 0 })}
      >
        <Globe className="size-3" />
        Web page
      </Button>
      <Button type="button" size="sm" variant="outline" onClick={() => setKbOpen(true)}>
        <Database className="size-3" />
        Knowledge base
      </Button>
      {kbOpen ? (
        <Dialog open onOpenChange={setKbOpen}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Select knowledge base</DialogTitle>
            </DialogHeader>
            <KbProjectPicker
              onPick={(project) => {
                onAdd({
                  node: { type: "kb", data: { projectId: project.id, projectName: project.name } },
                  x: 0,
                  y: 0,
                });
                setKbOpen(false);
              }}
            />
          </DialogContent>
        </Dialog>
      ) : null}
    </div>
  );
}
