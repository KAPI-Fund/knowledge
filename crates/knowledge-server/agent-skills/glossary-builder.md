---
name: glossary-builder
description: Extract domain terms from wiki pages and sources and build or update a glossary page with concise, cited definitions
---
You are building or updating a glossary for this project's knowledge base.

Workflow:
1. **Scope.** If the user named a topic, restrict extraction to it; otherwise cover the whole project. Check with `wiki.search` whether a glossary page already exists — if so, you are updating it, not starting over: read it first with `wiki.read_page`.
2. **Extract terms.** Scan the relevant material via `wiki.search` + `wiki.read_page` and `source.search`. Collect domain-specific terms, acronyms, named concepts, and entities that a newcomer would need defined. Use `graph.search` to catch important entities you might have missed. Skip everyday words.
3. **Define.** For each term, write 1–3 sentences: what it is, why it matters in this project, and (for acronyms) the expansion. Definitions must be grounded in what the knowledge base actually says — cite the page or source each definition came from. If the knowledge base uses a term inconsistently, note both usages.
4. **Organize.** Alphabetical within the reader's language conventions; group by theme instead only if the user asks. Link each term to its main wiki page when one exists.
5. **Save.** Write the glossary with `wiki.write_page` (default title: `Glossary` / `术语表`, matching the wiki's language). When updating an existing glossary, preserve entries that are still valid, refresh stale ones, and append new ones — list at the end of your reply which entries were added, changed, or kept.

Keep each definition self-contained: a reader should understand it without opening the cited page.
