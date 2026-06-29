import "katex/dist/katex.min.css";
import "highlight.js/styles/github-dark.css";

import { isValidElement, memo, type ReactNode, useMemo } from "react";
import hljs from "highlight.js/lib/common";
import { marked } from "marked";
import ReactMarkdown, { type Components } from "react-markdown";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";

import { Mermaid } from "./mermaid";

function highlightCode(code: string, language?: string): string {
  if (language && hljs.getLanguage(language)) {
    return hljs.highlight(code, { language, ignoreIllegals: true }).value;
  }
  return hljs.highlightAuto(code).value;
}

function hasClass(value: unknown, token: string): boolean {
  return typeof value === "string" && value.split(/\s+/).includes(token);
}

const components: Components = {
  h1: ({ children }) => <h1 className="mt-4 mb-2 text-lg font-semibold">{children}</h1>,
  h2: ({ children }) => <h2 className="mt-4 mb-2 text-base font-semibold">{children}</h2>,
  h3: ({ children }) => <h3 className="mt-3 mb-1 text-sm font-semibold">{children}</h3>,
  h4: ({ children }) => <h4 className="mt-3 mb-1 text-sm font-semibold">{children}</h4>,
  p: ({ children }) => <p className="my-2">{children}</p>,
  a: ({ children, href }) => (
    <a
      className="font-medium text-primary underline-offset-4 hover:underline"
      href={href}
      rel="noopener noreferrer"
      target="_blank"
    >
      {children}
    </a>
  ),
  ul: ({ className, children }) =>
    hasClass(className, "contains-task-list") ? (
      <ul className="my-2 list-none space-y-1 pl-0">{children}</ul>
    ) : (
      <ul className="my-2 list-disc space-y-1 pl-5">{children}</ul>
    ),
  ol: ({ children, start }) => (
    <ol className="my-2 list-decimal space-y-1 pl-5" start={start}>
      {children}
    </ol>
  ),
  li: ({ className, children }) =>
    hasClass(className, "task-list-item") ? (
      <li className="flex items-start gap-2 [&>input]:mt-1">{children}</li>
    ) : (
      <li className="marker:text-muted-foreground">{children}</li>
    ),
  blockquote: ({ children }) => (
    <blockquote className="my-2 border-l-2 border-border pl-3 text-muted-foreground italic">
      {children}
    </blockquote>
  ),
  hr: () => <hr className="my-4 border-border" />,
  table: ({ children }) => (
    <div className="my-3 overflow-x-auto">
      <table className="w-full border-collapse text-left text-xs">{children}</table>
    </div>
  ),
  th: ({ children }) => (
    <th className="border border-border bg-muted px-2 py-1 font-semibold">{children}</th>
  ),
  td: ({ children }) => <td className="border border-border px-2 py-1 align-top">{children}</td>,
  img: ({ src, alt }) => (
    <img
      alt={alt ?? ""}
      className="my-2 max-h-[28rem] max-w-full rounded-md border border-border"
      loading="lazy"
      src={typeof src === "string" ? src : undefined}
    />
  ),
  pre: ({ children }) => {
    if (isValidElement(children)) {
      const childClass = (children.props as { className?: string }).className ?? "";
      if (hasClass(childClass, "language-mermaid")) {
        return <>{children}</>;
      }
    }
    return <pre className="my-3">{children}</pre>;
  },
  code: ({ className, children, node, ...props }) => {
    const raw = String(children ?? "");
    const match = /language-([\w-]+)/.exec(className ?? "");
    const language = match?.[1];

    if (language === "mermaid") {
      return <Mermaid code={raw.replace(/\n$/, "")} />;
    }

    if (language) {
      const html = highlightCode(raw.replace(/\n$/, ""), language);
      return (
        <code
          className={`hljs language-${language} block overflow-x-auto rounded-md p-3 text-xs`}
          // eslint-disable-next-line react/no-danger
          dangerouslySetInnerHTML={{ __html: html }}
        />
      );
    }

    if (raw.includes("\n")) {
      return (
        <code className="hljs block overflow-x-auto rounded-md p-3 text-xs" {...props}>
          {children}
        </code>
      );
    }

    return (
      <code
        className="rounded bg-muted px-1.5 py-0.5 font-mono text-[0.85em] text-foreground"
        {...props}
      >
        {children}
      </code>
    );
  },
};

// Split markdown into its top-level blocks so each can be memoized
// independently. While streaming, only the final (still-growing) block
// changes, so completed blocks never re-parse or rebuild their DOM — this is
// what keeps token-by-token rendering from flickering or fighting the scroll.
function splitIntoBlocks(markdown: string): string[] {
  return marked.lexer(markdown).map((token) => token.raw);
}

const MarkdownBlock = memo(function MarkdownBlock({ content }: { content: string }) {
  return (
    <ReactMarkdown
      components={components}
      rehypePlugins={[rehypeKatex]}
      remarkPlugins={[remarkGfm, remarkMath]}
    >
      {content}
    </ReactMarkdown>
  );
});

export function MarkdownMessage({ content, id }: { content: string; id?: string }): ReactNode {
  const blocks = useMemo(() => splitIntoBlocks(content), [content]);
  return (
    <div className="text-sm leading-relaxed break-words [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
      {blocks.map((block, index) => (
        <MarkdownBlock content={block} key={id ? `${id}-block-${index}` : `block-${index}`} />
      ))}
    </div>
  );
}
