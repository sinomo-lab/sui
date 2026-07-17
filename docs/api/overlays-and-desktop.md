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
| Blocking task or confirmation | `Dialog` / `Modal` | Modal hit testing, focus trap, Escape and optional scrim dismissal |
| Search application commands | `CommandPalette` | Dialog lifecycle with command-palette diagnostics; query and ranking stay application-owned |
| Edge presentation | `SideSheet`, `Drawer`, `BottomSheet` | Modal sheet lifecycle and focus restoration |
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

With a desktop or web platform feature enabled, `NativeFileDialogs` implements
the asynchronous `FileDialogService` contract:

```rust,ignore
use sui::prelude::*;

let request = FileDialogRequest::new(FileDialogMode::OpenFiles)
    .title("Attach files")
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
web handles intentionally do not, so portable code should use `read` and
`write`. Folder selection is unavailable in web builds. SUI does not block the
UI thread or prescribe an async executor for application code.

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
tested, and do not take focus.

## Inspector Diagnostics

Embedding hosts and inspectors can call `Runtime::overlay_snapshot(window_id)`
to inspect the ordered owners, logical parents, rendered surfaces, active
modal, and focus trap. `Runtime::take_overlay_traces(window_id)` drains
`Opened`, `Closed`, `Reordered`, `DismissRequested`, `FocusEntered`, and
`FocusRestored` samples. Pair these with existing command and invalidation
traces when diagnosing an overlay that did not open, dismiss, or repaint as
expected.
