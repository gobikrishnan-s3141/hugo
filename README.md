# gobikrishnan.dev

Personal site. [Zola](https://www.getzola.org) + the
[ZolaNight](https://github.com/mxaddict/zolanight) theme.

Content is plain Markdown with TOML front matter.

## Requirements

Zola **0.23.6**, pinned.

```sh
curl -sSL -o zola.tar.gz \
  https://github.com/getzola/zola/releases/download/v0.23.6/zola-v0.23.6-x86_64-unknown-linux-gnu.tar.gz
tar xzf zola.tar.gz && mv zola ~/.local/bin/
```

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
config.toml              site config
content/
  _index.md              home page
  publications.md        papers and preprints
  posts/                 blog
themes/zolanight/        submodule
```

## Deploying

`.github/workflows/deploy.yml` builds on push to `main` and publishes to GitHub
Pages, passing the Pages URL via `--base-url` so a repo rename does not break
asset paths.
