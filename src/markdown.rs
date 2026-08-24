use pulldown_cmark::{html, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

#[derive(Debug, Clone, Serialize)]
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub text: String,
}

pub struct Rendered {
    pub html: String,
    pub headings: Vec<Heading>,
}

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH
        | Options::ENABLE_HEADING_ATTRIBUTES
}

/// Markdown to HTML, with build-time syntax highlighting, stable heading ids,
/// and math handed to KaTeX as `\(...\)` / `\[...\]` so the emphasis rules in
/// CommonMark never chew on a subscript.
pub fn render(src: &str) -> Rendered {
    let mut out: Vec<Event> = Vec::new();
    let mut headings: Vec<Heading> = Vec::new();
    let mut seen_ids: HashMap<String, usize> = HashMap::new();

    // Set while inside a fenced code block: (language, accumulated source).
    let mut code: Option<(String, String)> = None;
    // Set while inside a heading: (level, explicit id, buffered inner events, plain text).
    let mut heading: Option<(HeadingLevel, Option<String>, Vec<Event>, String)> = None;

    for event in Parser::new_ext(src, options()) {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match &kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                code = Some((lang, String::new()));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((lang, source)) = code.take() {
                    out.push(Event::Html(highlight(&lang, &source).into()));
                }
            }
            Event::Start(Tag::Heading { level, id, .. }) => {
                heading = Some((level, id.map(|i| i.to_string()), Vec::new(), String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, explicit_id, inner, text)) = heading.take() {
                    let id = unique_id(explicit_id.unwrap_or_else(|| slugify(&text)), &mut seen_ids);
                    let n = level_number(level);
                    out.push(Event::Html(format!("<h{n} id=\"{id}\">").into()));
                    out.extend(inner);
                    out.push(Event::Html(format!("</h{n}>").into()));
                    headings.push(Heading { level: n, id, text });
                }
            }
            Event::InlineMath(tex) => out.push(Event::Html(
                format!("\\({}\\)", escape_html(&tex)).into(),
            )),
            Event::DisplayMath(tex) => out.push(Event::Html(
                format!("\\[{}\\]", escape_html(&tex)).into(),
            )),
            other => {
                // Text inside a fence is source, not prose; text inside a heading
                // also feeds the anchor slug.
                if let Some((_, buf)) = code.as_mut() {
                    if let Event::Text(t) = &other {
                        buf.push_str(t);
                        continue;
                    }
                }
                if let Some((_, _, inner, text)) = heading.as_mut() {
                    match &other {
                        Event::Text(t) | Event::Code(t) => text.push_str(t),
                        _ => {}
                    }
                    inner.push(other);
                    continue;
                }
                out.push(other);
            }
        }
    }

    let mut html = String::with_capacity(src.len() * 3 / 2);
    html::push_html(&mut html, out.into_iter());
    Rendered { html, headings }
}

fn level_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let mut themes = ThemeSet::load_defaults();
        themes
            .themes
            .remove("InspiredGitHub")
            .expect("syntect ships InspiredGitHub")
    })
}

fn highlight(lang: &str, source: &str) -> String {
    let set = syntaxes();
    let syntax = if lang.is_empty() {
        None
    } else {
        set.find_syntax_by_token(lang)
            .or_else(|| set.find_syntax_by_extension(lang))
    };
    match syntax.and_then(|s| highlighted_html_for_string(source, set, s, theme()).ok()) {
        Some(html) => format!("<div class=\"highlight\">{html}</div>"),
        None => format!(
            "<div class=\"highlight\"><pre><code>{}</code></pre></div>",
            escape_html(source)
        ),
    }
}

fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            slug.extend(ch.to_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "section".into()
    } else {
        slug
    }
}

/// Second and later uses of an id get a numeric suffix, as Hugo does.
fn unique_id(id: String, seen: &mut HashMap<String, usize>) -> String {
    let count = seen.entry(id.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        id
    } else {
        format!("{id}-{}", *count - 1)
    }
}

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Strip tags and collapse whitespace — used to build feed/meta descriptions.
pub fn plain_text(html: &str, limit: usize) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            c if c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
        if out.chars().count() >= limit {
            break;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_get_slug_anchors() {
        let r = render("## Hello World\n\n## Hello World\n");
        assert!(r.html.contains("<h2 id=\"hello-world\">"));
        assert!(r.html.contains("<h2 id=\"hello-world-1\">"));
        assert_eq!(r.headings.len(), 2);
        assert_eq!(r.headings[0].text, "Hello World");
    }

    #[test]
    fn math_survives_underscores() {
        let r = render("$x_1 + y_2$\n");
        assert!(r.html.contains("\\(x_1 + y_2\\)"), "{}", r.html);
        assert!(!r.html.contains("<em>"));
    }

    #[test]
    fn fenced_code_is_highlighted_not_emphasised() {
        let r = render("```python\nx = 1\n```\n");
        assert!(r.html.contains("class=\"highlight\""));
        // syntect wraps each token, so look for the styled spans rather than
        // the raw line.
        assert!(r.html.contains("<span"), "{}", r.html);
        assert!(plain_text(&r.html, 100).contains("x = 1"), "{}", r.html);
    }

    #[test]
    fn tables_are_enabled() {
        let r = render("| a | b |\n| - | - |\n| 1 | 2 |\n");
        assert!(r.html.contains("<table>"));
    }
}
