import type { Column } from "@tanstack/react-table";
import { Check, PlusCircle } from "lucide-react";
import type { ComponentType } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

export type FacetedFilterOption = {
  label: string;
  value: string;
  icon?: ComponentType<{ className?: string }>;
};

export function DataTableFacetedFilter<TData, TValue>({
  column,
  title,
  options,
}: {
  column: Column<TData, TValue> | undefined;
  title: string;
  options: FacetedFilterOption[];
}) {
  if (!column) return null;
  const facets = column.getFacetedUniqueValues();
  const selected = new Set(column.getFilterValue() as string[] | undefined);

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm" className="h-8 border-dashed">
          <PlusCircle />
          {title}
          {selected.size > 0 ? (
            <>
              <Separator orientation="vertical" className="mx-1 data-[orientation=vertical]:h-4" />
              <Badge variant="secondary" className="rounded-sm px-1 font-normal lg:hidden">
                {selected.size}
              </Badge>
              <div className="hidden gap-1 lg:flex">
                {selected.size > 2 ? (
                  <Badge variant="secondary" className="rounded-sm px-1 font-normal">
                    {selected.size} selected
                  </Badge>
                ) : (
                  options
                    .filter((option) => selected.has(option.value))
                    .map((option) => (
                      <Badge
                        variant="secondary"
                        key={option.value}
                        className="rounded-sm px-1 font-normal"
                      >
                        {option.label}
                      </Badge>
                    ))
                )}
              </div>
            </>
          ) : null}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-48 p-0" align="start">
        <Command>
          <CommandInput placeholder={title} />
          <CommandList>
            <CommandEmpty>No results found.</CommandEmpty>
            <CommandGroup>
              {options.map((option) => {
                const isSelected = selected.has(option.value);
                return (
                  <CommandItem
                    key={option.value}
                    onSelect={() => {
                      const next = new Set(selected);
                      if (isSelected) next.delete(option.value);
                      else next.add(option.value);
                      const values = Array.from(next);
                      column.setFilterValue(values.length ? values : undefined);
                    }}
                  >
                    <div
                      className={cn(
                        "flex size-4 items-center justify-center rounded-[4px] border border-input",
                        isSelected
                          ? "border-primary bg-primary text-primary-foreground"
                          : "[&_svg]:invisible",
                      )}
                    >
                      <Check className="size-3.5 text-current" />
                    </div>
                    {option.icon ? <option.icon className="size-4 text-muted-foreground" /> : null}
                    <span>{option.label}</span>
                    {facets.get(option.value) ? (
                      <span className="ml-auto flex size-4 items-center justify-center font-mono text-xs text-muted-foreground">
                        {facets.get(option.value)}
                      </span>
                    ) : null}
                  </CommandItem>
                );
              })}
            </CommandGroup>
            {selected.size > 0 ? (
              <>
                <CommandSeparator />
                <CommandGroup>
                  <CommandItem
                    onSelect={() => column.setFilterValue(undefined)}
                    className="justify-center text-center"
                  >
                    Clear filters
                  </CommandItem>
                </CommandGroup>
              </>
            ) : null}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
