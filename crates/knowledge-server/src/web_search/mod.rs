pub mod config;
pub mod provider;
pub mod routes;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WebSearchResult {
  pub title: String,
  pub url: String,
  pub snippet: String,
  pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSearchProvider {
  Tavily,
  SerpApi,
  SearXng,
  Ollama,
  Brave,
  Firecrawl,
}

impl WebSearchProvider {
  pub fn as_str(&self) -> &'static str {
    match self {
      WebSearchProvider::Tavily => "tavily",
      WebSearchProvider::SerpApi => "serpapi",
      WebSearchProvider::SearXng => "searxng",
      WebSearchProvider::Ollama => "ollama",
      WebSearchProvider::Brave => "brave",
      WebSearchProvider::Firecrawl => "firecrawl",
    }
  }
}
