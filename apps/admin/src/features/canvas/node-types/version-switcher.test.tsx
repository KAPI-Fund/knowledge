import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { VersionSwitcher } from "./version-switcher";

describe("VersionSwitcher", () => {
  it("shows position and navigates between versions", () => {
    const onChange = vi.fn();
    render(
      <VersionSwitcher
        versions={[{ id: "v1" }, { id: "v2" }, { id: "v3" }]}
        activeId="v2"
        onChange={onChange}
      />,
    );
    expect(screen.getByText("2/3")).toBeInTheDocument();
    fireEvent.click(screen.getByLabelText("previous version"));
    expect(onChange).toHaveBeenCalledWith("v1");
    fireEvent.click(screen.getByLabelText("next version"));
    expect(onChange).toHaveBeenCalledWith("v3");
  });

  it("disables prev at first and next at last", () => {
    render(
      <VersionSwitcher versions={[{ id: "v1" }, { id: "v2" }]} activeId="v1" onChange={() => {}} />,
    );
    expect(screen.getByLabelText("previous version")).toBeDisabled();
    expect(screen.getByLabelText("next version")).not.toBeDisabled();
  });
});
