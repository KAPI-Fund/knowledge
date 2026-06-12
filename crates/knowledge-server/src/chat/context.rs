use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use knowledge_core::chat::{compute_context_budget, is_greeting};
use knowledge_core::project::root::ProjectRoot;
use knowledge_core::retrieval_graph::{build_retrieval_graph, related_nodes};
use knowledge_core::search::SearchOptions;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::retrieval::service::search_project_hybrid;

const RELATED_PAGE_LIMIT: usize = 3;

#[derive(Debug, Default)]
pub struct AssembledContext {
    pub context_blocks: Vec<String>,
    pub context_summary: Option<String>,
}

pub async fn assemble_chat_context(
    state: &AppState,
    project_id: &str,
    root: &ProjectRoot,
    user_text: &str,
    top_k: usize,
) -> Result<AssembledContext, ApiError> {
    // Upstream chat-panel short-circuits retrieval for pure greetings.
    if is_greeting(user_text) {
        return Ok(AssembledContext::default());
    }

    let budget = compute_context_budget(None);
    let results = search_project_hybrid(
        state,
        project_id,
        root,
        user_text,
        SearchOptions {
            top_k,
            include_content: true,
        },
    )
    .await?;

    let mut used = 0usize;
    let mut blocks = Vec::new();
    let mut summary = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for result in &results.results {
        let content = result
            .content
            .clone()
            .unwrap_or_else(|| result.snippet.clone());
        let truncated: String = content.chars().take(budget.max_page_size).collect();
        let cost = truncated.chars().count();
        // Upstream tryAddPage skips pages that overflow instead of stopping.
        if used + cost > budget.page_budget {
            continue;
        }
        used += cost;
        seen.insert(page_id_from_path(&result.path));
        blocks.push(format!(
            "[{}] {}\nTitle: {}\n{}",
            blocks.len() + 1,
            result.path,
            result.title,
            truncated
        ));
        summary.push(format!("{} ({})", result.path, result.title));
    }

    // Graph expansion from the top hit (upstream's related-page enrichment).
    if let Some(top) = results.results.first() {
        let wiki_root = root.as_path().join("wiki");
        let graph = build_retrieval_graph(&wiki_root);
        for (related_id, _score) in
            related_nodes(&graph, &page_id_from_path(&top.path), RELATED_PAGE_LIMIT)
        {
            if seen.contains(&related_id) {
                continue;
            }
            let Some(node) = graph.nodes.get(&related_id) else {
                continue;
            };
            let Ok(content) = fs::read_to_string(wiki_root.join(&node.relative_path)) else {
                continue;
            };
            let truncated: String = content.chars().take(budget.max_page_size).collect();
            let cost = truncated.chars().count();
            if used + cost > budget.page_budget {
                continue;
            }
            used += cost;
            seen.insert(related_id);
            blocks.push(format!(
                "[{}] wiki/{}\nTitle: {}\n{}",
                blocks.len() + 1,
                node.relative_path,
                node.title,
                truncated
            ));
            summary.push(format!("wiki/{} ({})", node.relative_path, node.title));
        }
    }

    let context_summary = if summary.is_empty() {
        None
    } else {
        Some(summary.join("; "))
    };

    Ok(AssembledContext {
        context_blocks: blocks,
        context_summary,
    })
}

fn page_id_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string()
}
