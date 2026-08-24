# gobikrishnan-s3141.github.io

Personal site. Markdown in `content/`, a ~1,600-line Rust generator in `src/`,
static HTML out. No Node, no theme, no build pipeline beyond `cargo`.

## The stack

| Piece | What it is |
| --- | --- |
| `src/` | `quill`, the generator: markdown → HTML, RSS, sitemap, dev server |
| `content/` | Every page, as `.md` with YAML front matter |
| `templates/` | Seven Tera templates (`base`, `index`, `list`, `single`, `archive`, `terms`, `tag`) |
| `static/` | `style.css`, CV, favicons, photo — copied verbatim to the site root |
| `site.toml` | Title, menu, social links, profile blurb |

Nine direct crates, all of them doing something visible: `pulldown-cmark`
(markdown), `syntect` (build-time syntax highlighting, so no JavaScript
highlighter ships), `tera` (templates), `serde` + `serde_yaml_ng` + `toml` +
`serde_json` (front matter, config, template context), `walkdir`, `chrono`.
The dev server and the file watcher are plain `std`.

## Daily use

```sh
cargo run -- serve                    # http://127.0.0.1:1313/hugo/, rebuilds on save
cargo run -- new posts "Post title"   # scaffolds content/posts/post-title/index.md
cargo run -- build                    # renders into public/
cargo run -- publish "new post"       # builds, commits, pushes — CI deploys
```

Install it once and drop the `cargo run --` prefix:

```sh
cargo install --path .
quill serve
```

## Writing a post

`quill new posts "Title"` creates a folder with an `index.md`. Everything the
post needs — figures, PDFs, data — goes in that same folder and is linked by
bare filename:

```markdown
---
title: "Benchmarking scGen"
date: 2026-08-24
tags: ["single-cell", "benchmarking"]
summary: "One paragraph for listings and the RSS feed."
---

![](umap.png)
[The full results](results.pdf)
```

Front matter keys, all optional except `title`: `date`, `lastmod`, `tags`,
`author` (string or list), `summary`, `description`, `draft`, `showToc`,
`hidemeta`, `math`, `weight`, `layout`, `cover: {image, alt}`,
`editPost: {Text, URL}`.

## How it maps to URLs

| File | URL |
| --- | --- |
| `content/papers/_index.md` | `/papers/` — section listing |
| `content/papers/paper1/index.md` | `/papers/paper1/` — bundle, siblings copied alongside |
| `content/data/data1.md` | `/data/data1/` |
| `content/location.md` | `/location/` — standalone page, kept out of listings |

Any directory with an `_index.md` becomes a section, listed and fed
automatically. Tags generate `/tags/<slug>/` pages without any configuration.
`layout: archives` and `layout: terms` in front matter pick the archive and tag
index templates.

## Deploying

Pushing to `main` runs `.github/workflows/deploy.yml`: build with cargo, upload
`public/`, deploy to GitHub Pages. The workflow passes the Pages URL in through
`QUILL_BASE_URL`, which overrides `base_url` in `site.toml` — that is what makes
the site work from a sub-path like `/hugo/` without editing anything.

## Tests

```sh
cargo test
```

Covers front-matter splitting, date parsing, base-path derivation, heading
anchors, math that survives underscores, and directory traversal in the dev
server.
