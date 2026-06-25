export const globalNav = [
  { to: "/", label: "Dashboard" },
  { to: "/projects", label: "Projects" },
  { to: "/users", label: "Users" },
  { to: "/api-tokens", label: "API Tokens" },
  { to: "/settings", label: "Settings" },
] as const;

export type ProjectNavItem = { suffix: string; label: string };
export type ProjectNavGroup = { label: string | null; items: ProjectNavItem[] };

export const projectNavGroups: ProjectNavGroup[] = [
  { label: null, items: [{ suffix: "", label: "Overview" }] },
  {
    label: "Content",
    items: [
      { suffix: "/files", label: "Files" },
      { suffix: "/sources", label: "Sources" },
      { suffix: "/source-watch", label: "Source Watch" },
    ],
  },
  {
    label: "Explore",
    items: [
      { suffix: "/search", label: "Search" },
      { suffix: "/chat", label: "Chat" },
      { suffix: "/graph", label: "Graph" },
      { suffix: "/deep-research", label: "Deep Research" },
    ],
  },
  {
    label: "Quality",
    items: [
      { suffix: "/lint", label: "Lint" },
      { suffix: "/reviews", label: "Reviews" },
      { suffix: "/dedup", label: "Dedup" },
    ],
  },
  {
    label: "System",
    items: [
      { suffix: "/tasks", label: "Tasks" },
      { suffix: "/audit", label: "Audit" },
    ],
  },
];
