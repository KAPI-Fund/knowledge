import { CopyX, Merge } from "lucide-react";
import { useState } from "react";
import { useParams } from "react-router-dom";
import { toast } from "sonner";

import { EmptyState } from "@/components/layout/empty-state";
import { ConfirmDialog } from "@/components/shared/confirm-dialog";
import { PageHeader } from "@/components/shared/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { normalizeAppError } from "@/lib/app-error";

import {
  useDetectDuplicatesMutation,
  useDismissDuplicateGroupMutation,
  useMergeDuplicateGroupMutation,
  useProjectDedupQuery,
} from "./queries";

function ConfidenceBadge({ confidence }: { confidence: string }) {
  if (confidence === "high") {
    return <Badge variant="destructive">{confidence}</Badge>;
  }
  if (confidence === "medium") {
    return (
      <Badge className="border-transparent bg-amber-100 text-amber-900 dark:bg-amber-950 dark:text-amber-200">
        {confidence}
      </Badge>
    );
  }
  return <Badge variant="outline">{confidence}</Badge>;
}

export function DedupPage() {
  const { projectId = "" } = useParams();
  const [canonicalByGroup, setCanonicalByGroup] = useState<Record<string, string>>({});
  const [mergeTarget, setMergeTarget] = useState<{ id: string; slugs: string[] } | null>(null);
  const dedup = useProjectDedupQuery(projectId);
  const detectDuplicates = useDetectDuplicatesMutation();
  const mergeGroup = useMergeDuplicateGroupMutation();
  const dismissGroup = useDismissDuplicateGroupMutation();

  const groups = dedup.data?.groups ?? [];
  const notDuplicates = dedup.data?.notDuplicates ?? [];
  const mergeCanonical = mergeTarget
    ? canonicalByGroup[mergeTarget.id] ?? mergeTarget.slugs[0] ?? ""
    : "";

  return (
    <div className="grid gap-6">
      <PageHeader
        actions={
          <Button
            disabled={detectDuplicates.isPending}
            onClick={async () => {
              try {
                await detectDuplicates.mutateAsync({ projectId });
                toast.success("Duplicate detection queued.");
              } catch (error) {
                toast.error(normalizeAppError(error).message);
              }
            }}
          >
            <CopyX />
            Detect Duplicates
          </Button>
        }
        description="Detect duplicate wiki pages, then merge them into one canonical page or dismiss false positives."
        title="Dedup"
      />

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
                    <ConfidenceBadge confidence={group.confidence} />
                  </div>
                </CardHeader>
                <CardContent className="grid gap-4">
                  <div className="grid max-w-sm gap-2">
                    <Label htmlFor={`canonical-${group.id}`}>Canonical Slug</Label>
                    <Select
                      onValueChange={(value) =>
                        setCanonicalByGroup((current) => ({
                          ...current,
                          [group.id]: value,
                        }))
                      }
                      value={canonicalSlug}
                    >
                      <SelectTrigger
                        id={`canonical-${group.id}`}
                        aria-label="Canonical Slug"
                        className="w-full"
                      >
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {group.slugs.map((slug) => (
                          <SelectItem key={slug} value={slug}>
                            {slug}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <Button
                      disabled={mergeGroup.isPending}
                      onClick={() => setMergeTarget({ id: group.id, slugs: group.slugs })}
                    >
                      <Merge />
                      Merge
                    </Button>
                    <Button
                      disabled={dismissGroup.isPending}
                      onClick={async () => {
                        try {
                          await dismissGroup.mutateAsync({ projectId, groupId: group.id });
                          toast.success("Group dismissed.");
                        } catch (error) {
                          toast.error(normalizeAppError(error).message);
                        }
                      }}
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
          icon={CopyX}
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

      <ConfirmDialog
        confirmLabel="Merge pages"
        description={
          mergeTarget
            ? `${mergeTarget.slugs.filter((slug) => slug !== mergeCanonical).join(", ")} will be merged into "${mergeCanonical}" and the duplicates removed.`
            : ""
        }
        destructive
        isPending={mergeGroup.isPending}
        onConfirm={async () => {
          if (!mergeTarget) return;
          try {
            await mergeGroup.mutateAsync({
              projectId,
              groupId: mergeTarget.id,
              canonicalSlug: mergeCanonical,
            });
            toast.success("Merge queued.");
          } catch (error) {
            toast.error(normalizeAppError(error).message);
          }
        }}
        onOpenChange={(open) => {
          if (!open) setMergeTarget(null);
        }}
        open={Boolean(mergeTarget)}
        title="Merge these pages?"
      />
    </div>
  );
}
