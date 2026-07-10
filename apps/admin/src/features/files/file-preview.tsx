// Ported from upstream_llm_wiki/src/components/editor/file-preview.tsx.
// convertFileSrc is replaced by the authenticated raw endpoint; system-open
// buttons are replaced by opening the raw URL in a new tab.
import "highlight.js/styles/github-dark.css";

import hljs from "highlight.js/lib/common";
import {
  Code2,
  Download,
  FileQuestion,
  FileSpreadsheet,
  FileText,
  Film,
  Image as ImageIcon,
  Maximize2,
  Minus,
  Music,
  Plus,
  RefreshCw,
  X,
} from "lucide-react";
import { useMemo, useState } from "react";

import { Mermaid } from "@/components/shared/mermaid";
import { MarkdownMessage } from "@/components/shared/markdown-message";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

import { rawFileUrl } from "../shared/api";
import {
  type FileCategory,
  getCodeLanguage,
  getFileCategory,
  getFileExtension,
  getFileName,
  isExtractedTextPreviewFile,
} from "./file-types";

export function FilePreview({
  projectId,
  path,
  content,
}: {
  projectId: string;
  path: string;
  // Text content from the files/content endpoint; null for binary files.
  content: string | null;
}) {
  const category = getFileCategory(path);
  const extension = getFileExtension(path);
  const fileName = getFileName(path);
  const rawUrl = rawFileUrl(projectId, path);

  // Upstream dispatch L60-95.
  switch (category) {
    case "image":
      return <ImagePreview fileName={fileName} rawUrl={rawUrl} />;
    case "video":
      return <VideoPreview fileName={fileName} rawUrl={rawUrl} />;
    case "audio":
      return <AudioPreview fileName={fileName} rawUrl={rawUrl} />;
    case "pdf":
      return <PdfPreview rawUrl={rawUrl} />;
    case "code":
      if (extension === "mmd" || extension === "mermaid") {
        return <MermaidPreview content={content ?? ""} />;
      }
      if (extension === "svg" && path.startsWith("agent-workspace/")) {
        return <ImagePreview fileName={fileName} rawUrl={rawUrl} />;
      }
      if (extension === "html" || extension === "htm") {
        return <HtmlPreview content={content ?? ""} fileName={fileName} rawUrl={rawUrl} />;
      }
      return <CodePreview content={content ?? ""} path={path} />;
    case "data":
      if (extension === "csv" || extension === "tsv") {
        return (
          <DelimitedTablePreview content={content ?? ""} delimiter={extension === "tsv" ? "\t" : ","} />
        );
      }
      return <CodePreview content={content ?? ""} path={path} />;
    case "markdown":
      return <MarkdownPreview content={content ?? ""} label="Markdown" path={path} />;
    case "text":
      return <MarkdownPreview content={content ?? ""} label="Text" path={path} />;
    case "document":
      if (isExtractedTextPreviewFile(path)) {
        return <MarkdownPreview content={content ?? ""} label="Extracted text" path={path} />;
      }
      return (
        <BinaryPlaceholder category={category} fileName={fileName} path={path} rawUrl={rawUrl} />
      );
    default:
      return (
        <BinaryPlaceholder category={category} fileName={fileName} path={path} rawUrl={rawUrl} />
      );
  }
}

function PreviewFrame({ children }: { children: React.ReactNode }) {
  return <div className="rounded-xl border border-border/70 bg-muted/30">{children}</div>;
}

function ImagePreview({ fileName, rawUrl }: { fileName: string; rawUrl: string }) {
  const [expanded, setExpanded] = useState(false);
  const [zoom, setZoom] = useState(1);
  return (
    <PreviewFrame>
      <div className="flex items-center justify-end gap-1 px-3 pt-2">
        <Button
          aria-label="Expand image"
          onClick={() => setExpanded(true)}
          size="icon"
          variant="ghost"
        >
          <Maximize2 />
        </Button>
      </div>
      <div className="flex max-h-[520px] items-center justify-center overflow-auto p-4 pt-0">
        <img alt={fileName} className="max-h-[480px] max-w-full rounded-md object-contain" src={rawUrl} />
      </div>
      {expanded ? (
        <div className="fixed inset-0 z-[100] flex flex-col bg-background/95 p-4 backdrop-blur-sm">
          <div className="flex justify-end gap-1">
            <Button
              aria-label="Zoom out"
              onClick={() => setZoom((value) => Math.max(0.25, value - 0.25))}
              size="icon"
              variant="ghost"
            >
              <Minus />
            </Button>
            <Button
              aria-label="Zoom in"
              onClick={() => setZoom((value) => Math.min(5, value + 0.25))}
              size="icon"
              variant="ghost"
            >
              <Plus />
            </Button>
            <Button
              aria-label="Close"
              onClick={() => setExpanded(false)}
              size="icon"
              variant="ghost"
            >
              <X />
            </Button>
          </div>
          <div className="min-h-0 flex-1 overflow-auto text-center">
            <img
              alt={fileName}
              className="mx-auto max-w-none object-contain"
              src={rawUrl}
              style={{ width: `${zoom * 100}%` }}
            />
          </div>
        </div>
      ) : null}
    </PreviewFrame>
  );
}

function VideoPreview({ fileName, rawUrl }: { fileName: string; rawUrl: string }) {
  return (
    <PreviewFrame>
      <div className="flex items-center justify-center rounded-xl bg-black p-2">
        {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
        <video className="max-h-[480px] max-w-full" controls src={rawUrl}>
          <track kind="captions" label={fileName} />
        </video>
      </div>
    </PreviewFrame>
  );
}

function AudioPreview({ fileName, rawUrl }: { fileName: string; rawUrl: string }) {
  return (
    <PreviewFrame>
      <div className="flex flex-col items-center justify-center gap-4 p-6">
        <Music className="h-16 w-16 text-muted-foreground/50" />
        <p className="text-sm font-medium">{fileName}</p>
        {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
        <audio className="w-full max-w-md" controls src={rawUrl}>
          <track kind="captions" label={fileName} />
        </audio>
      </div>
    </PreviewFrame>
  );
}

// Upstream PdfPreview L97-113 minus the extracted-text toggle (the server has
// no pdf text extraction).
function PdfPreview({ rawUrl }: { rawUrl: string }) {
  const [page, setPage] = useState(1);
  const [zoom, setZoom] = useState(100);
  const src = `${rawUrl}#page=${page}&zoom=${zoom}`;
  return (
    <PreviewFrame>
      <div className="flex items-center gap-1.5 px-3 py-2 text-xs text-muted-foreground">
        <Badge variant="outline">PDF</Badge>
        <span className="flex-1" />
        <Button
          aria-label="Zoom out"
          onClick={() => setZoom((value) => Math.max(50, value - 25))}
          size="icon"
          variant="ghost"
        >
          <Minus />
        </Button>
        <span className="w-10 text-center">{zoom}%</span>
        <Button
          aria-label="Zoom in"
          onClick={() => setZoom((value) => Math.min(300, value + 25))}
          size="icon"
          variant="ghost"
        >
          <Plus />
        </Button>
        <label className="ml-1 flex items-center gap-1">
          Page
          <Input
            className="h-8 w-16"
            min={1}
            onChange={(event) => setPage(Math.max(1, Number(event.target.value) || 1))}
            type="number"
            value={page}
          />
        </label>
      </div>
      <div className="h-[560px] overflow-hidden rounded-b-xl bg-white">
        <object className="h-full w-full" data={src} key={src} type="application/pdf">
          <iframe className="h-full w-full" src={src} title="PDF preview" />
        </object>
      </div>
    </PreviewFrame>
  );
}

// Upstream parseDelimitedContent L115-133.
export function parseDelimitedContent(content: string, delimiter: string, maxRows = 500): string[][] {
  const rows: string[][] = [];
  let cells: string[] = [];
  let current = "";
  let quoted = false;
  const normalized = content.replace(/\r\n/g, "\n");
  for (let index = 0; index < normalized.length && rows.length < maxRows; index += 1) {
    const char = normalized[index];
    if (char === '"') {
      if (quoted && normalized[index + 1] === '"') {
        current += '"';
        index += 1;
      } else {
        quoted = !quoted;
      }
    } else if (char === delimiter && !quoted) {
      cells.push(current);
      current = "";
    } else {
      current += char;
    }
    if (char === "\n" && !quoted) {
      current = current.slice(0, -1);
      cells.push(current);
      rows.push(cells);
      cells = [];
      current = "";
    }
  }
  if ((current || cells.length > 0) && rows.length < maxRows) {
    cells.push(current);
    rows.push(cells);
  }
  return rows;
}

function DelimitedTablePreview({ content, delimiter }: { content: string; delimiter: string }) {
  const rows = useMemo(() => parseDelimitedContent(content, delimiter), [content, delimiter]);
  return (
    <PreviewFrame>
      <ScrollArea className="max-h-[520px]">
        <Table>
          {rows[0] ? (
            <TableHeader>
              <TableRow>
                {rows[0].map((cell, index) => (
                  <TableHead key={index}>{cell}</TableHead>
                ))}
              </TableRow>
            </TableHeader>
          ) : null}
          <TableBody>
            {rows.slice(1).map((row, rowIndex) => (
              <TableRow key={rowIndex}>
                {row.map((cell, cellIndex) => (
                  <TableCell className="max-w-80 align-top" key={cellIndex}>
                    {cell}
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </ScrollArea>
    </PreviewFrame>
  );
}

function HtmlPreview({
  content,
  fileName,
  rawUrl,
}: {
  content: string;
  fileName: string;
  rawUrl: string;
}) {
  const [showSource, setShowSource] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  return (
    <PreviewFrame>
      <div className="flex items-center gap-1.5 px-3 py-2 text-xs text-muted-foreground">
        <Badge variant="outline">HTML</Badge>
        <span className="flex-1" />
        <Button
          aria-label={showSource ? "Show rendered" : "Show source"}
          onClick={() => setShowSource((value) => !value)}
          size="icon"
          variant="ghost"
        >
          <Code2 />
        </Button>
        {!showSource ? (
          <Button
            aria-label="Reload"
            onClick={() => setReloadKey((value) => value + 1)}
            size="icon"
            variant="ghost"
          >
            <RefreshCw />
          </Button>
        ) : null}
      </div>
      <div className="h-[560px] overflow-hidden rounded-b-xl border-t border-border/70 bg-background">
        {showSource ? (
          <ScrollArea className="h-full">
            <pre className="whitespace-pre-wrap break-words p-4 font-mono text-xs">{content}</pre>
          </ScrollArea>
        ) : (
          <iframe
            className="h-full w-full bg-white"
            key={reloadKey}
            referrerPolicy="no-referrer"
            // Generated HTML is untrusted agent output. Scripts are useful for
            // interactive reports, but same-origin access stays disabled so the
            // document cannot reach the parent DOM or authenticated app APIs.
            sandbox="allow-scripts"
            src={rawUrl}
            title={fileName}
          />
        )}
      </div>
    </PreviewFrame>
  );
}

function MermaidPreview({ content }: { content: string }) {
  return (
    <PreviewFrame>
      <div className="flex items-center gap-1.5 px-3 pt-2">
        <Badge variant="outline">Mermaid</Badge>
      </div>
      <div className="p-3">
        <Mermaid code={content} />
      </div>
    </PreviewFrame>
  );
}

function CodePreview({ content, path }: { content: string; path: string }) {
  const language = getCodeLanguage(path);
  const highlighted = useMemo(() => {
    if (language && hljs.getLanguage(language)) {
      return hljs.highlight(content, { ignoreIllegals: true, language }).value;
    }
    return null;
  }, [content, language]);
  return (
    <PreviewFrame>
      <div className="flex items-center gap-1.5 px-3 pt-2">
        <Badge variant="outline">{language || "text"}</Badge>
      </div>
      <ScrollArea className="max-h-[520px]">
        {highlighted ? (
          <pre className="p-4">
            <code
              className={`hljs language-${language} block overflow-x-auto rounded-md p-3 text-xs`}
              // eslint-disable-next-line react/no-danger
              dangerouslySetInnerHTML={{ __html: highlighted }}
            />
          </pre>
        ) : (
          <pre className="whitespace-pre-wrap break-words p-4 font-mono text-xs">{content}</pre>
        )}
      </ScrollArea>
    </PreviewFrame>
  );
}

function MarkdownPreview({ content, label, path }: { content: string; label: string; path: string }) {
  return (
    <PreviewFrame>
      <div className="flex items-center gap-1.5 px-3 pt-2">
        <Badge variant="outline">{label}</Badge>
      </div>
      <ScrollArea className="max-h-[560px]">
        <div className="p-4">
          <MarkdownMessage content={content} id={path} />
        </div>
      </ScrollArea>
    </PreviewFrame>
  );
}

function BinaryPlaceholder({
  category,
  fileName,
  path,
  rawUrl,
}: {
  category: FileCategory;
  fileName: string;
  path: string;
  rawUrl: string;
}) {
  const iconMap: Partial<Record<FileCategory, typeof FileText>> = {
    document: FileSpreadsheet,
    image: ImageIcon,
    unknown: FileQuestion,
    video: Film,
  };
  const Icon = iconMap[category] ?? FileQuestion;
  return (
    <PreviewFrame>
      <div className="flex flex-col items-center justify-center gap-4 p-10 text-center">
        <Icon className="h-16 w-16 text-muted-foreground/30" />
        <div>
          <p className="text-sm font-medium">{fileName}</p>
          <p className="mt-1 text-xs text-muted-foreground">{path}</p>
        </div>
        <p className="text-sm text-muted-foreground">Preview is not available for this file type.</p>
        <Button asChild variant="outline">
          <a href={rawUrl} rel="noopener noreferrer" target="_blank">
            <Download />
            Open raw file
          </a>
        </Button>
      </div>
    </PreviewFrame>
  );
}
