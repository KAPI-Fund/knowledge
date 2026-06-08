use knowledge_core::project::queries::{save_query_page, SaveQueryPageInput, SavedQueryCitation};
use knowledge_core::project::scaffold::initialize_project;
use tempfile::tempdir;

#[test]
fn save_query_page_writes_frontmatter_body_index_and_log() {
  let temp = tempdir().unwrap();
  let root = initialize_project(temp.path()).unwrap();
  let result = save_query_page(
    &root,
    SaveQueryPageInput {
      title: "Attention Notes".to_string(),
      slug: "attention-notes".to_string(),
      answer: "Attention focuses computation on relevant tokens.".to_string(),
      citations: vec![SavedQueryCitation {
        path: "wiki/concepts/attention.md".to_string(),
        title: "Attention".to_string(),
      }],
      context_summary: "wiki/concepts/attention.md (Attention)".to_string(),
    },
  )
  .unwrap();

  assert_eq!(result.relative_path, "wiki/queries/attention-notes.md");

  let page = std::fs::read_to_string(temp.path().join("wiki/queries/attention-notes.md")).unwrap();
  assert!(page.contains("type: query"));
  assert!(page.contains("title: Attention Notes"));
  assert!(page.contains("Attention focuses computation on relevant tokens."));
  assert!(page.contains("[[concepts/attention]]"));

  let index = std::fs::read_to_string(temp.path().join("wiki/index.md")).unwrap();
  assert!(index.contains("[[queries/attention-notes]]"));

  let log = std::fs::read_to_string(temp.path().join("wiki/log.md")).unwrap();
  assert!(log.contains("query | Attention Notes"));
}
