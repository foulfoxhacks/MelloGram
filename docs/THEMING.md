# Theming MelloGram

A theme is a `.css` file in the themes folder (Settings → Themes shows the
path). Enabled themes are applied in order, then plugin CSS, then QuickCSS.

## Header

```css
/**
 * @name Sunset
 * @author you
 * @version 1.0.0
 * @description Orange and plum.
 * @source https://example.com/sunset
 */
```

`@name` also becomes the file name when you install the theme.

## Variables

Everything in the base stylesheet is driven by variables. Declare overrides in
your theme's first `:root { }` block and they show up in the theme's
**variables** editor (with a colour picker for `#rrggbb` values), so users can
tweak your theme without editing it.

| Variable | Used for |
|---|---|
| `--mg-bg`, `--mg-bg-2`, `--mg-surface` | Window, sidebars/headers, cards |
| `--mg-text`, `--mg-muted` | Text |
| `--mg-accent`, `--mg-accent-text` | Buttons, selection, links |
| `--mg-border` | Dividers |
| `--mg-danger`, `--mg-success`, `--mg-warning` | Status colours |
| `--mg-bubble-in`, `--mg-bubble-in-text` | Incoming messages |
| `--mg-bubble-out`, `--mg-bubble-out-text` | Your messages |
| `--mg-radius` | Corner radius |
| `--mg-density` | Vertical spacing multiplier (1 = default) |
| `--mg-avatar` | Avatar size |
| `--mg-font`, `--mg-mono`, `--mg-font-size` | Typography |
| `--mg-sidebar-width` | Chat list width |

## Classes

Stable class names, safe to target:

`mg-app`, `mg-titlebar`, `mg-sidebar`, `mg-chat-list`, `mg-chat-item`
(`mg-selected`, `mg-pinned`, `mg-local-pin`), `mg-avatar`, `mg-unread`,
`mg-chat`, `mg-chat-header`, `mg-messages`, `mg-day`, `mg-message`
(`mg-out`, `mg-deleted`), `mg-sender`, `mg-text`, `mg-meta`, `mg-badge`,
`mg-reply-preview`, `mg-edit-history`, `mg-composer`, `mg-send`,
`mg-command-popup`, `mg-settings`, `mg-plugin-card`, `mg-toast`.

Messages carry `data-id`; chats carry `data-chat-id`; plugin buttons carry
`data-plugin`.

## Bundled examples

`themes/` contains **Midnight Mello** (dark), **Paper** (light) and
**Compact** (spacing only — stack it on a colour theme).
