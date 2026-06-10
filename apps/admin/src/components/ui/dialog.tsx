import {
  cloneElement,
  createContext,
  isValidElement,
  useContext,
  useMemo,
  useState,
  type HTMLAttributes,
  type ReactElement,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

import { cn } from "@/lib/utils";

type DialogContextValue = {
  open: boolean;
  setOpen: (open: boolean) => void;
};

const DialogContext = createContext<DialogContextValue | null>(null);

export function Dialog({
  children,
  open,
  defaultOpen,
  onOpenChange,
}: {
  children: ReactNode;
  open?: boolean;
  defaultOpen?: boolean;
  onOpenChange?: (open: boolean) => void;
}) {
  const [internalOpen, setInternalOpen] = useState(defaultOpen ?? false);
  const currentOpen = open ?? internalOpen;
  const contextValue = useMemo(
    () => ({
      open: currentOpen,
      setOpen: (nextOpen: boolean) => {
        if (open === undefined) {
          setInternalOpen(nextOpen);
        }
        onOpenChange?.(nextOpen);
      },
    }),
    [currentOpen, onOpenChange, open],
  );

  return <DialogContext.Provider value={contextValue}>{children}</DialogContext.Provider>;
}

export function DialogTrigger({
  children,
}: {
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const context = useDialogContext();
  if (!isValidElement(children)) {
    return null;
  }
  return cloneElement(children, {
    onClick: () => context.setOpen(true),
  });
}

export function DialogContent({ className, children, ...props }: HTMLAttributes<HTMLDivElement>) {
  const context = useDialogContext();
  if (!context.open) {
    return null;
  }

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/30 p-4 backdrop-blur-sm">
      <div
        className={cn("w-full max-w-lg rounded-2xl border border-border bg-card text-card-foreground shadow-2xl", className)}
        {...props}
      >
        {children}
      </div>
    </div>,
    document.body,
  );
}

export function DialogHeader({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("grid gap-1.5 p-6", className)} {...props} />;
}

export function DialogTitle({ className, ...props }: HTMLAttributes<HTMLHeadingElement>) {
  return <h2 className={cn("text-lg font-semibold", className)} {...props} />;
}

export function DialogDescription({ className, ...props }: HTMLAttributes<HTMLParagraphElement>) {
  return <p className={cn("text-sm text-muted-foreground", className)} {...props} />;
}

export function DialogFooter({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("flex items-center justify-end gap-3 p-6 pt-0", className)} {...props} />;
}

export function DialogClose({
  children,
}: {
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const context = useDialogContext();
  if (!isValidElement(children)) {
    return null;
  }
  return cloneElement(children, {
    onClick: () => context.setOpen(false),
  });
}

function useDialogContext() {
  const context = useContext(DialogContext);
  if (!context) {
    throw new Error("Dialog components must be used within Dialog");
  }
  return context;
}
