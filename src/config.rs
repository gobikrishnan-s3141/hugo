use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Full canonical URL, e.g. "https://example.org/hugo/".
    pub base_url: String,
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    /// chrono strftime pattern used for every rendered date.
    #[serde(default = "default_date_format")]
    pub date_format: String,
    /// Load KaTeX on every page (individual pages can opt in with `math: true`).
    #[serde(default)]
    pub math: bool,
    #[serde(default)]
    pub profile: Profile,
    #[serde(default)]
    pub menu: Vec<Link>,
    #[serde(default)]
    pub social: Vec<Link>,
    /// Order of sections on the archive page; unlisted sections follow, alphabetically.
    #[serde(default)]
    pub section_order: Vec<String>,

    /// Path component of `base_url`, e.g. "/hugo". Derived, not read from the file.
    #[serde(skip_deserializing, default)]
    pub base_path: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Profile {
    #[serde(default)]
    pub subtitle: String,
    /// Filename inside `static/`, e.g. "picture.jpg".
    #[serde(default)]
    pub image: String,
}

/// A label plus a destination. Aliases accept the PaperMod `editPost` spelling
/// (`Text`/`URL`) so existing front matter parses unchanged.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Link {
    #[serde(alias = "Text", alias = "text")]
    pub name: String,
    #[serde(alias = "URL", alias = "Url")]
    pub url: String,
}

fn default_date_format() -> String {
    "%B %Y".into()
}

impl Config {
    pub fn load(path: &Path) -> Result<Config, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut cfg: Config = toml::from_str(&raw)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        // CI deploys under a URL the repo does not know at commit time.
        if let Ok(url) = std::env::var("QUILL_BASE_URL") {
            if !url.trim().is_empty() {
                cfg.base_url = url;
            }
        }
        cfg.base_path = derive_base_path(&cfg.base_url);
        Ok(cfg)
    }

    /// Turn a site-absolute path into one the browser can follow under `base_path`.
    /// Anything already absolute (http, mailto, //) is passed through untouched.
    pub fn url(&self, path: &str) -> String {
        if path.contains("://") || path.starts_with("mailto:") || path.starts_with("//") {
            return path.to_string();
        }
        let path = path.trim_start_matches('/');
        format!("{}/{}", self.base_path, path)
    }

    /// Fully-qualified URL for a site-absolute path. `base_url` already carries
    /// `base_path`, so the path is appended to it directly.
    pub fn abs_url(&self, path: &str) -> String {
        if path.contains("://") || path.starts_with("mailto:") || path.starts_with("//") {
            return path.to_string();
        }
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

/// "https://example.org/hugo/" -> "/hugo"; "https://example.org/" -> "".
fn derive_base_path(base_url: &str) -> String {
    let after_scheme = match base_url.find("://") {
        Some(i) => &base_url[i + 3..],
        None => base_url,
    };
    match after_scheme.find('/') {
        Some(i) => after_scheme[i..].trim_end_matches('/').to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_path_is_stripped_of_host_and_trailing_slash() {
        assert_eq!(derive_base_path("https://example.org/hugo/"), "/hugo");
        assert_eq!(derive_base_path("https://example.org/"), "");
        assert_eq!(derive_base_path("https://example.org"), "");
    }
}
