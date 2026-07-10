import { MessagesSquare, Pencil, Plus, SendHorizontal, Trash2 } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useParams } from "react-router-dom";
import { toast } from "sonner";

import { EmptyState } from "@/components/layout/empty-state";
import { ConfirmDialog } from "@/components/shared/confirm-dialog";
import { MarkdownMessage } from "@/components/shared/markdown-message";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

import {
  conversationKeys,
  useConversationMessagesQuery,
  useConversationsQuery,
  useCreateConversationMutation,
  useDeleteConversationMutation,
  useRenameConversationMutation,
} from "./queries";
import { streamChatMessage } from "./stream";

export function ChatPage() {
  const { projectId = "" } = useParams();
  const queryClient = useQueryClient();
  const conversations = useConversationsQuery(projectId);
  const [activeId, setActiveId] = useState<string | null>(null);
  const messages = useConversationMessagesQuery(projectId, activeId);
  const createMutation = useCreateConversationMutation(projectId);
  const renameMutation = useRenameConversationMutation(projectId);
  const deleteMutation = useDeleteConversationMutation(projectId);

  const [draft, setDraft] = useState("");
  const [streamingText, setStreamingText] = useState<string | null>(null);
  const [pendingUserText, setPendingUserText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [renameTarget, setRenameTarget] = useState<{ id: string; title: string } | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; title: string } | null>(null);

  const isStreaming = streamingText !== null;

  const scrollRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  // Follow the bottom as content streams in, but stop hijacking the scroll once
  // the reader has scrolled up to revisit earlier messages.
  const stickToBottomRef = useRef(true);

  function pinToBottom() {
    const el = scrollRef.current;
    if (el && stickToBottomRef.current) {
      el.scrollTop = el.scrollHeight;
    }
  }

  function handleScroll() {
    const el = scrollRef.current;
    if (!el) {
      return;
    }
    stickToBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
  }

  useEffect(() => {
    stickToBottomRef.current = true;
  }, [activeId]);

  useLayoutEffect(pinToBottom, [messages.data, streamingText, pendingUserText]);

  // Diagrams, math, and images change the transcript height asynchronously,
  // after the text effect above has already run. Re-pin on any height change so
  // a late-rendering Mermaid diagram cannot leave the view scrolled mid-message.
  useEffect(() => {
    const content = contentRef.current;
    if (!content || typeof ResizeObserver === "undefined") {
      return;
    }
    const observer = new ResizeObserver(pinToBottom);
    observer.observe(content);
    return () => observer.disconnect();
  }, []);

  async function handleSend() {
    const content = draft.trim();
    if (!content || isStreaming) {
      return;
    }

    let conversationId = activeId;
    if (!conversationId) {
      const created = await createMutation.mutateAsync(content.slice(0, 60));
      conversationId = created.id;
      setActiveId(conversationId);
    }

    setDraft("");
    setError(null);
    setPendingUserText(content);
    setStreamingText("");
    stickToBottomRef.current = true;

    await streamChatMessage(
      { projectId, conversationId, content },
      {
        onDelta: (text) => {
          setStreamingText((current) => (current ?? "") + text);
        },
        onDone: (payload) => {
          const now = new Date().toISOString();
          // Seed the finished turn into the cache in the same commit that clears
          // the streaming bubble. Without this, clearing the local state before
          // the background refetch lands leaves a frame with neither the
          // streaming bubble nor the persisted message, which flashed at the
          // instant markdown laid out. The assistant id matches the server's, so
          // the later refetch reconciles silently without a remount.
          queryClient.setQueryData(
            conversationKeys.messages(projectId, conversationId),
            (old: typeof messages.data) => [
              ...(old ?? []),
              {
                id: `local-user-${now}`,
                role: "user",
                content,
                contextSummary: null,
                createdAt: now,
              },
              {
                id: payload.messageId,
                role: "assistant",
                content: payload.content,
                contextSummary: payload.contextSummary,
                createdAt: now,
              },
            ],
          );
          setStreamingText(null);
          setPendingUserText(null);
          void queryClient.invalidateQueries({
            queryKey: conversationKeys.messages(projectId, conversationId),
          });
          void queryClient.invalidateQueries({ queryKey: conversationKeys.list(projectId) });
        },
        onError: (message) => {
          setStreamingText(null);
          setPendingUserText(null);
          setError(message);
          // The user turn may already be persisted server-side; refetch so it
          // does not vanish from the transcript.
          void queryClient.invalidateQueries({
            queryKey: conversationKeys.messages(projectId, conversationId),
          });
          void queryClient.invalidateQueries({ queryKey: conversationKeys.list(projectId) });
        },
      },
    );
  }

  function openRename(conversationId: string, currentTitle: string) {
    setRenameTarget({ id: conversationId, title: currentTitle });
    setRenameDraft(currentTitle);
  }

  function submitRename() {
    if (!renameTarget || !renameDraft.trim()) {
      return;
    }
    renameMutation.mutate(
      { conversationId: renameTarget.id, title: renameDraft.trim() },
      {
        onError: (mutationError) =>
          toast.error(
            mutationError instanceof Error ? mutationError.message : "Failed to rename conversation",
          ),
      },
    );
    setRenameTarget(null);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-6">
      <PageHeader
        description="Chat with the project wiki using the configured provider; responses stream from retrieved context."
        title="Chat"
      />

      <div className="grid min-h-0 flex-1 grid-rows-[auto_1fr] gap-4 md:grid-cols-[16rem_1fr] md:grid-rows-1">
        <aside className="flex min-h-0 flex-col rounded-lg border border-border bg-card p-3">
          <Button
            className="mb-3 w-full"
            onClick={() => {
              createMutation.mutate(undefined, {
                onSuccess: (created) => setActiveId(created.id),
              });
            }}
            variant="outline"
          >
            <Plus />
            New conversation
          </Button>
          <TooltipProvider delayDuration={300}>
            {(conversations.data ?? []).length === 0 ? (
              <p className="px-2 py-4 text-center text-sm text-muted-foreground">
                No conversations yet.
              </p>
            ) : null}
            <ul className="min-h-0 flex-1 space-y-0.5 overflow-y-auto">
              {(conversations.data ?? []).map((conversation) => (
                <li className="group/conv relative" key={conversation.id}>
                  <Button
                    className="w-full justify-start truncate pr-14"
                    onClick={() => setActiveId(conversation.id)}
                    size="sm"
                    variant={conversation.id === activeId ? "secondary" : "ghost"}
                  >
                    {conversation.title}
                  </Button>
                  <div className="absolute inset-y-0 right-1 flex items-center gap-0.5 opacity-0 transition-opacity group-focus-within/conv:opacity-100 group-hover/conv:opacity-100">
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          aria-label={`Rename ${conversation.title}`}
                          className="text-muted-foreground"
                          onClick={() => openRename(conversation.id, conversation.title)}
                          size="icon-xs"
                          variant="ghost"
                        >
                          <Pencil />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>Rename</TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          aria-label={`Delete ${conversation.title}`}
                          className="text-muted-foreground hover:text-destructive"
                          onClick={() => setDeleteTarget({ id: conversation.id, title: conversation.title })}
                          size="icon-xs"
                          variant="ghost"
                        >
                          <Trash2 />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>Delete</TooltipContent>
                    </Tooltip>
                  </div>
                </li>
              ))}
            </ul>
          </TooltipProvider>
        </aside>

        <section className="flex min-h-0 min-w-0 flex-col rounded-lg border border-border bg-card">
          <div
            aria-live="polite"
            className="min-h-0 flex-1 overflow-y-auto"
            onScroll={handleScroll}
            ref={scrollRef}
          >
            <div className="space-y-3 p-4" ref={contentRef}>
            {(messages.data ?? []).length === 0 && pendingUserText === null && streamingText === null ? (
              <EmptyState
                description={
                  activeId
                    ? "Send a message to start this conversation."
                    : "Pick a conversation or just start typing below."
                }
                icon={MessagesSquare}
                title={activeId ? "No messages yet" : "Ask the wiki"}
              />
            ) : null}
            {(messages.data ?? []).map((message) => {
              const isUser = message.role === "user";
              return (
                <div
                  className={`max-w-prose rounded-md border px-3 py-2 text-sm ${
                    isUser
                      ? "ml-auto whitespace-pre-wrap border-primary/30 bg-accent"
                      : "border-border bg-muted"
                  }`}
                  data-role={message.role}
                  key={message.id}
                >
                  {isUser ? (
                    message.content
                  ) : (
                    <MarkdownMessage content={message.content} id={message.id} />
                  )}
                </div>
              );
            })}
            {pendingUserText !== null && (
              <div
                className="ml-auto max-w-prose whitespace-pre-wrap rounded-md border border-primary/30 bg-accent px-3 py-2 text-sm"
                data-role="user"
              >
                {pendingUserText}
              </div>
            )}
            {streamingText !== null && (
              <div
                className="max-w-prose rounded-md border border-border bg-muted px-3 py-2 text-sm"
                data-role="assistant"
                data-streaming="true"
              >
                {streamingText ? (
                  <MarkdownMessage content={streamingText} id="streaming" />
                ) : (
                  <span className="inline-flex gap-1 text-muted-foreground">
                    <span className="size-1.5 animate-pulse rounded-full bg-current" />
                    <span className="size-1.5 animate-pulse rounded-full bg-current [animation-delay:150ms]" />
                    <span className="size-1.5 animate-pulse rounded-full bg-current [animation-delay:300ms]" />
                  </span>
                )}
              </div>
            )}
            {error && <p className="text-sm text-destructive">{error}</p>}
            </div>
          </div>
          <form
            className="flex gap-2 border-t border-border p-3"
            onSubmit={(event) => {
              event.preventDefault();
              void handleSend();
            }}
          >
            <Textarea
              aria-label="Chat message"
              className="min-h-10 flex-1 resize-y"
              onChange={(event) => setDraft(event.target.value)}
              placeholder="Ask the wiki..."
              value={draft}
            />
            <Button disabled={isStreaming || !draft.trim()} type="submit">
              <SendHorizontal />
              Send
            </Button>
          </form>
        </section>
      </div>

      <Dialog
        onOpenChange={(open) => {
          if (!open) setRenameTarget(null);
        }}
        open={Boolean(renameTarget)}
      >
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Rename conversation</DialogTitle>
          </DialogHeader>
          <Input
            aria-label="Conversation title"
            onChange={(event) => setRenameDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") submitRename();
            }}
            value={renameDraft}
          />
          <DialogFooter>
            <Button onClick={() => setRenameTarget(null)} type="button" variant="outline">
              Cancel
            </Button>
            <Button
              disabled={!renameDraft.trim() || renameMutation.isPending}
              onClick={submitRename}
              type="button"
            >
              Save
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        confirmLabel="Delete conversation"
        description={
          deleteTarget ? `"${deleteTarget.title}" and its messages will be removed.` : ""
        }
        destructive
        isPending={deleteMutation.isPending}
        onConfirm={async () => {
          if (!deleteTarget) return;
          try {
            await deleteMutation.mutateAsync(deleteTarget.id);
            if (activeId === deleteTarget.id) {
              setActiveId(null);
            }
            toast.success("Conversation deleted.");
          } catch (mutationError) {
            toast.error(
              mutationError instanceof Error
                ? mutationError.message
                : "Failed to delete conversation",
            );
          }
        }}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
        open={Boolean(deleteTarget)}
        title="Delete this conversation?"
      />
    </div>
  );
}
