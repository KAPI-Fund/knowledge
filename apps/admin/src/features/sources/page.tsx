import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";
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

  useEffect(() => {
    if (!folderInputRef.current) {
      return;
    }

    folderInputRef.current.setAttribute("webkitdirectory", "");
    folderInputRef.current.setAttribute("directory", "");
  }, []);

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
    inputRef: React.RefObject<HTMLInputElement | null>,
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

  return (
    <section className="stack">
      <h1>Sources</h1>
      <ProjectNav projectId={projectId} />
      <div className="card stack panel">
        <h2>Text Import</h2>
        <label>
          File Name
          <input value={fileName} onChange={(event) => setFileName(event.target.value)} />
        </label>
        <label>
          Markdown Content
          <textarea value={content} onChange={(event) => setContent(event.target.value)} rows={6} />
        </label>
        <button type="button" onClick={handleImportSource}>
          Import Source
        </button>
      </div>

      <div className="card stack panel">
        <h2>File Upload</h2>
        <label>
          Files to Upload
          <input
            aria-label="Files to Upload"
            ref={fileInputRef}
            type="file"
            multiple
            onChange={(event) => setSelectedFiles(Array.from(event.target.files ?? []) as UploadFile[])}
          />
        </label>
        {selectedFiles.length ? (
          <ul className="results-list">
            {selectedFiles.map((file) => (
              <li key={`${sourcePathFromFile(file)}:${file.size}:${file.lastModified}`}>
                {sourcePathFromFile(file)}
              </li>
            ))}
          </ul>
        ) : (
          <p>No files selected.</p>
        )}
        <button
          type="button"
          onClick={handleUploadFiles}
          disabled={!selectedFiles.length || importSource.isPending}
        >
          Upload Files
        </button>
      </div>

      <div className="card stack panel">
        <h2>Folder Import</h2>
        <label>
          Folder to Import
          <input
            aria-label="Folder to Import"
            ref={folderInputRef}
            type="file"
            multiple
            onChange={(event) =>
              setSelectedFolderFiles(Array.from(event.target.files ?? []) as UploadFile[])
            }
          />
        </label>
        {selectedFolderFiles.length ? (
          <ul className="results-list">
            {selectedFolderFiles.map((file) => (
              <li key={`${sourcePathFromFile(file)}:${file.size}:${file.lastModified}`}>
                {sourcePathFromFile(file)}
              </li>
            ))}
          </ul>
        ) : (
          <p>No folder selected.</p>
        )}
        <button
          type="button"
          onClick={handleImportFolder}
          disabled={!selectedFolderFiles.length || importSource.isPending}
        >
          Import Folder
        </button>
      </div>

      <div className="card stack compact panel">
        <button type="button" onClick={() => rescanSources.mutateAsync({ projectId })}>
          Rescan Sources
        </button>
        {statusMessage ? <p aria-live="polite">{statusMessage}</p> : null}
      </div>
      <ul>
        {sources.data?.map((source) => (
          <li key={source.relativePath}>
            <span>{source.relativePath}</span> <span>{source.size}</span>
            <button
              type="button"
              onClick={() =>
                ingestSource.mutateAsync({
                  projectId,
                  relativePath: source.relativePath,
                })
              }
            >
              Ingest
            </button>
            <button
              type="button"
              onClick={() =>
                deleteSource.mutateAsync({
                  projectId,
                  relativePath: source.relativePath.replace(/^raw\/sources\//, ""),
                })
              }
            >
              Delete
            </button>
          </li>
        ))}
      </ul>
    </section>
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
