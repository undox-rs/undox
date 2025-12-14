<p align="center">
  <picture>
    <source srcset="https://github.com/undox-rs/undox/raw/main/assets/logo/wordmark_dark.png" media="(prefers-color-scheme: dark)">
    <img src="https://github.com/undox-rs/undox/raw/main/assets/logo/wordmark_light.png" alt="undox logo" width="360">
  </picture>
</p>

**undox** is a batteries-included static site generator for documentation, with first-class support for aggregating content from multiple repositories.

> This project is in early development. This readme is aspirational. Expect frequent breaking changes and missing features.

## Features

- **Multi-repo support** - Combine docs from multiple repositories into a unified site
- **Markdown** with YAML front matter
- **Syntax highlighting** for 80+ languages via tree-sitter
- **Full-text search** powered by Pagefind
- **Dark mode** with system/light/dark toggle
- **Auto-generated navigation** from file structure
- **Clean URLs** (`/guide/config` instead of `/guide/config.html`)

## Installation

```bash
cargo install undox
```

Or build from source:

```bash
git clone https://github.com/binarymuse/undox
cd undox
cargo build --release
```

## Quick Start

```bash
# Initialize a new docs site
undox init my-docs
cd my-docs

# Build the site
undox build

# View at _site/index.html
```

## Configuration

Create `undox.yaml` in your project root:

```yaml
site:
  name: "My Documentation"
  url: "https://docs.example.com"

sources:
  - name: docs
    path: ./content
```

### Multiple Sources

```yaml
sources:
  - name: main
    path: ./content
    url_prefix: /

  - name: cli
    path: ../cli-repo/docs
    url_prefix: /cli

  - name: api
    path: ../api-repo/docs
    url_prefix: /api
```

## Writing Content

Create markdown files in your content directory:

```markdown
---
title: Getting Started
description: Learn how to use the project
---

# Getting Started

Your content here...
```

### Front Matter

| Field | Description |
|-------|-------------|
| `title` | Page title (overrides filename) |
| `description` | Meta description for SEO |
| `hidden` | Hide from navigation |
| `slug` | Custom URL slug |

## Themes

Themes are configured via `undox-theme.yaml` at the theme root:

```yaml
name: my-theme

pagefind:
  root_selector: "main"
  exclude_selectors:
    - "nav"
    - ".sidebar"
```

## License

MIT
