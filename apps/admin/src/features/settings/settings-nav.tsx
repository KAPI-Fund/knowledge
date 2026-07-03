import { cn } from "@/lib/utils";

export type SettingsSectionId = "llm" | "embedding" | "image" | "search" | "defaults";

const ITEMS: { id: SettingsSectionId; label: string }[] = [
  { id: "llm", label: "LLM" },
  { id: "embedding", label: "Embedding" },
  { id: "image", label: "Image" },
  { id: "search", label: "Web Search" },
  { id: "defaults", label: "Defaults" },
];

interface SettingsNavProps {
  active: SettingsSectionId;
  onSelect: (id: SettingsSectionId) => void;
}

export function SettingsNav({ active, onSelect }: SettingsNavProps) {
  return (
    <nav aria-label="Settings sections" className="flex flex-col gap-1">
      {ITEMS.map((item) => (
        <button
          key={item.id}
          type="button"
          aria-current={active === item.id ? "page" : undefined}
          onClick={() => onSelect(item.id)}
          className={cn(
            "rounded-md px-3 py-2 text-left text-sm font-medium transition-colors",
            active === item.id
              ? "bg-accent text-accent-foreground"
              : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
          )}
        >
          {item.label}
        </button>
      ))}
    </nav>
  );
}
