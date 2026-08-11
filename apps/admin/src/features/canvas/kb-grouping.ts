export interface KbItem {
  id: string;
  name: string;
  role: "owner" | "editor" | "viewer";
  groupKey: string;
  groupLabel: string;
  groupOrder: number;
}

export interface KbGroup {
  key: string;
  label: string;
  items: KbItem[];
}

/**
 * Group accessible knowledge bases for display and apply a name filter.
 *
 * Items are bucketed by `groupKey`, groups are ordered by `groupOrder`
 * (personal first, then each org followed by its teams), and items inside a
 * group are sorted by name. The query is a case-insensitive substring match on
 * the item name; empty/whitespace queries pass everything through, and groups
 * with no surviving items are dropped.
 */
export function groupAndFilter(items: KbItem[], query: string): KbGroup[] {
  const q = query.trim().toLowerCase();
  const matched = q ? items.filter((i) => i.name.toLowerCase().includes(q)) : items;

  const byKey = new Map<string, KbGroup & { order: number }>();
  for (const item of matched) {
    let group = byKey.get(item.groupKey);
    if (!group) {
      group = { key: item.groupKey, label: item.groupLabel, order: item.groupOrder, items: [] };
      byKey.set(item.groupKey, group);
    }
    group.items.push(item);
  }

  return [...byKey.values()]
    .sort((a, b) => a.order - b.order)
    .map(({ key, label, items: groupItems }) => ({
      key,
      label,
      items: [...groupItems].sort((a, b) => a.name.localeCompare(b.name)),
    }));
}
