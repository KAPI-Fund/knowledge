import { lazy, Suspense } from "react";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";

import { AppShell } from "../components/layout/app-shell";
import { AuthGuard } from "../components/layout/auth-guard";
import { ProjectWorkspaceLayout } from "../components/layout/project-workspace-layout";
import { ApiTokensPage } from "../features/api-tokens/page";
import { LoginPage } from "../features/auth/login-page";
import { AuditPage } from "../features/audit/page";
import { ChatPage } from "../features/chat/page";
import { DashboardPage } from "../features/dashboard/page";
import { DedupPage } from "../features/dedup/page";
import { DeepResearchPage } from "../features/deep-research/page";
import { FilesPage } from "../features/files/page";
import { LintPage } from "../features/lint/page";
import { OrgMembersPage } from "../features/orgs/members-page";
import { OrgWorkspacePage } from "../features/orgs/workspace-page";
import { ProjectDetailPage } from "../features/projects/detail-page";
import { ProjectsPage } from "../features/projects/page";
import { ReviewsPage } from "../features/reviews/page";
import { SearchPage } from "../features/search/page";
import { SettingsPage } from "../features/settings/page";
import { SourceWatchPage } from "../features/source-watch/page";
import { SourcesPage } from "../features/sources/page";
import { TasksPage } from "../features/tasks/page";
import { TeamPage } from "../features/teams/team-page";
import { UsersPage } from "../features/users/page";

const GraphPage = lazy(() =>
  import("../features/graph/page").then((module) => ({ default: module.GraphPage })),
);

export function AppRoutes() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route element={<AuthGuard />}>
        <Route path="/" element={<AppShell />}>
          <Route index element={<DashboardPage />} />
          <Route path="projects" element={<ProjectsPage />} />
          <Route path="projects/:projectId" element={<ProjectWorkspaceLayout />}>
            <Route index element={<ProjectDetailPage />} />
            <Route path="files" element={<FilesPage />} />
            <Route path="sources" element={<SourcesPage />} />
            <Route path="source-watch" element={<SourceWatchPage />} />
            <Route path="search" element={<SearchPage />} />
            <Route path="chat" element={<ChatPage />} />
            <Route path="lint" element={<LintPage />} />
            <Route
              path="graph"
              element={
                <Suspense
                  fallback={<div className="p-6 text-sm text-muted-foreground">Loading graph…</div>}
                >
                  <GraphPage />
                </Suspense>
              }
            />
            <Route path="tasks" element={<TasksPage />} />
            <Route path="reviews" element={<ReviewsPage />} />
            <Route path="dedup" element={<DedupPage />} />
            <Route path="deep-research" element={<DeepResearchPage />} />
            <Route path="audit" element={<AuditPage />} />
          </Route>
          <Route path="orgs/:orgId" element={<OrgWorkspacePage />} />
          <Route path="orgs/:orgId/members" element={<OrgMembersPage />} />
          <Route path="orgs/:orgId/teams/:teamId" element={<TeamPage />} />
          <Route path="users" element={<UsersPage />} />
          <Route path="api-tokens" element={<ApiTokensPage />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
      </Route>
      <Route path="*" element={<Navigate replace to="/" />} />
    </Routes>
  );
}

export function AppRouter() {
  return (
    <BrowserRouter>
      <AppRoutes />
    </BrowserRouter>
  );
}
