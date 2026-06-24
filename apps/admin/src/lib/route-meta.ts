export const systemRoutes = [
  { to: "/", label: "Dashboard" },
  { to: "/projects", label: "Projects" },
  { to: "/users", label: "Users" },
  { to: "/api-tokens", label: "API Tokens" },
  { to: "/settings", label: "Settings" },
] as const;

export const projectRoutes = [
  { suffix: "", label: "Overview" },
  { suffix: "/files", label: "Files" },
  { suffix: "/sources", label: "Sources" },
  { suffix: "/source-watch", label: "Source Watch" },
  { suffix: "/search", label: "Search" },
  { suffix: "/chat", label: "Chat" },
  { suffix: "/lint", label: "Lint" },
  { suffix: "/graph", label: "Graph" },
  { suffix: "/tasks", label: "Tasks" },
  { suffix: "/reviews", label: "Reviews" },
  { suffix: "/dedup", label: "Dedup" },
  { suffix: "/deep-research", label: "Deep Research" },
  { suffix: "/audit", label: "Audit" },
] as const;
