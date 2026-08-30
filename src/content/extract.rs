use std::collections::{HashMap, HashSet};

use ammonia::{Builder, UrlRelative};
use chrono::{DateTime, NaiveDate, NaiveDateTime};
use scraper::{ElementRef, Html, Selector};
use sha2::{Digest, Sha256};
use url::Url;

use super::{ContentError, MAX_READER_BYTES, MAX_SOURCE_BYTES};

const MAX_URL_BYTES: usize = 8_192;
const MAX_TITLE_CHARACTERS: usize = 1_000;
const MAX_AUTHOR_CHARACTERS: usize = 500;
const MAX_EXCERPT_CHARACTERS: usize = 500;
const MAX_CANDIDATES_PER_SELECTOR: usize = 8;
const MAX_METADATA_TEXT_CANDIDATES_PER_SELECTOR: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedDocument {
    pub final_url: String,
    pub canonical_url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<i64>,
    pub excerpt: String,
    pub safe_html: String,
    pub plain_text: String,
    pub source_html: Vec<u8>,
    pub content_hash: String,
}

pub fn extract_document(
    final_url: &str,
    source_html: Vec<u8>,
) -> Result<ExtractedDocument, ContentError> {
    if source_html.len() > MAX_SOURCE_BYTES {
        return Err(ContentError::ResponseTooLarge);
    }
    let final_url = parse_document_url(final_url)?;
    let decoded = String::from_utf8_lossy(&source_html);
    let document = Html::parse_document(&decoded);

    let (safe_html, plain_text) =
        extract_reader_content(&document, &final_url)?.ok_or(ContentError::EmptyContent)?;
    let title = extract_title(&document).map(|value| truncate_chars(&value, MAX_TITLE_CHARACTERS));
    let author =
        extract_author(&document).map(|value| truncate_chars(&value, MAX_AUTHOR_CHARACTERS));
    let published_at = extract_published_at(&document);
    let excerpt = extract_description(&document)
        .map(|value| truncate_chars(&value, MAX_EXCERPT_CHARACTERS))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| truncate_chars(&plain_text, MAX_EXCERPT_CHARACTERS));
    let content_hash = format!("{:x}", Sha256::digest(safe_html.as_bytes()));

    Ok(ExtractedDocument {
        final_url: final_url.to_string(),
        canonical_url: extract_canonical_url(&document, &final_url),
        title,
        author,
        published_at,
        excerpt,
        safe_html,
        plain_text,
        source_html,
        content_hash,
    })
}

fn parse_document_url(input: &str) -> Result<Url, ContentError> {
    if input.len() > MAX_URL_BYTES {
        return Err(ContentError::InvalidUrl);
    }
    let mut url = Url::parse(input).map_err(|_| ContentError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ContentError::UnsupportedScheme);
    }
    if has_userinfo(&url) {
        return Err(ContentError::UserInfoNotAllowed);
    }
    if url.host().is_none() {
        return Err(ContentError::MissingHost);
    }
    url.set_fragment(None);
    Ok(url)
}

fn extract_reader_content(
    document: &Html,
    base_url: &Url,
) -> Result<Option<(String, String)>, ContentError> {
    let mut best: Option<(usize, ElementRef<'_>)> = None;
    let mut seen = HashSet::new();
    for (selector, weight) in [
        ("article, [itemprop=\"articleBody\"]", 4_usize),
        ("main, [role=\"main\"]", 3),
        (
            ".article-body, .article-content, .entry-content, .post-content, .story-content, #article-body, #article-content, #main-content, #content",
            3,
        ),
        ("[class*=\"content\"], [id*=\"content\"]", 2),
    ] {
        let selector = Selector::parse(selector).expect("valid reader selector");
        let mut candidate_count = 0;
        for element in document.select(&selector) {
            if !seen.insert(element.id()) {
                continue;
            }
            candidate_count += 1;
            if candidate_count > MAX_CANDIDATES_PER_SELECTOR {
                break;
            }
            let text_bytes = element
                .text()
                .map(str::trim)
                .map(str::len)
                .fold(0_usize, usize::saturating_add);
            if text_bytes == 0 {
                continue;
            }
            let score = text_bytes.saturating_mul(weight);
            if best
                .as_ref()
                .is_none_or(|(best_score, _)| score > *best_score)
            {
                best = Some((score, element));
            }
        }
    }
    if let Some((_, element)) = best
        && let Some(candidate) = sanitize_candidate(element, base_url)?
    {
        return Ok(Some(candidate));
    }

    let selector = Selector::parse("body").expect("valid body selector");
    let Some(element) = document.select(&selector).next() else {
        return Ok(None);
    };
    sanitize_candidate(element, base_url)
}

fn sanitize_candidate(
    element: ElementRef<'_>,
    base_url: &Url,
) -> Result<Option<(String, String)>, ContentError> {
    let candidate_html = element.inner_html();
    if candidate_html.len() > MAX_READER_BYTES {
        return Err(ContentError::ResponseTooLarge);
    }
    let remaining_growth = MAX_READER_BYTES - candidate_html.len();
    if relative_url_growth(element, base_url, remaining_growth).is_none() {
        return Err(ContentError::ResponseTooLarge);
    }

    let safe_html = sanitize_html(&candidate_html, base_url);
    if safe_html.len() > MAX_READER_BYTES {
        return Err(ContentError::ResponseTooLarge);
    }
    let plain_text = html_to_plain_text(&safe_html);
    if plain_text.len() > MAX_READER_BYTES {
        return Err(ContentError::ResponseTooLarge);
    }
    Ok((!plain_text.is_empty()).then_some((safe_html, plain_text)))
}

fn relative_url_growth(
    element: ElementRef<'_>,
    base_url: &Url,
    max_growth: usize,
) -> Option<usize> {
    let selector = Selector::parse("[href], [cite], [src]").expect("valid URL attribute selector");
    let values = element
        .select(&selector)
        .flat_map(|element| {
            [
                element.attr("href"),
                element.attr("cite"),
                element.attr("src"),
            ]
        })
        .flatten();
    bounded_relative_url_growth(values, base_url, max_growth)
}

fn bounded_relative_url_growth<'a>(
    values: impl IntoIterator<Item = &'a str>,
    base_url: &Url,
    max_growth: usize,
) -> Option<usize> {
    let mut growth = 0_usize;
    for value in values {
        let Ok(resolved) = base_url.join(value) else {
            continue;
        };
        let delta = resolved.as_str().len().saturating_sub(value.len());
        growth = growth.checked_add(delta)?;
        if growth > max_growth {
            return None;
        }
    }
    Some(growth)
}

fn sanitize_html(input: &str, base_url: &Url) -> String {
    let tags: HashSet<_> = [
        "a",
        "abbr",
        "address",
        "article",
        "b",
        "blockquote",
        "br",
        "caption",
        "cite",
        "code",
        "col",
        "colgroup",
        "dd",
        "del",
        "details",
        "dfn",
        "div",
        "dl",
        "dt",
        "em",
        "figcaption",
        "figure",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "header",
        "hr",
        "i",
        "img",
        "ins",
        "kbd",
        "li",
        "main",
        "mark",
        "ol",
        "p",
        "pre",
        "q",
        "s",
        "samp",
        "section",
        "small",
        "span",
        "strong",
        "sub",
        "summary",
        "sup",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "time",
        "tr",
        "u",
        "ul",
        "var",
        "wbr",
    ]
    .into_iter()
    .collect();
    let clean_content_tags: HashSet<_> = [
        "audio", "button", "canvas", "dialog", "embed", "footer", "form", "iframe", "input", "nav",
        "noscript", "object", "option", "script", "select", "style", "svg", "template", "textarea",
        "video",
    ]
    .into_iter()
    .collect();
    let tag_attributes: HashMap<_, HashSet<_>> = [
        ("a", ["href", "title"].into_iter().collect()),
        ("abbr", ["title"].into_iter().collect()),
        ("blockquote", ["cite"].into_iter().collect()),
        ("del", ["cite", "datetime"].into_iter().collect()),
        ("details", ["open"].into_iter().collect()),
        ("ins", ["cite", "datetime"].into_iter().collect()),
        (
            "img",
            ["alt", "height", "src", "title", "width"]
                .into_iter()
                .collect(),
        ),
        ("q", ["cite"].into_iter().collect()),
        ("time", ["datetime"].into_iter().collect()),
    ]
    .into_iter()
    .collect();
    let url_schemes: HashSet<_> = ["http", "https", "mailto"].into_iter().collect();
    let attribute_base_url = base_url.clone();

    let mut builder = Builder::new();
    builder
        .tags(tags)
        .clean_content_tags(clean_content_tags)
        .tag_attributes(tag_attributes)
        .generic_attributes(HashSet::new())
        .url_schemes(url_schemes)
        .set_tag_attribute_value("img", "loading", "lazy")
        .set_tag_attribute_value("img", "decoding", "async")
        .set_tag_attribute_value("img", "referrerpolicy", "no-referrer")
        .attribute_filter(move |_element, attribute, value| {
            if matches!(attribute, "href" | "cite" | "src") {
                let Ok(url) = attribute_base_url.join(value) else {
                    return None;
                };
                if has_userinfo(&url)
                    || (attribute == "src" && !matches!(url.scheme(), "http" | "https"))
                {
                    return None;
                }
            }
            Some(value.into())
        })
        .url_relative(UrlRelative::RewriteWithBase(base_url.clone()))
        .link_rel(Some("noopener noreferrer"));
    builder.clean(input).to_string()
}

fn html_to_plain_text(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    collapse_whitespace(&fragment.root_element().text().collect::<Vec<_>>().join(" "))
}

fn extract_title(document: &Html) -> Option<String> {
    first_attribute(
        document,
        &[
            ("meta[property=\"og:title\"]", "content"),
            ("meta[name=\"twitter:title\"]", "content"),
        ],
    )
    .or_else(|| first_text(document, &["title", "h1"]))
}

fn extract_author(document: &Html) -> Option<String> {
    first_attribute(
        document,
        &[
            ("meta[name=\"author\"]", "content"),
            ("meta[property=\"article:author\"]", "content"),
            ("meta[itemprop=\"author\"]", "content"),
        ],
    )
    .or_else(|| first_text(document, &["[itemprop=\"author\"]", "[rel=\"author\"]"]))
}

fn extract_description(document: &Html) -> Option<String> {
    first_attribute(
        document,
        &[
            ("meta[name=\"description\"]", "content"),
            ("meta[property=\"og:description\"]", "content"),
            ("meta[name=\"twitter:description\"]", "content"),
        ],
    )
}

fn extract_published_at(document: &Html) -> Option<i64> {
    let value = first_attribute(
        document,
        &[
            ("meta[property=\"article:published_time\"]", "content"),
            ("meta[itemprop=\"datePublished\"]", "content"),
            ("meta[name=\"date\"]", "content"),
            ("time[itemprop=\"datePublished\"]", "datetime"),
            ("time[datetime]", "datetime"),
        ],
    )?;
    parse_published_at(&value)
}

fn parse_published_at(value: &str) -> Option<i64> {
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Some(value.timestamp_millis());
    }
    if let Ok(value) = DateTime::parse_from_rfc2822(value) {
        return Some(value.timestamp_millis());
    }
    for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(value) = NaiveDateTime::parse_from_str(value, format) {
            return Some(value.and_utc().timestamp_millis());
        }
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .and_then(|value| value.and_hms_opt(0, 0, 0))
        .map(|value| value.and_utc().timestamp_millis())
}

fn extract_canonical_url(document: &Html, final_url: &Url) -> Option<String> {
    let selector = Selector::parse("link[href][rel]").expect("valid canonical selector");
    for element in document.select(&selector) {
        let is_canonical = element.attr("rel").is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|token| token.eq_ignore_ascii_case("canonical"))
        });
        if !is_canonical {
            continue;
        }
        let Some(href) = element.attr("href") else {
            continue;
        };
        let Ok(mut candidate) = final_url.join(href) else {
            continue;
        };
        if !matches!(candidate.scheme(), "http" | "https")
            || has_userinfo(&candidate)
            || candidate.host_str() != final_url.host_str()
        {
            continue;
        }
        candidate.set_fragment(None);
        if candidate.as_str().len() <= MAX_URL_BYTES {
            return Some(candidate.to_string());
        }
    }
    None
}

fn first_attribute(document: &Html, selectors: &[(&str, &str)]) -> Option<String> {
    selectors.iter().find_map(|(selector, attribute)| {
        let selector = Selector::parse(selector).expect("valid metadata selector");
        document.select(&selector).find_map(|element| {
            element
                .attr(attribute)
                .map(collapse_whitespace)
                .filter(|value| !value.is_empty())
        })
    })
}

fn first_text(document: &Html, selectors: &[&str]) -> Option<String> {
    selectors.iter().find_map(|selector| {
        let selector = Selector::parse(selector).expect("valid text selector");
        document
            .select(&selector)
            .take(MAX_METADATA_TEXT_CANDIDATES_PER_SELECTOR)
            .find_map(|element| {
                let value = collapse_whitespace(&element.text().collect::<Vec<_>>().join(" "));
                (!value.is_empty()).then_some(value)
            })
    })
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(value: &str, max_characters: usize) -> String {
    value.chars().take(max_characters).collect()
}

fn has_userinfo(url: &Url) -> bool {
    let Some((_, remainder)) = url.as_str().split_once("://") else {
        return false;
    };
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    remainder[..authority_end].contains('@')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_chinese_article_and_metadata() {
        let source = r#"
            <!doctype html>
            <html>
              <head>
                <title>  一篇安静的文章  </title>
                <meta name="author" content=" 林 风 ">
                <meta property="article:published_time" content="2026-08-24T02:03:04+08:00">
                <meta name="description" content="这是文章摘要。">
              </head>
              <body>
                <nav>首页 归档</nav>
                <main><article><h1>一篇安静的文章</h1><p>清晨的风穿过树叶，桌上的书翻到新的一页。</p><p>这里是第二段正文。</p></article></main>
              </body>
            </html>
        "#
        .as_bytes()
        .to_vec();

        let extracted = extract_document("https://example.com/read/1#comments", source.clone())
            .expect("document extracts");

        assert_eq!(extracted.final_url, "https://example.com/read/1");
        assert_eq!(extracted.title.as_deref(), Some("一篇安静的文章"));
        assert_eq!(extracted.author.as_deref(), Some("林 风"));
        assert_eq!(extracted.published_at, Some(1_787_508_184_000));
        assert_eq!(extracted.excerpt, "这是文章摘要。");
        assert!(extracted.plain_text.contains("清晨的风穿过树叶"));
        assert!(!extracted.plain_text.contains("首页 归档"));
        assert_eq!(extracted.source_html, source);
        assert_eq!(
            extracted.content_hash,
            format!("{:x}", Sha256::digest(extracted.safe_html.as_bytes()))
        );
    }

    #[test]
    fn sanitizes_malicious_markup_with_explicit_link_policy() {
        let source = br#"
            <main id="reader" class="content" style="color:red" onclick="steal()">
              <script>alert(document.cookie)</script>
              <style>body { display: none }</style>
              <form><input name="secret"><p>Form content</p></form>
              <iframe src="https://evil.example/"><p>Frame content</p></iframe>
              <nav>Navigation trap</nav>
              <p id="copy" class="lead" style="font-size:999px">Safe copy</p>
              <a href="javascript:alert(1)" target="_blank" onclick="steal()">Bad link</a>
              <a href="https://trusted.example@evil.example/read">Deceptive link</a>
              <a href="/next" rel="opener" style="color:red">Good link</a>
              <img src="/media/hero.jpg" onerror="steal()" alt="Article illustration" width="1200" height="800">
              <img src="data:image/png;base64,tracking" alt="Embedded tracker">
              <svg><script>alert(1)</script></svg>
            </main>
        "#
        .to_vec();

        let extracted = extract_document("https://example.com/articles/one", source)
            .expect("document extracts");

        for forbidden in [
            "<script",
            "<style",
            "<form",
            "<iframe",
            "<nav",
            "<svg",
            "onclick",
            "javascript:",
            "trusted.example@evil.example",
            "target=",
            "style=",
            "class=",
            "id=",
            "Form content",
            "Frame content",
            "Navigation trap",
        ] {
            assert!(
                !extracted.safe_html.contains(forbidden),
                "found {forbidden}"
            );
        }
        assert!(
            extracted
                .safe_html
                .contains("href=\"https://example.com/next\"")
        );
        assert!(extracted.safe_html.contains("rel=\"noopener noreferrer\""));
        assert!(
            extracted
                .safe_html
                .contains("src=\"https://example.com/media/hero.jpg\"")
        );
        assert!(extracted.safe_html.contains("alt=\"Article illustration\""));
        assert!(extracted.safe_html.contains("loading=\"lazy\""));
        assert!(extracted.safe_html.contains("decoding=\"async\""));
        assert!(
            extracted
                .safe_html
                .contains("referrerpolicy=\"no-referrer\"")
        );
        assert!(!extracted.safe_html.contains("data:image"));
        assert!(!extracted.safe_html.contains("onerror"));
        assert!(extracted.plain_text.contains("Safe copy"));
    }

    #[test]
    fn accepts_only_safe_same_host_canonical_url() {
        let cases = [
            ("/canonical#section", Some("https://example.com/canonical")),
            (
                "https://example.com:443/canonical",
                Some("https://example.com/canonical"),
            ),
            ("https://other.example/canonical", None),
            ("https://user@example.com/canonical", None),
            ("javascript:alert(1)", None),
        ];

        for (canonical, expected) in cases {
            let source = format!(
                "<html><head><link rel=\"alternate CANONICAL\" href=\"{canonical}\"></head><body><article><p>Body</p></article></body></html>"
            )
            .into_bytes();
            let extracted =
                extract_document("https://example.com/story", source).expect("document extracts");
            assert_eq!(extracted.canonical_url.as_deref(), expected, "{canonical}");
        }
    }

    #[test]
    fn derives_nonempty_excerpt_from_body() {
        let extracted = extract_document(
            "https://example.com/story",
            "<article><p>The body becomes the fallback excerpt.</p></article>"
                .as_bytes()
                .to_vec(),
        )
        .expect("document extracts");

        assert_eq!(extracted.excerpt, "The body becomes the fallback excerpt.");
    }

    #[test]
    fn does_not_prefer_tiny_article_over_substantial_main_content() {
        let extracted = extract_document(
            "https://example.com/story",
            br#"
                <article><p>Promo</p></article>
                <main><p>This is the substantial reader content with enough text to outrank a tiny article teaser.</p></main>
            "#
            .to_vec(),
        )
        .expect("document extracts");

        assert!(extracted.plain_text.contains("substantial reader content"));
        assert!(!extracted.plain_text.contains("Promo"));
    }

    #[test]
    fn rejects_reader_output_that_relative_links_would_expand_past_blob_limit() {
        let base_url = format!("https://example.com/{}/", "a".repeat(8_000));
        let links = "<a href=\"next\">Read</a>".repeat(1_100);
        let source = format!("<main>{links}</main>").into_bytes();

        assert_eq!(
            extract_document(&base_url, source),
            Err(ContentError::ResponseTooLarge)
        );
    }

    #[test]
    fn stops_relative_url_growth_scan_when_budget_is_exceeded() {
        let base_url = Url::parse("https://example.com/a/long/base/").expect("valid base URL");
        let values = std::iter::once("next").chain(std::iter::once_with(|| {
            panic!("relative URL scan continued after exceeding its budget")
        }));

        assert_eq!(bounded_relative_url_growth(values, &base_url, 0), None);
    }

    #[test]
    fn bounds_empty_nested_candidates_before_scoring_them() {
        let depth = 2_000;
        let source = format!(
            "{}{}<main><p>Bounded candidate scan.</p></main>",
            "<article>".repeat(depth),
            "</article>".repeat(depth)
        )
        .into_bytes();

        let extracted = extract_document("https://example.com/story", source)
            .expect("bounded candidate document extracts");

        assert_eq!(extracted.plain_text, "Bounded candidate scan.");
    }

    #[test]
    fn bounds_empty_nested_author_candidates_before_reading_text() {
        let depth = 2_000;
        let source = format!(
            "{}{}<span itemprop=\"author\">Ignored author</span><main><p>Reader body.</p></main>",
            "<span itemprop=\"author\">".repeat(depth),
            "</span>".repeat(depth)
        )
        .into_bytes();

        let extracted = extract_document("https://example.com/story", source)
            .expect("bounded author candidate document extracts");

        assert_eq!(extracted.author, None);
        assert_eq!(extracted.plain_text, "Reader body.");
    }

    #[test]
    fn ignores_unparseable_published_date() {
        let extracted = extract_document(
            "https://example.com/story",
            br#"<meta property="article:published_time" content="sometime soon"><article><p>Body</p></article>"#.to_vec(),
        )
        .expect("document extracts");

        assert_eq!(extracted.published_at, None);
    }

    #[test]
    fn rejects_empty_reader_content() {
        assert_eq!(
            extract_document(
                "https://example.com/empty",
                b"<main><script>alert(1)</script><nav>Only navigation</nav></main>".to_vec(),
            ),
            Err(ContentError::EmptyContent)
        );
    }
}
