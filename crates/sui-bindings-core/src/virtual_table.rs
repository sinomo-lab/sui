//! Thread-safe keyed row model and runtime adapter behind the `VirtualTable`
//! binding.
//!
//! The host language owns row data and sort policy. [`BindingTableModel`] is
//! shared between host threads and the UI thread; mutations bump a revision
//! signal and wake the bound UI loop. The runtime adapter realizes only the
//! visible rows of the Rust [`VirtualTable`] and keeps its scroll, focus,
//! column widths, and keyed selection across model updates.

use crate::actions::{BindingAction, BindingIdAction, BindingIdNumberAction};
use crate::errors::{ForeignCallbackError, ForeignCallbackPhase, ForeignWidgetId};
use crate::messages::BindingValue;
use crate::state::BindingState;
use crate::support::recover_lock;
use crate::tasks::BindingUiHandle;
use crate::values::BindingBool;
use crate::widget_descriptor::BindingBuildContext;
use std::cell::Cell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use sui::ArrangeCtx;
use sui::Constraints;
use sui::Event;
use sui::EventCtx;
use sui::KeyState;
use sui::MeasureCtx;
use sui::PaintCtx;
use sui::Rect;
use sui::SemanticsCtx;
use sui::Signal;
use sui::Size;
use sui::TableColumnAlignment;
use sui::TextCellPaint;
use sui::VirtualTable;
use sui::VirtualTableColumn;
use sui::VirtualTableRowActivationKind;
use sui::VirtualTableSortDirection;
use sui::VirtualTableState;
use sui::Widget;

/// Parse a sort direction name: `"ascending"`/`"asc"` or
/// `"descending"`/`"desc"`.
pub fn binding_sort_direction_from_name(value: &str) -> Option<VirtualTableSortDirection> {
    match value.trim().to_ascii_lowercase().as_str() {
        "ascending" | "asc" => Some(VirtualTableSortDirection::Ascending),
        "descending" | "desc" => Some(VirtualTableSortDirection::Descending),
        _ => None,
    }
}

pub fn binding_sort_direction_name(direction: VirtualTableSortDirection) -> &'static str {
    match direction {
        VirtualTableSortDirection::Ascending => "ascending",
        VirtualTableSortDirection::Descending => "descending",
    }
}

/// A column of a bound `VirtualTable`. The key identifies the column in
/// callbacks, retained widths, and [`BindingTableModel::set_sort`].
#[derive(Debug, Clone, PartialEq)]
pub struct BindingVirtualTableColumn {
    pub(crate) key: u64,
    pub(crate) title: String,
    pub(crate) width: Option<f32>,
    pub(crate) min_width: Option<f32>,
    pub(crate) max_width: Option<f32>,
    pub(crate) resizable: bool,
    pub(crate) alignment: TableColumnAlignment,
    pub(crate) sort_direction: Option<VirtualTableSortDirection>,
}

impl BindingVirtualTableColumn {
    pub fn new(key: u64, title: impl Into<String>) -> Self {
        Self {
            key,
            title: title.into(),
            width: None,
            min_width: None,
            max_width: None,
            resizable: true,
            alignment: TableColumnAlignment::Start,
            sort_direction: None,
        }
    }

    pub fn width(mut self, width: Option<f32>) -> Self {
        self.width = width;
        self
    }

    pub fn min_width(mut self, min_width: Option<f32>) -> Self {
        self.min_width = min_width;
        self
    }

    pub fn max_width(mut self, max_width: Option<f32>) -> Self {
        self.max_width = max_width;
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn alignment(mut self, alignment: TableColumnAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn sort_direction(mut self, direction: Option<VirtualTableSortDirection>) -> Self {
        self.sort_direction = direction;
        self
    }

    pub fn key(&self) -> u64 {
        self.key
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    fn into_sui(
        &self,
        sort: Option<Option<(u64, VirtualTableSortDirection)>>,
    ) -> VirtualTableColumn {
        let direction = match sort {
            Some(sort) => sort
                .filter(|(column, _)| *column == self.key)
                .map(|(_, direction)| direction),
            None => self.sort_direction,
        };
        let mut column = VirtualTableColumn::new(self.title.clone())
            .key(self.key)
            .resizable(self.resizable)
            .alignment(self.alignment)
            .sort_direction(direction);
        if let Some(width) = self.width {
            column = column.width(width);
        }
        if let Some(min_width) = self.min_width {
            column = column.min_width(min_width);
        }
        if let Some(max_width) = self.max_width {
            column = column.max_width(max_width);
        }
        column
    }
}

/// A keyed row of text cells, one per column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingVirtualTableRow {
    pub key: u64,
    pub cells: Vec<String>,
}

impl BindingVirtualTableRow {
    pub fn new(
        key: u64,
        cells: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, String> {
        if key == 0 {
            return Err("table row keys must be non-zero".to_string());
        }
        Ok(Self {
            key,
            cells: cells.into_iter().map(Into::into).collect(),
        })
    }
}

#[derive(Default)]
struct TableModelData {
    rows: Vec<BindingVirtualTableRow>,
    index_by_key: HashMap<u64, usize>,
    revision: u64,
    /// `None` until [`BindingTableModel::set_sort`] is first called; the
    /// columns' own sort directions apply until then.
    sort: Option<Option<(u64, VirtualTableSortDirection)>>,
    sort_revision: u64,
}

impl TableModelData {
    fn reindex_from(&mut self, start: usize) {
        if start == 0 {
            self.index_by_key.clear();
        }
        for (index, row) in self.rows.iter().enumerate().skip(start) {
            self.index_by_key.insert(row.key, index);
        }
    }
}

struct TableModelShared {
    data: Mutex<TableModelData>,
    revision: Signal<u64>,
    ui_handle: Mutex<Option<BindingUiHandle>>,
}

/// A thread-safe, keyed row model for a bound `VirtualTable`.
///
/// Any thread may mutate the model. Each change bumps a revision and wakes
/// the bound UI loop; the table then re-reads only the rows it shows. Row
/// keys are stable, non-zero integers: selection follows the key when the
/// application reorders rows, for example to sort them.
#[derive(Clone)]
pub struct BindingTableModel {
    shared: Arc<TableModelShared>,
}

impl BindingTableModel {
    pub fn new(rows: impl IntoIterator<Item = BindingVirtualTableRow>) -> Result<Self, String> {
        let model = Self {
            shared: Arc::new(TableModelShared {
                data: Mutex::new(TableModelData::default()),
                revision: Signal::named("TableModel", 0),
                ui_handle: Mutex::new(None),
            }),
        };
        model.replace(rows)?;
        Ok(model)
    }

    pub fn len(&self) -> usize {
        recover_lock(&self.shared.data).rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The cells of the row with `key`.
    pub fn get(&self, key: u64) -> Option<Vec<String>> {
        let data = recover_lock(&self.shared.data);
        data.index_by_key
            .get(&key)
            .and_then(|index| data.rows.get(*index))
            .map(|row| row.cells.clone())
    }

    /// Row keys in display order.
    pub fn keys(&self) -> Vec<u64> {
        recover_lock(&self.shared.data)
            .rows
            .iter()
            .map(|row| row.key)
            .collect()
    }

    /// Replace every row. Keys must be unique.
    pub fn replace(
        &self,
        rows: impl IntoIterator<Item = BindingVirtualTableRow>,
    ) -> Result<bool, String> {
        let rows = rows.into_iter().collect::<Vec<_>>();
        let mut keys = HashSet::with_capacity(rows.len());
        if rows.iter().any(|row| !keys.insert(row.key)) {
            return Err("table row keys must be unique".to_string());
        }
        Ok(self.mutate(|data| {
            if data.rows == rows {
                return false;
            }
            data.rows = rows;
            data.reindex_from(0);
            true
        }))
    }

    pub fn append(&self, row: BindingVirtualTableRow) -> Result<bool, String> {
        // The index is clamped under the lock, so concurrent appends keep order.
        self.insert(usize::MAX, row)
    }

    /// Insert a row at `index`, clamped to the current length.
    pub fn insert(&self, index: usize, row: BindingVirtualTableRow) -> Result<bool, String> {
        let mut result = Ok(true);
        self.mutate(|data| {
            if data.index_by_key.contains_key(&row.key) {
                result = Err(format!("table row key {} is already present", row.key));
                return false;
            }
            let index = index.min(data.rows.len());
            data.rows.insert(index, row);
            data.reindex_from(index);
            true
        });
        result
    }

    /// Replace the cells of the row with the same key. Returns `false` when
    /// the key is absent or the cells are unchanged.
    pub fn update(&self, row: BindingVirtualTableRow) -> bool {
        self.mutate(|data| {
            let Some(index) = data.index_by_key.get(&row.key).copied() else {
                return false;
            };
            if data.rows[index].cells == row.cells {
                return false;
            }
            data.rows[index].cells = row.cells;
            true
        })
    }

    /// Remove the row with `key`. Returns `false` when it is absent.
    pub fn remove(&self, key: u64) -> bool {
        self.mutate(|data| {
            let Some(index) = data.index_by_key.remove(&key) else {
                return false;
            };
            data.rows.remove(index);
            data.reindex_from(index);
            true
        })
    }

    /// Show `direction` on the column with `column_key` and clear the other
    /// columns' indicators; `None` for either clears every indicator. Before
    /// the first call, the columns' own sort directions apply.
    pub fn set_sort(
        &self,
        column_key: Option<u64>,
        direction: Option<VirtualTableSortDirection>,
    ) -> bool {
        let sort = column_key.zip(direction);
        let changed = {
            let mut data = recover_lock(&self.shared.data);
            if data.sort == Some(sort) {
                false
            } else {
                data.sort = Some(sort);
                data.sort_revision = data.sort_revision.wrapping_add(1);
                true
            }
        };
        if changed {
            self.notify();
        }
        changed
    }

    pub(crate) fn bind_ui_handle(&self, handle: &BindingUiHandle) {
        *recover_lock(&self.shared.ui_handle) = Some(handle.clone());
    }

    fn mutate(&self, change: impl FnOnce(&mut TableModelData) -> bool) -> bool {
        let changed = {
            let mut data = recover_lock(&self.shared.data);
            let changed = change(&mut data);
            if changed {
                data.revision = data.revision.wrapping_add(1);
            }
            changed
        };
        if changed {
            self.notify();
        }
        changed
    }

    fn notify(&self) {
        let _ = self
            .shared
            .revision
            .update(|revision| *revision = revision.wrapping_add(1));
        // An empty UI task wakes the loop and lays the window out again.
        let handle = recover_lock(&self.shared.ui_handle).clone();
        if let Some(handle) = handle {
            handle.post(|| {});
        }
    }

    fn revisions(&self) -> (u64, u64, usize) {
        let data = recover_lock(&self.shared.data);
        (data.revision, data.sort_revision, data.rows.len())
    }

    fn sort(&self) -> Option<Option<(u64, VirtualTableSortDirection)>> {
        recover_lock(&self.shared.data).sort
    }

    fn key_at(&self, index: usize) -> Option<u64> {
        recover_lock(&self.shared.data)
            .rows
            .get(index)
            .map(|row| row.key)
    }

    fn index_of(&self, key: u64) -> Option<usize> {
        recover_lock(&self.shared.data)
            .index_by_key
            .get(&key)
            .copied()
    }

    fn cells_at(&self, index: usize) -> Option<Vec<String>> {
        recover_lock(&self.shared.data)
            .rows
            .get(index)
            .map(|row| row.cells.clone())
    }
}

impl fmt::Debug for BindingTableModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingTableModel")
            .field("len", &self.len())
            .finish()
    }
}

/// Optional behavior of a bound `VirtualTable`.
#[derive(Debug, Clone, Default)]
pub struct BindingVirtualTableOptions {
    /// Selected row key; selection writes the key back as an integer. Zero
    /// or a missing key selects nothing.
    pub selected: Option<BindingState>,
    pub row_height: Option<f32>,
    /// Runs with the newly selected row key.
    pub on_change: Option<BindingIdAction>,
    /// Runs with the row key on double-click or Enter.
    pub on_row_activate: Option<BindingIdAction>,
    /// Runs with the column key when a header is clicked.
    pub on_header_activate: Option<BindingIdAction>,
    /// Runs with the column key and new width while a column is resized.
    pub on_column_resize: Option<BindingIdNumberAction>,
    /// Runs when scrolling comes close to the last row.
    pub on_near_end: Option<BindingAction>,
}

fn selected_key_from_value(value: &BindingValue) -> Option<u64> {
    match value {
        BindingValue::Integer(key) => u64::try_from(*key).ok(),
        BindingValue::Number(key) if key.is_finite() && *key >= 0.0 => Some(*key as u64),
        BindingValue::String(key) => key.trim().parse().ok(),
        _ => None,
    }
    .filter(|key| *key != 0)
}

/// Where the selected key lives: a bound state or the widget itself.
#[derive(Clone)]
enum SelectedKey {
    State(BindingState),
    Local(Rc<Cell<Option<u64>>>),
}

impl SelectedKey {
    fn get(&self) -> Option<u64> {
        match self {
            Self::State(state) => selected_key_from_value(&state.get()),
            Self::Local(key) => key.get(),
        }
    }

    fn set(&self, key: u64) {
        match self {
            Self::State(state) => state.set(BindingValue::Integer(key as i64)),
            Self::Local(local) => local.set(Some(key)),
        }
    }
}

fn report(errors: &BindingBuildContext, result: crate::errors::ForeignCallbackResult<()>) {
    if let Err(error) = result {
        errors.push(ForeignCallbackError::new(
            ForeignWidgetId::new(0),
            ForeignCallbackPhase::Event,
            error.message,
        ));
    }
}

/// Everything needed to (re)build the Rust table on the UI thread.
struct TableBuilder {
    name: String,
    columns: Vec<BindingVirtualTableColumn>,
    model: BindingTableModel,
    options: BindingVirtualTableOptions,
    context: BindingBuildContext,
    selected: SelectedKey,
    /// Retained across rebuilds: scroll offset and resized column widths.
    state: VirtualTableState,
    /// Whether the event being dispatched is a pointer event, so keyboard
    /// selection never counts as a double-click.
    pointer_event: Rc<Cell<bool>>,
}

impl TableBuilder {
    fn build(&self) -> VirtualTable {
        let sort = self.model.sort();
        let mut table = VirtualTable::new(self.name.clone())
            .columns(self.columns.iter().map(|column| column.into_sui(sort)))
            .state(self.state.clone())
            .row_count(self.model.len());
        if let Some(row_height) = self.options.row_height {
            table = table.row_height(row_height);
        }
        if let Some(theme) = self.context.theme.clone() {
            table = table.theme_when(move || theme.snapshot());
        }

        let model = self.model.clone();
        table = table.row_key(move |index| {
            // A row removed since the last layout keeps a distinct identity.
            model.key_at(index).unwrap_or(u64::MAX - index as u64)
        });

        let model = self.model.clone();
        let selected = self.selected.clone();
        table = table.selected_when(move || selected.get().and_then(|key| model.index_of(key)));

        let model = self.model.clone();
        table = table.row_name(move |index| {
            model
                .cells_at(index)
                .and_then(|cells| cells.into_iter().next())
                .unwrap_or_default()
        });

        let model = self.model.clone();
        let titles = self
            .columns
            .iter()
            .map(|column| column.title.clone())
            .collect::<Vec<_>>();
        table = table.row_description(move |index| {
            let cells = model.cells_at(index).unwrap_or_default();
            titles
                .iter()
                .zip(&cells)
                .skip(1)
                .map(|(title, cell)| format!("{title}: {cell}"))
                .collect::<Vec<_>>()
                .join(", ")
        });

        let model = self.model.clone();
        let theme = self.context.theme.clone();
        let alignments = self
            .columns
            .iter()
            .map(|column| column.alignment)
            .collect::<Vec<_>>();
        table = table.row_painter(move |ctx, row| {
            let Some(cells) = model.cells_at(row.row_index) else {
                return;
            };
            let theme = theme
                .as_ref()
                .map(|theme| theme.snapshot())
                .unwrap_or_default();
            let padding = theme.metrics.table_cell_padding;
            for ((rect, cell), alignment) in row.column_rects.iter().zip(&cells).zip(&alignments) {
                if rect.is_empty() {
                    continue;
                }
                sui::paint_text_cell(
                    ctx,
                    &theme,
                    *rect,
                    cell,
                    TextCellPaint::new()
                        .padding(padding, padding)
                        .alignment(*alignment),
                );
            }
        });

        let model = self.model.clone();
        let selected = self.selected.clone();
        let on_change = self.options.on_change.clone();
        let on_row_activate = self.options.on_row_activate.clone();
        let pointer_event = self.pointer_event.clone();
        let errors = self.context.clone();
        table = table.on_row_activate(move |index, kind| {
            let Some(key) = model.key_at(index) else {
                return;
            };
            // Selection is tracked by key here: the table's own index-based
            // change detection would miss changes after rows are reordered.
            if selected.get() != Some(key) {
                selected.set(key);
                if let Some(action) = &on_change {
                    report(&errors, action.run(key));
                }
            }
            if kind == VirtualTableRowActivationKind::Double
                && pointer_event.get()
                && let Some(action) = &on_row_activate
            {
                report(&errors, action.run(key));
            }
        });

        if let Some(action) = self.options.on_header_activate.clone() {
            let keys = self.column_keys();
            let errors = self.context.clone();
            table = table.on_header_activate(move |index| {
                if let Some(key) = keys.get(index) {
                    report(&errors, action.run(*key));
                }
            });
        }
        if let Some(action) = self.options.on_column_resize.clone() {
            let keys = self.column_keys();
            let errors = self.context.clone();
            table = table.on_column_resize(move |index, width| {
                if let Some(key) = keys.get(index) {
                    report(&errors, action.run(*key, f64::from(width)));
                }
            });
        }
        if let Some(action) = self.options.on_near_end.clone() {
            let errors = self.context.clone();
            table = table.on_near_end(move || report(&errors, action.run()));
        }
        table
    }

    fn column_keys(&self) -> Vec<u64> {
        self.columns.iter().map(|column| column.key).collect()
    }
}

/// Runtime adapter: realizes the Rust table and follows model revisions.
///
/// Row changes update the row count in place, so scroll position, hover,
/// focus, and double-click tracking survive streaming updates. Sort
/// indicator changes rebuild the table from the same retained state.
pub(crate) struct BindingVirtualTableWidget {
    table: VirtualTable,
    builder: TableBuilder,
    enabled: Option<BindingBool>,
    row_revision: u64,
    sort_revision: u64,
    /// The table was rebuilt outside an event; restore its focus ring at the
    /// next event.
    refocus: bool,
}

impl BindingVirtualTableWidget {
    pub(crate) fn new(
        name: String,
        columns: Vec<BindingVirtualTableColumn>,
        model: BindingTableModel,
        options: BindingVirtualTableOptions,
        enabled: Option<BindingBool>,
        context: BindingBuildContext,
    ) -> Self {
        let selected = match &options.selected {
            Some(state) => SelectedKey::State(state.clone()),
            None => SelectedKey::Local(Rc::new(Cell::new(None))),
        };
        let (row_revision, sort_revision, _) = model.revisions();
        let builder = TableBuilder {
            name,
            columns,
            model,
            options,
            context,
            selected,
            state: VirtualTableState::new(),
            pointer_event: Rc::new(Cell::new(false)),
        };
        Self {
            table: builder.build(),
            builder,
            enabled,
            row_revision,
            sort_revision,
            refocus: false,
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled.as_ref().is_none_or(BindingBool::resolve)
    }

    /// Follow model changes: `None` when nothing changed, otherwise whether
    /// the table was rebuilt rather than updated in place.
    fn sync(&mut self) -> Option<bool> {
        let (row_revision, sort_revision, len) = self.builder.model.revisions();
        if sort_revision != self.sort_revision {
            self.table = self.builder.build();
            self.row_revision = row_revision;
            self.sort_revision = sort_revision;
            return Some(true);
        }
        if row_revision != self.row_revision {
            let table = std::mem::replace(&mut self.table, VirtualTable::new(""));
            self.table = table.row_count(len);
            self.row_revision = row_revision;
            return Some(false);
        }
        None
    }

    /// Apply model changes during an event and invalidate the layout.
    fn sync_in_event(&mut self, ctx: &mut EventCtx) {
        let changed = self.sync();
        let refocus = std::mem::take(&mut self.refocus) || changed == Some(true);
        if refocus && ctx.is_focused() {
            // A rebuilt table starts unfocused; restore its focus ring.
            self.table.focus_changed(ctx, true);
        }
        if changed.is_some() {
            ctx.request_measure();
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn activate_selected(&self) -> bool {
        let Some(key) = self.builder.selected.get() else {
            return false;
        };
        if self.builder.model.index_of(key).is_none() {
            return false;
        }
        if let Some(action) = &self.builder.options.on_row_activate {
            report(&self.builder.context, action.run(key));
        }
        true
    }
}

impl Widget for BindingVirtualTableWidget {
    fn layer_options(&self) -> sui::LayerOptions {
        self.table.layer_options()
    }

    fn debug_name(&self) -> &'static str {
        "VirtualTable"
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.is_enabled() {
            return;
        }
        self.sync_in_event(ctx);
        self.builder
            .pointer_event
            .set(matches!(event, Event::Pointer(_)));
        self.table.event(ctx, event);
        if let Event::Keyboard(key) = event
            && key.state == KeyState::Pressed
            && key.key == "Enter"
            && ctx.is_focused()
            && !ctx.is_handled()
            && self.activate_selected()
        {
            ctx.set_handled();
        }
        // Callbacks may have reordered rows or changed sort indicators.
        self.sync_in_event(ctx);
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let _: u64 = ctx.observe(&self.builder.model.shared.revision);
        if self.sync() == Some(true) {
            self.refocus = true;
        }
        self.table.measure(ctx, constraints)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, bounds: Rect) {
        self.table.arrange(ctx, bounds);
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        self.table.paint(ctx);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        self.table.semantics(ctx);
    }

    fn accepts_focus(&self) -> bool {
        self.is_enabled() && self.table.accepts_focus()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, focused: bool) {
        self.table.focus_changed(ctx, focused);
    }
}
