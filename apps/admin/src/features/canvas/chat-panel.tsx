import { useEffect, useRef, useState } from "react";

import { MarkdownMessage } from "@/components/shared/markdown-message";
import {
  Command,
  CommandEmpty,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { uuid } from "@/lib/uuid";
import type { SkillMetadata } from "../shared/api";

import { streamCanvasChat } from "./stream";
import { useSkillsQuery } from "./use-skills-query";

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
  placementOrigin: { x: number; y: number };
  // Persist the current canvas before an SSE chat. Chat re-reads the canvas
  // from the DB, so an unsaved edit (new node, edge, note content) would be
  // invisible. Resolves false when the save fails, and the chat is aborted.
  onBeforeSend: () => Promise<boolean>;
}

export function ChatPanel({
  canvasId,
  selectedNodeIds,
  onSkillNode,
  placementOrigin,
  onBeforeSend,
}: ChatPanelProps) {
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [menuDismissed, setMenuDismissed] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const skills = useSkillsQuery();
  const hasSelection = selectedNodeIds.length > 0;

  // Typing anything reopens a menu the user had dismissed with Escape.
  useEffect(() => {
    setMenuDismissed(false);
  }, [input]);

  const menuOpen =
    input.startsWith("/") && !input.includes(" ") && !menuDismissed;
  const commandToken = input.slice(1).split(" ")[0].toLowerCase();
  const matches = (skills.data ?? []).filter(
    (skill) =>
      skill.command.toLowerCase().startsWith(commandToken) ||
      skill.name.toLowerCase().includes(commandToken),
  );
  const activeSkill = (skills.data ?? []).find(
    (skill) => `/${skill.command}` === input.trimEnd().split(" ")[0],
  );

  // A skill needing a selection can't be chosen when nothing is selected.
  const isBlocked = (skill: SkillMetadata) => skill.requiresSelection && !hasSelection;

  // Reset the highlight to the top whenever the visible matches change.
  useEffect(() => {
    setHighlightedIndex(0);
  }, [commandToken, menuOpen]);

  const chooseSkill = (command: string) => {
    setInput(`/${command} `);
    setMenuDismissed(true);
    inputRef.current?.focus();
  };

  const chooseHighlighted = () => {
    const skill = matches[highlightedIndex];
    if (skill && !isBlocked(skill)) {
      chooseSkill(skill.command);
    }
  };

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

    const userMessage: ChatMessage = { id: uuid(), role: "user", content: text };
    const assistantId = uuid();
    setMessages((prev) => [...prev, userMessage, { id: assistantId, role: "assistant", content: "" }]);
    setInput("");
    setStreaming(true);
    const saved = await onBeforeSend();
    if (!saved) {
      setMessages((prev) =>
        prev.map((message) =>
          message.id === assistantId
            ? { ...message, content: "> Error: could not save canvas" }
            : message,
        ),
      );
      setStreaming(false);
      return;
    }
    try {
      await streamCanvasChat(
        canvasId,
        text,
        selectedNodeIds,
        {
          onDelta: (delta) => appendAssistantDelta(assistantId, delta),
          onNode: (payload) => {
            onSkillNode(payload);
            setMessages((prev) =>
              prev.map((message) =>
                message.id === assistantId
                  ? { ...message, content: "Added a node to the canvas." }
                  : message,
              ),
            );
          },
          onDone: () => setStreaming(false),
          onError: (message) => {
            appendAssistantDelta(assistantId, `\n\n> Error: ${message}`);
            setStreaming(false);
          },
        },
        placementOrigin,
      );
    } finally {
      setStreaming(false);
    }
  };

  return (
    <section className="flex h-full w-80 shrink-0 flex-col border-l bg-card">
      <div className="border-b p-3 text-sm font-semibold">Chat</div>
      <ScrollArea className="min-h-0 flex-1 space-y-3 p-3">
        {messages.length === 0 ? (
          <p className="text-xs text-muted-foreground">Ask a question about this canvas.</p>
        ) : (
          messages.map((message) => (
            <div
              key={message.id}
              className={cn(
                "rounded-md px-3 py-2 text-sm",
                message.role === "user" ? "bg-primary/10" : "bg-muted",
              )}
            >
              <MarkdownMessage content={message.content || "..."} />
            </div>
          ))
        )}
      </ScrollArea>

      <div className="border-t p-3">
        <form
          className="relative"
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          {menuOpen ? (
            <div className="absolute bottom-full left-0 right-0 mb-2 overflow-hidden rounded-md border border-border bg-popover shadow-md">
              <Command
                shouldFilter={false}
                value={matches[highlightedIndex]?.command ?? ""}
                onValueChange={(value) => {
                  const index = matches.findIndex((skill) => skill.command === value);
                  if (index >= 0) {
                    setHighlightedIndex(index);
                  }
                }}
              >
                <CommandList>
                  <CommandEmpty>无匹配技能</CommandEmpty>
                  {matches.map((skill) => {
                    const blocked = isBlocked(skill);
                    return (
                      <CommandItem
                        key={skill.command}
                        value={skill.command}
                        disabled={blocked}
                        onSelect={() => chooseSkill(skill.command)}
                        className="flex flex-col items-start gap-0.5"
                      >
                        <span className="flex w-full items-center gap-2">
                          <span className="font-mono text-xs text-primary">{`/${skill.command}`}</span>
                          <span className="text-sm">{skill.name}</span>
                          {blocked ? (
                            <span className="ml-auto text-[10px] uppercase tracking-wide text-muted-foreground">
                              需选中节点
                            </span>
                          ) : null}
                        </span>
                        <span className="text-xs text-muted-foreground">{skill.description}</span>
                      </CommandItem>
                    );
                  })}
                </CommandList>
              </Command>
            </div>
          ) : null}
          <Input
            ref={inputRef}
            value={input}
            onChange={(event) => setInput(event.target.value)}
            onKeyDown={(event) => {
              if (menuOpen && matches.length > 0) {
                if (event.key === "ArrowDown") {
                  event.preventDefault();
                  setHighlightedIndex((i) => (i + 1) % matches.length);
                  return;
                }
                if (event.key === "ArrowUp") {
                  event.preventDefault();
                  setHighlightedIndex((i) => (i - 1 + matches.length) % matches.length);
                  return;
                }
                if (event.key === "Enter") {
                  // Choose the highlighted command instead of submitting the form.
                  event.preventDefault();
                  chooseHighlighted();
                  return;
                }
              }
              if (event.key === "Escape" && menuOpen) {
                event.preventDefault();
                setMenuDismissed(true);
              }
            }}
            placeholder={activeSkill?.argumentHint ?? "Ask a question"}
            disabled={streaming}
          />
        </form>
      </div>
    </section>
  );
}
