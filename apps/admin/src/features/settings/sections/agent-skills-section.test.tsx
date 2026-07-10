import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { AgentSkillsSection } from "./agent-skills-section";

const listProjects = vi.fn();
const listAgentSkills = vi.fn();
const getAgentSkill = vi.fn();

vi.mock("../../shared/api", () => ({
  listProjects: () => listProjects(),
  listAgentSkills: (projectId: string) => listAgentSkills(projectId),
  getAgentSkill: (input: { projectId: string; skillId: string }) => getAgentSkill(input),
}));

function renderSection() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <AgentSkillsSection />
    </QueryClientProvider>,
  );
}

describe("AgentSkillsSection", () => {
  it("lists skills for the selected project with source badges", async () => {
    const user = userEvent.setup();
    listProjects.mockResolvedValue([
      { id: "p1", name: "Docs", rootPath: "/data/p1", createdAt: "2026-01-01T00:00:00Z" },
    ]);
    listAgentSkills.mockResolvedValue([
      { id: "reviewer", name: "reviewer", description: "Review source quality", source: "project" },
      { id: "writer", name: "writer", description: "Write wiki pages", source: "global" },
    ]);
    renderSection();

    await user.click(await screen.findByRole("combobox", { name: /Project/i }));
    await user.click(await screen.findByRole("option", { name: /Docs/i }));

    expect(await screen.findByText("Review source quality")).toBeInTheDocument();
    expect(screen.getByText("project")).toBeInTheDocument();
    expect(screen.getByText("global")).toBeInTheDocument();
    expect(listAgentSkills).toHaveBeenCalledWith("p1");
  });

  it("opens the skill dialog with instructions", async () => {
    const user = userEvent.setup();
    listProjects.mockResolvedValue([
      { id: "p1", name: "Docs", rootPath: "/data/p1", createdAt: "2026-01-01T00:00:00Z" },
    ]);
    listAgentSkills.mockResolvedValue([
      { id: "reviewer", name: "reviewer", description: "Review source quality", source: "project" },
    ]);
    getAgentSkill.mockResolvedValue({
      id: "reviewer",
      name: "reviewer",
      description: "Review source quality",
      instructions: "Check claims carefully.",
      source: "project",
    });
    renderSection();

    await user.click(await screen.findByRole("combobox", { name: /Project/i }));
    await user.click(await screen.findByRole("option", { name: /Docs/i }));
    await user.click(await screen.findByRole("button", { name: /View/i }));

    expect(await screen.findByText("Check claims carefully.")).toBeInTheDocument();
    expect(getAgentSkill).toHaveBeenCalledWith({ projectId: "p1", skillId: "reviewer" });
  });

  it("shows an empty state when the project has no skills", async () => {
    const user = userEvent.setup();
    listProjects.mockResolvedValue([
      { id: "p1", name: "Docs", rootPath: "/data/p1", createdAt: "2026-01-01T00:00:00Z" },
    ]);
    listAgentSkills.mockResolvedValue([]);
    renderSection();

    await user.click(await screen.findByRole("combobox", { name: /Project/i }));
    await user.click(await screen.findByRole("option", { name: /Docs/i }));

    expect(await screen.findByText(/No skills found/i)).toBeInTheDocument();
  });
});
