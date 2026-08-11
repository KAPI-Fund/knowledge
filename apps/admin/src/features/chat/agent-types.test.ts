import { describe, expect, it } from "vitest";

import type { AgentUserInputRequest } from "./agent-types";
import { isShellApprovalRequest, parseAgentEvents, shellApprovalCommand } from "./agent-types";

function approvalRequest(overrides: Partial<AgentUserInputRequest> = {}): AgentUserInputRequest {
  return {
    requestId: "shell-approval:123e4567-e89b-12d3-a456-426614174000",
    title: "Approve shell command",
    description: "python make.py",
    fields: [
      {
        id: "approve",
        type: "confirm",
        label: "Allow this command to run in the sandbox",
        description: "python make.py",
      },
    ],
    ...overrides,
  };
}

describe("shell approval helpers", () => {
  it("detects shell approval requests by requestId prefix", () => {
    expect(isShellApprovalRequest(approvalRequest())).toBe(true);
    expect(isShellApprovalRequest(approvalRequest({ requestId: "req-1" }))).toBe(false);
  });

  it("extracts the command from the request description", () => {
    expect(shellApprovalCommand(approvalRequest())).toBe("python make.py");
  });

  it("falls back to the confirm field description", () => {
    expect(shellApprovalCommand(approvalRequest({ description: null }))).toBe("python make.py");
  });

  it("returns null for regular user.ask requests or empty commands", () => {
    expect(shellApprovalCommand(approvalRequest({ requestId: "req-1" }))).toBeNull();
    expect(
      shellApprovalCommand(
        approvalRequest({
          description: "  ",
          fields: [{ id: "approve", type: "confirm", label: "Allow" }],
        }),
      ),
    ).toBeNull();
  });
});

describe("parseAgentEvents", () => {
  it("keeps only objects with a string type", () => {
    const events = parseAgentEvents([
      { type: "toolStart", tool: "shell.exec", input: "python make.py" },
      { notAnEvent: true },
      "junk",
      null,
    ]);
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({ type: "toolStart", tool: "shell.exec" });
  });

  it("returns empty for non-arrays", () => {
    expect(parseAgentEvents(null)).toEqual([]);
    expect(parseAgentEvents({})).toEqual([]);
  });
});
