import { useEffect, useRef, useState } from "react";

let mermaidReady = false;

/**
 * Renders a Mermaid diagram from its source. Mermaid is large, so it is loaded
 * on demand only when a diagram actually appears. While the source is still
 * streaming (or invalid), the raw text is shown as a fallback instead of an
 * error, so a half-finished diagram never blanks the message.
 */
export function Mermaid({ code }: { code: string }) {
  const [svg, setSvg] = useState<string | null>(null);
  const idRef = useRef(`mermaid-${Math.random().toString(36).slice(2)}`);

  useEffect(() => {
    let cancelled = false;

    async function render() {
      try {
        const mermaid = (await import("mermaid")).default;
        if (!mermaidReady) {
          mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme: "default" });
          mermaidReady = true;
        }
        const result = await mermaid.render(idRef.current, code);
        if (!cancelled) {
          setSvg(result.svg);
        }
      } catch {
        if (!cancelled) {
          setSvg(null);
        }
      }
    }

    void render();
    return () => {
      cancelled = true;
    };
  }, [code]);

  if (svg) {
    return (
      <div
        className="my-3 flex justify-center overflow-x-auto rounded-md border border-border bg-card p-3"
        // eslint-disable-next-line react/no-danger
        dangerouslySetInnerHTML={{ __html: svg }}
      />
    );
  }

  return (
    <pre className="my-3 overflow-x-auto rounded-md border border-border bg-muted p-3 text-xs text-muted-foreground">
      <code>{code}</code>
    </pre>
  );
}
