# Theme Settings

Configure the default theme using the `theme.settings` section in your `undox.yaml`.

## Configuration

```yaml
theme:
  name: default
  settings:
    logo:
      light: ./assets/logo-light.png
      dark: ./assets/logo-dark.png
    hide_top_level_source: false
```

## Available Settings

### logo

Custom logo displayed in the site header. Supports separate images for light and dark modes.

| Property | Type | Description |
|----------|------|-------------|
| `light` | string | Path to logo for light mode |
| `dark` | string | Path to logo for dark mode |

You can specify both, or just one:

```yaml
# Both light and dark logos
theme:
  settings:
    logo:
      light: ./assets/logo-light.svg
      dark: ./assets/logo-dark.svg
```

```yaml
# Single logo (used for both modes)
theme:
  settings:
    logo:
      light: ./assets/logo.svg
```

If no logo is specified, the site name from `site.name` is displayed as text.

### hide_top_level_source

When `true`, hides the source selector in the header and sidebar when only one source exists or when the top-level source has `url_prefix: /`.

| Type | Default | Description |
|------|---------|-------------|
| boolean | `false` | Hide the top-level source tab |

```yaml
theme:
  settings:
    hide_top_level_source: true
```

This is useful for single-project documentation sites where showing a source tab would be redundant.

## Full Example

```yaml
site:
  name: "My Project"
  url: "https://docs.example.com"
  favicon: ./assets/favicon.png
  output: "_site"

theme:
  name: default
  settings:
    logo:
      light: ./assets/logo-light.svg
      dark: ./assets/logo-dark.svg
    hide_top_level_source: true

sources:
  - name: docs
    content_path: ./docs
    url_prefix: /
```
