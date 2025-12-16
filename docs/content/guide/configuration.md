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

theme:
  location:
    git: https://github.com/undox-rs/theme-default#main

sources:
  - name: docs
    title: Docs
    url_prefix: /
    local:
      path: ./content
```

## Site Configuration

The `site` section defines global settings for your documentation site.

```yaml
site:
  name: "My Documentation"   # Site title (appears in header)
  url: "https://docs.co.com" # Base URL for your site
  output: "_site"            # Output directory (default: _site)
```

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | The name of your documentation site |
| `url` | No | The base URL where your site will be hosted |
| `favicon` | No | Path to the favicon |
| `repository` | No | URL to your repository |
| `edit_path` | No | Path to append to repo URL for "Edit this page" links |
| `output` | No | Output directory for built files (default: `_site`) |

## Theme Configuration

Specify the location to the theme you want to use; this can be a git repository or a local path.

```yaml
theme:
  location:
    git: https://github.com/undox-rs/theme-default#main
```

or 

```yaml
theme:
  location:
    path: ./path/to/theme
```

If the theme takes any settings, you can specify them in a `settings` key:

```yaml
theme:
  location: # ...
  settings:
    logo:
      light: ./assets/logo/wordmark_light.png
      dark: ./assets/logo/wordmark_dark.png
```

## Sources

Sources define where your documentation content comes from. This is the key to undox's [multi-repo support](/guide/multi-repo).

```yaml
sources:
  - name: docs
    title: Docs
    url_prefix: /
    local:
      path: ./content
```

### Local Source

Pull content from a local directory:

```yaml
sources:
  - name: docs
    url_prefix: /
    local:
      path: ./docs/content
```

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Identifier for this source |
| `local` | Maybe | Path to the content directory; required if `remote` is not set. Must be a `path:` specifier |
| `remote` | Maybe | Location of the remote content; required if `local` is not set. Can be a `git:` or `path:` specifier |
| `url_prefix` | No | URL prefix for all pages from this source (default: `/`) |
| `nav` | No | Explicit navigation structure (see below) |

### Remote Source

Pull content from a remote repository, or another path on your filesystem:

```yaml
sources:
  - name: api
    title: API
    url_prefix: /api
    remote:
      path: ../local-api-repo/docs
  - name: theme
    title: Default Theme
    url_prefix: /theme
    remote:
      git: https://github.com/undox-rs/theme-default#main
```

Using a path is similar to using a `local` source, but treats the source as remote, just like a Git URL.

#### Remote Git Sources

You can specify Git sources via either a string, or a full Git configuration object. If you need to specify a subpath, use the configuration object form:

```yaml
sources:
  - name: api
    remote:
      git: https://github.com/example/api-repo#main
    url_prefix: /api 
  - name: cli
    remote:
      git:
        url: https://github.com/example/cli-repo
        ref: main           # Branch, tag, or commit
        path: docs/      # Path within the repo
    url_prefix: /cli
```

| Field | Required | Description |
|-------|----------|-------------|
| `git.url` | Yes | Repository URL (HTTPS or SSH) |
| `git.ref` | No | Branch, tag, or commit (default: `main`) |
| `git.path` | No | Path to docs within the repo (default: root) |

undox clones the repository to `.undox/cache/git/` and uses the specified path as the content source. The cache is reused between builds - run with a fresh clone by deleting the cache directory or running `undox clean`.

**SSH Authentication**: For private repositories, ensure your SSH keys are configured. undox uses your system's SSH agent.

### Multiple Sources

Combine documentation from multiple locations:

```yaml
sources:
  - name: main
    url_prefix: /
    local:
      path: ./content

  - name: cli
    url_prefix: /cli
    remote:
      path: ../cli-repo/docs

  - name: api
    url_prefix: /api
    remote:
      git: https://github.com/undox-rs/undox-api#main
```

Each source becomes a section in your navigation. See the [multi-repo](/guide/multi-repo) guide for more details.

### Custom Navigation

By default, navigation is auto-generated from your file structure, sorted alphabetically. To customize the order or grouping, use the `nav` field:

```yaml
sources:
  - name: docs
    local:
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

**Auto-generated navigation** also automatically merges files with matching directories. For example, if you have both `configuration.md` and a `configuration/` directory, the directory contents become children of the `configuration.md` link rather than a separate section.

You can also use titled items for custom link text:

```yaml
nav:
  - Welcome: index.md
  - section: Guides
    items:
      - Getting Started: quickstart.md
      - Config Reference: configuration.md
```

Child sources can specify their own navigation configuration in their local config file.

#### Links with Children

When a page has related sub-pages, you can nest them as children of the parent link. This creates a hierarchical navigation where the children appear indented under the parent:

```yaml
nav:
  - section: Guide
    items:
      - path: configuration.md
        title: Configuration  # optional
        children:
          - configuration/root.md
          - configuration/child.md
      - other-guide.md
```

This renders as:
- **Guide** (section heading)
  - **Configuration** (clickable link to `configuration.md`)
    - Root Config (child link, indented)
    - Child Config (child link, indented)
  - Other Guide

Children can be any nav item type, including sections or other links with children, allowing for deeply nested navigation structures.

## Dev Server Configuration

Configure the development server behavior:

```yaml
dev:
  live_reload: true    # Enable/disable browser auto-refresh (default: true)
  watch:
    poll: false        # Use polling instead of native FS events
    poll_interval_ms: 500   # Poll interval when polling is enabled
    debounce_ms: 100   # Debounce delay for file changes
```

### Live Reload

When running `undox serve --watch` (the default), the browser automatically refreshes when files change. Disable this with:

```yaml
dev:
  live_reload: false
```

### File Watching

By default, undox uses native filesystem events for efficient change detection. On some systems (Docker volumes, network filesystems, WSL), native events may be unreliable. Switch to polling mode:

```yaml
dev:
  watch:
    poll: true
    poll_interval_ms: 1000  # Check every second
```

| Field | Default | Description |
|-------|---------|-------------|
| `poll` | `false` | Use polling-based file watcher |
| `poll_interval_ms` | `500` | Polling interval in milliseconds |
| `debounce_ms` | `100` | Wait time before triggering rebuild |

## Environment Variables

You can use environment variables in your config:

```yaml
site:
  url: ${SITE_URL}
```
