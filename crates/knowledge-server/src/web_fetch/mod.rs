pub mod config;
pub mod firecrawl;

/// A page reduced to readable markdown plus its title. Firecrawl is the only
/// producer now that the built-in direct fetch is gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedPage {
    pub title: String,
    pub markdown: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchProvider {
    Firecrawl,
}

impl FetchProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            FetchProvider::Firecrawl => "firecrawl",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_provider_as_str_is_firecrawl() {
        assert_eq!(FetchProvider::Firecrawl.as_str(), "firecrawl");
    }
}
