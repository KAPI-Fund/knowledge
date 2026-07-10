import { useEffect, useState } from "react";
import { useSearchParams, useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { normalizeAppError } from "@/lib/app-error";

import { FileHistoryPanel } from "./file-history-panel";
import { FilePreview } from "./file-preview";
import { hasServerTextContent } from "./file-types";
import { useProjectFileContentQuery, useProjectFilesQuery, useSaveFileContentMutation } from "./queries";
import { WikiPageEditor } from "./wiki-page-editor";

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
  const [deleteNotice, setDeleteNotice] = useState("");
  const maxFilesNumber = Number(maxFiles) || 2000;
  const files = useProjectFilesQuery(projectId, {
    root,
    recursive,
    maxFiles: maxFilesNumber,
  });
  const textReadable = hasServerTextContent(selectedPath);
  const content = useProjectFileContentQuery(projectId, textReadable ? selectedPath : "");
  const createPage = useSaveFileContentMutation(projectId);
  const [newPagePath, setNewPagePath] = useState("");
  const [createError, setCreateError] = useState("");

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
    setDeleteNotice("");
    const nextParams = new URLSearchParams(searchParams);
    nextParams.set("root", root);
    nextParams.set("path", path);
    setSearchParams(nextParams);
  }

  function handleDeleted(summary: string) {
    setSelectedPath("");
    setDeleteNotice(summary);
    const nextParams = new URLSearchParams(searchParams);
    nextParams.delete("path");
    setSearchParams(nextParams);
  }

  async function handleCreatePage() {
    const path = newPagePath.trim();
    if (!path.startsWith("wiki/") || !path.endsWith(".md")) {
      setCreateError("Path must be a markdown file under wiki/, e.g. wiki/concepts/topic.md");
      return;
    }
    try {
      const stem = path.slice(path.lastIndexOf("/") + 1, -".md".length);
      await createPage.mutateAsync({ path, content: `# ${stem.replace(/-/g, " ")}\n` });
      setCreateError("");
      setNewPagePath("");
      handleSelectPath(path);
    } catch (error) {
      setCreateError(normalizeAppError(error).message);
    }
  }

  if (files.isLoading) {
    return <RouteStatePane description="Loading project files." state="loading" title="Files" />;
  }

  if (files.error) {
    const normalized = normalizeAppError(files.error);
    return <RouteStatePane description={normalized.message} state="failed" title="Files unavailable" />;
  }

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Browse indexed project files and inspect raw content previews."
        title="Files"
      />

      <Card>
        <CardContent className="grid gap-4 p-6 md:grid-cols-[180px_minmax(0,1fr)_auto] md:items-end">
          <div className="grid gap-2">
            <Label htmlFor="files-root">Root</Label>
            <Select onValueChange={handleRootChange} value={root}>
              <SelectTrigger id="files-root" aria-label="Root" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">all</SelectItem>
                <SelectItem value="wiki">wiki</SelectItem>
                <SelectItem value="sources">sources</SelectItem>
                <SelectItem value="workspace">workspace</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <label className="grid gap-2 text-sm font-medium">
            Max Files
            <Input
              aria-label="Max Files"
              onChange={(event) => setMaxFiles(event.target.value)}
              value={maxFiles}
            />
          </label>
          <div className="flex items-center gap-3 rounded-lg border px-4 py-2.5">
            <Checkbox
              id="files-recursive"
              checked={recursive}
              onCheckedChange={(checked) => setRecursive(checked === true)}
            />
            <Label htmlFor="files-recursive">Recursive</Label>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>New Wiki Page</CardTitle>
          <CardDescription>Create a markdown page under wiki/ and open it in the editor.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-end">
          <label className="grid gap-2 text-sm font-medium">
            New Page Path
            <Input
              aria-label="New Page Path"
              onChange={(event) => setNewPagePath(event.target.value)}
              placeholder="wiki/concepts/topic.md"
              value={newPagePath}
            />
          </label>
          <Button
            disabled={createPage.isPending || !newPagePath.trim()}
            onClick={() => void handleCreatePage()}
          >
            Create Page
          </Button>
          {createError ? <p className="text-sm text-destructive md:col-span-2">{createError}</p> : null}
        </CardContent>
      </Card>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,360px)_minmax(0,1fr)]">
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
          <CardHeader className="flex-row items-start justify-between gap-4 space-y-0">
            <div className="grid gap-1.5">
              <CardTitle>Preview</CardTitle>
              <CardDescription>
                {selectedPath ? selectedPath : "Select a file from the tree to inspect its content."}
              </CardDescription>
            </div>
            {selectedPath ? (
              <FileHistoryPanel
                currentContent={content.data?.content ?? null}
                path={selectedPath}
                projectId={projectId}
              />
            ) : null}
          </CardHeader>
          <CardContent>
            {deleteNotice ? (
              <p className="mb-4 text-sm text-muted-foreground">{deleteNotice}</p>
            ) : null}
            {!selectedPath ? (
              <EmptyState
                description="Choose a file from the tree to load its preview."
                title="No file selected"
              />
            ) : textReadable && content.isLoading ? (
              <RouteStatePane description="Loading file preview." state="loading" title="Preview" />
            ) : textReadable && content.error ? (
              <RouteStatePane description="Preview unavailable for the selected file." state="failed" title="Preview unavailable" />
            ) : (
              <div className="grid gap-4">
                <WikiPageEditor
                  content={content.data?.content ?? ""}
                  onDeleted={handleDeleted}
                  path={selectedPath}
                  projectId={projectId}
                />
                <FilePreview
                  content={textReadable ? (content.data?.content ?? "") : null}
                  path={selectedPath}
                  projectId={projectId}
                />
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
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
  if (root === "workspace") {
    return path.startsWith("agent-workspace/");
  }
  return false;
}

function isSupportedRoot(root: string) {
  return root === "all" || root === "wiki" || root === "sources" || root === "workspace";
}
