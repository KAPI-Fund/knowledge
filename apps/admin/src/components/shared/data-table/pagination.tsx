import type { Table } from "@tanstack/react-table";
import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

export function DataTablePagination<TData>({ table }: { table: Table<TData> }) {
  const pageCount = table.getPageCount();
  const rowCount = table.getFilteredRowModel().rows.length;

  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <span className="text-xs text-muted-foreground">
        {rowCount} row{rowCount === 1 ? "" : "s"}
      </span>
      <div className="flex items-center gap-4 lg:gap-6">
        <div className="hidden items-center gap-2 sm:flex">
          <span className="text-xs text-muted-foreground">Rows per page</span>
          <Select
            value={String(table.getState().pagination.pageSize)}
            onValueChange={(value) => table.setPageSize(Number(value))}
          >
            <SelectTrigger size="sm" className="h-8 w-[4.5rem]" aria-label="Rows per page">
              <SelectValue />
            </SelectTrigger>
            <SelectContent side="top">
              {[10, 20, 30, 50, 100].map((size) => (
                <SelectItem key={size} value={String(size)}>
                  {size}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <span className="text-xs text-muted-foreground">
          Page {pageCount === 0 ? 0 : table.getState().pagination.pageIndex + 1} of {pageCount}
        </span>
        <div className="flex items-center gap-1">
          <Button
            aria-label="First page"
            className="hidden size-8 lg:flex"
            disabled={!table.getCanPreviousPage()}
            onClick={() => table.setPageIndex(0)}
            size="icon"
            variant="outline"
          >
            <ChevronsLeft />
          </Button>
          <Button
            aria-label="Previous page"
            className="size-8"
            disabled={!table.getCanPreviousPage()}
            onClick={() => table.previousPage()}
            size="icon"
            variant="outline"
          >
            <ChevronLeft />
          </Button>
          <Button
            aria-label="Next page"
            className="size-8"
            disabled={!table.getCanNextPage()}
            onClick={() => table.nextPage()}
            size="icon"
            variant="outline"
          >
            <ChevronRight />
          </Button>
          <Button
            aria-label="Last page"
            className="hidden size-8 lg:flex"
            disabled={!table.getCanNextPage()}
            onClick={() => table.setPageIndex(pageCount - 1)}
            size="icon"
            variant="outline"
          >
            <ChevronsRight />
          </Button>
        </div>
      </div>
    </div>
  );
}
