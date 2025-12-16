---
title: undox
description: A static site generator for documentation with multi-repo support
---

<p align="center">
  {{ macros::image(light_src="./assets/logo/wordmark_light.png", dark_src="./assets/logo/wordmark_dark.png", alt="undox logo") }}
</p>

undox is a batteries-included static site generator built for documentation, with first-class support for aggregating content from multiple repositories.

> [!WARNING]
> undox is in very early development. Expect breaking changes and missing features.

## Why undox?

- **Multi-repo first**: Designed from the ground up for combining docs from multiple repositories into a unified site
- **Batteries included**: Syntax highlighting, search, and themes work out of the box
- **Simple configuration**: YAML config that's easy to read and write
- **No dependencies**: Ships as a single binary
- **Fast**: Built in Rust with tree-sitter powered syntax highlighting

## Quick Example

```yaml
# undox.yaml
site:
  name: "My Project"
  url: "https://docs.example.com"

sources:
  - name: docs
    local:
      path: ./content
```

```bash
undox build
```

That's it! Your documentation site is ready in `_site/`.

## Features

- **Markdown** with front matter support
- **80+ languages** for syntax highlighting
- **Auto-generated navigation** from your file structure
- **Clean URLs** (`/guide/config` instead of `/guide/config.html`)
- **Static file handling** for images and assets

## Getting Started

Ready to try it out? Head to the [Installation](/getting-started/installation) guide.
