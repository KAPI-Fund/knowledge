import type { ColumnDef } from "@tanstack/react-table";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DataTable } from "./data-table";

type Row = { id: string; name: string };

const columns: ColumnDef<Row>[] = [{ accessorKey: "name", header: "Name" }];

describe("DataTable", () => {
  it("renders rows from data", () => {
    render(
      <DataTable
        columns={columns}
        data={[
          { id: "1", name: "alpha" },
          { id: "2", name: "beta" },
        ]}
      />,
    );
    expect(screen.getByText("alpha")).toBeInTheDocument();
    expect(screen.getByText("beta")).toBeInTheDocument();
  });

  it("shows the empty message when there is no data", () => {
    render(<DataTable columns={columns} data={[]} emptyMessage="No tasks." />);
    expect(screen.getByText("No tasks.")).toBeInTheDocument();
  });

  it("renders skeleton placeholders while loading", () => {
    const { container } = render(<DataTable columns={columns} data={[]} isLoading />);
    expect(container.querySelectorAll('[data-slot="skeleton"]').length).toBeGreaterThan(0);
  });
});
