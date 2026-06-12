import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";

import { AppShell } from "../components/layout/app-shell";
import { AuthGuard } from "../components/layout/auth-guard";
import { ProjectWorkspaceLayout } from "../components/layout/project-workspace-layout";
import { LoginPage } from "../features/auth/login-page";
import { AuditPage } from "../features/audit/page";
import { ChatPage } from "../features/chat/page";
import { DashboardPage } from "../features/dashboard/page";
import { FilesPage } from "../features/files/page";
import { GraphPage } from "../features/graph/page";
import { LintPage } from "../features/lint/page";
import { ProjectDetailPage } from "../features/projects/detail-page";
import { ProjectsPage } from "../features/projects/page";
import { QueryPage } from "../features/query/page";
import { ReviewsPage } from "../features/reviews/page";
import { SearchPage } from "../features/search/page";
import { SettingsPage } from "../features/settings/page";
import { SourceWatchPage } from "../features/source-watch/page";
import { SourcesPage } from "../features/sources/page";
import { TasksPage } from "../features/tasks/page";
import { UsersPage } from "../features/users/page";

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
            <Route path="query" element={<QueryPage />} />
            <Route path="chat" element={<ChatPage />} />
            <Route path="lint" element={<LintPage />} />
            <Route path="graph" element={<GraphPage />} />
            <Route path="tasks" element={<TasksPage />} />
            <Route path="reviews" element={<ReviewsPage />} />
            <Route path="audit" element={<AuditPage />} />
          </Route>
          <Route path="users" element={<UsersPage />} />
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
