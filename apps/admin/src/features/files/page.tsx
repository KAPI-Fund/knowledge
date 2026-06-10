import { useEffect, useState } from "react";
import { useSearchParams, useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select } from "@/components/ui/select";
import { normalizeAppError } from "@/lib/app-error";

import { useProjectFileContentQuery, useProjectFilesQuery } from "./queries";

type ProjectFileNode = {
  name: string;
  path: string;
  isDir: boolean;
  size?: number | null;
  children?: ProjectFileNode[] | null;
};

export function FilesPage() {
  const { projectId = "" } = useParams();
  const [searchParams, setSearchParams] = useSearchParams();
  const [root, setRoot] = useState("all");
  const [recursive, setRecursive] = useState(true);
  const [maxFiles, setMaxFiles] = useState("2000");
  const [selectedPath, setSelectedPath] = useState("");
  const maxFilesNumber = Number(maxFiles) || 2000;
  const files = useProjectFilesQuery(projectId, {
    root,
    recursive,
    maxFiles: maxFilesNumber,
  });
  const content = useProjectFileContentQuery(projectId, selectedPath);

  useEffect(() => {
    const rootParam = searchParams.get("root");
    if (rootParam && rootParam !== root && isSupportedRoot(rootParam)) {
      setRoot(rootParam);
    }
  }, [root, searchParams]);

  useEffect(() => {
    const requestedPath = searchParams.get("path") ?? "";
    if (requestedPath) {
      if (requestedPath !== selectedPath) {
        setSelectedPath(requestedPath);
      }
      return;
    }

    const next = firstFilePath(files.data?.files ?? []);
    if (!selectedPath && next) {
      setSelectedPath(next);
    }
    if (selectedPath && files.data?.files?.length && !containsFilePath(files.data.files, selectedPath) && next) {
      setSelectedPath(next);
    }
  }, [files.data?.files, searchParams, selectedPath]);

  function handleRootChange(nextRoot: string) {
    setRoot(nextRoot);
    const nextParams = new URLSearchParams(searchParams);
    nextParams.set("root", nextRoot);
    if (selectedPath && nextRoot !== "all" && !pathMatchesRoot(selectedPath, nextRoot)) {
      nextParams.delete("path");
      setSelectedPath("");
    }
    setSearchParams(nextParams);
  }

  function handleSelectPath(path: string) {
    setSelectedPath(path);
    const nextParams = new URLSearchParams(searchParams);
    nextParams.set("root", root);
    nextParams.set("path", path);
    setSearchParams(nextParams);
  }

  if (files.isLoading) {
    return <RouteStatePane description="Loading project files." state="loading" title="Files" />;
  }

  if (files.error) {
    const normalized = normalizeAppError(files.error);
    return <RouteStatePane description={normalized.message} state="failed" title="Files unavailable" />;
  }

  return (
    <PageSection
      description="Browse indexed project files and inspect raw content previews."
      title="Files"
    >
      <Card>
        <CardContent className="grid gap-4 p-6 md:grid-cols-[180px_minmax(0,1fr)_auto] md:items-end">
          <label className="grid gap-2 text-sm font-medium">
            Root
            <Select aria-label="Root" onChange={(event) => handleRootChange(event.target.value)} value={root}>
              <option value="all">all</option>
              <option value="wiki">wiki</option>
              <option value="sources">sources</option>
            </Select>
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Max Files
            <Input
              aria-label="Max Files"
              onChange={(event) => setMaxFiles(event.target.value)}
              value={maxFiles}
            />
          </label>
          <label className="flex items-center gap-3 rounded-xl border border-border/70 px-4 py-2 text-sm font-medium">
            <input
              checked={recursive}
              onChange={(event) => setRecursive(event.target.checked)}
              type="checkbox"
            />
            Recursive
          </label>
        </CardContent>
      </Card>

      <div className="grid gap-6 xl:grid-cols-[320px_minmax(0,1fr)]">
        <Card>
          <CardHeader>
            <CardTitle>Tree</CardTitle>
            <CardDescription>Filesystem view rooted to the selected namespace.</CardDescription>
          </CardHeader>
          <CardContent>
            <ScrollArea className="max-h-[520px] pr-3">
              {files.data?.files?.length ? (
                <ul className="grid gap-2">
                  {files.data.files.map((node: ProjectFileNode) => (
                    <FileNodeView
                      key={node.path}
                      node={node}
                      onSelect={handleSelectPath}
                      selectedPath={selectedPath}
                    />
                  ))}
                </ul>
              ) : (
                <EmptyState
                  description="No files matched the current root and traversal settings."
                  title="No files found"
                />
              )}
            </ScrollArea>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Preview</CardTitle>
            <CardDescription>
              {selectedPath ? selectedPath : "Select a file from the tree to inspect its content."}
            </CardDescription>
          </CardHeader>
          <CardContent>
            {!selectedPath ? (
              <EmptyState
                description="Choose a file from the tree to load its preview."
                title="No file selected"
              />
            ) : content.isLoading ? (
              <RouteStatePane description="Loading file preview." state="loading" title="Preview" />
            ) : content.error ? (
              <RouteStatePane description="Preview unavailable for the selected file." state="failed" title="Preview unavailable" />
            ) : (
              <ScrollArea className="max-h-[520px] rounded-xl border border-border/70 bg-muted/30 p-4">
                <pre className="whitespace-pre-wrap break-words font-mono text-sm">{content.data?.content}</pre>
              </ScrollArea>
            )}
          </CardContent>
        </Card>
      </div>
    </PageSection>
  );
}

function FileNodeView({
  node,
  onSelect,
  selectedPath,
  depth = 0,
}: {
  node: ProjectFileNode;
  onSelect: (path: string) => void;
  selectedPath: string;
  depth?: number;
}) {
  return (
    <li className="grid gap-2">
      {node.isDir ? (
        <div className="rounded-lg border border-border/60 bg-muted/20 px-3 py-2" style={{ marginLeft: depth * 12 }}>
          <p className="font-medium">{node.name}</p>
        </div>
      ) : (
        <Button
          className="justify-between"
          onClick={() => onSelect(node.path)}
          style={{ marginLeft: depth * 12 }}
          variant={selectedPath === node.path ? "secondary" : "ghost"}
        >
          <span>{node.name}</span>
          {node.size != null ? (
            <span aria-hidden="true" className="text-xs text-muted-foreground">
              {node.size}
            </span>
          ) : null}
        </Button>
      )}
      {node.children?.length ? (
        <ul className="grid gap-2">
          {node.children.map((child) => (
            <FileNodeView
              key={child.path}
              depth={depth + 1}
              node={child}
              onSelect={onSelect}
              selectedPath={selectedPath}
            />
          ))}
        </ul>
      ) : null}
    </li>
  );
}

function firstFilePath(nodes: ProjectFileNode[]): string {
  for (const node of nodes) {
    if (!node.isDir) {
      return node.path;
    }
    if (node.children?.length) {
      const nested = firstFilePath(node.children);
      if (nested) {
        return nested;
      }
    }
  }

  return "";
}

function containsFilePath(nodes: ProjectFileNode[], targetPath: string): boolean {
  for (const node of nodes) {
    if (node.path === targetPath) {
      return true;
    }
    if (node.children?.length && containsFilePath(node.children, targetPath)) {
      return true;
    }
  }

  return false;
}

function pathMatchesRoot(path: string, root: string): boolean {
  if (root === "all") {
    return true;
  }
  if (root === "wiki") {
    return path.startsWith("wiki/");
  }
  if (root === "sources") {
    return path.startsWith("raw/sources/");
  }
  return false;
}

function isSupportedRoot(root: string) {
  return root === "all" || root === "wiki" || root === "sources";
}
