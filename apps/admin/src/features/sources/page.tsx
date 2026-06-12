import { useRef, useState, type RefObject } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { normalizeAppError } from "@/lib/app-error";

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
  const [statusMessage, setStatusMessage] = useState("");
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const folderInputRef = useRef<HTMLInputElement | null>(null);

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
      setStatusMessage(`Imported ${trimmedFileName}.`);
    } catch (error) {
      setStatusMessage(describeImportError(error, trimmedFileName));
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
      setStatusMessage(`Imported ${importedCount} source ${sourceLabel}.`);
    } catch (error) {
      const failedFile = files.find((file) => Boolean(sourcePathFromFile(file)));
      setStatusMessage(describeImportError(error, failedFile ? sourcePathFromFile(failedFile) : sourceLabel));
    }
  }

  if (sources.isLoading) {
    return <RouteStatePane description="Loading source inventory." state="loading" title="Sources" />;
  }

  if (sources.error) {
    const normalized = normalizeAppError(sources.error);
    return <RouteStatePane description={normalized.message} state="failed" title="Sources unavailable" />;
  }

  return (
    <PageSection
      actions={
        <Button onClick={() => rescanSources.mutateAsync({ projectId })}>
          Rescan Sources
        </Button>
      }
      description="Import raw material, upload assets, and trigger ingest operations."
      title="Sources"
    >
      <Tabs defaultValue="text-import">
        <TabsList>
          <TabsTrigger value="text-import">Text Import</TabsTrigger>
          <TabsTrigger value="upload">File Upload</TabsTrigger>
          <TabsTrigger value="folder">Folder Import</TabsTrigger>
        </TabsList>

        <TabsContent value="text-import">
          <Card>
            <CardHeader>
              <CardTitle>Text Import</CardTitle>
              <CardDescription>Create or replace a source file from inline markdown.</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-4">
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
                <Button disabled={importSource.isPending} onClick={handleImportSource}>
                  Import Source
                </Button>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="upload">
          <Card>
            <CardHeader>
              <CardTitle>File Upload</CardTitle>
              <CardDescription>Upload binary or text files directly into the source tree.</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-4">
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
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="folder">
          <Card>
            <CardHeader>
              <CardTitle>Folder Import</CardTitle>
              <CardDescription>Preserve nested folder structure during import.</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-4">
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
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      {statusMessage ? (
        <Card>
          <CardContent className="p-4">
            <p aria-live="polite" className="text-sm">
              {statusMessage}
            </p>
          </CardContent>
        </Card>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle>Project Sources</CardTitle>
          <CardDescription>Manage imported sources and enqueue ingest jobs.</CardDescription>
        </CardHeader>
        <CardContent className="p-0">
          {sources.data?.length ? (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Path</TableHead>
                  <TableHead>Size</TableHead>
                  <TableHead>Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {sources.data.map((source) => (
                  <TableRow key={source.relativePath}>
                    <TableCell className="font-medium">{source.relativePath}</TableCell>
                    <TableCell className="text-muted-foreground">{source.size}</TableCell>
                    <TableCell className="flex flex-wrap gap-2">
                      <Button
                        onClick={() =>
                          ingestSource.mutateAsync({
                            projectId,
                            relativePath: source.relativePath,
                          })
                        }
                        size="sm"
                        variant="secondary"
                      >
                        Ingest
                      </Button>
                      <Button
                        onClick={() =>
                          deleteSource.mutateAsync({
                            projectId,
                            relativePath: source.relativePath.replace(/^raw\/sources\//, ""),
                          })
                        }
                        size="sm"
                        variant="outline"
                      >
                        Delete
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          ) : (
            <CardContent className="pt-0">
              <EmptyState
                description="Import text, files, or a folder to populate this project."
                title="No sources imported"
              />
            </CardContent>
          )}
        </CardContent>
      </Card>
    </PageSection>
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
