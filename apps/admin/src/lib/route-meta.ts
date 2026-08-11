import {
  ClipboardCheck,
  CopyX,
  Database,
  Eye,
  FileText,
  FolderKanban,
  Frame,
  KeyRound,
  LayoutDashboard,
  ListTodo,
  MessagesSquare,
  ScrollText,
  Search,
  SearchCheck,
  Settings2,
  Telescope,
  Users,
  Waypoints,
  type LucideIcon,
} from "lucide-react";

export type GlobalNavItem = { to: string; label: string; icon: LucideIcon };

export const globalNav: GlobalNavItem[] = [
  { to: "/", label: "Dashboard", icon: LayoutDashboard },
  { to: "/projects", label: "Projects", icon: FolderKanban },
  { to: "/canvas", label: "Canvas", icon: Frame },
  { to: "/users", label: "Users", icon: Users },
  { to: "/api-tokens", label: "API Tokens", icon: KeyRound },
  { to: "/settings", label: "Settings", icon: Settings2 },
];

export type ProjectNavItem = { suffix: string; label: string; icon: LucideIcon };
export type ProjectNavGroup = { label: string | null; items: ProjectNavItem[] };

export const projectNavGroups: ProjectNavGroup[] = [
  { label: null, items: [{ suffix: "", label: "Overview", icon: LayoutDashboard }] },
  {
    label: "Content",
    items: [
      { suffix: "/files", label: "Files", icon: FileText },
      { suffix: "/sources", label: "Sources", icon: Database },
      { suffix: "/source-watch", label: "Source Watch", icon: Eye },
    ],
  },
  {
    label: "Explore",
    items: [
      { suffix: "/search", label: "Search", icon: Search },
      { suffix: "/chat", label: "Chat", icon: MessagesSquare },
      { suffix: "/graph", label: "Graph", icon: Waypoints },
      { suffix: "/deep-research", label: "Deep Research", icon: Telescope },
    ],
  },
  {
    label: "Quality",
    items: [
      { suffix: "/lint", label: "Lint", icon: SearchCheck },
      { suffix: "/reviews", label: "Reviews", icon: ClipboardCheck },
      { suffix: "/dedup", label: "Dedup", icon: CopyX },
    ],
  },
  {
    label: "System",
    items: [
      { suffix: "/tasks", label: "Tasks", icon: ListTodo },
      { suffix: "/audit", label: "Audit", icon: ScrollText },
    ],
  },
];

const staticLabels: Record<string, string> = {
  "": "Dashboard",
  projects: "Projects",
  canvas: "Canvas",
  users: "Users",
  "api-tokens": "API Tokens",
  settings: "Settings",
  orgs: "Organizations",
  members: "Members",
  teams: "Teams",
  llm: "LLM",
  embedding: "Embedding",
  image: "Image",
  fetch: "Web Fetch",
  defaults: "Defaults",
};

for (const group of projectNavGroups) {
  for (const item of group.items) {
    if (item.suffix) staticLabels[item.suffix.slice(1)] = item.label;
  }
}

export function routeLabel(segment: string): string | null {
  return staticLabels[segment] ?? null;
}
