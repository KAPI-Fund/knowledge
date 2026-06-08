import { useEffect, useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";

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

  return (
    <section className="stack">
      <h1>Files</h1>
      <ProjectNav projectId={projectId} />
      <div className="card stack compact panel">
        <label>
          Root
          <select value={root} onChange={(event) => handleRootChange(event.target.value)}>
            <option value="all">all</option>
            <option value="wiki">wiki</option>
            <option value="sources">sources</option>
          </select>
        </label>
        <label>
          Max Files
          <input value={maxFiles} onChange={(event) => setMaxFiles(event.target.value)} />
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={recursive}
            onChange={(event) => setRecursive(event.target.checked)}
          />
          Recursive
        </label>
      </div>

      <div className="files-layout">
        <section className="card stack compact">
          <h2>Tree</h2>
          {files.data?.files?.length ? (
            <ul className="tree-list">
              {files.data.files.map((node: ProjectFileNode) => (
                <FileNodeView
                  key={node.path}
                  node={node}
                  selectedPath={selectedPath}
                  onSelect={handleSelectPath}
                />
              ))}
            </ul>
          ) : (
            <p>No files found.</p>
          )}
        </section>

        <section className="card stack compact">
          <h2>Preview</h2>
          {selectedPath ? <strong>{selectedPath}</strong> : <p>Select a file.</p>}
          {content.isLoading ? <p>Loading preview...</p> : null}
          {content.error ? <p>Preview unavailable.</p> : null}
          {content.data ? <pre className="preview-pane">{content.data.content}</pre> : null}
        </section>
      </div>
    </section>
  );
}

function FileNodeView({
  node,
  onSelect,
  selectedPath,
}: {
  node: ProjectFileNode;
  onSelect: (path: string) => void;
  selectedPath: string;
}) {
  return (
    <li className="tree-item">
      {node.isDir ? (
        <div className="tree-label">
          <strong>{node.name}</strong>
        </div>
      ) : (
        <button
          type="button"
          className={selectedPath === node.path ? "ghost-button tree-button active" : "ghost-button tree-button"}
          onClick={() => onSelect(node.path)}
        >
          {node.name}
        </button>
      )}
      {node.children?.length ? (
        <ul className="tree-list nested">
          {node.children.map((child) => (
            <FileNodeView
              key={child.path}
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
