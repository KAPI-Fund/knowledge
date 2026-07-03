import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import { Switch } from "./switch";

function Controlled() {
  const [on, setOn] = useState(false);
  return <Switch aria-label="toggle" checked={on} onCheckedChange={setOn} />;
}

describe("Switch", () => {
  it("renders a switch role", () => {
    render(<Switch aria-label="toggle" />);
    expect(screen.getByRole("switch", { name: "toggle" })).toBeInTheDocument();
  });

  it("toggles checked state on click", async () => {
    const user = userEvent.setup();
    render(<Controlled />);
    const el = screen.getByRole("switch", { name: "toggle" });
    expect(el).toHaveAttribute("data-state", "unchecked");
    await user.click(el);
    expect(el).toHaveAttribute("data-state", "checked");
  });
});
