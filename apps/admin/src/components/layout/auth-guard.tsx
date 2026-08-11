import { Navigate, Outlet, useLocation } from "react-router-dom";

import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { useSession } from "@/features/auth/use-session";

export function AuthGuard() {
  const location = useLocation();
  const session = useSession();

  if (session.isLoading) {
    return (
      <main className="grid min-h-svh place-items-center px-6 py-10">
        <Card className="w-full max-w-md">
          <CardContent className="space-y-3 p-6">
            <Skeleton className="h-5 w-24" />
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
          </CardContent>
        </Card>
      </main>
    );
  }

  if (!session.user) {
    return <Navigate replace state={{ from: location }} to="/login" />;
  }

  return <Outlet />;
}
