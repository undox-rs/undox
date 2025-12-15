# Macros

The default theme provides Tera macros that can be used in both templates and markdown content. Macros are imported automatically and accessed via the `macros::` namespace.

## image

Displays different images for light and dark mode. The appropriate image is shown based on the user's current theme setting.

### Usage

{% raw %}
```html
{{ macros::image(light_src="./img/diagram-light.png", dark_src="./img/diagram-dark.png", alt="Architecture diagram") }}
```
{% endraw %}

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `light_src` | string | Yes | Path to the image shown in light mode |
| `dark_src` | string | Yes | Path to the image shown in dark mode |
| `alt` | string | No | Alt text for accessibility (default: empty) |

### Example

{% raw %}
```markdown
Here's how the system works:

{{ macros::image(
  light_src="./architecture-light.svg",
  dark_src="./architecture-dark.svg",
  alt="System architecture overview"
) }}
```
{% endraw %}

### How It Works

The macro renders two `<img>` elements with CSS classes:

- `.macro-image-light` - Visible only in light mode
- `.macro-image-dark` - Visible only in dark mode

The theme's CSS handles showing/hiding the appropriate image based on the active theme.

## Creating Custom Macros

You can add custom macros by editing the theme's `templates/macros.html` file. All macros defined there are automatically available in your content.

Example custom macro:

{% raw %}
```html
{% macro alert(type="info", title="") %}
<div class="alert alert-{{ type }}">
  {% if title %}<strong>{{ title }}</strong>{% endif %}
  {{ caller() }}
</div>
{% endmacro alert %}
```
{% endraw %}

Usage:

{% raw %}
```html
{% call macros::alert(type="warning", title="Caution") %}
This action cannot be undone.
{% endcall %}
```
{% endraw %}
