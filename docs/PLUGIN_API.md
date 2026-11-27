# Writing MelloGram plugins

A plugin has up to two halves:

1. **Backend (Rust)** — implements `mellogram_core::Plugin`. Sees messages
   before they are sent and as they arrive, can veto "typing…", provides slash
   commands and CSS. Runs in the app process; unit-testable with no network.
2. **UI script (JavaScript, optional)** — returned by `Plugin::ui_script()`,
   run in the window with an `MG` object. Adds buttons, menu items, renderers.

Built-in plugins live in `crates/mellogram-plugins`. Third-party plugin loading
(sandboxed) is on the roadmap; today a plugin is added by putting it in that
crate and listing it in `builtin()`.

## Backend

```rust
use mellogram_core::*;

pub struct Shout;

impl Plugin for Shout {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            description: "Adds /shout, which uppercases your message.",
            ..PluginManifest::new("shout", "Shout", Category::Chat)
        }
    }

    fn settings(&self) -> Vec<SettingDef> {
        vec![SettingDef::boolean("exclaim", "Add !!!", true)]
    }

    fn commands(&self, _: &PluginContext) -> Vec<(String, String)> {
        vec![("shout".into(), "SAY IT LOUDER".into())]
    }

    fn run_command(&self, ctx: &PluginContext, name: &str, msg: &mut OutgoingMessage) -> CommandOutcome {
        if name != "shout" { return CommandOutcome::NotHandled; }
        msg.text = msg.text.to_uppercase();
        if ctx.bool("exclaim") { msg.text.push_str("!!!"); }
        CommandOutcome::Send
    }
}
```

### Manifest

| Field | Meaning |
|---|---|
| `id` | Stable kebab-case id; the settings key. Never change it. |
| `name`, `description`, `authors`, `version` | Shown in the plugin manager. |
| `category` | `Appearance`, `Chat`, `Media`, `Privacy`, `Profiles`, `Notifications`, `Developer`, `Utility`. |
| `dependencies` | Plugin ids enabled automatically first. |
| `platforms` | Where it can run; others show "unsupported here". |
| `required` | Always on, cannot be disabled. |
| `enabled_by_default` | Initial state for new users. |
| `priority` | Lower runs earlier in hook chains (default 0; ties broken by id). |
| `inspired_by` | Free text shown as a tag, e.g. `"Vencord: ClearURLs"`. |
| `permissions` | Declared capabilities: `ReadMessages`, `ModifyOutgoing`, `Ui`, `Storage`, `Network`, `Clipboard`, `Styles`. Shown on the plugin card, in Privacy & Security, and when inspecting shared profiles. `Storage` is enforced: undeclared plugins cannot use `MG.storage`. |

### Hooks

| Hook | When | Notes |
|---|---|---|
| `start` / `stop` | Enable/disable, launch/quit | Return `Err` to refuse to start. |
| `before_send(ctx, &mut OutgoingMessage)` | Every send, after slash commands | Rewrite text, flags (`silent`, `link_preview`), attachments; or `SendDecision::Cancel(reason)`. |
| `commands` / `run_command` | `/name args` typed in the composer | `msg.text` holds just `args`. Return `Send`, `Consumed(Some(toast))` or `NotHandled`. `//text` escapes. |
| `on_message(ctx, &mut IncomingMessage)` | Live messages and loaded history | Set `display_name`, push `badges`, `add_class("…")`, set `highlight` (validate colours with `mellogram_core::appearance::safe_color`). |
| `on_edit(ctx, previous, &mut msg)` | Message edited | `previous` is the last version this session saw. |
| `on_delete(ctx, chat_id, ids) -> bool` | Messages deleted | `true` keeps them on screen marked deleted. `chat_id` is `None` for private chats/small groups (Telegram ids are unique per account there). |
| `allow_typing(ctx, chat_id) -> bool` | Before broadcasting "typing…" | Any `false` wins. |
| `css(ctx) -> Option<String>` | Whenever styles are recomposed | Injected after themes, before QuickCSS. Sanitise any setting you interpolate. |
| `ui_script() -> Option<&'static str>` | See below | Usually `include_str!("../ui/my-plugin.js")`. |

Hooks must be fast and must not block; spawn work from `start` if needed. A
panic in a hook is caught, logged on the plugin card, and after three panics
the plugin is paused for the session.

### Settings

`SettingDef::boolean / text / number / select / list` render automatically.
Values are validated against the definition before they are stored, so
`ctx.bool("x")`, `ctx.str`, `ctx.number`, `ctx.list` always see the declared
type (with defaults filled in). Mark a setting `restart_needed` to have the
plugin stopped and started when it changes.

### Testing

```rust
let p = Shout;
let ctx = PluginContext::with_defaults(&p);
let mut m = OutgoingMessage::text(1, "hi");
assert_eq!(p.run_command(&ctx, "shout", &mut m), CommandOutcome::Send);
assert_eq!(m.text, "HI!!!");
```

## UI scripts

The script body runs as an `async` function receiving `MG` (so top-level
`await` works). Return a cleanup function
(optional — everything registered through `MG` is removed automatically when
the plugin is disabled or its settings change, which re-runs the script).

```js
const btn = MG.composer.addButton({ id: "hello", label: "👋", title: "Wave", onClick() {
  MG.composer.el.value += " 👋";
}});
return () => btn.remove();
```

| API | Description |
|---|---|
| `MG.id`, `MG.settings` | Plugin id and its current settings (read-only copy). |
| `await MG.storage.get()` / `MG.storage.set(value)` | Per-plugin JSON file on this device (≤ 2 MB). Requires the `Storage` permission. |
| `MG.clipboard.write(text)`, `MG.download(name, text, mime)` | Clipboard and file download helpers. |
| `MG.setSetting(key, value)` | Persist a setting (validated by the backend). |
| `MG.invoke(cmd, args)` | Call a backend command. |
| `MG.toast(text, level)` | `info` / `success` / `warning` / `error`. |
| `MG.modal(title, nodes)` | Show a dialog; returns `close()`. |
| `MG.h(tag, props, ...children)` | Tiny DOM builder (`text:` sets textContent safely). |
| `MG.composer.el` / `.footer` | The textarea / the line under it. |
| `MG.composer.flags` | Merged into the next send (`silent`). |
| `MG.composer.addButton({id,label,title,onClick})` | Button left of the input. |
| `MG.composer.insert(text)` | Insert at the caret; `{cursor}` marks where the caret ends up. |
| `MG.composer.onInput(fn(text))`, `onSent(fn(result))` | `result.latency_ms` on success; `result.status === "cancelled"` if a plugin stopped it. |
| `MG.header.addButton({...})` | Button in the title bar. |
| `MG.messages.addAction({id,label,onClick(msg)})` | Message context-menu item. |
| `MG.messages.onRender(fn(msg, el))` | Decorate each rendered message. |
| `MG.messages.setTimeFormatter(fn(unixSeconds) → string)` | Replace message timestamps. |
| `MG.messages.loaded()`, `MG.messages.jumpTo(id)` | Loaded messages of the open chat (copies, oldest first); scroll to one. |
| `MG.chats.addSorter(fn(a, b))`, `onRender(fn(chat, el))`, `addAction({...})` | Chat list extension points. `label` may be a function of the chat. |
| `MG.chats.rerender()`, `refresh()` | Redraw / refetch the chat list. |
| `MG.chats.current()`, `await MG.chats.open(id)`, `MG.chats.onOpen(fn(chat))` | Open chat, open another chat, react to chat changes. |

Use `textContent` (or `MG.h(..., { text })`) for anything derived from message
content; never `innerHTML`.
