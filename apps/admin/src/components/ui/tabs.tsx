import {
  Children,
  cloneElement,
  createContext,
  isValidElement,
  useContext,
  useMemo,
  useState,
  type ButtonHTMLAttributes,
  type HTMLAttributes,
  type ReactElement,
  type ReactNode,
} from "react";

import { cn } from "@/lib/utils";

type TabsContextValue = {
  value: string;
  setValue: (value: string) => void;
};

const TabsContext = createContext<TabsContextValue | null>(null);

export function Tabs({
  defaultValue,
  value,
  onValueChange,
  className,
  children,
}: {
  defaultValue?: string;
  value?: string;
  onValueChange?: (value: string) => void;
  className?: string;
  children: ReactNode;
}) {
  const [internalValue, setInternalValue] = useState(defaultValue ?? "");
  const currentValue = value ?? internalValue;
  const contextValue = useMemo(
    () => ({
      value: currentValue,
      setValue: (nextValue: string) => {
        if (value === undefined) {
          setInternalValue(nextValue);
        }
        onValueChange?.(nextValue);
      },
    }),
    [currentValue, onValueChange, value],
  );

  return (
    <TabsContext.Provider value={contextValue}>
      <div className={cn("grid gap-4", className)}>{children}</div>
    </TabsContext.Provider>
  );
}

export function TabsList({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("inline-flex h-11 items-center gap-1 rounded-xl border border-border bg-muted/60 p-1", className)}
      role="tablist"
      {...props}
    />
  );
}

export function TabsTrigger({
  className,
  value,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { value: string }) {
  const context = useTabsContext();
  const active = context.value === value;

  return (
    <button
      aria-selected={active}
      className={cn(
        "inline-flex h-9 items-center justify-center rounded-lg px-3 text-sm font-medium transition-colors",
        active ? "bg-card text-card-foreground shadow-sm" : "text-muted-foreground hover:text-foreground",
        className,
      )}
      role="tab"
      type="button"
      onClick={() => context.setValue(value)}
      {...props}
    />
  );
}

export function TabsContent({
  className,
  value,
  ...props
}: HTMLAttributes<HTMLDivElement> & { value: string }) {
  const context = useTabsContext();
  if (context.value !== value) {
    return null;
  }
  return <div className={cn("outline-none", className)} role="tabpanel" {...props} />;
}

function useTabsContext() {
  const context = useContext(TabsContext);
  if (!context) {
    throw new Error("Tabs components must be used within Tabs");
  }
  return context;
}
