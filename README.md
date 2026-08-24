# gobikrishnan.dev

Personal site. [Zola](https://www.getzola.org) + the
[Duckquill](https://codeberg.org/daudix/duckquill) theme, with a light/dark/system
theme switcher.

Content is plain Markdown with TOML front matter.

## Requirements

Zola **0.22.1**, pinned.

```sh
curl -sSL -o zola.tar.gz \
  https://github.com/getzola/zola/releases/download/v0.22.1/zola-v0.22.1-x86_64-unknown-linux-gnu.tar.gz
tar xzf zola.tar.gz && mv zola ~/.local/bin/
```

> **Do not bump to 0.23.x.** Zola 0.23 removed shortcodes and moved to Tera 2.
> Duckquill uses shortcodes (`templates/shortcodes/`) and is in reduced
> maintenance mode, so it does not build on 0.23.

Clone with the theme submodule:

```sh
git clone --recurse-submodules <repo>
# already cloned:
git submodule update --init --recursive
```

## Writing

```sh
zola serve      # http://127.0.0.1:1111, live reload
zola build      # -> public/
zola check      # validate links
```

A new post is one file:

```sh
cat > content/posts/my-post.md <<'EOF'
+++
title = "My post"
date = 2026-08-24
[taxonomies]
tags = ["bioinformatics"]
+++

Body goes here.
EOF
```

Push to `main` and GitHub Actions deploys it.

## Layout

```
config.toml              site config, nav, footer, theme switcher
content/
  _index.md              home page
  publications.md        papers and preprints
  posts/                 blog
static/syntax.css        links Zola's generated highlight themes
themes/duckquill/        submodule
```

## Theme switcher

Set by `extra.default_theme` in `config.toml`. Setting it is also what makes the
switcher appear in the nav — without it the control is hidden. The switcher
offers light / dark / system and remembers the choice in `localStorage`.

## Known theme gap

Duckquill's `partials/head.html` gates the syntax-highlight stylesheets on
`config.markdown.highlight_code` and `highlight_theme == "css"` — pre-0.22 Zola
config keys that no longer exist. The condition is therefore always false and no
highlight CSS is linked, leaving code blocks colourless.

`static/syntax.css` works around it by importing the `giallo-light.css` and
`giallo-dark.css` files Zola generates, behind `prefers-color-scheme` media
queries. It is wired in through `extra.styles`.

Consequence: code blocks follow the **system** colour scheme, not the nav
switcher. Under the switcher's "system" setting the two agree; under an explicit
light or dark choice the page chrome changes but code colours do not. This
matches Duckquill's own intended behaviour, which also keys syntax CSS off
`prefers-color-scheme`.

## Deploying

`.github/workflows/deploy.yml` builds on push to `main` and publishes to GitHub
Pages, passing the Pages URL via `--base-url` so a repo rename does not break
asset paths.
