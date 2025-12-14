---
title: Configuration
description: Complete reference for undox.yaml configuration
---

# Configuration

undox is configured via a `undox.yaml` file in your project root.

## Full Example

```yaml
site:
  name: "My Documentation"
  url: "https://docs.example.com"
  output: "_site"

theme: default

sources:
  - name: docs
    path: ./content
    url_prefix: /
    repo_url: "https://github.com/example/repo"
    edit_path: "content/"
```

## Site Configuration

The `site` section defines global settings for your documentation site.

```yaml
site:
  name: "My Documentation"    # Site title (appears in header)
  url: "https://docs.example.com"  # Base URL for your site
  output: "_site"             # Output directory (default: _site)
```

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | The name of your documentation site |
| `url` | No | The base URL where your site will be hosted |
| `output` | No | Output directory for built files (default: `_site`) |

## Theme Configuration

```yaml
theme: default
```

Currently only the `default` theme is available. Custom themes coming soon!

## Sources

Sources define where your documentation content comes from. This is the key to undox's multi-repo support.

```yaml
sources:
  - name: docs
    path: ./content
    url_prefix: /
```

### Local Source

Pull content from a local directory:

```yaml
sources:
  - name: cli
    path: ./docs/cli
    url_prefix: /cli
```

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Identifier for this source |
| `path` | Yes | Path to the content directory |
| `url_prefix` | No | URL prefix for all pages from this source (default: `/`) |
| `repo_url` | No | GitHub/GitLab URL for "Edit this page" links |
| `edit_path` | No | Path within the repo to the docs folder |
| `nav` | No | Explicit navigation structure (see below) |

### Custom Navigation

By default, navigation is auto-generated from your file structure, sorted alphabetically. To customize the order or grouping, use the `nav` field:

```yaml
sources:
  - name: docs
    path: ./content
    nav:
      - section: Getting Started
        items:
          - quickstart.md
          - installation.md
      - section: Guide
        items:
          - configuration.md
          - advanced/
      - about.md
```

Nav items can be:
- **Filenames**: `installation.md` - link to a specific page
- **Directories**: `advanced/` - auto-expand all pages in that directory
- **Sections**: Group items under a heading

You can also use titled items for custom link text:

```yaml
nav:
  - Welcome: index.md
  - section: Guides
    items:
      - Getting Started: quickstart.md
      - Config Reference: configuration.md
```

### Multiple Sources

Combine documentation from multiple locations:

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

Each source becomes a section in your navigation.

## Environment Variables

You can use environment variables in your config:

```yaml
site:
  url: ${SITE_URL}
```

## Minimal Config

The simplest possible config:

```yaml
site:
  name: "Docs"

sources:
  - name: docs
    path: ./content
```
