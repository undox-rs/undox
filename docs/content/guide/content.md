---
title: Writing Content
description: Guide to writing documentation with undox
---

# Writing Content

undox uses Markdown for all documentation content, with optional YAML front matter for metadata.

## Front Matter

Front matter is YAML metadata at the top of your markdown files, enclosed in `---`:

```markdown
---
title: My Page Title
description: A brief description for SEO
---

# Your content starts here
```

### Supported Fields

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Page title (overrides filename-derived title) |
| `description` | string | Page description for SEO meta tags |
| `hidden` | boolean | Hide this page from navigation |
| `slug` | string | Custom URL slug |

### Custom Fields

You can add any custom fields and access them in templates:

```markdown
---
title: API Reference
author: Jane Doe
version: 2.0
---
```

Custom fields are available in templates as `page.author`, `page.version`, etc.

## Navigation Order

By default, pages are sorted alphabetically by their URL path. You have two options for custom ordering:

### Option 1: Filename Prefixes

Use numeric prefixes in filenames:

```
content/
  guide/
    01-installation.md    # First
    02-configuration.md   # Second
    03-advanced.md        # Third
```

### Option 2: Explicit Nav in Config

Define navigation explicitly in `undox.yaml`:

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
          - content.md
```

See [Configuration](/guide/configuration) for full nav options.

## Markdown Features

undox supports standard Markdown plus several extensions:

### Tables

```markdown
| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |
```

| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |

### Task Lists

```markdown
- [x] Completed task
- [ ] Incomplete task
```

- [x] Completed task
- [ ] Incomplete task

### Strikethrough

```markdown
~~deleted text~~
```

~~deleted text~~

### Footnotes

```markdown
Here's a sentence with a footnote[^1].

[^1]: This is the footnote content.
```

## Code Blocks

Fenced code blocks with language identifiers get syntax highlighting:

````markdown
```rust
fn main() {
    println!("Hello!");
}
```
````

Renders as:

```rust
fn main() {
    println!("Hello!");
}
```

Code blocks include a **copy button** that appears on hover, making it easy for readers to copy code snippets.

See [Syntax Highlighting](/guide/syntax-highlighting) for the full list of supported languages.

## File Organization

### Directory Structure

Your file structure determines your URL structure and navigation:

```
content/
  index.md              -> /
  getting-started/
    installation.md     -> /getting-started/installation
    quickstart.md       -> /getting-started/quickstart
  guide/
    configuration.md    -> /guide/configuration
```

### Naming Conventions

- Use lowercase filenames with hyphens: `getting-started.md`
- Directories become navigation sections
- `index.md` in a directory becomes the section's landing page

### Auto-generated Titles

If you don't specify a `title` in front matter, undox generates one from the filename:

- `installation.md` → "Installation"
- `getting-started.md` → "Getting Started"
- `api-reference.md` → "Api Reference"

## Static Files

Non-markdown files (images, PDFs, etc.) are copied to the output directory:

```
content/
  guide/
    screenshot.png      -> /guide/screenshot.png
    diagram.svg         -> /guide/diagram.svg
```

Reference them in markdown:

```markdown
![Screenshot](screenshot.png)
```

## Links

### Internal Links

Link to other pages using absolute paths:

```markdown
See the [Configuration](/guide/configuration) guide.
```

### External Links

```markdown
Visit [GitHub](https://github.com).
```
