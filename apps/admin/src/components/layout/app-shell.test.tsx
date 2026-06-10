import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { AppShell } from "./app-shell";

describe("AppShell", () => {
  it("renders system navigation and top-level shell regions", () => {
    render(
      <MemoryRouter>
        <AppShell />
      </MemoryRouter>,
    );

    expect(screen.getByRole("navigation", { name: /system/i })).toBeInTheDocument();
    expect(screen.getByRole("banner")).toBeInTheDocument();
  });
});
