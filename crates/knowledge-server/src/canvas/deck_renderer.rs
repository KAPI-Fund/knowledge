use std::path::Path;

use crate::providers::{OpenAiCompatibleProvider, ProviderError, ProviderTextRequest};
use crate::skills::SkillDescriptor;

#[derive(Debug)]
pub struct DeckOutcome {
    pub deck_html: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DeckError {
    #[error("skill file missing: {0}")]
    MissingFile(String),
    #[error("model returned no usable HTML")]
    NoHtml,
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// System prompt = the skill's authoring guidance + the chosen HTML template,
/// both inlined. The model must return ONE self-contained HTML document.
pub fn build_system_prompt(skill_md: &str, template_html: &str) -> String {
    format!(
        "You are a slide-deck generator. Follow these authoring instructions exactly:\n\n\
         {skill_md}\n\n\
         Use this HTML template as the structural and visual baseline; keep its styling \
         and produce ONE self-contained HTML file (inline CSS/JS, no external assets):\n\n\
         <template>\n{template_html}\n</template>\n\n\
         Output ONLY the final HTML document, starting with <!DOCTYPE html>. Do not wrap \
         it in Markdown fences or add commentary."
    )
}

pub fn build_user_prompt(selection: &str, argument: &str) -> String {
    let requirements = if argument.trim().is_empty() {
        "No extra requirements.".to_string()
    } else {
        format!("Extra requirements: {}", argument.trim())
    };
    format!("Source content to turn into slides:\n\n{selection}\n\n{requirements}")
}

/// Strip an optional ```html … ``` fence and assert the payload looks like HTML.
pub fn extract_html(raw: &str) -> Result<String, DeckError> {
    let trimmed = raw.trim();
    let body = trimmed
        .strip_prefix("```html")
        .or_else(|| trimmed.strip_prefix("```"))
        .map(str::trim_start)
        .and_then(|rest| rest.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    let lowered = body.to_ascii_lowercase();
    if lowered.starts_with("<!doctype") || lowered.contains("<html") {
        Ok(body.to_string())
    } else {
        Err(DeckError::NoHtml)
    }
}

fn read_skill_file(dir: &Path, name: &str) -> Result<String, DeckError> {
    std::fs::read_to_string(dir.join(name)).map_err(|_| DeckError::MissingFile(name.to_string()))
}

/// Read SKILL.md + template from the skill dir, run one completion, extract HTML.
pub async fn render_deck(
    descriptor: &SkillDescriptor,
    provider: &OpenAiCompatibleProvider,
    selection: &str,
    argument: &str,
) -> Result<DeckOutcome, DeckError> {
    let entry = descriptor.entry.as_deref().unwrap_or("SKILL.md");
    let skill_md = read_skill_file(&descriptor.dir, entry)?;
    // Design default: swiss template; fall back to the plain template.
    let template = read_skill_file(&descriptor.dir, "template-swiss.html")
        .or_else(|_| read_skill_file(&descriptor.dir, "template.html"))?;

    let response = provider
        .complete_text(ProviderTextRequest {
            system_prompt: build_system_prompt(&skill_md, &template),
            user_prompt: build_user_prompt(selection, argument),
        })
        .await?;

    let deck_html = extract_html(&response.text)?;
    Ok(DeckOutcome { deck_html })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_inlines_guidance_and_template() {
        let p = build_system_prompt("AUTHORING_RULES", "<div class=slide>");
        assert!(p.contains("AUTHORING_RULES"));
        assert!(p.contains("<div class=slide>"));
        assert!(p.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn user_prompt_handles_empty_and_present_argument() {
        assert!(build_user_prompt("content", "  ").contains("No extra requirements."));
        assert!(build_user_prompt("content", "swiss, dark").contains("Extra requirements: swiss, dark"));
    }

    #[test]
    fn extract_html_unwraps_fences_and_rejects_prose() {
        assert_eq!(
            extract_html("```html\n<!DOCTYPE html><html></html>\n```").unwrap(),
            "<!DOCTYPE html><html></html>"
        );
        assert_eq!(
            extract_html("<!doctype html><html>x</html>").unwrap(),
            "<!doctype html><html>x</html>"
        );
        assert!(matches!(extract_html("sorry, I can't do that"), Err(DeckError::NoHtml)));
    }
}
