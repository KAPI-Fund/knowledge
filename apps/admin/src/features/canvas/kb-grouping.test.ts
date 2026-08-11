import { describe, expect, it } from "vitest";

import { type KbItem, groupAndFilter } from "./kb-grouping";

function item(overrides: Partial<KbItem> & Pick<KbItem, "id" | "name">): KbItem {
  return {
    role: "viewer",
    groupKey: "personal",
    groupLabel: "Personal",
    groupOrder: 0,
    ...overrides,
  };
}

describe("groupAndFilter", () => {
  it("groups items by groupKey and orders groups by groupOrder", () => {
    const groups = groupAndFilter(
      [
        item({ id: "a", name: "Alpha", groupKey: "org:1", groupLabel: "Acme", groupOrder: 1 }),
        item({ id: "b", name: "Beta", groupKey: "personal", groupLabel: "Personal", groupOrder: 0 }),
      ],
      "",
    );
    expect(groups.map((g) => g.key)).toEqual(["personal", "org:1"]);
    expect(groups.map((g) => g.label)).toEqual(["Personal", "Acme"]);
  });

  it("sorts items within a group by name", () => {
    const [group] = groupAndFilter(
      [
        item({ id: "a", name: "Zebra" }),
        item({ id: "b", name: "Apple" }),
        item({ id: "c", name: "Mango" }),
      ],
      "",
    );
    expect(group.items.map((i) => i.name)).toEqual(["Apple", "Mango", "Zebra"]);
  });

  it("filters by case-insensitive name substring", () => {
    const groups = groupAndFilter(
      [
        item({ id: "a", name: "Marketing KB" }),
        item({ id: "b", name: "Engineering docs" }),
      ],
      "mark",
    );
    expect(groups).toHaveLength(1);
    expect(groups[0].items.map((i) => i.id)).toEqual(["a"]);
  });

  it("returns all items when the query is empty or whitespace", () => {
    const items = [item({ id: "a", name: "Alpha" }), item({ id: "b", name: "Beta" })];
    expect(groupAndFilter(items, "   ").flatMap((g) => g.items)).toHaveLength(2);
  });

  it("returns no groups when nothing matches", () => {
    const groups = groupAndFilter([item({ id: "a", name: "Alpha" })], "zzz");
    expect(groups).toEqual([]);
  });

  it("drops a group entirely when none of its items match", () => {
    const groups = groupAndFilter(
      [
        item({ id: "a", name: "Alpha", groupKey: "personal", groupOrder: 0 }),
        item({ id: "b", name: "Beta", groupKey: "org:1", groupLabel: "Acme", groupOrder: 1 }),
      ],
      "beta",
    );
    expect(groups.map((g) => g.key)).toEqual(["org:1"]);
  });
});
