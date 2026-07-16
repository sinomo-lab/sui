# Virtual Collections

[Previous: widgets and layout](widgets-and-layout.md) ·
[API guide](README.md) ·
[Next: input and text editing](input-and-editing.md)

SUI has two complementary virtualization paths:

- `VirtualScrollView` owns a static set of retained child widgets. It measures
  all of them so heterogeneous heights are exact, but arranges, visits, and
  paints only the visible and overscanned range.
- `VirtualList<K, T>` reads a keyed data source and realizes only the visible
  and overscanned rows. It is the appropriate foundation for large or
  incrementally changing application collections.

`VirtualTable` remains the lightweight painter-delegate path for very large
tables. It shares the collection extent index for variable-height rows and
adds stable row keys, range navigation, and resizable columns without
allocating a widget subtree for every row.

## Keyed Data Model

`VirtualCollectionModel<K, T>` is the default observable source:

```rust
use sui::prelude::*;

#[derive(Clone, PartialEq)]
struct Task {
    title: String,
    detail: String,
}

fn task_list() -> impl Widget {
    let tasks = VirtualCollectionModel::from_items(
        "Tasks",
        [
            (
                1_u64,
                Task {
                    title: "Index workspace".into(),
                    detail: "Queued".into(),
                },
            ),
            (
                2,
                Task {
                    title: "Run checks".into(),
                    detail: "Waiting".into(),
                },
            ),
        ],
    )
    .expect("unique task keys");

    VirtualList::new("Task queue", tasks, |_key, task| {
        let title = task.select_named("Task title", |task| task.title.clone());
        Label::new("").text_from(title)
    })
    .estimated_row_height(28.0)
}
```

The model accepts incremental changes:

```rust
tasks.apply(CollectionChange::Insert {
    index: 0,
    items: older_tasks,
})?;
tasks.update(task_id, changed_task)?;
tasks.move_to(task_id, new_index)?;
tasks.remove(task_id)?;
```

Keys must be unique. Invalid mutations fail without partially changing the
collection. The model retains a bounded key-only change journal; a consumer
that falls behind receives a complete key reset while preserving same-key row
state where possible.

Applications do not have to use this model. Implement
`VirtualCollectionSource<K, T>` to adapt a reducer, database window, actor
snapshot, or another observable store. A source without an incremental journal
may return `CollectionSync::Reset` after each revision.

## Variable Heights and Visible Ranges

`CollectionExtentIndex` is a safe implicit-tree prefix-sum index. It supports:

- index-to-offset and offset-to-index lookup
- measured extent corrections
- insert, remove, and move
- total content extent

These operations are expected logarithmic time. A `VirtualList` starts with
the configured estimated row height, realizes the overscanned viewport, and
corrects the index with measured row heights. The current visible key is held
as an anchor while corrections or model changes move preceding content.

Use a realistic estimate to reduce initial correction:

```rust
VirtualList::new("Messages", messages, build_message)
    .estimated_row_height(84.0)
    .overscan_viewports(0.75)
```

## Scroll Anchoring and Follow-End

Prepending items automatically preserves the first visible key and its offset
within the viewport. This is the normal history-loading behavior for chat,
logs, and event streams.

Enable follow-end through `VirtualListState`:

```rust
let state = VirtualListState::new();
let list = VirtualList::new("Operation log", entries, build_entry)
    .state(state.clone())
    .stick_to_end(true);
```

While following, appended rows keep the viewport at the end. A user scroll
beyond the end tolerance suspends following. Call `state.resume_follow_end()`
for an explicit "Jump to latest" action.

Programmatic key scrolling supports start, center, end, and nearest alignment:

```rust
state.scroll_to(message_id, ScrollAlignment::Center);
```

The underlying `ScrollState` is available through `state.scroll_state()` for a
standalone scroll bar or general offset inspection.

## Retained Rows and Recycling

Visible rows receive a per-item `Signal<T>` and a stable `WidgetPod` while
realized. Same-key updates change the signal rather than rebuilding the row.

Recently hidden rows stay in a bounded least-recently-used cache. A focused row
is retained automatically. Pin any other row whose local editor or interaction
state must survive a long off-screen interval:

```rust
state.pin(editing_record_id);
// Later, after committing or cancelling:
state.unpin(&editing_record_id);
```

Do not pin an unbounded number of rows. Durable document and form state should
live in the application model; pinning protects transient focus, IME,
selection, animation, and in-progress editor state.

`cache_capacity` controls additional recently hidden rows and defaults to 64:

```rust
VirtualList::new("Schema fields", fields, build_field)
    .cache_capacity(128)
```

## Selection, Navigation, and Loading Edges

`VirtualListState` stores selection by key, so insertion and sorting do not
silently select another item:

```rust
state.select(Some(task_id));
assert_eq!(state.selected_key(), Some(task_id));
```

Pointer activation and Up, Down, Home, End, Page Up, and Page Down navigation
use the same keyed selection. `on_change` reports the selected key.

Paged sources may use `on_near_start` and `on_near_end`. Notifications are
edge-triggered and reset after the viewport leaves the threshold:

```rust
VirtualList::new("History", history, build_row)
    .on_near_start(request_older_history)
    .on_near_end(request_newer_page)
```

Callbacks should enqueue or start work and return promptly.

## Accessibility

The virtual list exposes:

- total item count
- realized range
- counts before and after that range
- stable synthetic item identity by application key
- item position and set size
- focus and activation actions
- forward and backward range actions

Only realized rows emit item nodes. Synthetic actions route to the
graph-backed collection widget while retaining the original virtual item ID.
This avoids constructing an accessibility tree proportional to the complete
data set.

Rows may still emit their own child semantics. Interactive editors and buttons
therefore remain accessible without realizing off-screen siblings.

## VirtualTable and TreeView

`VirtualTable` supports:

- fixed or variable row heights with `row_heights`
- stable accessibility identity with `row_key`
- visible-range descriptions and range actions
- keyboard visibility tracking and page navigation
- resizable columns, optional per column, with `on_column_resize`
- leading pinned columns that remain fixed during horizontal trackpad scrolling
- application-owned sorting through sort indicators and header activation

Sorting remains a model responsibility: SUI emits the header action and paints
the configured direction; it does not reorder remote or paged data.

`TreeItem::key` provides stable tree-row identity. `TreeView` caches its
flattened expanded rows, exposes visible row semantics with level and position,
and routes accessible expand/collapse actions to the keyed row.

A data-backed table row factory remains a separate policy layer. It should
build on the keyed virtual collection source while retaining the existing
paint-delegate table for lightweight read-only data.

## Choosing a Collection Widget

| Situation | Widget |
| --- | --- |
| Small list with built-in row appearance | `ListView` |
| Modest heterogeneous retained children | `ScrollView` |
| Static retained children with paint/visit virtualization | `VirtualScrollView` |
| Large keyed interactive collection | `VirtualList` |
| Large lightweight painter-delegate table | `VirtualTable` |
| Hierarchical navigation | `TreeView` |
