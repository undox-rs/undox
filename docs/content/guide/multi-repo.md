---
title: Multi-Repository Documentation
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
│                    Root Site                         │
│  (defines theme, sources, site settings)            │
│                                                      │
│   Sources:                                           │
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

theme: default

sources:
  # Main documentation (local)
  - name: main
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
parent:
  git:
    url: https://github.com/example/docs-site
    ref: main
  source: cli   # Which source am I in the parent config?

# Local path to content (relative to this config)
path: ./docs
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
parent:
  git:
    url: https://github.com/example/docs-site
    ref: main
  source: cli

path: ./docs

# Override for local development
dev:
  parent: ../docs-site  # Local path to parent repo
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
    path: ./content
    url_prefix: /

  - name: cli
    git:
      url: https://github.com/atuinsh/atuin
      path: docs/
    url_prefix: /cli
    title: "CLI"

  - name: desktop
    git:
      url: https://github.com/atuinsh/desktop
      path: docs/
    url_prefix: /desktop
    title: "Desktop"
```

**CLI repo** (`atuin` repo, in `docs/undox.yaml`):
```yaml
parent:
  git:
    url: https://github.com/atuinsh/docs-site
  source: cli

path: .
```

**Desktop repo** (`desktop` repo, in `docs/undox.yaml`):
```yaml
parent:
  git:
    url: https://github.com/atuinsh/docs-site
  source: desktop

path: .
```

Now developers in each repo can run `undox serve --watch` to preview their documentation with the full site theme and navigation.

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
