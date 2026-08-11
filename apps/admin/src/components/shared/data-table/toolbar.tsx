import type { Table } from "@tanstack/react-table";
import { X } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

import { DataTableFacetedFilter, type FacetedFilterOption } from "./faceted-filter";

export type FacetedFilterConfig = {
  columnId: string;
  title: string;
  options: FacetedFilterOption[];
};

export function DataTableToolbar<TData>({
  table,
  searchKey,
  searchPlaceholder,
  facetedFilters,
  children,
}: {
  table: Table<TData>;
  searchKey?: string;
  searchPlaceholder?: string;
  facetedFilters?: FacetedFilterConfig[];
  children?: ReactNode;
}) {
  const isFiltered = table.getState().columnFilters.length > 0;

  return (
    <div className="flex flex-wrap items-center gap-2">
      {searchKey ? (
        <Input
          placeholder={searchPlaceholder ?? "Filter..."}
          value={(table.getColumn(searchKey)?.getFilterValue() as string) ?? ""}
          onChange={(event) => table.getColumn(searchKey)?.setFilterValue(event.target.value)}
          className="h-8 w-40 lg:w-64"
        />
      ) : null}
      {facetedFilters?.map((filter) => (
        <DataTableFacetedFilter
          column={table.getColumn(filter.columnId)}
          key={filter.columnId}
          options={filter.options}
          title={filter.title}
        />
      ))}
      {isFiltered ? (
        <Button
          variant="ghost"
          size="sm"
          className="h-8 px-2 lg:px-3"
          onClick={() => table.resetColumnFilters()}
        >
          Reset
          <X />
        </Button>
      ) : null}
      {children ? <div className="ml-auto flex items-center gap-2">{children}</div> : null}
    </div>
  );
}
