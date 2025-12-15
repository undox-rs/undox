# Icons

The default theme includes [Lucide icons](https://lucide.dev/) and provides an `icon()` function for inlining SVG icons directly into your pages.

## Usage

Use the `icon()` function in templates or markdown content:

{% raw %}
```html
{{ icon(name="search") }}
```
{% endraw %}

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Icon name (without `.svg` extension) |
| `class` | string | No | CSS class(es) to add to the SVG |
| `size` | number | No | Width and height in pixels (default: 24) |

### Examples

Basic icon:

{% raw %}
```html
{{ icon(name="search") }}
```
{% endraw %}

Icon with custom class:

{% raw %}
```html
{{ icon(name="menu", class="nav-icon") }}
```
{% endraw %}

Icon with custom size:

{% raw %}
```html
{{ icon(name="star", size=16) }}
```
{% endraw %}

Combined options:

{% raw %}
```html
{{ icon(name="heart", class="text-red", size=20) }}
```
{% endraw %}

## Available Icons

The theme includes the complete Lucide icon set. Browse all available icons at [lucide.dev/icons](https://lucide.dev/icons).

Common icons include:

- `search` - Magnifying glass
- `menu` - Hamburger menu
- `x` - Close/X mark
- `chevron-right`, `chevron-down` - Directional arrows
- `sun`, `moon` - Theme toggle icons
- `github`, `twitter` - Social icons
- `copy`, `check` - Action icons

## Styling

Icons inherit the current text color via `currentColor`, so they automatically match your text:

```css
.my-icon {
  color: blue;
}
```

{% raw %}
```html
<span class="my-icon">{{ icon(name="star") }}</span>
```
{% endraw %}

## Custom Icons

To use custom icons, add SVG files to your theme's `static/icons/` directory. The icon function looks for files at `theme/static/icons/{name}.svg`.
