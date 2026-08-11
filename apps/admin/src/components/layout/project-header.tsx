import { Card, CardContent } from "@/components/ui/card";

export function ProjectHeader({
  name,
  rootPath,
  sourceCount,
  taskCount,
  reviewCount,
}: {
  name: string;
  rootPath: string;
  sourceCount: number;
  taskCount: number;
  reviewCount: number;
}) {
  return (
    <Card>
      <CardContent className="flex flex-wrap items-start justify-between gap-4 p-6">
        <div className="space-y-1">
          <h1 className="text-2xl font-semibold tracking-tight">{name}</h1>
          <p className="text-sm text-muted-foreground">{rootPath}</p>
        </div>
        <div className="flex flex-wrap gap-3 text-sm text-muted-foreground">
          <span>{sourceCount} sources</span>
          <span>{taskCount} tasks</span>
          <span>{reviewCount} reviews</span>
        </div>
      </CardContent>
    </Card>
  );
}
