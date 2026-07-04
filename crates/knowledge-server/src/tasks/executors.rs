use std::collections::BTreeSet;

use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::app::state::AppState;
use crate::deep_research::collect_research_sources;
use crate::multimodal::{
  caption_image, inject_images_into_source_summary, load_cached_caption, save_cached_caption,
};
use crate::http::error::ApiError;
use crate::providers::{OpenAiCompatibleProvider, ProviderError, ProviderTextRequest};
use crate::projects::audit::{append_audit_log, CreateAuditLog};
use crate::projects::service::project_root_for_id;
use crate::web_search::config::load_web_search_config;
use crate::retrieval::dedup::{
  select_dedup_candidate_pages, DEDUP_MAX_CANDIDATE_PAGES, DEDUP_SIMILARITY_THRESHOLD,
};
use crate::retrieval::service::{ensure_project_embeddings, load_embedding_config};
use crate::retrieval::store::{delete_pages, load_project_chunks};
use crate::tasks::model::TaskRecord;
use crate::tasks::store;
use knowledge_core::ingest::{
  AnalysisResult, analyze_source, build_analysis_prompt, build_analysis_user_prompt,
  build_generation_prompt, build_generation_user_prompt, build_review_suggestion_prompt,
  check_ingest_cache, generate_wiki_from_analysis, parse_generation_file_blocks,
  render_generation_file_blocks, should_run_dedicated_review_stage, source_summary_path,
};
use knowledge_core::project::dedup::{
  build_detector_user_message, build_merger_user_message, collect_all_wiki_pages,
  collect_entity_pages, compute_dedup_merge, extract_entity_summary, filter_detected_groups,
  parse_detector_response, DedupBackupEntry, DETECTOR_SYSTEM_PROMPT, MERGER_SYSTEM_PROMPT,
};
use knowledge_core::project::dedup_store::{
  load_dedup_store, load_not_duplicates, save_dedup_store, DedupGroup, DedupStore,
};
use knowledge_core::project::page_merge::{
  build_page_merge_prompts, finalize_page_merge, prepare_page_merge, PageMergePlan,
};
use knowledge_core::project::wiki_pages::{delete_wiki_pages_with_refs, save_wiki_page};
use knowledge_core::project::lint::{
  build_semantic_lint_prompt, parse_semantic_lint_response, run_structural_lint,
};
use knowledge_core::project::research::{
  render_research_page, RenderResearchPageInput, RenderResearchReference,
};
use knowledge_core::project::reviews::{
  build_review_sweep_prompt, parse_review_resolution_ids, resolve_review_ids,
  sweep_resolved_reviews, update_review_status,
};
use knowledge_core::project::source_text::read_source_text;
use knowledge_core::project::multimodal::{
  build_image_markdown_section, extract_office_images, extract_pdf_images, save_extracted_images,
  ExtractOptions,
};
use knowledge_core::project::sources::{delete_source, import_source, rescan_sources};

const REVIEW_SWEEP_MAX_PAGES: usize = 300;
const REVIEW_SWEEP_SYSTEM_PROMPT: &str = "You judge whether stale wiki review items have already been resolved by the current wiki state. Return JSON only.";

pub async fn run_task_executor(state: &AppState, task: &TaskRecord) -> Result<(), ApiError> {
  let task = store::mark_task_running_attempt(state, &task.id).await?;

  let result: Result<Value, TaskExecutionError> = match task.task_type.as_str() {
    "project.import_source" => run_import_source_executor(state, &task).await.map_err(Into::into),
    "project.rescan_sources" => run_rescan_sources_executor(state, &task).await.map_err(Into::into),
    "project.delete_source" => run_delete_source_executor(state, &task).await.map_err(Into::into),
    "project.run_lint" => run_lint_executor(state, &task).await.map_err(Into::into),
    "project.ingest_source" => run_ingest_source_executor(state, &task).await.map_err(Into::into),
    "project.sweep_reviews" => run_manual_review_sweep_executor(state, &task).await.map_err(Into::into),
    "project.update_review" => run_update_review_executor(state, &task).await.map_err(Into::into),
    "project.dedup_detect" => run_dedup_detect_executor(state, &task).await.map_err(Into::into),
    "project.dedup_merge" => run_dedup_merge_executor(state, &task).await.map_err(Into::into),
    "project.deep_research" => run_deep_research_executor(state, &task).await.map_err(Into::into),
    _ => Err(TaskExecutionError::from(ApiError::bad_request("unsupported task type"))),
  };

  match result {
    Ok(value) => {
      store::complete_task(state, &task.id, value).await?;
      append_task_audit_log(state, &task, "task.succeeded", "Task completed").await?;
      Ok(())
    }
    Err(error) => {
      let retryable = error.retryable() && task.attempt_count < task.max_attempts;
      let error_payload = error.payload(retryable);

      if retryable {
        let next_retry_at = OffsetDateTime::now_utc()
          .checked_add(Duration::seconds(2))
          .ok_or_else(|| ApiError::internal("failed to compute retry timestamp"))?
          .format(&Rfc3339)
          .map_err(|_| ApiError::internal("failed to format retry timestamp"))?;
        store::retry_task(state, &task.id, error_payload.clone(), next_retry_at).await?;
        append_task_audit_log(state, &task, "task.retry_waiting", "Task scheduled for retry").await?;
      } else {
        store::fail_task(state, &task.id, error_payload.clone()).await?;
        append_task_audit_log(state, &task, "task.failed", "Task failed").await?;
      }

      Err(error.into_api_error())
    }
  }
}

#[derive(Debug)]
struct TaskExecutionError {
  api_error: ApiError,
  code: String,
  retryable: bool,
  provider_status: Option<u16>,
}

impl TaskExecutionError {
  fn from_provider_error(error: ProviderError) -> Self {
    let api_error = if error.retryable() {
      ApiError::internal(error.message().to_string())
    } else {
      ApiError::bad_request(error.message().to_string())
    };

    Self {
      api_error,
      code: error.code().to_string(),
      retryable: error.retryable(),
      provider_status: error.provider_status(),
    }
  }

  fn retryable(&self) -> bool {
    self.retryable
  }

  fn payload(&self, retryable: bool) -> Value {
    let mut payload = json!({
      "code": self.code,
      "message": self.api_error.to_string(),
      "retryable": retryable
    });

    if let Some(provider_status) = self.provider_status {
      payload["providerStatus"] = json!(provider_status);
    }

    payload
  }

  fn into_api_error(self) -> ApiError {
    self.api_error
  }
}

impl From<ApiError> for TaskExecutionError {
  fn from(api_error: ApiError) -> Self {
    Self {
      api_error,
      code: "task_execution_failed".to_string(),
      retryable: false,
      provider_status: None,
    }
  }
}

async fn run_import_source_executor(state: &AppState, task: &TaskRecord) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let file_name = read_string(&task.payload, "fileName")?;
  let content_base64 = read_string(&task.payload, "contentBase64")?;
  let source = import_source(&root, &file_name, &content_base64)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;

  Ok(json!({
    "relativePath": source.relative_path,
    "size": source.size
  }))
}

async fn run_rescan_sources_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let discovered_count =
    rescan_sources(&root).map_err(|error| ApiError::bad_request(error.to_string()))?;

  Ok(json!({
    "discoveredCount": discovered_count
  }))
}

async fn run_delete_source_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let relative_path = read_string(&task.payload, "relativePath")?;
  delete_source(&root, &relative_path).map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(json!({}))
}

async fn run_lint_executor(state: &AppState, task: &TaskRecord) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let mode = read_string(&task.payload, "mode")?;
  if mode == "structural" {
    let result = run_structural_lint(&root).map_err(|error| ApiError::bad_request(error.to_string()))?;
    return serde_json::to_value(result).map_err(|error| ApiError::internal(error.to_string()));
  }

  if mode == "semantic" {
    let provider = load_ingest_provider(state)
      .await?
      .ok_or_else(|| ApiError::bad_request("semantic lint requires an openai-compatible provider"))?;
    let prompt = build_semantic_lint_prompt(&root)
      .map_err(|error| ApiError::bad_request(error.to_string()))?
      .unwrap_or_default();
    let response = provider
      .complete_text(ProviderTextRequest {
        system_prompt: "You are a wiki quality analyst.".to_string(),
        user_prompt: prompt,
      })
      .await
      .map_err(TaskExecutionError::from_provider_error)
      .map_err(TaskExecutionError::into_api_error)?;
    let result = parse_semantic_lint_response(&response.text);
    return serde_json::to_value(result).map_err(|error| ApiError::internal(error.to_string()));
  }

  Err(ApiError::bad_request("unsupported lint mode"))
}

async fn run_ingest_source_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let relative_path = read_string(&task.payload, "relativePath")?;
  let source_path = root
    .safe_join(&relative_path)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  let source_identity = source_identity_from_relative_path(&relative_path);
  let content = read_source_text(&source_path).map_err(|error| ApiError::bad_request(error.to_string()))?;
  let source_name = source_path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| ApiError::bad_request("invalid source name"))?;
  let summary_title = analyze_source(source_name, &content).title;
  let ingest_provider = load_ingest_provider(state).await?;
  let source_slug = knowledge_core::project::source_identity::source_summary_slug_from_identity(
    &source_identity,
  );
  let wiki_root = root
    .safe_join("wiki")
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  let media_dir = root
    .safe_join(&format!("wiki/media/{source_slug}"))
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  let image_options = ExtractOptions::default();
  let extracted_images = match source_path
    .extension()
    .and_then(|value| value.to_str())
    .unwrap_or_default()
    .to_ascii_lowercase()
    .as_str()
  {
    "pdf" => extract_pdf_images(&source_path, &image_options).unwrap_or_default(),
    "pptx" | "docx" | "xlsx" | "xls" | "ods" => {
      extract_office_images(&source_path, &image_options).unwrap_or_default()
    }
    _ => Vec::new(),
  };
  let saved_images = save_extracted_images(&extracted_images, &media_dir, &wiki_root)
    .unwrap_or_default();
  let mut captions_by_sha = std::collections::HashMap::new();

  if let Some(provider) = ingest_provider.as_ref() {
    for (saved_image, extracted_image) in saved_images.iter().zip(extracted_images.iter()) {
      let caption = if let Some(cached) = load_cached_caption(&state.cache, &saved_image.sha256)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
      {
        cached
      } else {
        let caption = caption_image(provider, &extracted_image.data_base64, &saved_image.mime_type)
          .await
          .map_err(TaskExecutionError::from_provider_error)
          .map_err(TaskExecutionError::into_api_error)?;
        let _ = save_cached_caption(&state.cache, &saved_image.sha256, &caption).await;
        caption
      };
      captions_by_sha.insert(saved_image.sha256.clone(), caption);
    }
  }
  let image_section = build_image_markdown_section(&saved_images, Some(&captions_by_sha));
  let content = if image_section.is_empty() {
    content
  } else {
    format!("{content}\n\n{image_section}")
  };
  let summary_path = source_summary_path(&source_identity);

  let result = if let Some(cached) = check_ingest_cache(&root, &source_identity, &content)
    .map_err(|error| ApiError::bad_request(error.to_string()))?
  {
    cached
  } else if let Some(provider) = ingest_provider.clone() {
    let purpose = try_read_project_file(&root, "purpose.md");
    let schema = try_read_project_file(&root, "schema.md");
    let index = try_read_project_file(&root, "wiki/index.md");
    let overview = try_read_project_file(&root, "wiki/overview.md");
    let analysis_text = provider
      .complete_text(ProviderTextRequest {
        system_prompt: build_analysis_prompt(&purpose, &index, &content),
        user_prompt: build_analysis_user_prompt(source_name, &content),
      })
      .await
      .map_err(TaskExecutionError::from_provider_error)
      .map_err(TaskExecutionError::into_api_error)?;
    let analysis = AnalysisResult {
      title: analysis_title_from_text(source_name, &content, &analysis_text.text),
      summary_markdown: content.trim().to_string(),
      analysis: analysis_text.text,
    };
    let generation = provider
      .complete_text(ProviderTextRequest {
        system_prompt: build_generation_prompt(
          &schema,
          &purpose,
          &index,
          source_name,
          &overview,
          &content,
          &source_summary_path(&source_identity),
        ),
        user_prompt: build_generation_user_prompt(source_name, &analysis.analysis, &content),
      })
      .await
      .map_err(TaskExecutionError::from_provider_error)
      .map_err(TaskExecutionError::into_api_error)?;
    let review_suggestions = if should_run_dedicated_review_stage(&generation.text) {
      provider
        .complete_text(ProviderTextRequest {
          system_prompt: build_review_suggestion_prompt(
            &purpose,
            &index,
            &source_identity,
            &analysis.analysis,
            &content,
            &generation.text,
          ),
          user_prompt:
            "Emit only high-value REVIEW blocks for follow-up research or unresolved knowledge gaps. Output nothing if there are none."
              .to_string(),
        })
        .await
        .map(|response| response.text)
        .unwrap_or_default()
    } else {
      String::new()
    };
    let combined_generation = if review_suggestions.trim().is_empty() {
      generation.text
    } else {
      format!("{}\n\n{}", generation.text.trim_end(), review_suggestions.trim())
    };
    let merged_generation = merge_generated_pages_with_existing_content(
      &provider,
      &root,
      source_name,
      &combined_generation,
    )
    .await?;

    generate_wiki_from_analysis(
      &root,
      &source_identity,
      source_name,
      &content,
      &analysis,
      &merged_generation,
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?
  } else {
    let analysis = analyze_source(source_name, &content);
    generate_wiki_from_analysis(&root, &source_identity, source_name, &content, &analysis, "")
      .map_err(|error| ApiError::bad_request(error.to_string()))?
  };

  let _ = inject_images_into_source_summary(
    root.as_path(),
    &summary_path,
    &source_identity,
    &summary_title,
    &saved_images,
    &captions_by_sha,
  );

  let review_sweep = if should_run_review_sweep(state, &task.project_id, &task.id).await? {
    match run_review_sweep(state, task, &root, ingest_provider.as_ref()).await {
      Ok(Some(result)) => Some(result),
      Ok(None) => None,
      Err(_) => None,
    }
  } else {
    None
  };

  let mut response = json!({
    "summaryPath": result.summary_path,
    "cacheHit": result.cache_hit,
    "writtenPaths": result.written_paths
  });

  if let Some(review_sweep) = review_sweep {
    response["reviewSweep"] = review_sweep;
  }

  Ok(response)
}

async fn run_review_sweep(
  state: &AppState,
  task: &TaskRecord,
  root: &knowledge_core::project::root::ProjectRoot,
  provider: Option<&OpenAiCompatibleProvider>,
) -> Result<Option<Value>, ApiError> {
  let mut result = match sweep_resolved_reviews(root) {
    Ok(result) => result,
    Err(_) => return Ok(None),
  };

  if !result.unresolved_ids.is_empty()
    && let Some(provider) = provider
    && let Ok(Some(prompt)) =
      build_review_sweep_prompt(root, &result.unresolved_ids, REVIEW_SWEEP_MAX_PAGES)
    && let Ok(response) = provider
      .complete_text(ProviderTextRequest {
        system_prompt: REVIEW_SWEEP_SYSTEM_PROMPT.to_string(),
        user_prompt: prompt,
      })
      .await
  {
    let resolved_ids = parse_review_resolution_ids(&response.text, &result.unresolved_ids);
    if !resolved_ids.is_empty() && let Ok(llm_result) = resolve_review_ids(root, &resolved_ids) {
      result
        .resolved_ids
        .extend(llm_result.resolved_ids.iter().cloned());
      result
        .unresolved_ids
        .retain(|review_id| !llm_result.resolved_ids.iter().any(|id| id == review_id));
    }
  }

  if !result.resolved_ids.is_empty() {
    let _ = append_audit_log(
      state,
      CreateAuditLog {
        project_id: Some(task.project_id.clone()),
        actor_id: task.created_by.clone(),
        action: "review.sweep.completed".to_string(),
        target_type: "review".to_string(),
        target_id: "batch".to_string(),
        task_id: Some(task.id.clone()),
        summary: format!(
          "Auto-resolved {} stale review item{}",
          result.resolved_ids.len(),
          if result.resolved_ids.len() == 1 { "" } else { "s" }
        ),
        metadata: json!({
          "resolvedIds": result.resolved_ids,
          "unresolvedIds": result.unresolved_ids
        }),
      },
    )
    .await?;
  }

  Ok(Some(json!({
    "resolvedIds": result.resolved_ids,
    "unresolvedIds": result.unresolved_ids
  })))
}

async fn run_update_review_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let review_id = read_string(&task.payload, "reviewId")?;
  let status = read_string(&task.payload, "status")?;
  let updated =
    update_review_status(&root, &review_id, &status).map_err(|error| ApiError::bad_request(error.to_string()))?;

  Ok(json!({
    "id": updated.id,
    "status": updated.status,
    "title": updated.title,
    "description": updated.description
  }))
}

async fn run_manual_review_sweep_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let provider = load_ingest_provider(state).await?;
  Ok(
    run_review_sweep(state, task, &root, provider.as_ref())
      .await?
      .unwrap_or_else(|| json!({ "resolvedIds": [], "unresolvedIds": [] })),
  )
}

async fn run_dedup_detect_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let provider = load_ingest_provider(state)
    .await?
    .ok_or_else(|| ApiError::bad_request("dedup detection requires an openai-compatible provider"))?;
  let embedding_config = load_embedding_config(state)
    .await?
    .ok_or_else(|| ApiError::bad_request("dedup detection requires a configured embedding model"))?;

  ensure_project_embeddings(state, &task.project_id, &root, &embedding_config).await?;
  let chunks = load_project_chunks(&state.pool, &task.project_id).await?;
  let candidate_paths =
    select_dedup_candidate_pages(&chunks, DEDUP_SIMILARITY_THRESHOLD, DEDUP_MAX_CANDIDATE_PAGES);

  let mut summaries = Vec::new();
  let mut seen_slugs = BTreeSet::new();
  for path in &candidate_paths {
    let content = try_read_project_file(&root, path);
    if content.is_empty() {
      continue;
    }
    if let Some(summary) = extract_entity_summary(path, &content)
      && seen_slugs.insert(summary.slug.clone())
    {
      summaries.push(summary);
    }
  }

  let mut groups = Vec::new();
  if summaries.len() >= 2 {
    let response = provider
      .complete_text(ProviderTextRequest {
        system_prompt: DETECTOR_SYSTEM_PROMPT.to_string(),
        user_prompt: build_detector_user_message(&summaries),
      })
      .await
      .map_err(TaskExecutionError::from_provider_error)
      .map_err(TaskExecutionError::into_api_error)?;

    let valid_slugs = summaries
      .iter()
      .map(|summary| summary.slug.clone())
      .collect::<BTreeSet<_>>();
    let not_duplicates =
      load_not_duplicates(&root).map_err(|error| ApiError::internal(error.to_string()))?;
    let detected = filter_detected_groups(
      parse_detector_response(&response.text),
      &valid_slugs,
      &not_duplicates,
    );

    let now = OffsetDateTime::now_utc()
      .format(&Rfc3339)
      .map_err(|_| ApiError::internal("failed to format timestamp"))?;
    for candidate in detected {
      groups.push(DedupGroup {
        id: uuid::Uuid::new_v4().to_string(),
        slugs: candidate.slugs,
        reason: candidate.reason,
        confidence: candidate.confidence,
        status: "candidate".to_string(),
        created_at: now.clone(),
      });
    }
  }

  let group_count = groups.len();
  save_dedup_store(&root, &DedupStore { groups })
    .map_err(|error| ApiError::internal(error.to_string()))?;

  Ok(json!({
    "groupCount": group_count,
    "candidatePages": candidate_paths
  }))
}

async fn run_dedup_merge_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let group_id = read_string(&task.payload, "groupId")?;
  let canonical_slug = read_string(&task.payload, "canonicalSlug")?;
  let root = project_root_for_id(state, &task.project_id).await?;
  let provider = load_ingest_provider(state)
    .await?
    .ok_or_else(|| ApiError::bad_request("dedup merge requires an openai-compatible provider"))?;

  let mut store_data =
    load_dedup_store(&root).map_err(|error| ApiError::internal(error.to_string()))?;
  let group = store_data
    .groups
    .iter()
    .find(|group| group.id == group_id)
    .cloned()
    .ok_or_else(|| ApiError::bad_request("dedup group not found"))?;

  let entity_pages =
    collect_entity_pages(&root).map_err(|error| ApiError::internal(error.to_string()))?;
  let mut group_pages = Vec::new();
  for slug in &group.slugs {
    let matches = entity_pages
      .iter()
      .filter(|page| &page.slug == slug)
      .collect::<Vec<_>>();
    if matches.len() > 1 {
      return Err(ApiError::bad_request(format!(
        "slug \"{slug}\" is ambiguous across multiple wiki pages"
      )));
    }
    let page = matches
      .first()
      .ok_or_else(|| ApiError::bad_request(format!("page for slug \"{slug}\" not found")))?;
    group_pages.push((*page).clone());
  }

  let group_paths = group_pages
    .iter()
    .map(|page| page.path.clone())
    .collect::<BTreeSet<_>>();
  let other_pages = collect_all_wiki_pages(&root)
    .map_err(|error| ApiError::internal(error.to_string()))?
    .into_iter()
    .filter(|page| !group_paths.contains(&page.path) && page.path != "wiki/index.md")
    .collect::<Vec<_>>();

  let response = provider
    .complete_text(ProviderTextRequest {
      system_prompt: MERGER_SYSTEM_PROMPT.to_string(),
      user_prompt: build_merger_user_message(&group_pages),
    })
    .await
    .map_err(TaskExecutionError::from_provider_error)
    .map_err(TaskExecutionError::into_api_error)?;

  let now = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))?;
  let today = now[..10].to_string();
  let outcome = compute_dedup_merge(
    &group_pages,
    &canonical_slug,
    &other_pages,
    &response.text,
    &today,
  )
  .map_err(|error| ApiError::bad_request(error.to_string()))?;

  let stamp = now.replace([':', '.'], "-");
  for entry in &outcome.backup {
    write_dedup_backup(&root, &stamp, entry)?;
  }

  save_wiki_page(&root, &outcome.canonical_path, &outcome.canonical_content)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  for rewrite in &outcome.rewrites {
    save_wiki_page(&root, &rewrite.path, &rewrite.new_content)
      .map_err(|error| ApiError::bad_request(error.to_string()))?;
  }
  let delete_result = delete_wiki_pages_with_refs(&root, &outcome.pages_to_delete)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  delete_pages(&state.pool, &task.project_id, &outcome.pages_to_delete).await?;

  let removed_slugs = group
    .slugs
    .iter()
    .filter(|slug| *slug != &canonical_slug)
    .cloned()
    .collect::<BTreeSet<_>>();
  store_data.groups.retain(|candidate| {
    candidate.id != group_id
      && !candidate.slugs.iter().any(|slug| removed_slugs.contains(slug))
  });
  save_dedup_store(&root, &store_data)
    .map_err(|error| ApiError::internal(error.to_string()))?;

  Ok(json!({
    "canonicalPath": outcome.canonical_path,
    "deletedPaths": delete_result.deleted_paths,
    "rewrittenFiles": delete_result.rewritten_files,
    "rewrites": outcome.rewrites.iter().map(|rewrite| rewrite.path.clone()).collect::<Vec<_>>()
  }))
}

fn write_dedup_backup(
  root: &knowledge_core::project::root::ProjectRoot,
  stamp: &str,
  entry: &DedupBackupEntry,
) -> Result<(), ApiError> {
  let backup_path = root
    .safe_join(&format!(".knowledge/dedup/backups/{stamp}/{}", entry.path))
    .map_err(|error| ApiError::internal(error.to_string()))?;
  if let Some(parent) = backup_path.parent() {
    std::fs::create_dir_all(parent).map_err(|error| ApiError::internal(error.to_string()))?;
  }
  std::fs::write(&backup_path, &entry.content)
    .map_err(|error| ApiError::internal(error.to_string()))?;
  Ok(())
}

const RESEARCH_WIKI_INDEX_MAX_BYTES: usize = 8 * 1024;

async fn run_deep_research_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let provider = load_ingest_provider(state)
    .await?
    .ok_or_else(|| ApiError::bad_request("deep research requires an openai-compatible provider"))?;
  let search_config = load_web_search_config(state)
    .await?
    .ok_or_else(|| ApiError::bad_request("deep research requires a configured web search provider"))?;

  let topic = read_string(&task.payload, "topic")?;
  let topic = topic.trim();
  if topic.is_empty() {
    return Err(ApiError::bad_request("topic is required"));
  }
  let queries_value = task.payload.get("searchQueries");
  let queries: Vec<String> = queries_value
    .and_then(Value::as_array)
    .map(|values| {
      values
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .filter(|value| !value.trim().is_empty())
        .collect()
    })
    .unwrap_or_default();
  let queries = if queries.is_empty() {
    vec![topic.to_string()]
  } else {
    queries
  };

  let sources = collect_research_sources(&queries, &search_config).await?;
  if sources.results.is_empty() {
    return Err(ApiError::bad_request(format!(
      "no research sources found{}",
      if sources.errors.is_empty() {
        String::new()
      } else {
        format!(": {}", sources.errors.join("; "))
      }
    )));
  }

  let source_context = sources
    .results
    .iter()
    .enumerate()
    .map(|(index, item)| {
      format!(
        "[{}] **{}** ({})\n{}",
        index + 1,
        item.title,
        item.source,
        item.snippet
      )
    })
    .collect::<Vec<_>>()
    .join("\n\n");

  let mut wiki_index = try_read_project_file(&root, "wiki/index.md");
  if wiki_index.len() > RESEARCH_WIKI_INDEX_MAX_BYTES {
    tracing::debug!(
      "truncating wiki/index.md from {} bytes to {} for deep research synthesizer",
      wiki_index.len(),
      RESEARCH_WIKI_INDEX_MAX_BYTES
    );
    wiki_index.truncate(RESEARCH_WIKI_INDEX_MAX_BYTES);
  }

  let mut system_parts = vec![
    "You are a research assistant. Synthesize the collected research sources into a comprehensive wiki page.".to_string(),
    String::new(),
    "## Cross-referencing (IMPORTANT)".to_string(),
    "- The wiki already has existing pages listed in the Wiki Index below.".to_string(),
    "- When your synthesis mentions an entity or concept that exists in the wiki, ALWAYS use [[wikilink]] syntax to link to it.".to_string(),
    "- For example, if the wiki has an entity 'anthropic', write [[anthropic]] when mentioning it.".to_string(),
    "- This is critical for connecting new research to existing knowledge in the graph.".to_string(),
    String::new(),
    "## Writing Rules".to_string(),
    "- Organize into clear sections with headings".to_string(),
    "- Cite sources using [N] notation".to_string(),
    "- Note contradictions or gaps".to_string(),
    "- Suggest additional sources worth finding".to_string(),
    "- Neutral, encyclopedic tone".to_string(),
  ];
  if !wiki_index.trim().is_empty() {
    system_parts.push(String::new());
    system_parts.push(format!(
      "## Existing Wiki Index (link to these pages with [[wikilink]])\n{wiki_index}"
    ));
  }
  let system_prompt = system_parts.join("\n");

  let user_prompt = format!(
    "Research topic: **{topic}**\n\n## Research Sources\n\n{source_context}\n\nSynthesize into a wiki page."
  );

  let response = provider
    .complete_text(ProviderTextRequest {
      system_prompt,
      user_prompt,
    })
    .await
    .map_err(TaskExecutionError::from_provider_error)
    .map_err(TaskExecutionError::into_api_error)?;

  let now = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))?;
  let date = now[..10].to_string();
  let slug = research_slug(topic);
  let references = sources
    .results
    .iter()
    .map(|item| RenderResearchReference {
      title: item.title.clone(),
      url: item.url.clone(),
      source: item.source.clone(),
    })
    .collect::<Vec<_>>();
  let page_content = render_research_page(&RenderResearchPageInput {
    topic: topic.to_string(),
    slug: slug.clone(),
    date,
    synthesis: response.text,
    references,
  });

  let relative_path = format!("wiki/queries/research-{slug}.md");
  save_wiki_page(&root, &relative_path, &page_content)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  research_update_index(&root, &slug, topic)
    .map_err(|error| ApiError::internal(error.to_string()))?;

  Ok(json!({
    "savedPath": relative_path,
    "sourceCount": sources.results.len(),
    "errors": sources.errors
  }))
}

fn research_slug(topic: &str) -> String {
  let mut slug = String::new();
  let mut last_dash = false;
  for ch in topic.chars() {
    if ch.is_ascii_alphanumeric() {
      slug.push(ch.to_ascii_lowercase());
      last_dash = false;
    } else if !last_dash && !slug.is_empty() {
      slug.push('-');
      last_dash = true;
    }
  }
  let trimmed = slug.trim_end_matches('-').to_string();
  if trimmed.is_empty() { "research".to_string() } else { trimmed }
}

fn research_update_index(
  root: &knowledge_core::project::root::ProjectRoot,
  slug: &str,
  topic: &str,
) -> std::io::Result<()> {
  let path = root.as_path().join("wiki/index.md");
  let existing = std::fs::read_to_string(&path).unwrap_or_default();
  let entry = format!("- [[queries/research-{slug}]] - Research: {topic}\n");
  if existing.contains(entry.trim_end()) {
    return Ok(());
  }
  let marker = "## Queries\n";
  let updated = if let Some(index) = existing.find(marker) {
    let insert_at = index + marker.len();
    let mut updated = existing.clone();
    updated.insert_str(insert_at, &entry);
    updated
  } else {
    format!("{existing}\n## Queries\n{entry}")
  };
  std::fs::write(path, updated)
}

async fn append_task_audit_log(
  state: &AppState,
  task: &TaskRecord,
  action: &str,
  summary: &str,
) -> Result<(), ApiError> {
  let _ = append_audit_log(
    state,
    CreateAuditLog {
      project_id: Some(task.project_id.clone()),
      actor_id: task.created_by.clone(),
      action: action.to_string(),
      target_type: "task".to_string(),
      target_id: task.id.clone(),
      task_id: Some(task.id.clone()),
      summary: format!("{summary}: {}", task.title),
      metadata: json!({
        "taskType": task.task_type,
        "status": task.status
      }),
    },
  )
  .await?;

  Ok(())
}

fn read_string(payload: &Value, key: &str) -> Result<String, ApiError> {
  payload
    .get(key)
    .and_then(Value::as_str)
    .map(str::to_string)
    .ok_or_else(|| ApiError::bad_request(format!("missing {key} payload")))
}

async fn should_run_review_sweep(
  state: &AppState,
  project_id: &str,
  current_task_id: &str,
) -> Result<bool, ApiError> {
  let active_ingests = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*)
     FROM project_tasks
     WHERE project_id = $1
       AND task_type = 'project.ingest_source'
       AND status IN ('queued', 'running', 'retry_waiting')
       AND id <> $2",
  )
  .bind(project_id)
  .bind(current_task_id)
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(active_ingests == 0)
}

async fn load_ingest_provider(state: &AppState) -> Result<Option<OpenAiCompatibleProvider>, ApiError> {
  let provider_mode = sqlx::query_scalar::<_, String>(
    "SELECT provider_mode FROM system_settings WHERE id = 1",
  )
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  // Deterministic mode uses no real LLM (test/offline fixture path).
  if provider_mode != "openai-compatible" {
    return Ok(None);
  }

  // The active provider connection is the source of truth now; the legacy flat
  // columns are only a fallback for a DB that predates the seed step.
  let connections = crate::providers::list_connections(&state.pool).await?;
  if let Some(active) = crate::providers::resolve_active(&connections) {
    return Ok(Some(crate::providers::ActiveConnection::from(active).provider()));
  }

  let (provider_base_url, provider_api_key, provider_model, provider_timeout_seconds) =
    sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>, Option<i64>)>(
      "SELECT provider_base_url, provider_api_key, provider_model, provider_timeout_seconds
       FROM system_settings
       WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

  if provider_base_url.as_deref().unwrap_or("").trim().is_empty()
    || provider_model.as_deref().unwrap_or("").trim().is_empty()
  {
    return Ok(None);
  }

  Ok(Some(OpenAiCompatibleProvider::new(
    provider_base_url.unwrap_or_default(),
    provider_api_key.unwrap_or_default(),
    provider_model.unwrap_or_default(),
    provider_timeout_seconds.unwrap_or(30),
  )))
}

fn try_read_project_file(
  root: &knowledge_core::project::root::ProjectRoot,
  relative_path: &str,
) -> String {
  root
    .safe_join(relative_path)
    .ok()
    .and_then(|path| std::fs::read_to_string(path).ok())
    .unwrap_or_default()
}

fn source_identity_from_relative_path(relative_path: &str) -> String {
  relative_path
    .trim_start_matches("raw/sources/")
    .replace('\\', "/")
}

fn analysis_title_from_text(source_name: &str, source_content: &str, analysis_text: &str) -> String {
  analysis_text
    .lines()
    .find_map(|line| {
      let trimmed = line.trim();
      trimmed
        .strip_prefix("title:")
        .or_else(|| trimmed.strip_prefix("Title:"))
        .map(str::trim)
    })
    .filter(|title| !title.is_empty())
    .map(str::to_string)
    .unwrap_or_else(|| analyze_source(source_name, source_content).title)
}

async fn merge_generated_pages_with_existing_content(
  provider: &OpenAiCompatibleProvider,
  root: &knowledge_core::project::root::ProjectRoot,
  source_name: &str,
  generation_text: &str,
) -> Result<String, ApiError> {
  let review_suffix = review_block_suffix(generation_text);
  let mut blocks = parse_generation_file_blocks(generation_text);

  for block in &mut blocks {
    if block.path == "wiki/index.md" || block.path == "wiki/log.md" || block.path == "wiki/overview.md" {
      continue;
    }

    let absolute = root.safe_join(&block.path).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let existing_content = if absolute.exists() {
      Some(std::fs::read_to_string(&absolute).map_err(|error| ApiError::bad_request(error.to_string()))?)
    } else {
      None
    };

    match prepare_page_merge(&block.content, existing_content.as_deref()) {
      PageMergePlan::Final(final_content) => {
        block.content = final_content;
      }
      PageMergePlan::NeedsProvider {
        existing_content,
        array_merged,
      } => {
        let (system_prompt, user_prompt) =
          build_page_merge_prompts(&existing_content, &array_merged, source_name);
        let llm_output = provider
          .complete_text(ProviderTextRequest {
            system_prompt,
            user_prompt,
          })
          .await
          .map(|response| response.text)
          .ok();
        block.content = finalize_page_merge(
          &existing_content,
          &array_merged,
          llm_output.as_deref(),
          &today_utc(),
        );
      }
    }
  }

  let rendered = render_generation_file_blocks(&blocks);
  if review_suffix.is_empty() {
    Ok(rendered)
  } else {
    Ok(format!("{rendered}\n\n{review_suffix}"))
  }
}

fn today_utc() -> String {
  let now = OffsetDateTime::now_utc().date();
  format!(
    "{:04}-{:02}-{:02}",
    now.year(),
    u8::from(now.month()),
    now.day()
  )
}

fn review_block_suffix(generation_text: &str) -> String {
  generation_text
    .find("---REVIEW:")
    .map(|index| generation_text[index..].trim().to_string())
    .unwrap_or_default()
}
