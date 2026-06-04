import { NavLink, Outlet } from "react-router-dom";

export function AppShell() {
  return (
    <div className="shell">
      <aside className="sidebar">
        <NavLink to="/">Dashboard</NavLink>
        <NavLink to="/projects">Projects</NavLink>
        <NavLink to="/users">Users</NavLink>
      </aside>
      <main className="content">
        <Outlet />
      </main>
    </div>
  );
}
