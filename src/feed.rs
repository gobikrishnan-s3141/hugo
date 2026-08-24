use crate::markdown::{escape_html, plain_text};
use crate::render::Site;
use chrono::{NaiveDate, Utc};
use std::path::Path;

/// RSS 2.0 feed, sitemap and robots.txt. Hand-rolled XML keeps the dependency
/// list short; every interpolated value goes through `escape_html`.
pub fn write(site: &Site, out: &Path) -> Result<(), String> {
    let cfg = &site.cfg;
    let now = Utc::now().format("%a, %d %b %Y %H:%M:%S +0000").to_string();

    let mut rss = String::new();
    rss.push_str("<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n");
    rss.push_str("<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n  <channel>\n");
    rss.push_str(&format!("    <title>{}</title>\n", escape_html(&cfg.title)));
    rss.push_str(&format!("    <link>{}</link>\n", escape_html(&cfg.abs_url("/"))));
    rss.push_str(&format!(
        "    <description>{}</description>\n",
        escape_html(&cfg.description)
    ));
    rss.push_str("    <language>en</language>\n");
    rss.push_str(&format!("    <lastBuildDate>{now}</lastBuildDate>\n"));
    rss.push_str(&format!(
        "    <atom:link href=\"{}\" rel=\"self\" type=\"application/rss+xml\"/>\n",
        escape_html(&cfg.abs_url("/index.xml"))
    ));

    for page in site.recent(30) {
        let description = if page.summary.is_empty() {
            plain_text(&page.content, 400)
        } else {
            page.summary.clone()
        };
        rss.push_str("    <item>\n");
        rss.push_str(&format!("      <title>{}</title>\n", escape_html(&page.title)));
        rss.push_str(&format!("      <link>{}</link>\n", escape_html(&page.permalink)));
        rss.push_str(&format!(
            "      <guid isPermaLink=\"true\">{}</guid>\n",
            escape_html(&page.permalink)
        ));
        if let Some(rfc) = page.date_iso.as_deref().and_then(to_rfc822) {
            rss.push_str(&format!("      <pubDate>{rfc}</pubDate>\n"));
        }
        rss.push_str(&format!(
            "      <description>{}</description>\n",
            escape_html(&description)
        ));
        rss.push_str("    </item>\n");
    }
    rss.push_str("  </channel>\n</rss>\n");
    std::fs::write(out.join("index.xml"), rss).map_err(|e| format!("index.xml: {e}"))?;

    let mut urls: Vec<(String, Option<String>)> = vec![(cfg.abs_url("/"), None)];
    for section in &site.sections {
        urls.push((cfg.abs_url(&section.url), None));
        for page in &section.pages {
            urls.push((page.permalink.clone(), page.date_iso.clone()));
        }
    }
    for page in site.pages.iter().filter(|p| p.url != "/") {
        urls.push((page.permalink.clone(), page.date_iso.clone()));
    }
    for tag in &site.tags {
        urls.push((cfg.abs_url(&format!("/tags/{}/", tag.slug)), None));
    }

    let mut sitemap = String::from("<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n");
    sitemap.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    for (url, lastmod) in urls {
        sitemap.push_str("  <url>\n");
        sitemap.push_str(&format!("    <loc>{}</loc>\n", escape_html(&url)));
        if let Some(date) = lastmod {
            sitemap.push_str(&format!("    <lastmod>{date}</lastmod>\n"));
        }
        sitemap.push_str("  </url>\n");
    }
    sitemap.push_str("</urlset>\n");
    std::fs::write(out.join("sitemap.xml"), sitemap).map_err(|e| format!("sitemap.xml: {e}"))?;

    let robots = format!(
        "User-agent: *\nAllow: /\n\nSitemap: {}\n",
        cfg.abs_url("/sitemap.xml")
    );
    std::fs::write(out.join("robots.txt"), robots).map_err(|e| format!("robots.txt: {e}"))?;
    Ok(())
}

fn to_rfc822(iso: &str) -> Option<String> {
    let date = NaiveDate::parse_from_str(iso, "%Y-%m-%d").ok()?;
    Some(
        date.and_hms_opt(0, 0, 0)?
            .format("%a, %d %b %Y %H:%M:%S +0000")
            .to_string(),
    )
}
