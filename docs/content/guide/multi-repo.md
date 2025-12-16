---
title: Multi-Repository Docs
description: Aggregate documentation from multiple repositories into a unified site
---

# Multi-Repository Documentation

undox is designed from the ground up to aggregate documentation from multiple repositories into a single, unified site. This is perfect for projects with separate repos for CLI tools, libraries, desktop apps, etc.

## Architecture Overview

There are two types of undox configurations:

1. **Root config** - The main site that defines sources, theme, and site settings
2. **Child config** - A minimal config in a child repo that points to a parent site

```
┌─────────────────────────────────────────────────────┐
│                    Root Site                        │
│  (defines theme, sources, site settings)            │
│                                                     │
│   Sources:                                          │
│   ├── /          → Local ./content                  │
│   ├── /cli       → git: cli-repo/docs               │
│   └── /desktop   → git: desktop-repo/docs           │
└─────────────────────────────────────────────────────┘
```

## Setting Up a Root Site

The root site aggregates content from multiple sources:

```yaml
# undox.yaml (root site)
site:
  name: "My Project Docs"
  url: "https://docs.example.com"

theme:
  location:
    git: https://github.com/undox-rs/theme-default#main

sources:
  # Main documentation (local)
  - name: main
    local:
      path: ./content
    url_prefix: /

  # CLI docs from another repo
  - name: cli
    git:
      url: https://github.com/example/cli
      ref: main
      path: docs/
    url_prefix: /cli
    title: "CLI"

  # Desktop app docs
  - name: desktop
    git:
      url: https://github.com/example/desktop
      ref: main
      path: docs/
    url_prefix: /desktop
    title: "Desktop"
```

Each source appears as a tab in the site header, allowing users to navigate between different parts of your documentation.

## Setting Up a Child Config

Child repos can run their documentation locally while using the parent site's theme and configuration. This ensures consistency and lets developers preview their changes with the full site context.

Create an `undox.yaml` in the child repo:

```yaml
# undox.yaml (child repo)
name: cli   # Which source am I in the parent config?
parent:
  git: https://github.com/example/docs-site#main

# Local path to content (relative to this config)
content:
  path: ./docs

overrides:
  site:
    # site settings overrides for the child pages
  theme:
    # theme settings overrides for the child pages

# Navigation can be explicit, or auto-generated from the filesystem
# If not set, will use the parent's settings
nav:
  - name: Home
    url: /
  - name: CLI
    url: /cli
  - name: Desktop
    url: /desktop
```

### How It Works

When you run `undox serve` in a child repo:

1. undox fetches the parent config from the specified git repository
2. It loads the parent's theme and site settings
3. Your local content is used instead of the git source defined in the parent
4. Navigation, styling, and URLs all match the parent site

This means you can edit documentation locally and see exactly how it will appear on the live site.

### Local Development Override

During development, you might want to point to a local copy of the parent site instead of fetching from git:

```yaml
# undox.yaml (child repo)
name: cli
parent:
  git:
    url: https://github.com/example/docs-site
    ref: main

content:
  path: ./docs

# Override for local development
dev:
  parent:
    path: ../docs-site  # Local path to parent repo
```

When `dev.parent` is set, undox uses that path instead of cloning from git. This is useful when you're working on both the parent site and child content simultaneously.

## Example: Atuin-style Setup

Consider a project like Atuin with separate CLI and Desktop applications:

**Main docs site** (`docs-site` repo):
```yaml
site:
  name: "Atuin Docs"
  url: "https://docs.atuin.sh"

sources:
  - name: main
    url_prefix: /
    local:
      path: ./content

  - name: cli
    title: "CLI"
    url_prefix: /cli
    git:
      url: https://github.com/atuinsh/atuin
      path: docs/

  - name: desktop
    title: "Desktop"
    url_prefix: /desktop
    git:
      url: https://github.com/atuinsh/desktop
      path: docs/
```

**CLI repo** (`atuin` repo, in `docs/undox.yaml`):
```yaml
name: cli
parent:
  git:
    url: https://github.com/atuinsh/docs-site

content:
  path: ./content
```

**Desktop repo** (`desktop` repo, in `docs/undox.yaml`):
```yaml
name: desktop
parent:
  git:
    url: https://github.com/atuinsh/docs-site

content:
  path: ./content
```

Now developers in each repo can run `undox serve` to preview their documentation with the full site theme and navigation.

## Best Practices

1. **Keep the parent source-of-truth**: Define theme, navigation structure, and site settings in the parent config only

2. **Use consistent `url_prefix`**: The prefix in the parent config determines the final URLs - child configs inherit this

3. **Pin git refs for production**: Use specific tags or commits for production builds to ensure reproducibility
   ```yaml
   git:
     url: https://github.com/example/repo
     ref: v1.2.3  # Tag instead of branch
   ```

4. **Use `dev.parent` for local development**: Avoid constantly fetching from git while actively developing

5. **Add `.undox/` to `.gitignore`**: The cache directory shouldn't be committed
   ```
   # .gitignore
   .undox/
   _site/
   ```
