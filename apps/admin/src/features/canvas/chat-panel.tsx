import { useMemo, useState } from "react";

import { MarkdownMessage } from "@/components/shared/markdown-message";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

import { useProjectsQuery } from "../projects/queries";
import { streamCanvasChat } from "./stream";

export const CANVAS_SKILLS = [
  { name: "/search", description: "Web search -> result node" },
  { name: "/image", description: "Text to image -> image node" },
  { name: "/analyze", description: "Analyze selected/connected nodes" },
  { name: "/kb", description: "Add a knowledge-base node" },
] as const;

interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
}

export interface SkillNodePayload {
  node: unknown;
  x: number;
  y: number;
}

interface ChatPanelProps {
  canvasId: string;
  selectedNodeIds: string[];
  onSkillNode: (payload: SkillNodePayload) => void;
}

export function ChatPanel({ canvasId, selectedNodeIds, onSkillNode }: ChatPanelProps) {
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [kbOpen, setKbOpen] = useState(false);

  const skillMatches = useMemo(() => {
    if (!input.startsWith("/")) {
      return [];
    }
    return CANVAS_SKILLS.filter((skill) => skill.name.startsWith(input.split(" ")[0]));
  }, [input]);

  const appendAssistantDelta = (id: string, text: string) => {
    setMessages((prev) =>
      prev.map((message) =>
        message.id === id ? { ...message, content: message.content + text } : message,
      ),
    );
  };

  const submit = async () => {
    const text = input.trim();
    if (!text || streaming || !canvasId) {
      return;
    }
    if (text === "/kb" || text.startsWith("/kb ")) {
      setKbOpen(true);
      setInput("");
      return;
    }

    const userMessage: ChatMessage = { id: crypto.randomUUID(), role: "user", content: text };
    const assistantId = crypto.randomUUID();
    setMessages((prev) => [...prev, userMessage, { id: assistantId, role: "assistant", content: "" }]);
    setInput("");
    setStreaming(true);
    try {
      await streamCanvasChat(canvasId, text, selectedNodeIds, {
        onDelta: (delta) => appendAssistantDelta(assistantId, delta),
        onNode: (payload) => {
          onSkillNode(payload);
          setMessages((prev) =>
            prev.map((message) =>
              message.id === assistantId
                ? { ...message, content: "已在画布中添加节点。" }
                : message,
            ),
          );
        },
        onDone: () => setStreaming(false),
        onError: (message) => {
          appendAssistantDelta(assistantId, `\n\n> 出错了：${message}`);
          setStreaming(false);
        },
      });
    } finally {
      setStreaming(false);
    }
  };

  return (
    <section className="flex h-full w-80 shrink-0 flex-col border-l bg-card">
      <div className="border-b p-3 text-sm font-semibold">对话</div>
      <ScrollArea className="min-h-0 flex-1 space-y-3 p-3">
        {messages.length === 0 ? (
          <p className="text-xs text-muted-foreground">输入问题，或以 “/” 触发技能。</p>
        ) : (
          messages.map((message) => (
            <div
              key={message.id}
              className={cn(
                "rounded-md px-3 py-2 text-sm",
                message.role === "user" ? "bg-primary/10" : "bg-muted",
              )}
            >
              <MarkdownMessage content={message.content || "…"} />
            </div>
          ))
        )}
      </ScrollArea>

      <div className="relative border-t p-3">
        {skillMatches.length > 0 ? (
          <ul className="absolute bottom-full left-3 right-3 mb-1 overflow-hidden rounded-md border bg-popover shadow-md">
            {skillMatches.map((skill) => (
              <li key={skill.name}>
                <button
                  type="button"
                  onClick={() => setInput(`${skill.name} `)}
                  className="flex w-full flex-col items-start px-3 py-1.5 text-left hover:bg-accent"
                >
                  <span className="font-mono text-sm">{skill.name}</span>
                  <span className="text-xs text-muted-foreground">{skill.description}</span>
                </button>
              </li>
            ))}
          </ul>
        ) : null}
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          <Input
            value={input}
            onChange={(event) => setInput(event.target.value)}
            placeholder="/ 或提问"
            disabled={streaming}
          />
        </form>
      </div>

      {kbOpen ? (
        <Dialog open onOpenChange={setKbOpen}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>选择知识库</DialogTitle>
            </DialogHeader>
            <KbProjectPicker
              onPick={(project) => {
                onSkillNode({
                  node: { type: "kb", data: { projectId: project.id, projectName: project.name } },
                  x: 80,
                  y: 80,
                });
                setKbOpen(false);
              }}
            />
          </DialogContent>
        </Dialog>
      ) : null}
    </section>
  );
}

interface KbProjectPickerProps {
  onPick: (project: { id: string; name: string }) => void;
}

function KbProjectPicker({ onPick }: KbProjectPickerProps) {
  const projects = useProjectsQuery();
  const items = projects.data ?? [];

  if (projects.isLoading) {
    return <p className="text-sm text-muted-foreground">加载项目中…</p>;
  }
  if (items.length === 0) {
    return <p className="text-sm text-muted-foreground">没有可用的知识库。</p>;
  }
  return (
    <div className="space-y-2">
      <ul className="max-h-64 space-y-1 overflow-auto">
        {items.map((project) => (
          <li key={project.id}>
            <Button
              type="button"
              variant="ghost"
              className="w-full justify-start"
              onClick={() => onPick(project)}
            >
              {project.name}
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}
