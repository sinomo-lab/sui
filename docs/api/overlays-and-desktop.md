# Overlays and Desktop Interaction

[Previous: widgets and layout](widgets-and-layout.md) · [API guide](README.md) ·
[Next: virtual collections](virtual-collections.md)

SUI coordinates transient interface layers at the window level. Widgets still
own their content and state, while the runtime owns cross-overlay policy:
ordering, nesting, modal input, focus containment and restoration, dismissal,
semantic isolation, and lifecycle diagnostics. This keeps an overlay's logical
ownership independent from the scene layer that paints it.

## Built-in Presentations

Use the narrowest built-in policy for the interaction:

| Need | Widget | Runtime behavior |
| --- | --- | --- |
| Hover help | `Tooltip` | Non-modal and non-dismissible; does not move focus |
| Anchored transient content | `Popover` | Collision-aware placement, Escape/outside dismissal, normal Tab navigation |
| Commands at a pointer target | `ContextMenu` | Menu semantics, arrow-key navigation, nested-overlay participation |
| Blocking task or confirmation | `Dialog` | Modal hit testing, focus trap, Escape and optional scrim dismissal |
| Search application commands | `CommandPalette` | Dialog lifecycle with command-palette diagnostics; query and ranking stay application-owned |
| Edge presentation | `SideSheet`, `BottomSheet` | Modal sheet lifecycle and focus restoration |
| Transient status | `NotificationHost` | Non-interactive overlay stack with live-region semantics and timed expiry |

Windows are stacking hosts automatically. `OverlayHost` creates an independent
z-order root for an embedded viewport or workspace region, while the window
manager still coordinates focus, dismissal, and modality. Overlay surfaces use
retained stack layers and are not clipped by ordinary ancestor widgets; they
retain their own paint-bounds clip and remain subject to the window viewport.

## Collision-Aware Placement

`Popover`, `ContextMenu`, and `Select` use the shared placement solver. Custom
presentations can call `place_overlay` directly:

```rust
use sui::place_overlay;
use sui::prelude::*;

let result = place_overlay(
    &OverlayPlacementRequest::new(
        anchor_bounds,
        Size::new(320.0, 240.0),
        viewport_bounds,
        OverlayPlacement::BOTTOM_START,
    )
    .fallbacks([
        OverlayPlacement::TOP_START,
        OverlayPlacement::RIGHT_START,
    ])
    .gap(6.0)
    .margin(8.0),
);
```

The adaptive policy first chooses a preferred or fallback side that fits, then
shifts and finally resizes inside the safe viewport. Use
`OverlayCollisionPolicy::NONE` only when overflow is intentional.

## Context-Menu Submenus

`MenuItem::submenu` builds recursive context-menu branches. Each child panel
inherits the side selected by its parent and falls back across the anchor when
the preferred side would leave the viewport:

```rust
use sui::prelude::*;

let menu = ContextMenu::new("File actions", Button::new("Actions"))
    .activation_button(PointerButton::Primary)
    .item(MenuItem::new("Move to").submenu([
        MenuItem::new("Archive"),
        MenuItem::new("Shared").submenu([
            MenuItem::new("Team workspace"),
            MenuItem::new("Project workspace"),
        ]),
    ]))
    .on_activate_path(|path, item| {
        println!("activated {path:?}: {}", item.label());
    });
```

Pointer hover opens a branch. Arrow Right enters it, Arrow Left returns to its
owner, and the ordinary Up, Down, Home, End, Enter, Space, and Escape behavior
continues at the active level. Submenu owners expose menu-popup and
expanded/collapsed semantics. Existing `on_activate` callbacks remain valid for
flat menus; use `on_activate_path` or `on_activate_path_with_ctx` when sibling
branches can contain different leaves.

## Showing Overlays in Place

Galleries, documentation, and screenshots often need an overlay visible next
to other content rather than floating over the window. `Tooltip`, `Popover`,
`ContextMenu`, `Select`, and `Dialog` provide `show_inline()`: the surface stays
open, joins the widget's own measured size below (or above) its trigger, and
paints in flow instead of in the window overlay stack. Inline overlays ignore
dismissal but keep their normal pointer and keyboard behavior.

```rust
use sui::prelude::*;

let open_menu = ContextMenu::new("Layer menu", Label::new("Layer 08"))
    .item(MenuItem::new("Rename"))
    .item(MenuItem::new("Move to").submenu([MenuItem::new("Archive")]))
    .show_inline()
    .highlighted_path([1]);

let hint = Tooltip::new("Copy link", Button::new("Share"))
    .placement(TooltipPlacement::Below)
    .show_inline();
```

`ContextMenu::highlighted_path` highlights an item and opens the submenus that
lead to it, which is useful for showing nested panels.

## Custom Managed Overlays

A custom overlay remains a normal retained widget. Declare its active policy
with `Widget::overlay_options`, emit an `Overlay` or `Effect` layer, and handle
the typed dismissal request:

```rust,ignore
impl Widget for InspectorPopup {
    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        let Some(request) = command.get(OVERLAY_DISMISS_REQUEST) else {
            return;
        };
        self.open = false;
        self.last_dismiss_reason = Some(request.reason);
        ctx.request_measure();
        ctx.request_semantics();
        ctx.set_handled();
    }

    fn overlay_options(&self) -> Option<OverlayOptions> {
        self.open.then_some(
            OverlayOptions::new(OverlayKind::Popover)
                .dismiss(OverlayDismissPolicy::TRANSIENT)
                .focus(OverlayFocusBehavior::NONE),
        )
    }
}
```

`OverlayFocusBehavior::CONTAINED` moves focus to the first focusable descendant,
wraps Tab and Shift+Tab within the overlay, and restores the previous live
widget when the overlay closes. Modal overlays also suppress background
semantic nodes and retarget background pointer input to the modal owner.
Nested overlays receive a logical `parent` in the manager snapshot. Escape and
outside-pointer dismissal always select the topmost eligible overlay.

Use `OverlayFocusBehavior::RESTORE` for a modeless presentation that should
return focus after programmatic dismissal. If focus has already moved to a
live widget outside that presentation, the manager preserves the new target.

## Accessibility Relationships

Built-ins expose dialog, menu, list-box, tooltip, and status semantics together
with modal state and live-region urgency. Custom widgets can populate
`SemanticsNode::relations` (`controls`, `labelled_by`, `described_by`, and
`owns`), `popup`, and `live_region`. On Windows these fields map to the
corresponding AccessKit properties.

## Notifications

`NotificationCenter` is a thread-safe producer. Keep one center per window or
application presentation policy and place one `NotificationHost` in that
window's retained tree:

```rust
use sui::prelude::*;

let notifications = NotificationCenter::new();
let background_notifications = notifications.clone();

let host = NotificationHost::new(notifications.clone());
notifications.notify("Saved", "Workspace settings were updated");
background_notifications.push(
    TransientNotification::new("Build failed", "Open the operation log")
        .urgency(NotificationUrgency::Assertive)
        .persistent(),
);
```

Timed notifications schedule runtime timers rather than a permanent animation
pump. The host is non-hit-tested; applications that need buttons or contextual
actions should build a managed interactive overlay instead.

## File Dialogs

With a desktop, web, or mobile platform feature enabled, `NativeFileDialogs`
implements the asynchronous `FileDialogService` contract:

```rust,ignore
use sui::prelude::*;

let request = FileDialogRequest::new(FileDialogMode::OpenFiles)
    .title("Attach files")
    .parent_window(window_id)
    .filter(FileDialogFilter::new("Documents", ["md", "txt", "pdf"]));

if let Some(selection) = NativeFileDialogs.show(request).await? {
    for file in selection.files {
        let name = file.file_name();
        let bytes = file.read().await?;
        process_attachment(name, bytes);
    }
}
```

Cancellation returns `Ok(None)`. Desktop handles expose a filesystem `path`;
web and Android handles do not, so portable code should use `read` and
`write`. SUI does not block the UI thread or prescribe an async executor for
application code.

### Folders

`FileDialogMode::OpenFolder` and `OpenFolders` return the chosen folders in
`selection.folders` as `PlatformFolder` handles. A folder lists the files and
folders directly inside it and opens or creates them by name, so the same code
works with or without a filesystem path:

```rust,ignore
use sui::prelude::*;

let request = FileDialogRequest::new(FileDialogMode::OpenFolder);
if let Some(selection) = NativeFileDialogs.show(request).await? {
    let Some(folder) = selection.folders.first() else { return Ok(()) };
    for entry in folder.entries().await? {
        if let FolderEntry::File(file) = entry {
            index_document(file.file_name(), file.read().await?);
        }
    }
    let notes = folder.create_folder("notes").await?;
    notes.create_file("today.md").await?.write(b"# Today\n").await?;
}
```

`file(name)` and `folder(name)` return `None` when nothing of that kind has the
name. `create_file` and `create_folder` return the existing entry when there
is one, without changing a file's contents, and fail when the name belongs to
the other kind. Names are single names, never paths: `""`, `.`, `..`, and
names containing `/` or `\` are errors, so a folder only reaches what is
inside it. Entries come in no particular order.

Desktop folders have a `path`, and desktop folder dialogs also list the chosen
folders in `selection.files` as path-backed files, as they did before
`folders` existed. Web and Android pickers choose one folder at a time, so
`OpenFolders` returns at most one there.

On the web, folder dialogs use the File System Access API's
`showDirectoryPicker`, which Chromium browsers such as Chrome and Edge
provide; other browsers return an error. SUI asks for read and write access
when the user picks the folder, so the browser may ask the user to allow
editing.

### Android

On Android, opening and saving show the system document picker. Each file is
a document that its provider reads and writes, which may be a cloud or app
storage provider rather than local storage, and `file_name` is the name the
provider shows. The picker filters by MIME type rather than by extension, so
SUI combines the request's filters into the types Android knows for their
extensions; if any extension has no known type, every file shows. A save
request's suggested name becomes the new document's name, and its type comes
from that name's extension or else the first filter. Android ignores the
title, starting directory, and parent window.

Folder dialogs show the system folder picker, and the user confirms that the
app may access the folder. Android does not let apps choose the root of
storage, the Download folder itself, or app data folders, so users pick or
create a folder inside them. Access lasts while the app runs. Listing a
folder and finding an entry by name ask the folder's provider each time.

### Showing dialogs from handlers

Call `NativeFileDialogs.show(request)` inside the UI callback that initiated
the action. It resolves the logical parent window and constructs the rfd
operation synchronously, satisfying macOS main-thread setup; the returned
native future is `Send` and may then be polled by a background executor. The
parent registry stores weak window references. Once resolved, the returned
future retains its parent window while the future is active; a request for a
window that has already closed returns an error. Web and Android ignore
`parent_window`. Browsers show dialogs only during the user activation that
asked for them, so on the web the returned future must begin polling then;
simply storing it for later is insufficient. The free
`show_file_dialog(request)` async helper defers setup until first poll, so use
it only when that poll already occurs on the required UI/user-activation path.

## Opening Links

Activating a `Link`, or a link in a `RichDocumentView`, only runs the
application's callback; SUI never opens a URL on its own. To open one in the
default browser, mail client, or the app registered for its scheme, call
`EventCtx::open_url` from the callback. A button that opens a page works the
same way:

```rust,no_run
use sui::prelude::*;

let docs = Link::new("Read the docs", "https://example.com/docs").on_open_with_ctx(|ctx, url| {
    ctx.open_url(url);
});

let document = RichDocumentModel::from_markdown("See [the guide](https://example.com/guide).");
let view = RichDocumentView::new(document).on_link_with_ctx(|ctx, destination| {
    ctx.open_url(destination);
});

let release_notes = Button::new("Release notes").on_press_with_ctx(|ctx| {
    ctx.open_url("https://example.com/releases");
});
# let _ = (docs, view, release_notes);
```

Because the callback decides, it can also open some URLs inside the app and
pass the rest on:

```rust,no_run
# use sui::prelude::*;
# fn navigate(_route: &str) {}
let settings = Link::new("Settings", "app://settings").on_open_with_ctx(|ctx, url| {
    match url.strip_prefix("app://") {
        Some(route) => navigate(route),
        None => {
            ctx.open_url(url);
        }
    }
});
# let _ = settings;
```

`open_url` returns whether a platform handler accepted the URL. A `UrlPolicy`
decides which URLs it hands to the platform. The default opens only absolute
`http`, `https`, and `mailto` URLs, so a link in untrusted content, such as
Markdown from the network or a model, cannot launch a local file, a script, or
an arbitrary protocol handler, and relative Markdown destinations never open.

An application that needs more replaces the policy with its own check, or
turns the check off:

```rust,no_run
# use sui::prelude::*;
// Also open the application's own scheme.
let app = App::new().url_policy(UrlPolicy::new(|url| {
    url.starts_with("myapp:") || UrlPolicy::allows_by_default(url)
}));

// Open every URL. Use this only when the app trusts every URL it opens.
let trusting = App::new().url_policy(UrlPolicy::allow_all());
# let _ = (app, trusting);
```

The policy belongs to the runtime. `App::url_policy` sets it before the app
starts; to change it later, call `set_policy` on the `UrlOpener` that
`Runtime::url_opener` and `EventCtx::url_opener` return, and pass
`UrlPolicy::default()` to restore the default.

The desktop platform opens URLs on Linux, macOS, and Windows through the
system's registered handler, the web platform opens them in a new tab, and
Android starts a view activity for them. Browsers block tabs opened outside a
user gesture, so open URLs while handling the click or key press that asked
for them. Runtimes without a platform, such as headless tests, open nothing;
install a `LocalUrlOpenerBackend` with `Runtime::set_url_opener_backend` and
read its `opened()` list to check what a test asked to open.

## Platform File Drag and Drop

Desktop window events normalize file hover, cancellation, and drop. Wrap the
window content in a root `DragDropHost` to consume them without mixing platform
events into ordinary drop-target payload logic:

```rust,ignore
let root = DragDropHost::new(scope, content)
    .on_external_file_hover(|ctx, paths| {
        // Update a drop affordance.
        ctx.request_paint();
    })
    .on_external_file_drop(|ctx, path| {
        queue_import(path);
        ctx.request_semantics();
    })
    .on_external_file_hover_cancelled(|ctx| ctx.request_paint());
```

Internal drag previews also participate in the overlay stack, are never hit
tested, and do not take focus. `DragDropHost::preview` returns the widget to
draw under the pointer for a drag, such as a lifted card, or `None` to keep the
label.

A source allows one effect or several (`Draggable::effects`). Targets usually
accept `drag.preferred_effect()`, which follows the platform's modifier keys
(Control copies and Shift moves; Option and Command on macOS) and changes when
those keys do, without the pointer moving. `DropTarget::on_hover_state` says
whether a target accepts or refuses the drag over it. Escape cancels a drag,
and scroll views scroll while a drag is held near their edges.

## Inspector Diagnostics

Embedding hosts and inspectors can call `Runtime::overlay_snapshot(window_id)`
to inspect the ordered owners, logical parents, rendered surfaces, active
modal, and focus trap. `Runtime::take_overlay_traces(window_id)` drains
`Opened`, `Closed`, `Reordered`, `DismissRequested`, `FocusEntered`, and
`FocusRestored` samples. Pair these with existing command and invalidation
traces when diagnosing an overlay that did not open, dismiss, or repaint as
expected.
