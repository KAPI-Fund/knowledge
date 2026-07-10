import { FolderKanban, Moon, Sun, SunMoon } from "lucide-react";
import { useTheme } from "next-themes";
import { useCallback, useEffect, useState } from "react";
import { useMatch, useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import { useProjectsQuery } from "@/features/projects/queries";
import { globalNav, projectNavGroups } from "@/lib/route-meta";

export function CommandMenu() {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const { setTheme } = useTheme();

  const nestedMatch = useMatch("/projects/:projectId/*");
  const exactMatch = useMatch("/projects/:projectId");
  const projectId = (nestedMatch ?? exactMatch)?.params.projectId ?? null;

  const projects = useProjectsQuery();

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "k" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        setOpen((value) => !value);
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  const run = useCallback((action: () => void) => {
    setOpen(false);
    action();
  }, []);

  return (
    <>
      <Button
        variant="outline"
        size="sm"
        className="relative h-8 w-full justify-start gap-2 pr-1.5 text-sm font-normal text-muted-foreground shadow-none sm:w-40 lg:w-56"
        onClick={() => setOpen(true)}
      >
        <span className="truncate">Search...</span>
        <kbd className="pointer-events-none absolute top-1.5 right-1.5 hidden h-5 items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] font-medium text-muted-foreground select-none sm:flex">
          <span className="text-xs">⌘</span>K
        </kbd>
      </Button>
      <CommandDialog open={open} onOpenChange={setOpen}>
        <CommandInput placeholder="Type a command or search..." />
        <CommandList>
          <CommandEmpty>No results found.</CommandEmpty>
          <CommandGroup heading="Pages">
            {globalNav.map((item) => (
              <CommandItem key={item.to} onSelect={() => run(() => navigate(item.to))}>
                <item.icon />
                {item.label}
              </CommandItem>
            ))}
          </CommandGroup>
          {projectId ? (
            <>
              <CommandSeparator />
              <CommandGroup heading="Project">
                {projectNavGroups
                  .flatMap((group) => group.items)
                  .map((item) => (
                    <CommandItem
                      key={item.suffix || "overview"}
                      onSelect={() => run(() => navigate(`/projects/${projectId}${item.suffix}`))}
                    >
                      <item.icon />
                      {item.label}
                    </CommandItem>
                  ))}
              </CommandGroup>
            </>
          ) : null}
          {projects.data?.length ? (
            <>
              <CommandSeparator />
              <CommandGroup heading="Projects">
                {projects.data.map((project) => (
                  <CommandItem
                    key={project.id}
                    value={`project ${project.name}`}
                    onSelect={() => run(() => navigate(`/projects/${project.id}`))}
                  >
                    <FolderKanban />
                    {project.name}
                  </CommandItem>
                ))}
              </CommandGroup>
            </>
          ) : null}
          <CommandSeparator />
          <CommandGroup heading="Theme">
            <CommandItem onSelect={() => run(() => setTheme("light"))}>
              <Sun />
              Light
            </CommandItem>
            <CommandItem onSelect={() => run(() => setTheme("dark"))}>
              <Moon />
              Dark
            </CommandItem>
            <CommandItem onSelect={() => run(() => setTheme("system"))}>
              <SunMoon />
              System
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </CommandDialog>
    </>
  );
}
