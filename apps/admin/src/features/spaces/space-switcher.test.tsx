import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const navigateMock = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>(
    "react-router-dom",
  );
  return { ...actual, useNavigate: () => navigateMock };
});

vi.mock("./use-spaces", () => ({
  useSpacesQuery: () => ({
    data: {
      personal: { spaceId: "personal-space" },
      orgs: [
        { id: "org-1", slug: "acme", name: "Acme", spaceId: "s1", role: "org_admin" },
        { id: "org-2", slug: "globex", name: "Globex", spaceId: "s2", role: "org_member" },
      ],
      teams: [],
    },
    isLoading: false,
  }),
}));

import { SpaceSwitcher } from "./space-switcher";

function renderSwitcher() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <SpaceSwitcher />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("SpaceSwitcher", () => {
  it("lists Personal and each org", () => {
    renderSwitcher();
    expect(screen.getByText("Personal")).toBeInTheDocument();
    expect(screen.getByText("Acme")).toBeInTheDocument();
    expect(screen.getByText("Globex")).toBeInTheDocument();
  });

  it("navigates to /projects when Personal is chosen", () => {
    renderSwitcher();
    fireEvent.click(screen.getByText("Personal"));
    expect(navigateMock).toHaveBeenCalledWith("/projects");
  });

  it("navigates to the org workspace when an org is chosen", () => {
    renderSwitcher();
    fireEvent.click(screen.getByText("Acme"));
    expect(navigateMock).toHaveBeenCalledWith("/orgs/org-1");
  });

  it("opens the New org dialog", () => {
    renderSwitcher();
    fireEvent.click(screen.getByText("New org"));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
