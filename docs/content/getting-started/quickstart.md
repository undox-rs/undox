---
title: Quickstart
description: Create your first documentation site in 5 minutes
---

# Quickstart

This guide will walk you through creating a documentation site from scratch.

## 1. Create a Project Directory

```bash
mkdir my-docs
cd my-docs
```

## 2. Initialize undox

```bash
undox init
```

This creates:
- `undox.yaml` - your site configuration
- `content/` - where your documentation lives
- `content/index.md` - your homepage

## 3. Start the Dev Server

```bash
undox serve --watch --open
```

This starts a local development server with:
- **Live reload** - Your browser automatically refreshes when you edit files
- **File watching** - Changes to content, templates, or config trigger rebuilds

The `--open` flag opens your browser automatically.

## 4. Build for Production

When you're ready to deploy:

```bash
undox build
```

Your site is now in `_site/`, ready to deploy to any static hosting service.

## 5. Add More Pages

Create new markdown files in the `content/` directory:

```bash
mkdir -p content/guide
```

Create `content/guide/introduction.md`:

```markdown
---
title: Introduction
---

# Introduction

Welcome to the guide!
```

Rebuild and your new page appears in the sidebar automatically.

## Project Structure

After setup, your project looks like this:

```
my-docs/
  undox.yaml          # Site configuration
  content/
    index.md          # Homepage
    guide/
      introduction.md # Nested pages become sections
  _site/              # Built output (don't edit)
```

## What's Next?

- Learn about [Configuration](/guide/configuration) options
- Explore [Content](/guide/content) authoring with front matter
- Set up [Multi-Repository Documentation](/guide/multi-repo) for larger projects
- See the full list of [supported languages](/guide/syntax-highlighting) for code blocks
