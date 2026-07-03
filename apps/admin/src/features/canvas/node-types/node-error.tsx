import { AlertCircle } from "lucide-react";

interface NodeErrorProps {
  title: string;
  message: string;
}

export function NodeError({ title, message }: NodeErrorProps) {
  return (
    <div className="rounded-md border border-destructive/30 bg-destructive/5 p-2 text-destructive">
      <div className="flex items-center gap-1">
        <AlertCircle className="size-3" />
        <span className="text-[10px] font-semibold uppercase tracking-wider">{title}</span>
      </div>
      <pre className="mt-1 max-h-28 overflow-auto whitespace-pre-wrap break-all font-mono text-[11px] leading-snug">
        {message}
      </pre>
    </div>
  );
}
