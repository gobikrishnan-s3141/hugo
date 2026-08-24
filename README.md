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
static/syntax.css        generated; see Syntax highlighting
scripts/                 syntax stylesheet generator
themes/duckquill/        submodule
```

## Theme switcher

Set by `extra.default_theme` in `config.toml`. Setting it is also what makes the
switcher appear in the nav — without it the control is hidden. The switcher
offers light / dark / system and remembers the choice in `localStorage`.

## Syntax highlighting

Duckquill's `partials/head.html` gates the syntax-highlight stylesheets on
`config.markdown.highlight_code` and `highlight_theme == "css"` — pre-0.22 Zola
config keys that no longer exist. The condition is always false, so the theme
never links the `giallo-light.css` / `giallo-dark.css` files Zola generates and
code blocks render without colour.

Linking them directly is not enough either: a plain `@import` cannot be scoped
to a selector, so code would follow `prefers-color-scheme` while the rest of the
page follows the switcher's `data-theme` attribute.

`scripts/gen-syntax-css.py` flattens both palettes into `static/syntax.css`:

| Scope | Applies when |
| --- | --- |
| unscoped | light, the default |
| `:root[data-theme="dark"]` | dark chosen in the switcher |
| `:root:not([data-theme])` + `prefers-color-scheme: dark` | switcher set to system |

Code colours therefore follow the switcher, including its system setting.
`static/syntax.css` is generated but **committed**, since CI only runs
`zola build`. Regenerate it after changing the highlight themes in
`config.toml`:

```sh
zola build && python3 scripts/gen-syntax-css.py
```

## Deploying

`.github/workflows/deploy.yml` builds on push to `main` and publishes to GitHub
Pages, passing the Pages URL via `--base-url` so a repo rename does not break
asset paths.
