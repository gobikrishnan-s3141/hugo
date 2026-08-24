use crate::config::Config;
use crate::content::{self, Page, Section};
use crate::feed;
use chrono::Datelike;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tera::{Context, Tera};

#[derive(Debug, Clone, Serialize)]
pub struct Tag {
    pub name: String,
    pub slug: String,
    pub url: String,
    pub count: usize,
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone, Serialize)]
struct YearGroup {
    year: i32,
    pages: Vec<Page>,
}

pub struct Site {
    pub cfg: Config,
    pub pages: Vec<Page>,
    pub sections: Vec<Section>,
    pub tags: Vec<Tag>,
    tera: Tera,
    root: PathBuf,
}

pub fn build(root: &Path, out: &Path) -> Result<usize, String> {
    let cfg = Config::load(&root.join("site.toml"))?;
    let (pages, sections) = content::load(root, &cfg)?;

    let glob = format!("{}/templates/**/*.html", root.display());
    let mut tera = Tera::new(&glob).map_err(|e| format!("templates: {e}"))?;
    if tera.get_template_names().count() == 0 {
        return Err(format!("no templates found in {}/templates", root.display()));
    }
    // `url(path="/papers/")` is the only way templates should build internal
    // links: it is what makes the site work under a `base_url` sub-path.
    // Tera's default escaper also encodes `/`, which turns every href into
    // `&#x2F;`-soup. Escape the five characters that actually matter instead.
    tera.set_escape_fn(crate::markdown::escape_html);
    tera.autoescape_on(vec![".html"]);
    let base_path = cfg.base_path.clone();
    tera.register_function(
        "url",
        move |args: &std::collections::HashMap<String, tera::Value>| {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("/")
                .to_string();
            let joined = if path.contains("://") || path.starts_with("mailto:") {
                path
            } else {
                format!("{}/{}", base_path, path.trim_start_matches('/'))
            };
            Ok(tera::Value::String(joined))
        },
    );

    let tags = collect_tags(&pages, &sections, &cfg);
    let site = Site {
        cfg,
        pages,
        sections,
        tags,
        tera,
        root: root.to_path_buf(),
    };
    site.write_all(out)
}

fn collect_tags(pages: &[Page], sections: &[Section], cfg: &Config) -> Vec<Tag> {
    let mut by_name: BTreeMap<String, Vec<Page>> = BTreeMap::new();
    let all = pages.iter().chain(sections.iter().flat_map(|s| s.pages.iter()));
    for page in all {
        for tag in &page.tags {
            by_name.entry(tag.clone()).or_default().push(page.clone());
        }
    }
    by_name
        .into_iter()
        .map(|(name, mut pages)| {
            pages.sort_by(|a, b| b.date_iso.cmp(&a.date_iso).then_with(|| a.title.cmp(&b.title)));
            pages.dedup_by(|a, b| a.url == b.url);
            let slug = slugify(&name);
            Tag {
                url: cfg.url(&format!("/tags/{slug}/")),
                count: pages.len(),
                name,
                slug,
                pages,
            }
        })
        .collect()
}

impl Site {
    fn write_all(&self, out: &Path) -> Result<usize, String> {
        if out.exists() {
            std::fs::remove_dir_all(out).map_err(|e| format!("clearing {}: {e}", out.display()))?;
        }
        std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;

        copy_tree(&self.root.join("static"), out)?;

        let mut written = 0usize;

        // Home page: config-driven profile, plus content/_index.md if it exists.
        let home = self.pages.iter().find(|p| p.url == "/");
        let mut ctx = self.base_context();
        ctx.insert("page", &home);
        ctx.insert("recent", &self.recent(5));
        self.write(out, "/", "index.html", &ctx)?;
        written += 1;

        for section in &self.sections {
            let template = self.template_for(&section.index, "list.html");
            let mut ctx = self.base_context();
            ctx.insert("page", &section.index);
            ctx.insert("section", section);
            ctx.insert("pages", &section.pages);
            if template == "archive.html" {
                ctx.insert("years", &self.by_year());
            }
            self.write(out, &section.url, &template, &ctx)?;
            self.copy_assets(out, &section.index)?;
            written += 1;

            for page in &section.pages {
                written += self.write_leaf(out, page)?;
            }
        }

        for page in self.pages.iter().filter(|p| p.url != "/") {
            written += self.write_leaf(out, page)?;
        }

        // Tag pages exist whether or not content/tags/_index.md does.
        if !self.tags.is_empty() {
            for tag in &self.tags {
                let mut ctx = self.base_context();
                ctx.insert("tag", tag);
                ctx.insert("pages", &tag.pages);
                self.write(out, &format!("/tags/{}/", tag.slug), "tag.html", &ctx)?;
                written += 1;
            }
            if !self.sections.iter().any(|s| s.name == "tags") {
                let ctx = self.base_context();
                self.write(out, "/tags/", "terms.html", &ctx)?;
                written += 1;
            }
        }

        let ctx = self.base_context();
        if self.tera.get_template_names().any(|n| n == "404.html") {
            let html = self
                .tera
                .render("404.html", &ctx)
                .map_err(|e| format!("404.html: {}", chain(&e)))?;
            std::fs::write(out.join("404.html"), html).map_err(|e| e.to_string())?;
            written += 1;
        }

        feed::write(self, out)?;
        Ok(written)
    }

    fn write_leaf(&self, out: &Path, page: &Page) -> Result<usize, String> {
        let template = self.template_for(page, "single.html");
        let mut ctx = self.base_context();
        ctx.insert("page", page);
        if template == "archive.html" {
            ctx.insert("years", &self.by_year());
        }
        if template == "terms.html" {
            ctx.insert("pages", &Vec::<Page>::new());
        }
        self.write(out, &page.url, &template, &ctx)?;
        self.copy_assets(out, page)?;
        Ok(1)
    }

    /// `layout:` in front matter wins, if a template of that name exists.
    fn template_for(&self, page: &Page, fallback: &str) -> String {
        let candidate = match page.layout.as_deref() {
            Some("archives") | Some("archive") => "archive.html".to_string(),
            Some("terms") => "terms.html".to_string(),
            Some(other) => format!("{other}.html"),
            None => fallback.to_string(),
        };
        if self.tera.get_template_names().any(|n| n == candidate) {
            candidate
        } else {
            fallback.to_string()
        }
    }

    fn base_context(&self) -> Context {
        let mut ctx = Context::new();
        ctx.insert("site", &self.cfg);
        ctx.insert("menu", &self.resolved(&self.cfg.menu));
        ctx.insert("social", &self.resolved(&self.cfg.social));
        ctx.insert("sections", &self.section_summaries());
        ctx.insert("tags", &self.tag_summaries());
        ctx.insert("year", &chrono::Local::now().year());
        ctx.insert("home_url", &self.cfg.url("/"));
        // Templates guard on `page`; give it a value everywhere so `{% if page %}`
        // never trips over an undefined name.
        ctx.insert("page", &Option::<Page>::None);
        ctx.insert("feed_url", &self.cfg.url("/index.xml"));
        ctx
    }

    fn resolved(&self, links: &[crate::config::Link]) -> Vec<crate::config::Link> {
        links
            .iter()
            .map(|l| crate::config::Link {
                name: l.name.clone(),
                url: self.cfg.url(&l.url),
            })
            .collect()
    }

    fn section_summaries(&self) -> Vec<serde_json::Value> {
        self.sections
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "title": s.title,
                    "description": s.description,
                    "url": self.cfg.url(&s.url),
                    "count": s.pages.len(),
                })
            })
            .collect()
    }

    fn tag_summaries(&self) -> Vec<serde_json::Value> {
        self.tags
            .iter()
            .map(|t| serde_json::json!({ "name": t.name, "slug": t.slug, "url": t.url, "count": t.count }))
            .collect()
    }

    /// Every dated page inside a section, newest first — the source for the
    /// home page, the archive and the feed. Standalone pages (location, office
    /// hours) are navigation, not entries, so they stay out.
    pub fn recent(&self, limit: usize) -> Vec<Page> {
        let mut all: Vec<Page> = self
            .sections
            .iter()
            .flat_map(|s| s.pages.iter().cloned())
            .filter(|p| p.date_iso.is_some())
            .collect();
        all.sort_by(|a, b| b.date_iso.cmp(&a.date_iso).then_with(|| a.title.cmp(&b.title)));
        all.truncate(limit);
        all
    }

    fn by_year(&self) -> Vec<YearGroup> {
        let mut groups: BTreeMap<i32, Vec<Page>> = BTreeMap::new();
        for page in self.recent(usize::MAX) {
            let year = page
                .date_iso
                .as_deref()
                .and_then(|d| d[..4].parse::<i32>().ok())
                .unwrap_or(0);
            groups.entry(year).or_default().push(page);
        }
        groups
            .into_iter()
            .rev()
            .map(|(year, pages)| YearGroup { year, pages })
            .collect()
    }

    fn write(&self, out: &Path, url: &str, template: &str, ctx: &Context) -> Result<(), String> {
        let html = self
            .tera
            .render(template, ctx)
            .map_err(|e| format!("{template} for {url}: {}", chain(&e)))?;
        let dir = out.join(url.trim_matches('/'));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        std::fs::write(dir.join("index.html"), html).map_err(|e| format!("{}: {e}", dir.display()))
    }

    /// Page-bundle siblings (PDFs, figures) ship next to the page that links them.
    fn copy_assets(&self, out: &Path, page: &Page) -> Result<(), String> {
        if page.assets.is_empty() {
            return Ok(());
        }
        let dir = out.join(page.url.trim_matches('/'));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for asset in &page.assets {
            let Some(name) = asset.file_name() else { continue };
            std::fs::copy(asset, dir.join(name))
                .map_err(|e| format!("copying {}: {e}", asset.display()))?;
        }
        Ok(())
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    if !from.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.map_err(|e| format!("copying {}: {e}", from.display()))?;
        let rel = entry.path().strip_prefix(from).unwrap();
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            std::fs::copy(entry.path(), &dest)
                .map_err(|e| format!("copying {}: {e}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Tera nests the useful part of a failure in `source`; flatten it for the CLI.
fn chain(err: &tera::Error) -> String {
    let mut msg = err.to_string();
    let mut source = std::error::Error::source(err);
    while let Some(e) = source {
        msg.push_str(": ");
        msg.push_str(&e.to_string());
        source = e.source();
    }
    msg
}

pub fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            slug.extend(ch.to_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "untitled".into()
    } else {
        trimmed
    }
}

