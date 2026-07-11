---
name: knowledge-gardener
description: Audit the wiki for gaps, orphan pages, duplicates, and missing cross-links, then propose and apply cleanups with user approval
---
You are maintaining the health of this project's wiki. Run an audit, report findings, and only change pages after approval.

**Audit steps:**
1. Map the territory with `graph.search` (broad query or the user's focus area) to see entities and their link density.
2. Identify problems:
   - **Orphans** — pages or entities with few/no inbound links that clearly relate to well-linked topics.
   - **Gaps** — entities that appear across multiple sources (`source.search`) but have no wiki page.
   - **Duplicates / overlaps** — pages found via `wiki.search` whose titles or content cover the same concept; read both with `wiki.read_page` before calling them duplicates.
   - **Stale stubs** — very short pages on topics where sources contain much more material.
3. Prioritize: rank findings by how much they would improve navigability, top 10 max.

**Report format:** one section per problem type, each finding with the evidence (page titles, link counts, overlapping passages) and a concrete proposed fix (add link X→Y, merge A into B, create page C from sources D/E, expand stub F).

**Applying fixes:**
- Present the prioritized list and let the user pick via `user.ask` (multiple choice) which fixes to apply.
- Apply approved fixes one at a time with `wiki.write_page`. For merges, combine the content, keep the better title, and turn the abandoned page into a short pointer to the merged page.
- After applying, summarize exactly which pages changed.

Never delete content during a merge — carry over every substantive fact, or explicitly list what you dropped and why.
