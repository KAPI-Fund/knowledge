import { useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageHeader } from "@/components/shared/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";

import { ProjectFileLink } from "../shared/file-links";

import { useProjectSearchMutation } from "./queries";

type SearchResponse = {
  mode: string;
  tokenHits: number;
  vectorHits: number;
  results: Array<{
    path: string;
    title: string;
    snippet: string;
    score: number;
    images?: Array<{
      url: string;
      alt: string;
    }>;
    content?: string;
  }>;
};

export function SearchPage() {
  const { projectId = "" } = useParams();
  const [query, setQuery] = useState("");
  const [topK, setTopK] = useState("10");
  const [includeContent, setIncludeContent] = useState(false);
  const [response, setResponse] = useState<SearchResponse | null>(null);
  const search = useProjectSearchMutation();

  async function handleSearch() {
    const trimmed = query.trim();
    if (!trimmed) {
      setResponse(null);
      return;
    }

    const next = await search.mutateAsync({
      projectId,
      query: trimmed,
      topK: Number(topK) || 10,
      includeContent,
    });
    setResponse(next);
  }

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Search the indexed project corpus and inspect ranked matches."
        title="Search"
      />

      <Card>
        <CardHeader>
          <CardTitle>Search Query</CardTitle>
          <CardDescription>Run keyword and vector search against the current project.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <label className="grid gap-2 text-sm font-medium">
            Search Query
            <Input
              aria-label="Search Query"
              onChange={(event) => setQuery(event.target.value)}
              value={query}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Top K
            <Input aria-label="Top K" onChange={(event) => setTopK(event.target.value)} value={topK} />
          </label>
          <label className="flex items-center gap-3 rounded-xl border border-border/70 px-4 py-3 text-sm font-medium">
            <input
              checked={includeContent}
              onChange={(event) => setIncludeContent(event.target.checked)}
              type="checkbox"
            />
            Include Content
          </label>
          <div className="flex justify-end">
            <Button disabled={search.isPending} onClick={handleSearch}>
              Run Search
            </Button>
          </div>
        </CardContent>
      </Card>

      {response ? (
        <Card>
          <CardHeader>
            <CardTitle>Search Summary</CardTitle>
            <CardDescription>Result distribution returned by the backend.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-3">
            <Badge variant="secondary">{`Mode: ${response.mode}`}</Badge>
            <Badge variant="outline">{`Token Hits: ${response.tokenHits}`}</Badge>
            <Badge variant="outline">{`Vector Hits: ${response.vectorHits}`}</Badge>
          </CardContent>
        </Card>
      ) : null}

      {response?.results.length ? (
        <ScrollArea className="grid max-h-[680px] gap-4 pr-1">
          {response.results.map((result) => (
            <Card key={result.path}>
              <CardHeader>
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="space-y-1">
                    <CardTitle>{result.title}</CardTitle>
                    <CardDescription>
                      <ProjectFileLink projectId={projectId} path={result.path} />
                    </CardDescription>
                  </div>
                  <Badge variant="secondary">{`Score ${result.score}`}</Badge>
                </div>
              </CardHeader>
              <CardContent className="grid gap-3">
                <p className="text-sm text-muted-foreground">{result.snippet}</p>
                {result.images?.length ? (
                  <div className="grid gap-2">
                    <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                      Images
                    </p>
                    <ul className="grid gap-2">
                      {result.images.map((image) => (
                        <li
                          key={`${result.path}:${image.url}`}
                          className="rounded-xl border border-border/70 bg-muted/20 px-3 py-2 text-sm"
                        >
                          <span className="font-medium">{image.alt}</span>{" "}
                          <code className="text-xs text-muted-foreground">{image.url}</code>
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
                {result.content ? (
                  <ScrollArea className="max-h-64 rounded-xl border border-border/70 bg-muted/30 p-4">
                    <pre className="whitespace-pre-wrap break-words font-mono text-sm">{result.content}</pre>
                  </ScrollArea>
                ) : null}
              </CardContent>
            </Card>
          ))}
        </ScrollArea>
      ) : response ? (
        <EmptyState
          description="The backend returned no results for this search."
          title="No matches found"
        />
      ) : (
        <EmptyState
          description="Enter a query and run search to inspect ranked matches."
          title="Ready to search"
        />
      )}
    </div>
  );
}
