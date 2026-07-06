use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use scraper::{Html, Selector};

/// Reference: upstream_llm_wiki/extension/Readability.js (content extraction)
/// + upstream_llm_wiki/extension/Turndown.js (HTML -> markdown).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedPage {
    pub title: String,
    pub markdown: String,
}

pub fn html_to_markdown(html: &str) -> ExtractedPage {
    let doc = Html::parse_document(html);

    let title = doc
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|t| t.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Untitled".to_string());

    // Prefer <article>/<main> as the content root; fall back to <body>.
    let root_html = ["article", "main", "body"]
        .iter()
        .find_map(|sel| {
            doc.select(&Selector::parse(sel).unwrap())
                .next()
                .map(|el| el.html())
        })
        .unwrap_or_else(|| html.to_string());

    let root = Html::parse_fragment(&root_html);

    // Collect NodeIds that belong to skip subtrees (nav, footer, script, style, aside).
    // Use HashSet with inferred NodeId type so we don't need to name ego_tree directly.
    let skip_sel = Selector::parse("nav, footer, script, style, aside").unwrap();
    let mut skip_ids = HashSet::new();
    for skip_el in root.select(&skip_sel) {
        skip_ids.insert(skip_el.id());
        for descendant in skip_el.descendants() {
            skip_ids.insert(descendant.id());
        }
    }

    let block_sel = Selector::parse("h1, h2, h3, h4, p, li, pre").unwrap();
    let mut out = String::new();

    for el in root.select(&block_sel) {
        if skip_ids.contains(&el.id()) {
            continue;
        }
        if el.ancestors().any(|a| skip_ids.contains(&a.id())) {
            continue;
        }
        let text = inline_markdown(el);
        if text.trim().is_empty() {
            continue;
        }
        let name = el.value().name();
        match name {
            "h1" => out.push_str(&format!("# {text}\n\n")),
            "h2" => out.push_str(&format!("## {text}\n\n")),
            "h3" => out.push_str(&format!("### {text}\n\n")),
            "h4" => out.push_str(&format!("#### {text}\n\n")),
            "li" => out.push_str(&format!("- {text}\n")),
            "pre" => out.push_str(&format!("```\n{text}\n```\n\n")),
            _ => out.push_str(&format!("{text}\n\n")),
        }
    }

    ExtractedPage { title, markdown: out.trim().to_string() }
}

fn inline_markdown(el: scraper::ElementRef) -> String {
    let mut s = String::new();
    for child in el.children() {
        if let Some(text) = child.value().as_text() {
            s.push_str(text);
        } else if let Some(child_el) = scraper::ElementRef::wrap(child) {
            if child_el.value().name() == "a" {
                let href = child_el.value().attr("href").unwrap_or("");
                let label = child_el.text().collect::<String>();
                if href.is_empty() {
                    s.push_str(&label);
                } else {
                    s.push_str(&format!("[{label}]({href})"));
                }
            } else {
                s.push_str(&inline_markdown(child_el));
            }
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cap on the response body we will buffer from an extracted page (5 MiB).
pub const MAX_EXTRACT_BYTES: usize = 5 * 1024 * 1024;

/// Guard against SSRF: only allow http/https to public hosts. A literal-IP
/// host in a private/loopback/link-local/unspecified range is rejected, as is
/// `localhost`. This blocks the direct vectors (cloud metadata, internal
/// services). It does NOT by itself defend against a public hostname that
/// resolves to a private IP (DNS rebinding); that vector is closed at connect
/// time by PublicOnlyResolver, which screens every resolved address.
///
/// `allow_private` bypasses the IP/localhost screening (scheme + host validity
/// are still enforced) for trusted deployments behind a fake-IP proxy.
pub fn validate_public_url(raw: &str, allow_private: bool) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "invalid URL".to_string())?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(format!("unsupported URL scheme: {other}")),
    }
    let host = url.host_str().ok_or_else(|| "URL has no host".to_string())?;
    if allow_private {
        return Ok(url);
    }
    if host.eq_ignore_ascii_case("localhost") {
        return Err("URL host is not allowed".to_string());
    }
    // IPv6 literals arrive bracketed (e.g. "[::1]"); strip them before parsing.
    let host_ip = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    if let Ok(ip) = host_ip.parse::<IpAddr>()
        && is_blocked_ip(&ip)
    {
        return Err("URL host is not allowed".to_string());
    }
    Ok(url)
}

// Block anything that is not a globally routable public address. std's
// IpAddr::is_global is still unstable, so the special-use ranges (RFC 6890 and
// friends) are enumerated explicitly rather than derived.
fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                // 100.64.0.0/10 shared address space (CGNAT).
                || (o[0] == 100 && (o[1] & 0xc0) == 0x40)
                // 192.0.0.0/24 IETF protocol assignments.
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                // 192.88.99.0/24 6to4 relay anycast (deprecated).
                || (o[0] == 192 && o[1] == 88 && o[2] == 99)
                // 198.18.0.0/15 benchmarking.
                || (o[0] == 198 && (o[1] & 0xfe) == 18)
                // 240.0.0.0/4 reserved / future use.
                || o[0] >= 240
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_blocked_ip(&IpAddr::V4(mapped));
            }
            let s = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // Unique local fc00::/7.
                || (s[0] & 0xfe00) == 0xfc00
                // Link-local fe80::/10.
                || (s[0] & 0xffc0) == 0xfe80
                // Documentation 2001:db8::/32.
                || (s[0] == 0x2001 && s[1] == 0x0db8)
        }
    }
}

/// Screen the addresses a hostname resolved to. Reject if the host resolved to
/// nothing, or if ANY resolved address is in a blocked range. Screening every
/// address (not just the first) is what closes the DNS-rebinding vector that
/// validate_public_url cannot see: it only inspects the hostname/literal IP.
///
/// `allow_private` skips the blocked-range screening (a resolved address is still
/// required) for trusted deployments behind a fake-IP proxy.
pub fn screen_resolved_addrs(
    addrs: Vec<SocketAddr>,
    allow_private: bool,
) -> Result<Vec<SocketAddr>, String> {
    if addrs.is_empty() {
        return Err("host did not resolve to any address".to_string());
    }
    if !allow_private && addrs.iter().any(|addr| is_blocked_ip(&addr.ip())) {
        return Err("URL host resolves to a non-public address".to_string());
    }
    Ok(addrs)
}

/// A reqwest DNS resolver that performs normal system resolution, then rejects
/// the connection if any resolved address is non-public. Paired with
/// validate_public_url (hostname/literal-IP screening at request build time),
/// this ensures reqwest only ever connects to public IPs, closing the
/// DNS-rebinding hole. tokio's "net" feature is not enabled in this workspace,
/// so resolution runs on a blocking thread via std::net::ToSocketAddrs.
///
/// `allow_private` disables the address screening (the resolver then behaves like
/// a plain system resolver) for trusted fake-IP-proxy deployments.
#[derive(Debug, Clone, Default)]
pub struct PublicOnlyResolver {
    allow_private: bool,
}

impl PublicOnlyResolver {
    pub fn new(allow_private: bool) -> Self {
        Self { allow_private }
    }
}

impl reqwest::dns::Resolve for PublicOnlyResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let allow_private = self.allow_private;
        Box::pin(async move {
            let host = name.as_str().to_string();
            let resolved = tokio::task::spawn_blocking(move || {
                (host.as_str(), 0u16).to_socket_addrs().map(|it| it.collect::<Vec<_>>())
            })
            .await
            .map_err(box_dns_err)?
            .map_err(box_dns_err)?;

            let screened = screen_resolved_addrs(resolved, allow_private)
                .map_err(|msg| box_dns_err(std::io::Error::other(msg)))?;
            let addrs: reqwest::dns::Addrs = Box::new(screened.into_iter());
            Ok(addrs)
        })
    }
}

fn box_dns_err<E>(err: E) -> Box<dyn std::error::Error + Send + Sync>
where
    E: std::error::Error + Send + Sync + 'static,
{
    Box::new(err)
}

/// Build the reqwest client used for URL extraction: a request timeout, a
/// redirect policy that re-validates every hop's URL, and a DNS resolver that
/// rejects non-public resolved IPs. Centralized here so the SSRF defenses stay
/// in one place rather than being re-declared at each call site.
pub fn build_extractor_client(allow_private: bool) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.error("too many redirects");
            }
            match validate_public_url(attempt.url().as_str(), allow_private) {
                Ok(_) => attempt.follow(),
                Err(_) => attempt.stop(),
            }
        }))
        // Disable any system/environment proxy. A proxy would resolve the
        // target host itself, so PublicOnlyResolver would never run and could
        // not screen the final address -- reopening the SSRF hole this client
        // exists to close.
        .no_proxy()
        .dns_resolver(Arc::new(PublicOnlyResolver::new(allow_private)))
        .build()
}

/// Extract an HTML attribute's value from a single tag's raw text, matching the
/// attribute name case-insensitively and handling double-quoted, single-quoted,
/// and unquoted values. Returns None if the attribute is absent. ASCII-lowercasing
/// preserves byte length, so indices from the lowercased copy map 1:1 onto `tag`.
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let bytes = tag.as_bytes();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(name) {
        let name_start = from + rel;
        from = name_start + name.len();
        // The name must begin at a word boundary so we don't match it as a
        // substring of another attribute name or value.
        if name_start != 0 && !bytes[name_start - 1].is_ascii_whitespace() {
            continue;
        }
        let mut i = from;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            continue;
        }
        i += 1; // past '='
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        let quote = bytes[i];
        if quote == b'"' || quote == b'\'' {
            let vstart = i + 1;
            let vend = tag[vstart..].find(quote as char).map(|r| vstart + r)?;
            return Some(tag[vstart..vend].to_string());
        }
        // Unquoted: read until the next whitespace or the end of the tag.
        let vstart = i;
        let vend = tag[vstart..]
            .find(|c: char| c.is_whitespace())
            .map(|r| vstart + r)
            .unwrap_or(tag.len());
        return Some(tag[vstart..vend].to_string());
    }
    None
}

/// Scan raw HTML for a `<meta http-equiv="refresh" content="...; url=TARGET">`
/// redirect and return the resolved absolute target URL. The scan runs on the
/// raw HTML (not the parsed DOM) because such stubs frequently place the meta
/// inside `<noscript>`, which html5ever surfaces as inert text rather than a real
/// element. Matching is case-insensitive; the `url=` value may be double-quoted,
/// single-quoted, or unquoted, and a relative target resolves against `base`.
/// Returns None when there is no refresh directive or it carries no `url=` (a
/// plain timed self-refresh is not a redirect target).
fn meta_refresh_target(html: &str, base: &reqwest::Url) -> Option<reqwest::Url> {
    let lower = html.to_ascii_lowercase();
    let mut search_from = 0;
    while search_from < lower.len() {
        let Some(rel) = lower[search_from..].find("<meta") else { break };
        let start = search_from + rel;
        let end = html[start..].find('>').map(|r| start + r).unwrap_or(html.len());
        let tag = &html[start..end];
        search_from = (end + 1).min(html.len());

        let is_refresh = attr_value(tag, "http-equiv")
            .map(|v| v.trim().eq_ignore_ascii_case("refresh"))
            .unwrap_or(false);
        if !is_refresh {
            continue;
        }
        let Some(content) = attr_value(tag, "content") else { continue };
        let lower_content = content.to_ascii_lowercase();
        let Some(url_pos) = lower_content.find("url=") else { continue };
        let raw_target = content[url_pos + 4..].trim().trim_matches(|c| c == '\'' || c == '"');
        if raw_target.is_empty() {
            continue;
        }
        if let Ok(resolved) = base.join(raw_target) {
            return Some(resolved);
        }
    }
    None
}

/// Fetch a URL and extract readable markdown. Beyond the HTTP 3xx redirects the
/// reqwest client follows, this also follows HTML `<meta http-equiv="refresh">`
/// stub pages (e.g. baidu's HTTPS homepage, which meta-refreshes to its real
/// HTTP page), re-validating each hop against the SSRF guard.
pub async fn fetch_url(
    client: &reqwest::Client,
    url: &str,
    allow_private: bool,
) -> Result<ExtractedPage, String> {
    // Cap on HTML-level meta-refresh hops. Guards against a stub that refreshes
    // in a loop; a self-refresh to the same URL falls through to extraction.
    const MAX_META_REFRESH_HOPS: usize = 3;

    let mut current = validate_public_url(url, allow_private)?;
    for _ in 0..=MAX_META_REFRESH_HOPS {
        let mut response = client
            .get(current.clone())
            .header("User-Agent", "Mozilla/5.0 (compatible; KnowledgeCanvas/1.0)")
            .send()
            .await
            .map_err(|e| format!("fetch failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("fetch failed: HTTP {}", response.status()));
        }
        let mut body = Vec::new();
        while let Some(chunk) =
            response.chunk().await.map_err(|e| format!("read body failed: {e}"))?
        {
            if body.len() + chunk.len() > MAX_EXTRACT_BYTES {
                return Err("response too large".to_string());
            }
            body.extend_from_slice(&chunk);
        }
        let html = String::from_utf8_lossy(&body);

        if let Some(target) = meta_refresh_target(&html, &current) {
            let validated = validate_public_url(target.as_str(), allow_private)?;
            if validated != current {
                current = validated;
                continue;
            }
        }
        return Ok(html_to_markdown(&html));
    }
    Err("too many meta-refresh redirects".to_string())
}

use crate::canvas::document::CanvasDocument;

#[derive(Debug, Clone)]
pub struct SearchResultEntry {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Text blocks from incoming Note/URL/prior-analysis nodes (KB handled
/// separately via RAG).
pub fn collect_reference_blocks(doc: &CanvasDocument, node_id: &str) -> Vec<String> {
    let sources = doc.incoming_source_ids(node_id);
    let mut blocks = Vec::new();
    for src in sources {
        let Some(node) = doc.node(&src) else { continue };
        match node.r#type.as_str() {
            "note" => {
                if let Some(md) = node.data.get("markdown").and_then(|v| v.as_str())
                    && !md.trim().is_empty()
                {
                    blocks.push(format!("Note:\n{md}"));
                }
            }
            "url" => {
                let title = node.data.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let md = node.data.get("markdown").and_then(|v| v.as_str()).unwrap_or("");
                if !md.trim().is_empty() {
                    blocks.push(format!("Web page: {title}\n{md}"));
                }
            }
            "ai_analyze" => {
                if let Some(content) = crate::canvas::document::active_version_content(&node.data) {
                    blocks.push(format!("Prior analysis:\n{content}"));
                }
            }
            _ => {}
        }
    }
    blocks
}

/// Project ids of incoming KB nodes, for RAG retrieval at run time.
pub fn referenced_kb_project_ids(doc: &CanvasDocument, node_id: &str) -> Vec<String> {
    doc.incoming_source_ids(node_id)
        .into_iter()
        .filter_map(|src| doc.node(&src).cloned())
        .filter(|n| n.r#type == "kb")
        .filter_map(|n| n.data.get("projectId").and_then(|v| v.as_str()).map(String::from))
        .collect()
}

pub fn search_results_to_markdown(query: &str, results: &[SearchResultEntry]) -> String {
    let mut out = format!("Search results for \"{query}\":\n\n");
    for r in results {
        out.push_str(&format!("- [{}]({})\n  {}\n", r.title, r.url, r.snippet));
    }
    out.trim_end().to_string()
}

pub fn build_analyze_prompt(node_prompt: &str, blocks: &[String]) -> String {
    let mut prompt = String::new();
    if !blocks.is_empty() {
        prompt.push_str("Use the following referenced sources to answer.\n\n");
        for (i, b) in blocks.iter().enumerate() {
            prompt.push_str(&format!("--- Source {} ---\n{}\n\n", i + 1, b));
        }
    }
    prompt.push_str("Task:\n");
    prompt.push_str(node_prompt);
    prompt
}

#[cfg(test)]
mod url_tests {
    use super::*;

    #[test]
    fn html_to_markdown_extracts_title_and_text() {
        let html = r#"
            <html><head><title>Sample Page</title></head>
            <body>
              <nav>ignore me</nav>
              <article>
                <h1>Big Heading</h1>
                <p>First paragraph with a <a href="https://x.test">link</a>.</p>
                <h2>Sub</h2>
                <p>Second paragraph.</p>
              </article>
              <footer>footer junk</footer>
            </body></html>
        "#;
        let extracted = html_to_markdown(html);
        assert_eq!(extracted.title, "Sample Page");
        assert!(extracted.markdown.contains("# Big Heading"));
        assert!(extracted.markdown.contains("## Sub"));
        assert!(extracted.markdown.contains("First paragraph with a [link](https://x.test)."));
        assert!(extracted.markdown.contains("Second paragraph."));
        assert!(!extracted.markdown.contains("ignore me"));
        assert!(!extracted.markdown.contains("footer junk"));
    }

    #[test]
    fn html_to_markdown_falls_back_to_untitled() {
        let extracted = html_to_markdown("<html><body><p>hi</p></body></html>");
        assert_eq!(extracted.title, "Untitled");
        assert!(extracted.markdown.contains("hi"));
    }

    #[test]
    fn validate_public_url_accepts_public_http_and_https() {
        assert!(validate_public_url("http://93.184.216.34/", false).is_ok());
        assert!(validate_public_url("https://example.com/page", false).is_ok());
    }

    #[test]
    fn validate_public_url_rejects_non_http_schemes() {
        assert!(validate_public_url("file:///etc/passwd", false).is_err());
        assert!(validate_public_url("ftp://example.com/x", false).is_err());
        assert!(validate_public_url("gopher://example.com/", false).is_err());
    }

    #[test]
    fn validate_public_url_rejects_localhost_and_loopback() {
        assert!(validate_public_url("http://localhost/", false).is_err());
        assert!(validate_public_url("http://127.0.0.1/", false).is_err());
        assert!(validate_public_url("http://[::1]/", false).is_err());
    }

    #[test]
    fn validate_public_url_rejects_private_and_link_local() {
        assert!(validate_public_url("http://10.0.0.1/", false).is_err());
        assert!(validate_public_url("http://192.168.1.1/", false).is_err());
        assert!(validate_public_url("http://172.16.5.4/", false).is_err());
        // Cloud metadata endpoint (link-local).
        assert!(validate_public_url("http://169.254.169.254/latest/meta-data/", false).is_err());
    }

    #[test]
    fn validate_public_url_rejects_unspecified_and_mapped() {
        assert!(validate_public_url("http://0.0.0.0/", false).is_err());
        // IPv4-mapped IPv6 form of a loopback address.
        assert!(validate_public_url("http://[::ffff:127.0.0.1]/", false).is_err());
    }

    #[test]
    fn validate_public_url_rejects_ipv4_special_use_ranges() {
        // Benchmarking 198.18.0.0/15.
        assert!(validate_public_url("http://198.18.0.1/", false).is_err());
        assert!(validate_public_url("http://198.19.255.255/", false).is_err());
        // Multicast 224.0.0.0/4.
        assert!(validate_public_url("http://224.0.0.1/", false).is_err());
        assert!(validate_public_url("http://239.255.255.255/", false).is_err());
        // Reserved / future use 240.0.0.0/4.
        assert!(validate_public_url("http://240.0.0.1/", false).is_err());
        assert!(validate_public_url("http://255.255.255.254/", false).is_err());
        // IETF protocol assignments 192.0.0.0/24.
        assert!(validate_public_url("http://192.0.0.1/", false).is_err());
        // 6to4 relay anycast 192.88.99.0/24.
        assert!(validate_public_url("http://192.88.99.1/", false).is_err());
    }

    #[test]
    fn validate_public_url_rejects_ipv6_special_use_ranges() {
        // Multicast ff00::/8.
        assert!(validate_public_url("http://[ff02::1]/", false).is_err());
        // Documentation 2001:db8::/32.
        assert!(validate_public_url("http://[2001:db8::1]/", false).is_err());
    }

    #[test]
    fn validate_public_url_still_accepts_ordinary_public_ips() {
        // Guard against the extended denylist over-blocking real public hosts.
        assert!(validate_public_url("http://8.8.8.8/", false).is_ok());
        assert!(validate_public_url("http://1.1.1.1/", false).is_ok());
        assert!(validate_public_url("http://93.184.216.34/", false).is_ok());
    }

    #[test]
    fn validate_public_url_allow_private_admits_blocked_hosts() {
        // With allow_private, scheme + host validity are still enforced, but the
        // IP/localhost screening is bypassed so fake-IP-proxy targets resolve.
        assert!(validate_public_url("http://198.18.0.25/", true).is_ok());
        assert!(validate_public_url("http://127.0.0.1/", true).is_ok());
        assert!(validate_public_url("http://192.168.1.1/", true).is_ok());
        assert!(validate_public_url("http://localhost/", true).is_ok());
        // Scheme validation is not relaxed by allow_private.
        assert!(validate_public_url("file:///etc/passwd", true).is_err());
    }

    #[test]
    fn screen_resolved_addrs_rejects_empty() {
        assert!(screen_resolved_addrs(Vec::new(), false).is_err());
    }

    #[test]
    fn screen_resolved_addrs_rejects_any_blocked_address() {
        // A hostname that resolves to both a public and a loopback address must
        // be rejected: this is the DNS-rebinding vector.
        let addrs: Vec<SocketAddr> =
            vec!["93.184.216.34:0".parse().unwrap(), "127.0.0.1:0".parse().unwrap()];
        assert!(screen_resolved_addrs(addrs, false).is_err());
    }

    #[test]
    fn screen_resolved_addrs_rejects_link_local_metadata() {
        let addrs: Vec<SocketAddr> = vec!["169.254.169.254:0".parse().unwrap()];
        assert!(screen_resolved_addrs(addrs, false).is_err());
    }

    #[test]
    fn screen_resolved_addrs_accepts_all_public() {
        let addrs: Vec<SocketAddr> =
            vec!["93.184.216.34:0".parse().unwrap(), "8.8.8.8:0".parse().unwrap()];
        assert_eq!(screen_resolved_addrs(addrs.clone(), false).unwrap(), addrs);
    }

    #[test]
    fn screen_resolved_addrs_allow_private_admits_blocked_but_still_requires_addr() {
        // allow_private lets fake-IP-proxy placeholders through...
        let addrs: Vec<SocketAddr> = vec!["198.18.0.25:0".parse().unwrap()];
        assert_eq!(screen_resolved_addrs(addrs.clone(), true).unwrap(), addrs);
        // ...but an empty resolution is still an error.
        assert!(screen_resolved_addrs(Vec::new(), true).is_err());
    }

    #[test]
    fn meta_refresh_target_follows_noscript_stub() {
        // Baidu's HTTPS homepage: a JS `location.replace` downgrade plus a
        // `<noscript>`-wrapped meta-refresh to the HTTP page. We must find the
        // meta even though it lives inside noscript (html5ever hides it from the
        // DOM), so the scan runs over the raw HTML string.
        let base = reqwest::Url::parse("https://www.baidu.com/").unwrap();
        let html = r#"<html><head><script>location.replace(x)</script></head>
            <body><noscript><meta http-equiv="refresh" content="0;url=http://www.baidu.com/"></noscript></body></html>"#;
        let target = meta_refresh_target(html, &base).expect("should find meta refresh target");
        assert_eq!(target.as_str(), "http://www.baidu.com/");
    }

    #[test]
    fn meta_refresh_target_resolves_relative_and_is_case_insensitive() {
        let base = reqwest::Url::parse("https://example.com/a/b").unwrap();
        let html = r#"<META HTTP-EQUIV="Refresh" CONTENT="5; URL=/next/page">"#;
        let target = meta_refresh_target(html, &base).unwrap();
        assert_eq!(target.as_str(), "https://example.com/next/page");
    }

    #[test]
    fn meta_refresh_target_handles_unquoted_and_single_quoted() {
        let base = reqwest::Url::parse("https://example.com/").unwrap();
        let single = r#"<meta http-equiv='refresh' content='0;url=https://example.com/x'>"#;
        assert_eq!(meta_refresh_target(single, &base).unwrap().as_str(), "https://example.com/x");
    }

    #[test]
    fn meta_refresh_target_none_when_not_a_redirect() {
        let base = reqwest::Url::parse("https://example.com/").unwrap();
        // No meta at all.
        assert!(meta_refresh_target(
            "<html><head><title>Hi</title></head><body><p>real content</p></body></html>",
            &base
        )
        .is_none());
        // A self-refresh (no url=) is not a redirect target.
        assert!(meta_refresh_target(r#"<meta http-equiv="refresh" content="30">"#, &base).is_none());
        // A non-refresh http-equiv must be ignored even if its value mentions url=.
        assert!(meta_refresh_target(
            r#"<meta http-equiv="content-type" content="text/html; url=http://evil/">"#,
            &base
        )
        .is_none());
    }
}

#[cfg(test)]
mod context_tests {
    use super::*;
    use crate::canvas::document::{CanvasEdge, CanvasNode, Viewport};

    fn node(id: &str, ty: &str, data: serde_json::Value) -> CanvasNode {
        CanvasNode {
            id: id.to_string(),
            r#type: ty.to_string(),
            x: 0.0,
            y: 0.0,
            w: 280.0,
            h: 160.0,
            data,
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> CanvasEdge {
        CanvasEdge { id: id.to_string(), source: source.to_string(), target: target.to_string() }
    }

    #[test]
    fn collect_reference_blocks_includes_note_and_url_text() {
        let doc = CanvasDocument {
            nodes: vec![
                node("n1", "note", serde_json::json!({ "markdown": "a note body" })),
                node(
                    "u1",
                    "url",
                    serde_json::json!({ "title": "Example", "markdown": "page body" }),
                ),
                node("t", "ai_analyze", serde_json::json!({})),
            ],
            edges: vec![edge("e1", "n1", "t"), edge("e2", "u1", "t")],
            viewport: Viewport::default(),
        };
        let blocks = collect_reference_blocks(&doc, "t");
        assert_eq!(blocks.len(), 2);
        assert!(blocks.iter().any(|b| b.contains("Note:") && b.contains("a note body")));
        assert!(blocks
            .iter()
            .any(|b| b.contains("Web page: Example") && b.contains("page body")));
    }

    #[test]
    fn collect_reference_blocks_uses_active_version_of_prior_analysis() {
        let analysis = serde_json::json!({
            "versions": [{ "id": "v1", "content": "prior result" }],
            "activeVersionId": "v1"
        });
        let doc = CanvasDocument {
            nodes: vec![
                node("a1", "ai_analyze", analysis),
                node("t", "ai_analyze", serde_json::json!({})),
            ],
            edges: vec![edge("e1", "a1", "t")],
            viewport: Viewport::default(),
        };
        let blocks = collect_reference_blocks(&doc, "t");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("Prior analysis:") && blocks[0].contains("prior result"));
    }

    #[test]
    fn referenced_kb_project_ids_collects_incoming_kb_projects() {
        let doc = CanvasDocument {
            nodes: vec![
                node("k1", "kb", serde_json::json!({ "projectId": "proj-1" })),
                node("n1", "note", serde_json::json!({ "markdown": "x" })),
                node("t", "ai_analyze", serde_json::json!({})),
            ],
            edges: vec![edge("e1", "k1", "t"), edge("e2", "n1", "t")],
            viewport: Viewport::default(),
        };
        let ids = referenced_kb_project_ids(&doc, "t");
        assert_eq!(ids, vec!["proj-1".to_string()]);
    }

    #[test]
    fn search_results_to_markdown_formats_entries() {
        let results = vec![
            SearchResultEntry {
                title: "First".into(),
                url: "https://a.test".into(),
                snippet: "snippet one".into(),
            },
            SearchResultEntry {
                title: "Second".into(),
                url: "https://b.test".into(),
                snippet: "snippet two".into(),
            },
        ];
        let md = search_results_to_markdown("cats", &results);
        assert!(md.starts_with("Search results for \"cats\":"));
        assert!(md.contains("- [First](https://a.test)"));
        assert!(md.contains("snippet two"));
    }

    #[test]
    fn build_analyze_prompt_combines_prompt_and_blocks() {
        let blocks = vec!["block A".to_string(), "block B".to_string()];
        let prompt = build_analyze_prompt("summarize", &blocks);
        assert!(prompt.contains("--- Source 1 ---\nblock A"));
        assert!(prompt.contains("--- Source 2 ---\nblock B"));
        assert!(prompt.contains("Task:\nsummarize"));
    }

    #[test]
    fn build_analyze_prompt_omits_sources_when_empty() {
        let prompt = build_analyze_prompt("just do it", &[]);
        assert!(!prompt.contains("referenced sources"));
        assert!(prompt.contains("Task:\njust do it"));
    }
}
