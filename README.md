# gobikrishnan.dev

Personal site. [Zola](https://www.getzola.org) + the
[neovim](https://github.com/Super-Botman/neovim-theme) theme — a keyboard-driven
file-browser layout styled after the editor.

Content is plain Markdown with TOML front matter. Nothing else.

## Requirements

Zola **0.22.1**, pinned.

```sh
curl -sSL -o zola.tar.gz \
  https://github.com/getzola/zola/releases/download/v0.22.1/zola-v0.22.1-x86_64-unknown-linux-gnu.tar.gz
tar xzf zola.tar.gz && mv zola ~/.local/bin/
```

> **Do not bump to 0.23.x.** Zola 0.23 replaced Tera 1 with Tera 2, which drops
> the `self::macro()` call syntax the theme's recursive sidebar tree relies on
> (`themes/neovim-theme/templates/components/files.html`). 0.22.1 is the last
> release that builds this theme unmodified. Upgrading means porting that macro
> to a Tera 2 component first.

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
title = "my post"
date = 2026-08-24
+++

Body goes here.
EOF
```

Push to `main` and GitHub Actions deploys it.

## Layout

```
config.toml              site config
content/
  _index.md              home page
  readme.md              keyboard shortcuts reference
  publications.md        papers and preprints
  posts/                 blog
sass/css/custom.scss     font-path fix + overflow guards
static/js/config.js      keybindings and : commands
templates/
  base.html              overrides the theme's
  index.html             overrides the theme's
themes/neovim-theme/     submodule
```

## Theme overrides

The theme hardcodes root-absolute paths, which 404 when the site is served from
a sub-path (this one deploys under `/hugo/`). Three files exist purely to fix
that, and each says so in a header comment:

- `templates/base.html` — routes every asset through `get_url()`, adds per-page
  `<title>` and meta tags, and exports `window.BASE_URL` for the static JS.
- `static/js/config.js` — replaces the theme's copy (Zola's `static/` overlays
  the theme's), so `:help` and `:home` resolve against `window.BASE_URL`.
- `sass/css/custom.scss` — redeclares the JetBrainsMono `@font-face` with a
  path relative to the stylesheet instead of the theme's absolute one.

## Deploying

`.github/workflows/deploy.yml` builds on push to `main` and publishes to GitHub
Pages, passing the Pages URL via `--base-url` so a repo rename does not break
asset paths.
