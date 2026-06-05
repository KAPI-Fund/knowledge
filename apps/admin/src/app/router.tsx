import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";

import { AppShell } from "../components/layout/app-shell";
import { LoginPage } from "../features/auth/login-page";
import { AuditPage } from "../features/audit/page";
import { DashboardPage } from "../features/dashboard/page";
import { GraphPage } from "../features/graph/page";
import { ProjectDetailPage } from "../features/projects/detail-page";
import { ProjectsPage } from "../features/projects/page";
import { ReviewsPage } from "../features/reviews/page";
import { SearchPage } from "../features/search/page";
import { SettingsPage } from "../features/settings/page";
import { SourcesPage } from "../features/sources/page";
import { TasksPage } from "../features/tasks/page";
import { UsersPage } from "../features/users/page";

export function AppRouter() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="/" element={<AppShell />}>
          <Route index element={<DashboardPage />} />
          <Route path="projects" element={<ProjectsPage />} />
          <Route path="projects/:projectId" element={<ProjectDetailPage />} />
          <Route path="projects/:projectId/sources" element={<SourcesPage />} />
          <Route path="projects/:projectId/search" element={<SearchPage />} />
          <Route path="projects/:projectId/graph" element={<GraphPage />} />
          <Route path="projects/:projectId/tasks" element={<TasksPage />} />
          <Route path="projects/:projectId/reviews" element={<ReviewsPage />} />
          <Route path="projects/:projectId/audit" element={<AuditPage />} />
          <Route path="users" element={<UsersPage />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}
