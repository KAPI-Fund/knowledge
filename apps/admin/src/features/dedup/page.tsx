import { useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Select } from "@/components/ui/select";

import {
  useDetectDuplicatesMutation,
  useDismissDuplicateGroupMutation,
  useMergeDuplicateGroupMutation,
  useProjectDedupQuery,
} from "./queries";

export function DedupPage() {
  const { projectId = "" } = useParams();
  const [canonicalByGroup, setCanonicalByGroup] = useState<Record<string, string>>({});
  const dedup = useProjectDedupQuery(projectId);
  const detectDuplicates = useDetectDuplicatesMutation();
  const mergeGroup = useMergeDuplicateGroupMutation();
  const dismissGroup = useDismissDuplicateGroupMutation();

  const groups = dedup.data?.groups ?? [];
  const notDuplicates = dedup.data?.notDuplicates ?? [];

  return (
    <PageSection
      actions={
        <Button
          disabled={detectDuplicates.isPending}
          onClick={() => detectDuplicates.mutateAsync({ projectId })}
        >
          Detect Duplicates
        </Button>
      }
      description="Detect duplicate wiki pages, then merge them into one canonical page or dismiss false positives."
      title="Dedup"
    >
      {groups.length ? (
        <div className="grid gap-4">
          {groups.map((group) => {
            const canonicalSlug = canonicalByGroup[group.id] ?? group.slugs[0] ?? "";
            return (
              <Card key={group.id}>
                <CardHeader>
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="space-y-1">
                      <CardTitle>{group.slugs.join(" / ")}</CardTitle>
                      <CardDescription>{group.reason}</CardDescription>
                    </div>
                    <Badge variant="outline">{group.confidence}</Badge>
                  </div>
                </CardHeader>
                <CardContent className="grid gap-4">
                  <label className="grid max-w-sm gap-2 text-sm font-medium">
                    Canonical Slug
                    <Select
                      aria-label="Canonical Slug"
                      onChange={(event) =>
                        setCanonicalByGroup((current) => ({
                          ...current,
                          [group.id]: event.target.value,
                        }))
                      }
                      value={canonicalSlug}
                    >
                      {group.slugs.map((slug) => (
                        <option key={slug} value={slug}>
                          {slug}
                        </option>
                      ))}
                    </Select>
                  </label>
                  <div className="flex flex-wrap gap-2">
                    <Button
                      disabled={mergeGroup.isPending}
                      onClick={() =>
                        mergeGroup.mutateAsync({
                          projectId,
                          groupId: group.id,
                          canonicalSlug,
                        })
                      }
                    >
                      Merge
                    </Button>
                    <Button
                      disabled={dismissGroup.isPending}
                      onClick={() => dismissGroup.mutateAsync({ projectId, groupId: group.id })}
                      variant="outline"
                    >
                      Not Duplicates
                    </Button>
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      ) : (
        <EmptyState
          description="Run detection to scan wiki pages for duplicate candidates."
          title="No duplicate candidates"
        />
      )}

      {notDuplicates.length ? (
        <Card>
          <CardHeader>
            <CardTitle>Dismissed Pairs</CardTitle>
            <CardDescription>These slug groups will not be reported as duplicates again.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-2">
            {notDuplicates.map((slugs) => (
              <Badge key={slugs.join(",")} variant="outline">
                {slugs.join(", ")}
              </Badge>
            ))}
          </CardContent>
        </Card>
      ) : null}
    </PageSection>
  );
}
