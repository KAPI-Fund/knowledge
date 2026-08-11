import {
  type ColumnDef,
  type ColumnFiltersState,
  type SortingState,
  flexRender,
  getCoreRowModel,
  getFacetedRowModel,
  getFacetedUniqueValues,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { ArrowDown, ArrowUp, ChevronsUpDown } from "lucide-react";
import { useState, type ReactNode } from "react";

import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";

import { DataTablePagination } from "./pagination";
import { DataTableToolbar, type FacetedFilterConfig } from "./toolbar";

export { DataTableColumnHeader } from "./column-header";
export { DataTableFacetedFilter, type FacetedFilterOption } from "./faceted-filter";
export { DataTablePagination } from "./pagination";
export { DataTableRowActions } from "./row-actions";
export { DataTableToolbar, type FacetedFilterConfig } from "./toolbar";

type DataTableProps<TData, TValue> = {
  columns: ColumnDef<TData, TValue>[];
  data: TData[];
  emptyMessage?: string;
  isLoading?: boolean;
  pageSize?: number;
  // When true the table fills its parent's height and the rows scroll inside a
  // bounded box (sticky header, pagination pinned to the bottom) instead of
  // growing the page. Use inside a height-constrained flex/grid cell.
  fillHeight?: boolean;
  searchKey?: string;
  searchPlaceholder?: string;
  facetedFilters?: FacetedFilterConfig[];
  toolbar?: ReactNode;
  onRowClick?: (row: TData) => void;
};

export function DataTable<TData, TValue>({
  columns,
  data,
  emptyMessage = "No results.",
  isLoading = false,
  pageSize = 10,
  fillHeight = false,
  searchKey,
  searchPlaceholder,
  facetedFilters,
  toolbar,
  onRowClick,
}: DataTableProps<TData, TValue>) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([]);

  const table = useReactTable({
    columns,
    data,
    state: { sorting, columnFilters },
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getFacetedRowModel: getFacetedRowModel(),
    getFacetedUniqueValues: getFacetedUniqueValues(),
    getPaginationRowModel: getPaginationRowModel(),
    initialState: { pagination: { pageSize } },
  });

  const rows = table.getRowModel().rows;
  const hasToolbar = Boolean(searchKey || facetedFilters?.length || toolbar);

  return (
    <div className={cn("grid gap-3", fillHeight && "flex h-full min-h-0 flex-col")}>
      {hasToolbar ? (
        <DataTableToolbar
          facetedFilters={facetedFilters}
          searchKey={searchKey}
          searchPlaceholder={searchPlaceholder}
          table={table}
        >
          {toolbar}
        </DataTableToolbar>
      ) : null}
      <div
        className={cn(
          "rounded-md border border-border",
          fillHeight ? "min-h-0 flex-1 overflow-auto" : "overflow-hidden",
        )}
      >
        <Table>
          <TableHeader className={cn(fillHeight && "sticky top-0 z-10")}>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow className="bg-muted hover:bg-muted" key={headerGroup.id}>
                {headerGroup.headers.map((header) => {
                  const canSort = header.column.getCanSort();
                  const sorted = header.column.getIsSorted();
                  return (
                    <TableHead
                      className="text-[11px] uppercase tracking-[0.04em] text-muted-foreground"
                      key={header.id}
                    >
                      {header.isPlaceholder ? null : canSort ? (
                        <button
                          className="inline-flex items-center gap-1 hover:text-foreground"
                          onClick={header.column.getToggleSortingHandler()}
                          type="button"
                        >
                          {flexRender(header.column.columnDef.header, header.getContext())}
                          {sorted === "asc" ? (
                            <ArrowUp className="size-3" />
                          ) : sorted === "desc" ? (
                            <ArrowDown className="size-3" />
                          ) : (
                            <ChevronsUpDown className="size-3 opacity-50" />
                          )}
                        </button>
                      ) : (
                        flexRender(header.column.columnDef.header, header.getContext())
                      )}
                    </TableHead>
                  );
                })}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {isLoading ? (
              Array.from({ length: 5 }).map((_, rowIndex) => (
                <TableRow key={`skeleton-${rowIndex}`}>
                  {columns.map((_column, colIndex) => (
                    <TableCell key={`skeleton-${rowIndex}-${colIndex}`}>
                      <Skeleton className="h-4 w-full" />
                    </TableCell>
                  ))}
                </TableRow>
              ))
            ) : rows.length ? (
              rows.map((row) => (
                <TableRow
                  className={cn(onRowClick && "cursor-pointer")}
                  key={row.id}
                  onClick={onRowClick ? () => onRowClick(row.original) : undefined}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id}>
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell
                  className="h-24 text-center text-sm text-muted-foreground"
                  colSpan={columns.length}
                >
                  {emptyMessage}
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <DataTablePagination table={table} />
    </div>
  );
}
