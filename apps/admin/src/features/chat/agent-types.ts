// Display shapes for the server agent loop. The execution engine lives in
// Rust (`crates/knowledge-server/src/agent`); this file mirrors its event and
// options contracts for the React UI only.

export type AgentMode = "fast" | "standard" | "deep";

export type AgentSkillMode = "auto" | "explicit";

export interface AgentMessageOptions {
  mode: AgentMode;
  skill?: string;
  skillMode?: AgentSkillMode;
  web?: boolean;
  resumeRequestId?: string;
  formResult?: Record<string, unknown>;
  // Session whitelist for shell.exec: commands the user approved in this
  // conversation. Sent on every agent request; the server only runs a shell
  // command when it matches an entry exactly.
  approvedShellCommands?: string[];
}

// userInputRequired requests whose requestId carries this prefix are shell
// command approvals: a single confirm field with the command as description.
export const SHELL_APPROVAL_PREFIX = "shell-approval:";

export function isShellApprovalRequest(request: AgentUserInputRequest): boolean {
  return request.requestId.startsWith(SHELL_APPROVAL_PREFIX);
}

export function shellApprovalCommand(request: AgentUserInputRequest): string | null {
  if (!isShellApprovalRequest(request)) {
    return null;
  }
  const command = request.description ?? request.fields[0]?.description ?? "";
  return command.trim() === "" ? null : command;
}

export interface AgentReference {
  title: string;
  path: string;
  kind: string;
  snippet?: string | null;
  score?: number | null;
}

export interface AgentUserInputOption {
  label: string;
  value: string;
  description?: string | null;
  recommended?: boolean | null;
}

export type AgentUserInputFieldType = "single" | "multi" | "text" | "textarea" | "confirm";

export interface AgentUserInputField {
  id: string;
  type: AgentUserInputFieldType;
  label: string;
  description?: string | null;
  placeholder?: string | null;
  options?: AgentUserInputOption[];
  defaultValue?: unknown;
}

export interface AgentUserInputRequest {
  requestId: string;
  title: string;
  description?: string | null;
  fields: AgentUserInputField[];
}

export type AgentEvent =
  | { type: "agentStart"; sessionId: string }
  | { type: "turnStart"; mode: string }
  | { type: "toolStart"; tool: string; input?: string | null }
  | { type: "toolEnd"; tool: string; output?: string | null }
  | { type: "referenceAdded"; reference: AgentReference }
  | { type: "fileChanged"; path: string; tool: string; existedBefore: boolean }
  | { type: "messageDelta"; text: string }
  | { type: "error"; message: string }
  | { type: "userInputRequired"; request: AgentUserInputRequest }
  | { type: "done"; sessionId: string };

export interface AvailableAgentSkill {
  id: string;
  name: string;
  description: string;
  source: "project" | "global";
}

export function parseAgentEvents(value: unknown): AgentEvent[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter(
    (entry): entry is AgentEvent =>
      typeof entry === "object" &&
      entry !== null &&
      typeof (entry as { type?: unknown }).type === "string",
  );
}
