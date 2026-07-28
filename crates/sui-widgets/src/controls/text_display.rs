use super::*;

pub struct Label {
    text: String,
    text_reader: Option<Box<dyn Fn() -> String>>,
    text_source: Option<Arc<dyn Observable<String>>>,
    semantic_name: Option<String>,
    style: TextStyle,
    style_reader: Option<Box<dyn Fn() -> TextStyle>>,
    font_size_override: Option<f32>,
    line_height_override: Option<f32>,
    color_override: Option<Color>,
    color_reader: Option<Box<dyn Fn() -> Color>>,
    measurement: Option<TextMeasurement>,
    layout: Option<PersistentTextLayout>,
    selection_scope: Option<SelectionScope>,
    clipboard_behavior: SelectionClipboardBehavior,
    selection: TextSelection,
    dragging_selection: Option<u64>,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            text_reader: None,
            text_source: None,
            semantic_name: None,
            style: DefaultTheme::default().body_text_style(),
            style_reader: None,
            font_size_override: None,
            line_height_override: None,
            color_override: None,
            color_reader: None,
            measurement: None,
            layout: None,
            selection_scope: None,
            clipboard_behavior: SelectionClipboardBehavior::AppManaged,
            selection: TextSelection::new(TextCursor::new(0), TextCursor::new(0)),
            dragging_selection: None,
        }
    }

    pub fn dynamic<F>(fallback: impl Into<String>, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        Self::new(fallback).text_when(reader)
    }

    pub fn text_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.text_reader = Some(Box::new(reader));
        self.text_source = None;
        self
    }

    /// Bind label text to an observable value.
    ///
    /// Changes automatically invalidate text measurement, paint, and
    /// semantics for this retained label instance.
    pub fn text_from<O>(mut self, source: O) -> Self
    where
        O: Observable<String> + 'static,
    {
        self.text = source.get();
        self.text_reader = None;
        self.text_source = Some(Arc::new(source));
        self
    }

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn selectable(mut self, selection_scope: SelectionScope) -> Self {
        self.selection_scope = Some(selection_scope);
        self
    }

    pub fn clipboard_behavior(mut self, behavior: SelectionClipboardBehavior) -> Self {
        self.clipboard_behavior = behavior;
        self
    }

    pub fn copy_to_clipboard(self, enabled: bool) -> Self {
        self.clipboard_behavior(if enabled {
            SelectionClipboardBehavior::WidgetManaged
        } else {
            SelectionClipboardBehavior::AppManaged
        })
    }

    pub fn selection_scope(&self) -> Option<&SelectionScope> {
        self.selection_scope.as_ref()
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.style = theme.body_text_style();
        self.clear_style_overrides();
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.text_reader = None;
        self.text_source = None;
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color_override = Some(color);
        self.color_reader = None;
        self
    }

    pub fn color_when<F>(mut self, color: F) -> Self
    where
        F: Fn() -> Color + 'static,
    {
        self.color_override = None;
        self.color_reader = Some(Box::new(color));
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size_override = Some(font_size.max(1.0));
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.line_height_override = Some(line_height.max(1.0));
        self
    }

    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self.clear_style_overrides();
        self
    }

    /// Resolve the complete text style each time the label is measured or painted.
    ///
    /// As with [`Self::style`], this replaces earlier whole-style and per-property
    /// builders. Later `font_size`, `line_height`, `color`, or `color_when` calls
    /// layer their respective property over the dynamically resolved style.
    pub fn style_when<F>(mut self, style: F) -> Self
    where
        F: Fn() -> TextStyle + 'static,
    {
        self.clear_style_overrides();
        self.style_reader = Some(Box::new(style));
        self
    }

    fn current_text(&self) -> String {
        self.text_source
            .as_ref()
            .map(|source| source.get())
            .or_else(|| self.text_reader.as_ref().map(|reader| reader()))
            .unwrap_or_else(|| self.text.clone())
    }

    fn observed_text(&self, ctx: &MeasureCtx) -> String {
        self.text_source
            .as_ref()
            .map(|source| ctx.observe_with(source.as_ref(), InvalidationKind::Text))
            .unwrap_or_else(|| self.current_text())
    }

    pub(super) fn resolved_style(&self) -> TextStyle {
        let mut style = self
            .style_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.style.clone());
        if let Some(font_size) = self.font_size_override {
            style.font_size = font_size;
        }
        if let Some(line_height) = self.line_height_override {
            style.line_height = line_height;
        }
        if let Some(color) = self.color_override {
            style.color = color;
        }
        if let Some(color_reader) = &self.color_reader {
            style.color = color_reader();
        }
        style
    }

    fn clear_style_overrides(&mut self) {
        self.style_reader = None;
        self.font_size_override = None;
        self.line_height_override = None;
        self.color_override = None;
        self.color_reader = None;
    }

    fn has_explicit_line_break(text: &str) -> bool {
        text.bytes().any(|byte| matches!(byte, b'\n' | b'\r'))
    }

    fn owner_id(widget_id: WidgetId) -> SelectionOwnerId {
        SelectionOwnerId::from(widget_id)
    }

    fn selected_range(&self, text_len: usize) -> std::ops::Range<usize> {
        selection_range(&self.selection, text_len)
    }

    fn active_selection_range(
        &self,
        widget_id: WidgetId,
        text_len: usize,
    ) -> Option<std::ops::Range<usize>> {
        let owner = Self::owner_id(widget_id);
        let range = self.selected_range(text_len);
        self.selection_scope
            .as_ref()
            .is_some_and(|scope| scope.has_owner_selection(owner))
            .then_some(range)
            .filter(|range| !range.is_empty())
    }

    fn sync_selection_scope(&self, ctx: &mut EventCtx, text: &str) {
        let Some(scope) = &self.selection_scope else {
            return;
        };
        let owner = Self::owner_id(ctx.widget_id());
        let range = self.selected_range(text.len());
        let selected = text.get(range.clone()).unwrap_or("").to_string();
        let change = scope.replace_text(owner, owner, range, text.len(), selected);
        request_selection_change(ctx, change);
    }

    fn label_layout_origin_for_event(&self, bounds: Rect, layout: &PersistentTextLayout) -> Point {
        let measurement = layout.measurement();
        let height = self
            .resolved_style()
            .line_height
            .max(measurement.height)
            .min(bounds.height());
        Point::new(
            bounds.x() - measurement.bounds.x(),
            bounds.y() + ((bounds.height() - height).max(0.0) * 0.5),
        )
    }

    fn hit_test_offset(&self, bounds: Rect, position: Point, text_len: usize) -> usize {
        self.layout
            .as_ref()
            .map(|layout| {
                let origin = self.label_layout_origin_for_event(bounds, layout);
                layout
                    .hit_test_point(Point::new(position.x - origin.x, position.y - origin.y))
                    .utf8_offset
                    .min(text_len)
            })
            .unwrap_or(text_len)
    }

    fn set_selection(&mut self, anchor: usize, focus: usize, text_len: usize) {
        self.selection = TextSelection::new(
            TextCursor::new(anchor.min(text_len)),
            TextCursor::new(focus.min(text_len)),
        );
    }

    fn handles_implicit_clipboard(&self) -> bool {
        self.clipboard_behavior.is_widget_managed()
    }

    fn copy_selection(&self, ctx: &mut EventCtx) -> bool {
        let text = self.current_text();
        let range = self.selected_range(text.len());
        let Some(selected) = text.get(range).filter(|selected| !selected.is_empty()) else {
            return false;
        };
        ctx.set_clipboard_text(selected);
        true
    }
}

impl Widget for Label {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if self.selection_scope.is_none() {
            return;
        }

        match event {
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary)
                    && ctx.phase() != sui_runtime::EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                let text = self.current_text();
                let offset = self.hit_test_offset(ctx.bounds(), pointer.position, text.len());
                let anchor = if pointer.modifiers.shift {
                    self.selection.anchor.utf8_offset
                } else {
                    offset
                };
                self.set_selection(anchor, offset, text.len());
                self.dragging_selection = Some(pointer.pointer_id);
                self.sync_selection_scope(ctx, &text);
                ctx.request_focus();
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Move
                    && self.dragging_selection == Some(pointer.pointer_id)
                    && pointer.buttons.contains(PointerButton::Primary) =>
            {
                let text = self.current_text();
                let anchor = self.selection.anchor.utf8_offset;
                let focus = self.hit_test_offset(ctx.bounds(), pointer.position, text.len());
                self.set_selection(anchor, focus, text.len());
                self.sync_selection_scope(ctx, &text);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && self.dragging_selection == Some(pointer.pointer_id) =>
            {
                self.dragging_selection = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Cancel
                    && self.dragging_selection == Some(pointer.pointer_id) =>
            {
                self.dragging_selection = None;
                ctx.release_pointer_capture(pointer.pointer_id);
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Secondary)
                    && ctx.phase() != EventPhase::Capture
                    && ctx.bounds().contains(pointer.position) =>
            {
                // Preserve the active selection while handing the same press
                // to a wrapping ContextMenu. Its activation can route a
                // TextCommand back to this label after focus moves to the menu.
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && (key.modifiers.control || key.modifiers.meta)
                    && matches!(key.key.as_str(), "a" | "A") =>
            {
                let text = self.current_text();
                self.set_selection(0, text.len(), text.len());
                self.sync_selection_scope(ctx, &text);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && (key.modifiers.control || key.modifiers.meta)
                    && matches!(key.key.as_str(), "c" | "C")
                    && self.handles_implicit_clipboard() =>
            {
                if self.copy_selection(ctx) {
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed && ctx.is_focused() && key.key == "Escape" =>
            {
                if let Some(scope) = &self.selection_scope {
                    let change = scope.clear_owner(Self::owner_id(ctx.widget_id()));
                    request_selection_change(ctx, change);
                }
                self.set_selection(0, 0, 0);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Semantics(semantics) if semantics.target == ctx.widget_id() => {
                match &semantics.action {
                    SemanticsActionRequest::SetSelection(selection) => {
                        let text = self.current_text();
                        self.set_selection(selection.start, selection.end, text.len());
                        self.sync_selection_scope(ctx, &text);
                        ctx.request_paint();
                        ctx.request_semantics();
                        ctx.set_handled();
                    }
                    SemanticsActionRequest::Copy
                        if self.handles_implicit_clipboard() && self.copy_selection(ctx) =>
                    {
                        ctx.set_handled();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn command(&mut self, ctx: &mut EventCtx, command: &Command<'_>) {
        let Some(command) = TextCommand::from_command(command) else {
            return;
        };
        match command {
            TextCommand::SelectAll => {
                let text = self.current_text();
                self.set_selection(0, text.len(), text.len());
                self.sync_selection_scope(ctx, &text);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            TextCommand::Copy => {
                if self.copy_selection(ctx) {
                    ctx.set_handled();
                }
            }
            TextCommand::Cut | TextCommand::Paste => {}
        }
    }

    fn intrinsic_size(
        &mut self,
        ctx: &mut MeasureCtx,
        axis: Axis,
        available_cross: f32,
    ) -> IntrinsicSize {
        if axis == Axis::Horizontal {
            let text = self.observed_text(ctx);
            let style = self.resolved_style();
            let natural = measure_text(ctx, &text, &style).width;
            let minimum = text
                .split_whitespace()
                .map(|segment| measure_text(ctx, segment, &style).width)
                .fold(0.0_f32, f32::max);
            return IntrinsicSize::new(minimum, natural);
        }

        let width = if available_cross.is_finite() {
            available_cross.max(0.0)
        } else {
            f32::INFINITY
        };
        let size = self.measure(
            ctx,
            Constraints::new(Size::ZERO, Size::new(width, f32::INFINITY)),
        );
        IntrinsicSize::fixed(size.height)
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let text = self.observed_text(ctx);
        let style = self.resolved_style();
        let natural_measurement = measure_text(ctx, &text, &style);
        let max_width = constraints.max.width;
        let wraps_to_constraint = max_width.is_finite() && natural_measurement.width > max_width;
        let needs_layout = self.selection_scope.is_some()
            || wraps_to_constraint
            || Self::has_explicit_line_break(&text);
        let mut measured_width = if wraps_to_constraint {
            max_width.max(0.0)
        } else {
            natural_measurement.width
        };
        let mut measurement = natural_measurement;

        if needs_layout {
            let layout_width = if max_width.is_finite() {
                measured_width.min(max_width).max(1.0)
            } else {
                measured_width.max(1.0)
            };
            measurement = ctx
                .layout()
                .shape_text(
                    text.clone(),
                    Size::new(layout_width, f32::INFINITY),
                    style.clone(),
                )
                .map(|layout| layout.measurement())
                .unwrap_or(measurement);
            if !wraps_to_constraint {
                measured_width = if max_width.is_finite() {
                    measurement.width.min(max_width).max(0.0)
                } else {
                    measurement.width.max(0.0)
                };
            }
            self.layout = ctx
                .layout()
                .shape_text_persistent(
                    self.layout.as_ref().map(|layout| layout.handle()),
                    text,
                    Size::new(
                        measured_width.max(1.0),
                        measurement.height.max(style.line_height).max(1.0),
                    ),
                    style.clone(),
                )
                .ok();
        } else {
            self.layout = None;
        }
        self.measurement = Some(measurement);
        constraints.clamp(Size::new(
            measured_width,
            measurement.height.max(style.line_height),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let text = self.current_text();
        let style = self.resolved_style();
        if let Some(layout) = &self.layout {
            let layout_bounds = layout.measurement().bounds;
            let mut layout_rect = aligned_text_rect_for_layout_with_mode(
                ctx,
                ctx.bounds(),
                layout.layout(),
                style.line_height,
                0.0,
                HorizontalTextAlignmentMode::Optical,
            );
            if layout.lines().len() > 1 {
                let block_height = style
                    .line_height
                    .max(layout.measurement().height)
                    .min(ctx.bounds().height());
                layout_rect = Rect::new(
                    layout_rect.x(),
                    ctx.bounds().y() + ((ctx.bounds().height() - block_height).max(0.0) * 0.5),
                    layout_rect.width(),
                    block_height,
                );
            }
            let origin = Point::new(layout_rect.x() - layout_bounds.x(), layout_rect.y());
            if let Some(range) = self.active_selection_range(ctx.widget_id(), text.len()) {
                let theme = DefaultTheme::default();
                for rect in layout.selection_rects(range) {
                    ctx.fill_rect(rect.translate(origin.to_vector()), theme.palette.selection);
                }
            }
            ctx.draw_persistent_text_layout(origin, layout);
        } else {
            paint_aligned_text(ctx, ctx.bounds(), &text, &style, style.line_height, 0.0);
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let text = self.current_text();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.semantic_name.clone().unwrap_or_else(|| text.clone()));
        if self.semantic_name.is_some() {
            node.value = Some(SemanticsValue::Text(text));
        }
        if self.selection_scope.is_some() {
            node.actions = vec![SemanticsAction::Focus, SemanticsAction::SetSelection];
            if self.handles_implicit_clipboard() {
                node.actions.push(SemanticsAction::Copy);
            }
            node.state.focused = ctx.is_focused();
        }
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        self.selection_scope.is_some()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}

pub struct Link {
    theme: Box<DefaultTheme>,
    theme_reader: Option<Box<dyn Fn() -> DefaultTheme>>,
    label: String,
    label_reader: Option<Box<dyn Fn() -> String>>,
    url: String,
    url_reader: Option<Box<dyn Fn() -> String>>,
    semantic_name: Option<String>,
    text_style: Option<TextStyle>,
    enabled: bool,
    enabled_reader: Option<Box<dyn Fn() -> bool>>,
    hovered: bool,
    pressed: bool,
    measurement: Option<TextMeasurement>,
    on_open: Option<Box<dyn FnMut(&str)>>,
    on_open_with_ctx: Option<Box<dyn FnMut(&mut EventCtx, &str)>>,
}

impl Link {
    pub fn new(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            theme: Box::new(DefaultTheme::default()),
            theme_reader: None,
            label: label.into(),
            label_reader: None,
            url: url.into(),
            url_reader: None,
            semantic_name: None,
            text_style: None,
            enabled: true,
            enabled_reader: None,
            hovered: false,
            pressed: false,
            measurement: None,
            on_open: None,
            on_open_with_ctx: None,
        }
    }

    pub fn url(url: impl Into<String>) -> Self {
        let url = url.into();
        Self::new(url.clone(), url)
    }

    pub fn label_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.label_reader = Some(Box::new(reader));
        self
    }

    pub fn url_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> String + 'static,
    {
        self.url_reader = Some(Box::new(reader));
        self
    }

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn theme(mut self, theme: DefaultTheme) -> Self {
        self.theme = Box::new(theme);
        self.theme_reader = None;
        self
    }

    pub fn theme_when<F>(mut self, theme: F) -> Self
    where
        F: Fn() -> DefaultTheme + 'static,
    {
        self.theme_reader = Some(Box::new(theme));
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.enabled_reader = None;
        self
    }

    pub fn enabled_when<F>(mut self, enabled: F) -> Self
    where
        F: Fn() -> bool + 'static,
    {
        self.enabled_reader = Some(Box::new(enabled));
        self
    }

    pub fn on_open<F>(mut self, on_open: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_open = Some(Box::new(on_open));
        self
    }

    pub fn on_open_with_ctx<F>(mut self, on_open: F) -> Self
    where
        F: FnMut(&mut EventCtx, &str) + 'static,
    {
        self.on_open_with_ctx = Some(Box::new(on_open));
        self
    }

    fn current_label(&self) -> String {
        single_line_text(
            self.label_reader
                .as_ref()
                .map(|reader| reader())
                .unwrap_or_else(|| self.label.clone()),
        )
    }

    fn current_url(&self) -> String {
        single_line_text(
            self.url_reader
                .as_ref()
                .map(|reader| reader())
                .unwrap_or_else(|| self.url.clone()),
        )
        .trim()
        .to_string()
    }

    fn is_enabled(&self) -> bool {
        self.enabled_reader
            .as_ref()
            .map(|enabled| enabled())
            .unwrap_or(self.enabled)
    }

    fn resolved_theme(&self) -> DefaultTheme {
        self.theme_reader
            .as_ref()
            .map(|theme| theme())
            .unwrap_or(*self.theme)
    }

    fn resolved_text_style(&self, color: Color) -> TextStyle {
        let mut style = self
            .text_style
            .clone()
            .unwrap_or_else(|| self.resolved_theme().body_text_style());
        style.color = color;
        style
    }

    fn resolved_color(&self, theme: &DefaultTheme) -> Color {
        if !self.is_enabled() {
            return theme
                .palette
                .placeholder
                .with_alpha(theme.interaction.disabled_content_opacity);
        }
        if self.pressed {
            theme.palette.accent_pressed
        } else if self.hovered {
            theme.palette.accent_hover
        } else {
            theme.palette.accent
        }
    }

    fn is_visible_parts(label: &str, url: &str) -> bool {
        !label.trim().is_empty() && !url.trim().is_empty()
    }

    fn is_visible(&self) -> bool {
        Self::is_visible_parts(&self.current_label(), &self.current_url())
    }

    fn can_activate_parts(&self, label: &str, url: &str) -> bool {
        self.is_enabled() && Self::is_visible_parts(label, url)
    }

    fn activate(&mut self, ctx: &mut EventCtx) {
        let label = self.current_label();
        let url = self.current_url();
        if !self.can_activate_parts(&label, &url) {
            return;
        }
        if let Some(on_open) = &mut self.on_open {
            on_open(&url);
        }
        if let Some(on_open) = &mut self.on_open_with_ctx {
            on_open(ctx, &url);
        }
    }

    fn set_hovered(&mut self, hovered: bool, ctx: &mut EventCtx) {
        if self.hovered != hovered {
            self.hovered = hovered;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }

    fn reset_interaction(&mut self, ctx: &mut EventCtx) {
        if self.hovered || self.pressed {
            self.hovered = false;
            self.pressed = false;
            ctx.request_paint();
            ctx.request_semantics();
        }
    }
}

impl Widget for Link {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if !self.is_visible() || !self.is_enabled() {
            self.reset_interaction(ctx);
            return;
        }

        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Enter => {
                self.set_hovered(ctx.bounds().contains(pointer.position), ctx);
            }
            Event::Pointer(_pointer) if _pointer.kind == PointerEventKind::Leave => {
                self.set_hovered(false, ctx);
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Down
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                self.pressed = true;
                self.hovered = true;
                ctx.request_pointer_capture(pointer.pointer_id);
                ctx.request_focus();
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer)
                if pointer.kind == PointerEventKind::Up
                    && pointer.button == Some(PointerButton::Primary) =>
            {
                let hovered = ctx.bounds().contains(pointer.position);
                let activate = self.pressed && hovered;
                self.pressed = false;
                self.hovered = hovered;
                ctx.release_pointer_capture(pointer.pointer_id);
                if activate {
                    self.activate(ctx);
                }
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Cancel => {
                if self.pressed {
                    self.pressed = false;
                    self.hovered = false;
                    ctx.release_pointer_capture(pointer.pointer_id);
                    ctx.request_paint();
                    ctx.request_semantics();
                    ctx.set_handled();
                }
            }
            Event::Keyboard(key)
                if key.state == KeyState::Pressed
                    && ctx.is_focused()
                    && matches!(key.key.as_str(), "Enter" | " ") =>
            {
                self.activate(ctx);
                ctx.request_paint();
                ctx.request_semantics();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let label = self.current_label();
        let url = self.current_url();
        if !Self::is_visible_parts(&label, &url) {
            self.measurement = None;
            return constraints.clamp(Size::ZERO);
        }

        let theme = self.resolved_theme();
        let style = self.resolved_text_style(self.resolved_color(&theme));
        let measurement = measure_text(ctx, &label, &style);
        self.measurement = Some(measurement);
        constraints.clamp(Size::new(
            measurement.width,
            measurement.height.max(style.line_height),
        ))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let label = self.current_label();
        let url = self.current_url();
        if !Self::is_visible_parts(&label, &url) {
            return;
        }

        let theme = self.resolved_theme();
        let color = self.resolved_color(&theme);
        let style = self.resolved_text_style(color);
        let bounds = ctx.bounds();
        ctx.push_clip_rect(bounds);
        paint_single_line_aligned_text(ctx, bounds, &label, &style, style.line_height, 0.0);
        ctx.pop_clip();

        let measured_width = self
            .measurement
            .map(|measurement| measurement.width)
            .unwrap_or(bounds.width())
            .min(bounds.width())
            .max(0.0);
        if measured_width > 0.0 {
            let underline_y = bounds.y() + bounds.height() - physical_pixels(ctx, 2.0);
            let mut underline = PathBuilder::new();
            underline
                .move_to(Point::new(bounds.x(), underline_y))
                .line_to(Point::new(bounds.x() + measured_width, underline_y));
            ctx.stroke(
                underline.build(),
                color,
                StrokeStyle::new(physical_pixels(ctx, 1.0)),
            );
        }

        if ctx.is_focused() && self.is_enabled() {
            ctx.stroke_rect(
                bounds.inflate(physical_pixels(ctx, 2.0), physical_pixels(ctx, 1.0)),
                theme.palette.focus_ring,
                StrokeStyle::new(physical_pixels(ctx, 1.0)),
            );
        }
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let label = self.current_label();
        let url = self.current_url();
        if !Self::is_visible_parts(&label, &url) {
            return;
        }

        let enabled = self.is_enabled();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Link, ctx.bounds());
        node.name = Some(self.semantic_name.clone().unwrap_or(label));
        node.value = Some(SemanticsValue::Text(url));
        node.state.focused = ctx.is_focused();
        node.state.hovered = self.hovered && enabled;
        node.state.disabled = !enabled;
        node.actions = if enabled {
            vec![SemanticsAction::Focus, SemanticsAction::Activate]
        } else {
            Vec::new()
        };
        ctx.push(node);
    }

    fn accepts_focus(&self) -> bool {
        self.is_enabled() && self.is_visible()
    }

    fn focus_changed(&mut self, ctx: &mut EventCtx, _focused: bool) {
        ctx.request_paint();
        ctx.request_semantics();
    }
}
