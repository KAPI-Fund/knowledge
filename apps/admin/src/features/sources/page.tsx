import type { ColumnDef } from "@tanstack/react-table";
import { RefreshCw } from "lucide-react";
import { useMemo, useRef, useState, type RefObject } from "react";
import { useParams } from "react-router-dom";
import { toast } from "sonner";

import { EmptyState } from "@/components/layout/empty-state";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { ConfirmDialog } from "@/components/shared/confirm-dialog";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { normalizeAppError } from "@/lib/app-error";

import { ProjectFileLink } from "../shared/file-links";
import { fileToBase64 } from "../shared/api";
import {
  useDeleteSourceMutation,
  useImportSourceMutation,
  useIngestSourceMutation,
  useProjectSourcesQuery,
  useRescanSourcesMutation,
} from "./queries";

type UploadFile = File & {
  webkitRelativePath?: string;
};

type SourceRow = NonNullable<ReturnType<typeof useProjectSourcesQuery>["data"]>[number];

export function SourcesPage() {
  const { projectId = "" } = useParams();
  const sources = useProjectSourcesQuery(projectId);
  const importSource = useImportSourceMutation();
  const ingestSource = useIngestSourceMutation();
  const rescanSources = useRescanSourcesMutation();
  const deleteSource = useDeleteSourceMutation();
  const [fileName, setFileName] = useState("");
  const [content, setContent] = useState("");
  const [selectedFiles, setSelectedFiles] = useState<UploadFile[]>([]);
  const [selectedFolderFiles, setSelectedFolderFiles] = useState<UploadFile[]>([]);
  const [deleteTarget, setDeleteTarget] = useState<SourceRow | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const folderInputRef = useRef<HTMLInputElement | null>(null);

  const columns = useMemo<ColumnDef<SourceRow>[]>(
    () => [
      {
        accessorKey: "relativePath",
        header: "Path",
        cell: ({ row }) => (
          <ProjectFileLink path={row.original.relativePath} projectId={projectId}>
            {row.original.relativePath}
          </ProjectFileLink>
        ),
      },
      {
        accessorKey: "size",
        header: "Size",
        cell: ({ row }) => (
          <span className="block text-right font-mono text-[11px] text-muted-foreground">
            {row.original.size}
          </span>
        ),
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) => (
          <div className="flex flex-wrap justify-end gap-2">
            <Button
              onClick={async () => {
                try {
                  await ingestSource.mutateAsync({
                    projectId,
                    relativePath: row.original.relativePath,
                  });
                  toast.success(`Ingest queued for ${row.original.relativePath}.`);
                } catch (error) {
                  toast.error(normalizeAppError(error).message);
                }
              }}
              size="sm"
              variant="secondary"
            >
              Ingest
            </Button>
            <Button onClick={() => setDeleteTarget(row.original)} size="sm" variant="outline">
              Delete
            </Button>
          </div>
        ),
      },
    ],
    [projectId, ingestSource],
  );

  function registerFolderInput(node: HTMLInputElement | null) {
    folderInputRef.current = node;
    // The folder input lives in a tab panel that unmounts when inactive, so
    // the directory attributes must be applied on every mount via callback ref.
    node?.setAttribute("webkitdirectory", "");
    node?.setAttribute("directory", "");
  }

  async function handleImportSource() {
    const trimmedFileName = fileName.trim();
    if (!trimmedFileName) {
      return;
    }

    try {
      await importSource.mutateAsync({
        projectId,
        fileName: trimmedFileName,
        content,
      });
      setFileName("");
      setContent("");
      toast.success(`Imported ${trimmedFileName}.`);
    } catch (error) {
      toast.error(describeImportError(error, trimmedFileName));
    }
  }

  async function handleUploadFiles() {
    await importSelectedFiles(selectedFiles, fileInputRef, "files");
  }

  async function handleImportFolder() {
    await importSelectedFiles(selectedFolderFiles, folderInputRef, "folder");
  }

  async function importSelectedFiles(
    files: UploadFile[],
    inputRef: RefObject<HTMLInputElement | null>,
    sourceLabel: string,
  ) {
    if (!files.length || !projectId) {
      return;
    }

    try {
      let importedCount = 0;
      for (const file of files) {
        const sourcePath = sourcePathFromFile(file);
        await importSource.mutateAsync({
          projectId,
          fileName: sourcePath,
          contentBase64: await fileToBase64(file),
        });
        importedCount += 1;
      }

      if (inputRef === fileInputRef) {
        setSelectedFiles([]);
      } else {
        setSelectedFolderFiles([]);
      }
      if (inputRef.current) {
        inputRef.current.value = "";
      }
      toast.success(`Imported ${importedCount} source ${sourceLabel}.`);
    } catch (error) {
      const failedFile = files.find((file) => Boolean(sourcePathFromFile(file)));
      toast.error(describeImportError(error, failedFile ? sourcePathFromFile(failedFile) : sourceLabel));
    }
  }

  if (sources.isLoading) {
    return <RouteStatePane description="Loading source inventory." state="loading" title="Sources" />;
  }

  if (sources.error) {
    const normalized = normalizeAppError(sources.error);
    return <RouteStatePane description={normalized.message} state="failed" title="Sources unavailable" />;
  }

  const allSources = sources.data ?? [];

  return (
    <div className="grid gap-6">
      <PageHeader
        actions={
          <Button
            disabled={rescanSources.isPending}
            onClick={async () => {
              try {
                await rescanSources.mutateAsync({ projectId });
                toast.success("Source rescan queued.");
              } catch (error) {
                toast.error(normalizeAppError(error).message);
              }
            }}
          >
            <RefreshCw />
            Rescan Sources
          </Button>
        }
        description="Import raw material, upload assets, and trigger ingest operations."
        title="Sources"
      />

      <Card>
        <CardHeader>
          <CardTitle>Import sources</CardTitle>
          <CardDescription>Add raw material via inline text, file upload, or folder import.</CardDescription>
        </CardHeader>
        <CardContent>
          <Tabs defaultValue="text-import">
            <TabsList>
              <TabsTrigger value="text-import">Text Import</TabsTrigger>
              <TabsTrigger value="upload">File Upload</TabsTrigger>
              <TabsTrigger value="folder">Folder Import</TabsTrigger>
            </TabsList>

            <TabsContent className="grid gap-4 pt-4" value="text-import">
              <label className="grid gap-2 text-sm font-medium">
                File Name
                <Input onChange={(event) => setFileName(event.target.value)} value={fileName} />
              </label>
              <label className="grid gap-2 text-sm font-medium">
                Markdown Content
                <Textarea
                  onChange={(event) => setContent(event.target.value)}
                  rows={10}
                  value={content}
                />
              </label>
              <div className="flex justify-end">
                <Button
                  disabled={importSource.isPending || !fileName.trim()}
                  onClick={handleImportSource}
                >
                  Import Source
                </Button>
              </div>
            </TabsContent>

            <TabsContent className="grid gap-4 pt-4" value="upload">
              <label className="grid gap-2 text-sm font-medium">
                Files to Upload
                <Input
                  aria-label="Files to Upload"
                  multiple
                  onChange={(event) => setSelectedFiles(Array.from(event.target.files ?? []) as UploadFile[])}
                  ref={fileInputRef}
                  type="file"
                />
              </label>
              {selectedFiles.length ? (
                <ul className="grid gap-2 rounded-xl border border-border/70 bg-muted/20 p-4 text-sm">
                  {selectedFiles.map((file) => (
                    <li key={`${sourcePathFromFile(file)}:${file.size}:${file.lastModified}`}>
                      {sourcePathFromFile(file)}
                    </li>
                  ))}
                </ul>
              ) : (
                <EmptyState
                  description="Select one or more files to upload into the project."
                  title="No files selected"
                />
              )}
              <div className="flex justify-end">
                <Button
                  disabled={!selectedFiles.length || importSource.isPending}
                  onClick={handleUploadFiles}
                >
                  Upload Files
                </Button>
              </div>
            </TabsContent>

            <TabsContent className="grid gap-4 pt-4" value="folder">
              <label className="grid gap-2 text-sm font-medium">
                Folder to Import
                <Input
                  aria-label="Folder to Import"
                  multiple
                  onChange={(event) => setSelectedFolderFiles(Array.from(event.target.files ?? []) as UploadFile[])}
                  ref={registerFolderInput}
                  type="file"
                />
              </label>
              {selectedFolderFiles.length ? (
                <ul className="grid gap-2 rounded-xl border border-border/70 bg-muted/20 p-4 text-sm">
                  {selectedFolderFiles.map((file) => (
                    <li key={`${sourcePathFromFile(file)}:${file.size}:${file.lastModified}`}>
                      {sourcePathFromFile(file)}
                    </li>
                  ))}
                </ul>
              ) : (
                <EmptyState
                  description="Choose a folder to import nested source files."
                  title="No folder selected"
                />
              )}
              <div className="flex justify-end">
                <Button
                  disabled={!selectedFolderFiles.length || importSource.isPending}
                  onClick={handleImportFolder}
                >
                  Import Folder
                </Button>
              </div>
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Project Sources</CardTitle>
          <CardDescription>Raw files currently tracked under raw/sources.</CardDescription>
        </CardHeader>
        <CardContent>
          {allSources.length ? (
            <DataTable
              columns={columns}
              data={allSources}
              searchKey="relativePath"
              searchPlaceholder="Filter by path"
            />
          ) : (
            <EmptyState
              description="Import text, files, or a folder to populate this project."
              title="No sources imported"
            />
          )}
        </CardContent>
      </Card>

      <ConfirmDialog
        confirmLabel="Delete source"
        description={
          deleteTarget
            ? `${deleteTarget.relativePath} will be removed from this project.`
            : ""
        }
        destructive
        isPending={deleteSource.isPending}
        onConfirm={async () => {
          if (!deleteTarget) return;
          try {
            await deleteSource.mutateAsync({
              projectId,
              relativePath: deleteTarget.relativePath.replace(/^raw\/sources\//, ""),
            });
            toast.success(`Deleted ${deleteTarget.relativePath}.`);
          } catch (error) {
            toast.error(normalizeAppError(error).message);
          }
        }}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
        open={Boolean(deleteTarget)}
        title="Delete this source?"
      />
    </div>
  );
}

function sourcePathFromFile(file: UploadFile) {
  const relativePath = file.webkitRelativePath?.trim().replaceAll("\\", "/");
  return relativePath ? relativePath.replace(/^\/+/, "") : file.name;
}

function describeImportError(error: unknown, sourceName: string) {
  if (error instanceof Error && error.message) {
    return `Import failed for ${sourceName}: ${error.message}`;
  }

  return `Import failed for ${sourceName}.`;
}
