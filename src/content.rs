use crate::config::{Config, Link};
use crate::markdown::{self, Heading};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Front matter exactly as it appears in the `.md` file. Aliases keep the
/// Hugo/PaperMod spellings working so existing content needs no migration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Meta {
    title: String,
    date: Option<String>,
    lastmod: Option<String>,
    description: String,
    summary: String,
    tags: Vec<String>,
    author: Authors,
    layout: Option<String>,
    weight: i64,
    draft: bool,
    math: Option<bool>,
    #[serde(alias = "showToc", alias = "ShowToc")]
    toc: bool,
    hidemeta: bool,
    cover: Option<Cover>,
    #[serde(alias = "editPost")]
    edit_post: Option<Link>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Authors {
    One(String),
    Many(Vec<String>),
}

impl Default for Authors {
    fn default() -> Self {
        Authors::Many(Vec::new())
    }
}

impl Authors {
    fn into_vec(self) -> Vec<String> {
        match self {
            Authors::One(s) if s.is_empty() => Vec::new(),
            Authors::One(s) => vec![s],
            Authors::Many(v) => v,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cover {
    pub image: String,
    #[serde(default)]
    pub alt: String,
}

/// A single output page, flattened so templates can read fields directly.
#[derive(Debug, Clone, Serialize)]
pub struct Page {
    pub url: String,
    pub permalink: String,
    pub title: String,
    pub description: String,
    pub summary: String,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    /// Tag name plus its resolved `/tags/<slug>/` URL, so templates never have
    /// to re-derive a slug.
    pub tag_links: Vec<Link>,
    /// Human-readable date, formatted with `date_format` from the config.
    pub date: Option<String>,
    pub date_iso: Option<String>,
    pub lastmod: Option<String>,
    pub section: Option<String>,
    pub is_section_index: bool,
    pub layout: Option<String>,
    pub weight: i64,
    pub math: bool,
    pub toc: bool,
    pub hidemeta: bool,
    pub cover: Option<Cover>,
    pub edit_post: Option<Link>,
    pub content: String,
    pub headings: Vec<Heading>,

    #[serde(skip)]
    pub sort_key: i64,
    /// Sibling files of a page bundle (`index.md` plus its PDFs, images, ...).
    #[serde(skip)]
    pub assets: Vec<PathBuf>,
}

/// A content directory that has an `_index.md`, plus the pages inside it.
#[derive(Debug, Clone, Serialize)]
pub struct Section {
    pub name: String,
    pub url: String,
    pub title: String,
    pub description: String,
    pub weight: i64,
    pub pages: Vec<Page>,
    pub index: Page,
}

pub fn load(root: &Path, cfg: &Config) -> Result<(Vec<Page>, Vec<Section>), String> {
    let content_dir = root.join("content");
    let mut pages = Vec::new();

    for entry in WalkDir::new(&content_dir).sort_by_file_name() {
        let entry = entry.map_err(|e| format!("walking content/: {e}"))?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let page = parse(path, &content_dir, cfg)?;
        if let Some(page) = page {
            pages.push(page);
        }
    }

    // Split section indexes off, then hang the remaining pages under them.
    let (indexes, mut leaves): (Vec<Page>, Vec<Page>) =
        pages.into_iter().partition(|p| p.is_section_index);

    let mut sections: Vec<Section> = Vec::new();
    for index in indexes {
        let Some(name) = index.section.clone() else { continue };
        let mut pages: Vec<Page> = leaves
            .iter()
            .filter(|p| p.section.as_deref() == Some(name.as_str()))
            .cloned()
            .collect();
        sort_pages(&mut pages);
        sections.push(Section {
            name,
            url: index.url.clone(),
            title: index.title.clone(),
            description: index.description.clone(),
            weight: index.weight,
            pages,
            index,
        });
    }
    sections.sort_by(|a, b| a.weight.cmp(&b.weight).then_with(|| a.name.cmp(&b.name)));

    // Pages that a section claimed are owned by that section from here on;
    // what is left are the standalone pages (about, location, ...).
    let names: Vec<&str> = sections.iter().map(|s| s.name.as_str()).collect();
    leaves.retain(|p| match p.section.as_deref() {
        Some(section) => !names.contains(&section),
        None => true,
    });
    sort_pages(&mut leaves);
    Ok((leaves, sections))
}

/// Newest first; undated pages fall to the bottom, ordered by title.
fn sort_pages(pages: &mut [Page]) {
    pages.sort_by(|a, b| {
        b.sort_key
            .cmp(&a.sort_key)
            .then_with(|| a.title.cmp(&b.title))
    });
}

fn parse(path: &Path, content_dir: &Path, cfg: &Config) -> Result<Option<Page>, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (front, body) = split_front_matter(&raw);

    let meta: Meta = match front {
        Some(front) => serde_yaml_ng::from_str(front)
            .map_err(|e| format!("{}: bad front matter: {e}", path.display()))?,
        None => Meta::default(),
    };
    if meta.draft {
        return Ok(None);
    }

    let rel = path
        .strip_prefix(content_dir)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let file_name = rel.file_name().and_then(|f| f.to_str()).unwrap_or_default();
    let parent = rel.parent().unwrap_or(Path::new(""));

    // Where the page lives in the URL space, without leading or trailing slash.
    let (url_path, is_section_index) = match file_name {
        "_index.md" => (slash_path(parent), true),
        "index.md" => (slash_path(parent), false),
        _ => {
            let stem = rel.with_extension("");
            (slash_path(&stem), false)
        }
    };

    let comps: Vec<&str> = url_path.split('/').filter(|s| !s.is_empty()).collect();
    let section = if is_section_index {
        comps.first().map(|s| s.to_string())
    } else if comps.len() > 1 {
        Some(comps[0].to_string())
    } else {
        None
    };

    // A bundle is a directory whose markdown file is index.md / _index.md;
    // its siblings ship alongside the generated HTML.
    let assets = if file_name == "index.md" || file_name == "_index.md" {
        sibling_assets(path)
    } else {
        Vec::new()
    };

    let url = if url_path.is_empty() {
        "/".to_string()
    } else {
        format!("/{url_path}/")
    };

    let rendered = markdown::render(body);
    let date = meta.date.as_deref().and_then(parse_date);
    let lastmod = meta.lastmod.as_deref().and_then(parse_date);
    let cover = meta.cover.clone();
    let tag_links = meta
        .tags
        .iter()
        .map(|t| Link {
            name: t.clone(),
            url: cfg.url(&format!("/tags/{}/", crate::render::slugify(t))),
        })
        .collect();

    Ok(Some(Page {
        permalink: cfg.abs_url(&url),
        url,
        title: meta.title,
        description: meta.description,
        summary: meta.summary,
        authors: meta.author.into_vec(),
        tags: meta.tags,
        tag_links,
        date: date.map(|d| d.format(&cfg.date_format).to_string()),
        date_iso: date.map(|d| d.format("%Y-%m-%d").to_string()),
        lastmod: lastmod.map(|d| d.format(&cfg.date_format).to_string()),
        section,
        is_section_index,
        layout: meta.layout,
        weight: meta.weight,
        math: meta.math.unwrap_or(cfg.math),
        toc: meta.toc,
        hidemeta: meta.hidemeta,
        cover,
        edit_post: meta.edit_post,
        content: rendered.html,
        headings: rendered.headings,
        sort_key: date.map(|d| d.num_days_from_ce() as i64).unwrap_or(i64::MIN),
        assets,
    }))
}

fn sibling_assets(md_path: &Path) -> Vec<PathBuf> {
    let Some(dir) = md_path.parent() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|x| x.to_str()) != Some("md"))
        .collect()
}

fn slash_path(p: &Path) -> String {
    p.components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

/// Returns (front matter, body). Front matter is a leading `---` fenced block.
fn split_front_matter(raw: &str) -> (Option<&str>, &str) {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let Some(rest) = raw.strip_prefix("---") else {
        return (None, raw);
    };
    let rest = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n'));
    let Some(rest) = rest else {
        return (None, raw);
    };
    for (offset, line) in line_offsets(rest) {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            let body = &rest[offset + line.len()..];
            return (Some(&rest[..offset]), body.trim_start_matches(['\r', '\n']));
        }
    }
    (None, raw)
}

/// Yields each line together with its byte offset, keeping the line terminator.
fn line_offsets(s: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut start = 0usize;
    std::iter::from_fn(move || {
        if start >= s.len() {
            return None;
        }
        let end = match s[start..].find('\n') {
            Some(i) => start + i + 1,
            None => s.len(),
        };
        let item = (start, &s[start..end]);
        start = end;
        Some(item)
    })
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim().trim_matches('"');
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d);
    }
    // Tolerate full timestamps by keeping only the date part.
    let head: String = s.chars().take(10).collect();
    NaiveDate::parse_from_str(&head, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_matter_is_split_from_body() {
        let (front, body) = split_front_matter("---\ntitle: Hi\n---\n\nBody text\n");
        assert_eq!(front, Some("title: Hi\n"));
        assert_eq!(body, "Body text\n");
    }

    #[test]
    fn missing_front_matter_leaves_the_body_intact() {
        let (front, body) = split_front_matter("# Just markdown\n");
        assert_eq!(front, None);
        assert_eq!(body, "# Just markdown\n");
    }

    #[test]
    fn dates_accept_plain_and_timestamp_forms() {
        assert_eq!(parse_date("2013-01-15"), NaiveDate::from_ymd_opt(2013, 1, 15));
        assert_eq!(
            parse_date("2013-01-15T09:00:00Z"),
            NaiveDate::from_ymd_opt(2013, 1, 15)
        );
        assert_eq!(parse_date("not a date"), None);
    }
}
