#![forbid(unsafe_code)]

use std::sync::Arc;

use sui_core::{DpiInfo, ImageHandle, Point, Rect, Size};
use sui_scene::ImageRegistry;
use sui_text::{
    FontRegistry, PersistentTextLayout, TextDocument, TextLayout, TextLayoutHandle,
    TextLayoutRequest, TextMeasurement, TextStyle, TextSystem,
};

const FLEX_EPSILON: f32 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexJustify {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexAlignContent {
    Start,
    Center,
    End,
    Stretch,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexBasis {
    Auto,
    Points(f32),
    Fraction(f32),
    GapAwareFraction(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexStyle {
    pub axis: Axis,
    pub wrap: FlexWrap,
    pub main_gap: f32,
    pub cross_gap: f32,
    pub justify: FlexJustify,
    pub align_items: Alignment,
    pub align_content: FlexAlignContent,
}

impl FlexStyle {
    pub const fn new(axis: Axis) -> Self {
        Self {
            axis,
            wrap: FlexWrap::NoWrap,
            main_gap: 0.0,
            cross_gap: 0.0,
            justify: FlexJustify::Start,
            align_items: Alignment::Start,
            align_content: FlexAlignContent::Start,
        }
    }

    pub const fn horizontal() -> Self {
        Self::new(Axis::Horizontal)
    }

    pub const fn vertical() -> Self {
        Self::new(Axis::Vertical)
    }

    pub fn wrap(mut self, wrap: FlexWrap) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        let gap = finite_non_negative(gap);
        self.main_gap = gap;
        self.cross_gap = gap;
        self
    }

    pub fn main_gap(mut self, gap: f32) -> Self {
        self.main_gap = finite_non_negative(gap);
        self
    }

    pub fn cross_gap(mut self, gap: f32) -> Self {
        self.cross_gap = finite_non_negative(gap);
        self
    }

    pub fn justify(mut self, justify: FlexJustify) -> Self {
        self.justify = justify;
        self
    }

    pub fn align_items(mut self, alignment: Alignment) -> Self {
        self.align_items = alignment;
        self
    }

    pub fn align_content(mut self, alignment: FlexAlignContent) -> Self {
        self.align_content = alignment;
        self
    }
}

impl Default for FlexStyle {
    fn default() -> Self {
        Self::horizontal()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexItem {
    pub grow: f32,
    pub shrink: f32,
    pub basis: FlexBasis,
    pub min_size: Size,
    pub max_size: Size,
    pub align_self: Option<Alignment>,
}

impl FlexItem {
    pub const fn new() -> Self {
        Self {
            grow: 0.0,
            shrink: 1.0,
            basis: FlexBasis::Auto,
            min_size: Size::ZERO,
            max_size: Size::new(f32::INFINITY, f32::INFINITY),
            align_self: None,
        }
    }

    pub const fn fill() -> Self {
        Self {
            grow: 1.0,
            shrink: 1.0,
            basis: FlexBasis::Points(0.0),
            min_size: Size::ZERO,
            max_size: Size::new(f32::INFINITY, f32::INFINITY),
            align_self: None,
        }
    }

    pub fn flex(grow: f32) -> Self {
        Self::fill().grow(grow)
    }

    pub fn fixed(basis: f32) -> Self {
        Self::new().basis(basis).no_shrink()
    }

    pub fn grow(mut self, grow: f32) -> Self {
        self.grow = finite_non_negative(grow);
        self
    }

    pub fn shrink(mut self, shrink: f32) -> Self {
        self.shrink = finite_non_negative(shrink);
        self
    }

    pub fn no_shrink(mut self) -> Self {
        self.shrink = 0.0;
        self
    }

    pub fn basis(mut self, basis: f32) -> Self {
        self.basis = FlexBasis::Points(finite_non_negative(basis));
        self
    }

    pub fn basis_fraction(mut self, fraction: f32) -> Self {
        self.basis = FlexBasis::Fraction(finite_non_negative(fraction));
        self
    }

    pub fn basis_gap_aware_fraction(mut self, fraction: f32) -> Self {
        self.basis = FlexBasis::GapAwareFraction(finite_non_negative(fraction));
        self
    }

    pub fn auto_basis(mut self) -> Self {
        self.basis = FlexBasis::Auto;
        self
    }

    pub fn min_size(mut self, min_size: Size) -> Self {
        self.min_size = Size::new(
            finite_non_negative(min_size.width),
            finite_non_negative(min_size.height),
        );
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_size.width = finite_non_negative(min_width);
        self
    }

    pub fn min_height(mut self, min_height: f32) -> Self {
        self.min_size.height = finite_non_negative(min_height);
        self
    }

    pub fn max_size(mut self, max_size: Size) -> Self {
        self.max_size = Size::new(
            non_negative_or_infinity(max_size.width),
            non_negative_or_infinity(max_size.height),
        );
        self
    }

    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_size.width = non_negative_or_infinity(max_width);
        self
    }

    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_size.height = non_negative_or_infinity(max_height);
        self
    }

    pub fn align_self(mut self, alignment: Alignment) -> Self {
        self.align_self = Some(alignment);
        self
    }

    pub fn inherit_alignment(mut self) -> Self {
        self.align_self = None;
        self
    }
}

impl Default for FlexItem {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlexLayout {
    pub size: Size,
    pub items: Vec<FlexItemLayout>,
    pub lines: Vec<FlexLineLayout>,
}

impl FlexLayout {
    pub fn empty(size: Size) -> Self {
        Self {
            size,
            items: Vec::new(),
            lines: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexItemLayout {
    pub rect: Rect,
    pub measured_size: Size,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexLineLayout {
    pub item_start: usize,
    pub item_end: usize,
    pub rect: Rect,
}

pub fn flex_layout<F>(
    style: FlexStyle,
    items: &[FlexItem],
    constraints: Constraints,
    mut measure_child: F,
) -> FlexLayout
where
    F: FnMut(usize, Constraints) -> Size,
{
    let initial_measurements = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let child_constraints = flex_child_constraints(style, *item, constraints);
            let measured_size = clamp_item_size(*item, measure_child(index, child_constraints));
            (child_constraints, measured_size)
        })
        .collect::<Vec<_>>();
    let measured_sizes = initial_measurements
        .iter()
        .map(|(_, measured_size)| *measured_size)
        .collect::<Vec<_>>();
    let initial_layout = flex_layout_from_measured(style, items, constraints, &measured_sizes);

    // Flex basis and grow/shrink resolution can assign a child a very different
    // main-axis extent from the one used for its intrinsic measurement. Text is
    // the important example: `FlexItem::flex(1.0)` has a zero-point basis, so an
    // initial width of zero produces a one-glyph-per-line layout even though the
    // item is then grown across the row. Measure again at the resolved allocation
    // so width-dependent children can rebuild wrapping and report the resulting
    // cross-axis size before the final line layout is computed.
    let resolved_sizes = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let resolved_main = initial_layout
                .items
                .get(index)
                .map(|layout| axis_main(style.axis, layout.rect.size))
                .unwrap_or(0.0);
            let child_constraints =
                resolved_flex_child_constraints(style, *item, constraints, resolved_main);
            let (initial_constraints, initial_size) = initial_measurements[index];
            if child_constraints == initial_constraints {
                initial_size
            } else {
                clamp_item_size(*item, measure_child(index, child_constraints))
            }
        })
        .collect::<Vec<_>>();

    flex_layout_from_measured(style, items, constraints, &resolved_sizes)
}

pub fn arrange_flex(
    style: FlexStyle,
    items: &[FlexItem],
    container_size: Size,
    measured_sizes: &[Size],
) -> FlexLayout {
    flex_layout_from_measured(
        style,
        items,
        Constraints::tight(Size::new(
            finite_non_negative(container_size.width),
            finite_non_negative(container_size.height),
        )),
        measured_sizes,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Padding {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Padding {
    pub const ZERO: Self = Self::all(0.0);

    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    pub fn inset(self, size: Size) -> Size {
        Size::new(
            (size.width - (self.left + self.right)).max(0.0),
            (size.height - (self.top + self.bottom)).max(0.0),
        )
    }

    pub const fn offset(self) -> Point {
        Point::new(self.left, self.top)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    pub min: Size,
    pub max: Size,
}

impl Constraints {
    pub const UNBOUNDED: Self = Self {
        min: Size::ZERO,
        max: Size::new(f32::INFINITY, f32::INFINITY),
    };

    pub const fn new(min: Size, max: Size) -> Self {
        Self { min, max }
    }

    pub const fn tight(size: Size) -> Self {
        Self {
            min: size,
            max: size,
        }
    }

    pub fn loosen(self) -> Self {
        Self {
            min: Size::ZERO,
            max: self.max,
        }
    }

    pub fn clamp(self, size: Size) -> Size {
        Size::new(
            size.width.clamp(self.min.width, self.max.width),
            size.height.clamp(self.min.height, self.max.height),
        )
    }
}

#[derive(Debug, Clone)]
pub struct LayoutContext {
    dpi_info: DpiInfo,
    text_system: Arc<TextSystem>,
    font_registry: Arc<FontRegistry>,
    image_registry: Arc<ImageRegistry>,
}

impl LayoutContext {
    pub fn new(
        dpi_info: DpiInfo,
        text_system: Arc<TextSystem>,
        font_registry: Arc<FontRegistry>,
        image_registry: Arc<ImageRegistry>,
    ) -> Self {
        Self {
            dpi_info,
            text_system,
            font_registry,
            image_registry,
        }
    }

    pub const fn dpi(&self) -> DpiInfo {
        self.dpi_info
    }

    pub fn measure_text(
        &self,
        text: impl Into<String>,
        style: TextStyle,
    ) -> sui_core::Result<TextMeasurement> {
        self.text_system
            .measure_text(text, style, self.font_registry.as_ref())
    }

    pub fn measure_document(&self, document: TextDocument) -> sui_core::Result<TextMeasurement> {
        self.text_system
            .measure_document(document, self.font_registry.as_ref())
    }

    pub fn shape_text(
        &self,
        text: impl Into<String>,
        box_size: Size,
        style: TextStyle,
    ) -> sui_core::Result<TextLayout> {
        self.text_system
            .shape_text(text, box_size, style, self.font_registry.as_ref())
    }

    pub fn shape_text_persistent(
        &self,
        handle: Option<TextLayoutHandle>,
        text: impl Into<String>,
        box_size: Size,
        style: TextStyle,
    ) -> sui_core::Result<PersistentTextLayout> {
        self.text_system.shape_text_persistent(
            handle,
            text,
            box_size,
            style,
            self.font_registry.as_ref(),
        )
    }

    pub fn layout_document(&self, request: TextLayoutRequest) -> sui_core::Result<TextLayout> {
        self.text_system
            .layout_document(request, self.font_registry.as_ref())
    }

    pub fn layout_document_persistent(
        &self,
        handle: Option<TextLayoutHandle>,
        request: TextLayoutRequest,
    ) -> sui_core::Result<PersistentTextLayout> {
        self.text_system
            .layout_document_persistent(handle, request, self.font_registry.as_ref())
    }

    pub fn image_size(&self, image: ImageHandle) -> Option<Size> {
        self.image_registry
            .dimensions(image)
            .map(|(width, height)| Size::new(width as f32, height as f32))
    }
}

impl Default for Constraints {
    fn default() -> Self {
        Self::UNBOUNDED
    }
}

#[derive(Debug, Clone, Copy)]
struct ResolvedFlexItem {
    measured_size: Size,
    base_size: Size,
}

#[derive(Debug, Clone, Copy)]
struct FlexLine {
    start: usize,
    end: usize,
    main: f32,
    cross: f32,
}

fn flex_layout_from_measured(
    style: FlexStyle,
    items: &[FlexItem],
    constraints: Constraints,
    measured_sizes: &[Size],
) -> FlexLayout {
    if items.is_empty() {
        return FlexLayout::empty(constraints.clamp(Size::ZERO));
    }

    let (_, initial_lines) = resolve_flex_lines(style, items, constraints, measured_sizes);
    let natural_size = natural_flex_size(style, &initial_lines);
    let size = constraints.clamp(natural_size);
    let arrange_constraints = Constraints::tight(size);
    let (resolved, lines) = resolve_flex_lines(style, items, arrange_constraints, measured_sizes);

    arrange_resolved_flex(style, items, size, &resolved, &lines)
}

fn resolve_flex_lines(
    style: FlexStyle,
    items: &[FlexItem],
    constraints: Constraints,
    measured_sizes: &[Size],
) -> (Vec<ResolvedFlexItem>, Vec<FlexLine>) {
    let main_gap = finite_non_negative(style.main_gap);
    let available_main = axis_main(style.axis, constraints.max);
    let wrap_limit = if style.wrap == FlexWrap::Wrap && available_main.is_finite() {
        available_main.max(0.0)
    } else {
        f32::INFINITY
    };

    let resolved = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let measured = measured_sizes.get(index).copied().unwrap_or(Size::ZERO);
            resolve_flex_item(style.axis, style.main_gap, *item, constraints, measured)
        })
        .collect::<Vec<_>>();

    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut main = 0.0f32;
    let mut cross = 0.0f32;

    for (index, item) in resolved.iter().enumerate() {
        let item_main = axis_main(style.axis, item.base_size);
        let item_cross = axis_cross(style.axis, item.base_size);
        let spacing_before = if index == start { 0.0 } else { main_gap };

        if style.wrap == FlexWrap::Wrap
            && index > start
            && main + spacing_before + item_main > wrap_limit + FLEX_EPSILON
        {
            lines.push(FlexLine {
                start,
                end: index,
                main,
                cross,
            });
            start = index;
            main = item_main;
            cross = item_cross;
        } else {
            main += spacing_before + item_main;
            cross = cross.max(item_cross);
        }
    }

    lines.push(FlexLine {
        start,
        end: resolved.len(),
        main,
        cross,
    });

    (resolved, lines)
}

fn resolve_flex_item(
    axis: Axis,
    main_gap: f32,
    item: FlexItem,
    constraints: Constraints,
    measured_size: Size,
) -> ResolvedFlexItem {
    let measured_size = clamp_item_size(item, measured_size);
    let min_main = item_min_main(axis, item);
    let max_main = item_max_main(axis, item);
    let min_cross = item_min_cross(axis, item);
    let max_cross = item_max_cross(axis, item);

    let base_main = basis_main_size(axis, main_gap, item.basis, constraints, measured_size)
        .clamp(min_main, max_main);
    let base_cross = axis_cross(axis, measured_size).clamp(min_cross, max_cross);

    ResolvedFlexItem {
        measured_size,
        base_size: axis_size(axis, base_main, base_cross),
    }
}

fn natural_flex_size(style: FlexStyle, lines: &[FlexLine]) -> Size {
    let main = lines.iter().map(|line| line.main).fold(0.0f32, f32::max);
    let cross_gap = finite_non_negative(style.cross_gap);
    let cross = lines
        .iter()
        .enumerate()
        .map(|(index, line)| line.cross + if index == 0 { 0.0 } else { cross_gap })
        .sum();

    axis_size(style.axis, main, cross)
}

fn arrange_resolved_flex(
    style: FlexStyle,
    items: &[FlexItem],
    size: Size,
    resolved: &[ResolvedFlexItem],
    lines: &[FlexLine],
) -> FlexLayout {
    if lines.is_empty() {
        return FlexLayout::empty(size);
    }

    let main_gap = finite_non_negative(style.main_gap);
    let cross_gap = finite_non_negative(style.cross_gap);
    let container_main = axis_main(style.axis, size);
    let container_cross = axis_cross(style.axis, size);
    let natural_cross = lines
        .iter()
        .enumerate()
        .map(|(index, line)| line.cross + if index == 0 { 0.0 } else { cross_gap })
        .sum::<f32>();
    let extra_cross = (container_cross - natural_cross).max(0.0);
    let line_count = lines.len();
    let (cross_start, arranged_cross_gap, stretch_cross) =
        cross_distribution(style.align_content, line_count, cross_gap, extra_cross);

    let mut item_layouts = Vec::with_capacity(resolved.len());
    let mut line_layouts = Vec::with_capacity(lines.len());
    let mut cross_cursor = cross_start;

    for (line_index, line) in lines.iter().enumerate() {
        let count = line.end - line.start;
        let line_cross = line.cross + stretch_cross;
        let line_items = &resolved[line.start..line.end];
        let target_mains = flex_line_main_sizes(
            style.axis,
            &items[line.start..line.end],
            line_items,
            container_main,
            main_gap,
        );
        let used_main = target_mains.iter().sum::<f32>();
        let gap_total = main_gap * count.saturating_sub(1) as f32;
        let leftover_main = (container_main - used_main - gap_total).max(0.0);
        let (main_start, arranged_main_gap) =
            main_distribution(style.justify, count, main_gap, leftover_main);
        let mut main_cursor = main_start;

        for (offset, target_main) in target_mains.into_iter().enumerate() {
            let index = line.start + offset;
            let item = items[index];
            let resolved_item = resolved[index];
            let alignment = item.align_self.unwrap_or(style.align_items);
            let item_cross = arranged_cross_size(
                style.axis,
                item,
                alignment,
                resolved_item.base_size,
                line_cross,
            );
            let cross_offset = aligned_offset(alignment, line_cross - item_cross);
            let rect = axis_rect(
                style.axis,
                main_cursor,
                cross_cursor + cross_offset,
                target_main,
                item_cross,
            );

            item_layouts.push(FlexItemLayout {
                rect,
                measured_size: resolved_item.measured_size,
                line: line_index,
            });
            main_cursor += target_main + arranged_main_gap;
        }

        line_layouts.push(FlexLineLayout {
            item_start: line.start,
            item_end: line.end,
            rect: axis_rect(style.axis, 0.0, cross_cursor, container_main, line_cross),
        });
        cross_cursor += line_cross + arranged_cross_gap;
    }

    FlexLayout {
        size,
        items: item_layouts,
        lines: line_layouts,
    }
}

fn flex_line_main_sizes(
    axis: Axis,
    items: &[FlexItem],
    line_items: &[ResolvedFlexItem],
    container_main: f32,
    main_gap: f32,
) -> Vec<f32> {
    let count = line_items.len();
    let gap_total = finite_non_negative(main_gap) * count.saturating_sub(1) as f32;
    let available_for_items = if container_main.is_finite() {
        (container_main - gap_total).max(0.0)
    } else {
        line_items
            .iter()
            .map(|item| axis_main(axis, item.base_size))
            .sum()
    };
    let base_sum = line_items
        .iter()
        .map(|item| axis_main(axis, item.base_size))
        .sum::<f32>();
    let free = available_for_items - base_sum;

    if free > 0.0 {
        if items
            .iter()
            .any(|item| finite_non_negative(item.grow) > 0.0)
        {
            return distribute_grow_space(axis, items, line_items, available_for_items);
        }
    } else if free < 0.0 {
        let deficit = -free;
        let total_shrink = items
            .iter()
            .zip(line_items.iter())
            .map(|(item, resolved)| {
                finite_non_negative(item.shrink) * axis_main(axis, resolved.base_size)
            })
            .sum::<f32>();
        if total_shrink > 0.0 {
            return items
                .iter()
                .zip(line_items.iter())
                .map(|(item, resolved)| {
                    let base = axis_main(axis, resolved.base_size);
                    let weight = finite_non_negative(item.shrink) * base;
                    let min = item_min_main(axis, *item);
                    (base - (deficit * weight / total_shrink)).max(min)
                })
                .collect();
        }
    }

    line_items
        .iter()
        .map(|item| axis_main(axis, item.base_size))
        .collect()
}

fn distribute_grow_space(
    axis: Axis,
    items: &[FlexItem],
    line_items: &[ResolvedFlexItem],
    available_for_items: f32,
) -> Vec<f32> {
    let mut sizes = line_items
        .iter()
        .map(|item| axis_main(axis, item.base_size))
        .collect::<Vec<_>>();
    let mut frozen = vec![false; sizes.len()];

    loop {
        let remaining = available_for_items - sizes.iter().sum::<f32>();
        if remaining <= FLEX_EPSILON {
            break;
        }

        let total_grow = items
            .iter()
            .zip(frozen.iter())
            .filter(|(_, frozen)| !**frozen)
            .map(|(item, _)| finite_non_negative(item.grow))
            .sum::<f32>();
        if total_grow <= FLEX_EPSILON {
            break;
        }

        let mut clamped = false;
        for index in 0..sizes.len() {
            if frozen[index] {
                continue;
            }

            let grow = finite_non_negative(items[index].grow);
            if grow <= 0.0 {
                frozen[index] = true;
                continue;
            }

            let proposed = sizes[index] + (remaining * grow / total_grow);
            let max = item_max_main(axis, items[index]);
            if proposed >= max - FLEX_EPSILON {
                sizes[index] = max;
                frozen[index] = true;
                clamped = true;
            } else {
                sizes[index] = proposed;
            }
        }

        if !clamped {
            break;
        }
    }

    sizes
}

fn flex_child_constraints(
    style: FlexStyle,
    item: FlexItem,
    constraints: Constraints,
) -> Constraints {
    let min_main = item_min_main(style.axis, item);
    let max_main = match item.basis {
        FlexBasis::Auto => axis_main(style.axis, constraints.max),
        FlexBasis::Points(points) => finite_non_negative(points),
        FlexBasis::Fraction(fraction) => flex_fraction_constraint_main_size(
            style.axis,
            constraints,
            finite_non_negative(fraction),
        ),
        FlexBasis::GapAwareFraction(fraction) => flex_gap_aware_fraction_constraint_main_size(
            style.axis,
            constraints,
            style.main_gap,
            finite_non_negative(fraction),
        ),
    }
    .min(item_max_main(style.axis, item))
    .max(min_main);
    let max_cross = axis_cross(style.axis, constraints.max)
        .min(item_max_cross(style.axis, item))
        .max(0.0);
    let alignment = item.align_self.unwrap_or(style.align_items);
    let min_cross = if alignment == Alignment::Stretch && max_cross.is_finite() {
        max_cross
    } else {
        item_min_cross(style.axis, item).min(max_cross)
    };

    axis_constraints(style.axis, min_main, max_main, min_cross, max_cross)
}

fn resolved_flex_child_constraints(
    style: FlexStyle,
    item: FlexItem,
    constraints: Constraints,
    resolved_main: f32,
) -> Constraints {
    let intrinsic = flex_child_constraints(style, item, constraints);
    let min_cross = axis_cross(style.axis, intrinsic.min);
    let max_cross = axis_cross(style.axis, intrinsic.max);
    let resolved_main = finite_non_negative(resolved_main).clamp(
        item_min_main(style.axis, item),
        item_max_main(style.axis, item),
    );

    axis_constraints(
        style.axis,
        resolved_main,
        resolved_main,
        min_cross,
        max_cross,
    )
}

fn arranged_cross_size(
    axis: Axis,
    item: FlexItem,
    alignment: Alignment,
    base_size: Size,
    line_cross: f32,
) -> f32 {
    let min = item_min_cross(axis, item);
    let max = item_max_cross(axis, item);
    if alignment == Alignment::Stretch {
        line_cross.clamp(min, max)
    } else {
        axis_cross(axis, base_size).min(line_cross).clamp(min, max)
    }
}

fn basis_main_size(
    axis: Axis,
    main_gap: f32,
    basis: FlexBasis,
    constraints: Constraints,
    measured_size: Size,
) -> f32 {
    match basis {
        FlexBasis::Auto => axis_main(axis, measured_size),
        FlexBasis::Points(points) => finite_non_negative(points),
        FlexBasis::Fraction(fraction) => {
            let available = axis_main(axis, constraints.max);
            if available.is_finite() {
                available.max(0.0) * finite_non_negative(fraction)
            } else {
                axis_main(axis, measured_size)
            }
        }
        FlexBasis::GapAwareFraction(fraction) => {
            let available = axis_main(axis, constraints.max);
            if available.is_finite() {
                gap_aware_fraction_basis(
                    available.max(0.0),
                    main_gap,
                    finite_non_negative(fraction),
                )
            } else {
                axis_main(axis, measured_size)
            }
        }
    }
}

fn flex_fraction_constraint_main_size(axis: Axis, constraints: Constraints, fraction: f32) -> f32 {
    let available = axis_main(axis, constraints.max);
    if available.is_finite() {
        available.max(0.0) * fraction
    } else {
        available
    }
}

fn flex_gap_aware_fraction_constraint_main_size(
    axis: Axis,
    constraints: Constraints,
    main_gap: f32,
    fraction: f32,
) -> f32 {
    let available = axis_main(axis, constraints.max);
    if available.is_finite() {
        gap_aware_fraction_basis(available.max(0.0), main_gap, fraction)
    } else {
        available
    }
}

fn gap_aware_fraction_basis(available: f32, gap: f32, fraction: f32) -> f32 {
    let gap_share = (1.0 - fraction).max(0.0);
    (available * fraction - finite_non_negative(gap) * gap_share).max(0.0)
}

fn cross_distribution(
    alignment: FlexAlignContent,
    count: usize,
    gap: f32,
    extra: f32,
) -> (f32, f32, f32) {
    if count == 0 || extra <= 0.0 {
        return (0.0, gap, 0.0);
    }

    match alignment {
        FlexAlignContent::Start => (0.0, gap, 0.0),
        FlexAlignContent::Center => (extra * 0.5, gap, 0.0),
        FlexAlignContent::End => (extra, gap, 0.0),
        FlexAlignContent::Stretch => (0.0, gap, extra / count as f32),
        FlexAlignContent::SpaceBetween if count > 1 => {
            (0.0, gap + (extra / (count - 1) as f32), 0.0)
        }
        FlexAlignContent::SpaceAround => {
            let distributed = extra / count as f32;
            (distributed * 0.5, gap + distributed, 0.0)
        }
        FlexAlignContent::SpaceEvenly => {
            let distributed = extra / (count + 1) as f32;
            (distributed, gap + distributed, 0.0)
        }
        FlexAlignContent::SpaceBetween => (0.0, gap, 0.0),
    }
}

fn main_distribution(justify: FlexJustify, count: usize, gap: f32, leftover: f32) -> (f32, f32) {
    if count == 0 || leftover <= 0.0 {
        return (0.0, gap);
    }

    match justify {
        FlexJustify::Start => (0.0, gap),
        FlexJustify::Center => (leftover * 0.5, gap),
        FlexJustify::End => (leftover, gap),
        FlexJustify::SpaceBetween if count > 1 => (0.0, gap + (leftover / (count - 1) as f32)),
        FlexJustify::SpaceAround => {
            let distributed = leftover / count as f32;
            (distributed * 0.5, gap + distributed)
        }
        FlexJustify::SpaceEvenly => {
            let distributed = leftover / (count + 1) as f32;
            (distributed, gap + distributed)
        }
        FlexJustify::SpaceBetween => (0.0, gap),
    }
}

fn clamp_item_size(item: FlexItem, size: Size) -> Size {
    Size::new(
        size.width.clamp(
            finite_non_negative(item.min_size.width),
            item_max_width(item),
        ),
        size.height.clamp(
            finite_non_negative(item.min_size.height),
            item_max_height(item),
        ),
    )
}

fn axis_constraints(
    axis: Axis,
    min_main: f32,
    max_main: f32,
    min_cross: f32,
    max_cross: f32,
) -> Constraints {
    match axis {
        Axis::Horizontal => Constraints::new(
            Size::new(min_main, min_cross),
            Size::new(max_main, max_cross),
        ),
        Axis::Vertical => Constraints::new(
            Size::new(min_cross, min_main),
            Size::new(max_cross, max_main),
        ),
    }
}

fn axis_main(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

fn axis_cross(axis: Axis, size: Size) -> f32 {
    match axis {
        Axis::Horizontal => size.height,
        Axis::Vertical => size.width,
    }
}

fn axis_size(axis: Axis, main: f32, cross: f32) -> Size {
    match axis {
        Axis::Horizontal => Size::new(main, cross),
        Axis::Vertical => Size::new(cross, main),
    }
}

fn axis_rect(axis: Axis, main: f32, cross: f32, main_size: f32, cross_size: f32) -> Rect {
    match axis {
        Axis::Horizontal => Rect::new(main, cross, main_size, cross_size),
        Axis::Vertical => Rect::new(cross, main, cross_size, main_size),
    }
}

fn aligned_offset(alignment: Alignment, remaining: f32) -> f32 {
    let remaining = remaining.max(0.0);
    match alignment {
        Alignment::Start | Alignment::Stretch => 0.0,
        Alignment::Center => remaining * 0.5,
        Alignment::End => remaining,
    }
}

fn item_min_main(axis: Axis, item: FlexItem) -> f32 {
    axis_main(axis, item.min_size).max(0.0)
}

fn item_max_main(axis: Axis, item: FlexItem) -> f32 {
    axis_main(axis, Size::new(item_max_width(item), item_max_height(item)))
        .max(item_min_main(axis, item))
}

fn item_min_cross(axis: Axis, item: FlexItem) -> f32 {
    axis_cross(axis, item.min_size).max(0.0)
}

fn item_max_cross(axis: Axis, item: FlexItem) -> f32 {
    axis_cross(axis, Size::new(item_max_width(item), item_max_height(item)))
        .max(item_min_cross(axis, item))
}

fn item_max_width(item: FlexItem) -> f32 {
    non_negative_or_infinity(item.max_size.width).max(finite_non_negative(item.min_size.width))
}

fn item_max_height(item: FlexItem) -> f32 {
    non_negative_or_infinity(item.max_size.height).max(finite_non_negative(item.min_size.height))
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn non_negative_or_infinity(value: f32) -> f32 {
    if value.is_nan() || value < 0.0 {
        0.0
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        Alignment, Constraints, FlexAlignContent, FlexItem, FlexJustify, FlexStyle, FlexWrap,
        LayoutContext, arrange_flex, flex_layout,
    };
    use sui_core::{Color, DpiInfo, ImageHandle, Rect, Size};
    use sui_scene::{ImageRegistry, RegisteredImage};
    use sui_text::{FontRegistry, TextStyle, TextSystem};

    fn assert_rect_approx_eq(actual: Rect, expected: Rect) {
        assert!((actual.x() - expected.x()).abs() < 0.001);
        assert!((actual.y() - expected.y()).abs() < 0.001);
        assert!((actual.width() - expected.width()).abs() < 0.001);
        assert!((actual.height() - expected.height()).abs() < 0.001);
    }

    #[test]
    fn layout_context_measures_text_and_images_without_runtime_widget_state() {
        let mut images = ImageRegistry::new();
        images.insert(
            ImageHandle::new(7),
            RegisteredImage::from_rgba8(4, 2, vec![255; 4 * 2 * 4]).unwrap(),
        );

        let layout = LayoutContext::new(
            DpiInfo::new(
                2.0,
                Some(192.0),
                Size::new(320.0, 180.0),
                Size::new(640.0, 360.0),
            ),
            Arc::new(TextSystem::new()),
            Arc::new(FontRegistry::new()),
            Arc::new(images),
        );

        let measurement = layout
            .measure_text("hello", TextStyle::new(Color::WHITE))
            .unwrap();

        assert!(measurement.width > 0.0);
        assert_eq!(layout.dpi().effective_dpi(), 192.0);
        assert_eq!(
            layout.image_size(ImageHandle::new(7)),
            Some(Size::new(4.0, 2.0))
        );
    }

    #[test]
    fn flex_layout_distributes_grow_space() {
        let items = [
            FlexItem::new(),
            FlexItem::new().grow(1.0),
            FlexItem::new().grow(2.0),
        ];
        let measured = [
            Size::new(20.0, 10.0),
            Size::new(10.0, 10.0),
            Size::new(10.0, 10.0),
        ];
        let layout = flex_layout(
            FlexStyle::horizontal(),
            &items,
            Constraints::tight(Size::new(100.0, 20.0)),
            |index, _| measured[index],
        );

        assert_eq!(layout.size, Size::new(100.0, 20.0));
        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 20.0, 10.0));
        assert_rect_approx_eq(layout.items[1].rect, Rect::new(20.0, 0.0, 30.0, 10.0));
        assert_rect_approx_eq(layout.items[2].rect, Rect::new(50.0, 0.0, 50.0, 10.0));
    }

    #[test]
    fn flex_item_fill_creates_equal_columns() {
        let items = [FlexItem::fill(), FlexItem::fill()];
        let layout = flex_layout(
            FlexStyle::horizontal(),
            &items,
            Constraints::tight(Size::new(100.0, 12.0)),
            |_, _| Size::new(20.0, 10.0),
        );

        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 50.0, 10.0));
        assert_rect_approx_eq(layout.items[1].rect, Rect::new(50.0, 0.0, 50.0, 10.0));
    }

    #[test]
    fn flex_layout_remeasures_grown_children_at_their_resolved_main_size() {
        let items = [FlexItem::flex(1.0)];
        let mut seen = Vec::new();
        let layout = flex_layout(
            FlexStyle::horizontal(),
            &items,
            Constraints::new(Size::new(240.0, 0.0), Size::new(240.0, 200.0)),
            |_, constraints| {
                seen.push(constraints);
                if constraints.max.width <= 1.0 {
                    Size::new(constraints.max.width, 160.0)
                } else {
                    Size::new(constraints.max.width, 20.0)
                }
            },
        );

        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].max.width, 0.0);
        assert_eq!(seen[1].min.width, 240.0);
        assert_eq!(seen[1].max.width, 240.0);
        assert_eq!(layout.size, Size::new(240.0, 20.0));
        assert_eq!(layout.items[0].measured_size, Size::new(240.0, 20.0));
        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 240.0, 20.0));
    }

    #[test]
    fn flex_layout_supports_fraction_basis() {
        let items = [
            FlexItem::new().basis_fraction(0.25),
            FlexItem::new().basis_fraction(0.75),
        ];
        let layout = flex_layout(
            FlexStyle::horizontal(),
            &items,
            Constraints::tight(Size::new(200.0, 12.0)),
            |_, _| Size::new(10.0, 10.0),
        );

        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 50.0, 10.0));
        assert_rect_approx_eq(layout.items[1].rect, Rect::new(50.0, 0.0, 150.0, 10.0));
    }

    #[test]
    fn flex_layout_supports_gap_aware_fraction_basis() {
        let items = [
            FlexItem::new().basis_gap_aware_fraction(1.0 / 3.0),
            FlexItem::new().basis_gap_aware_fraction(1.0 / 3.0),
            FlexItem::new().basis_gap_aware_fraction(1.0 / 3.0),
        ];
        let layout = flex_layout(
            FlexStyle::horizontal().gap(10.0),
            &items,
            Constraints::tight(Size::new(320.0, 12.0)),
            |_, _| Size::new(10.0, 10.0),
        );

        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 100.0, 10.0));
        assert_rect_approx_eq(layout.items[1].rect, Rect::new(110.0, 0.0, 100.0, 10.0));
        assert_rect_approx_eq(layout.items[2].rect, Rect::new(220.0, 0.0, 100.0, 10.0));
    }

    #[test]
    fn flex_layout_wraps_gap_aware_fraction_lines_after_full_rows() {
        let items = [
            FlexItem::new().basis_gap_aware_fraction(1.0 / 3.0),
            FlexItem::new().basis_gap_aware_fraction(1.0 / 3.0),
            FlexItem::new().basis_gap_aware_fraction(1.0 / 3.0),
            FlexItem::new().basis_gap_aware_fraction(0.5),
            FlexItem::new().basis_gap_aware_fraction(0.5),
        ];
        let layout = flex_layout(
            FlexStyle::horizontal().wrap(FlexWrap::Wrap).gap(10.0),
            &items,
            Constraints::tight(Size::new(320.0, 100.0)),
            |_, _| Size::new(10.0, 10.0),
        );

        assert_eq!(layout.lines.len(), 2);
        assert_eq!(layout.items[0].line, 0);
        assert_eq!(layout.items[2].line, 0);
        assert_eq!(layout.items[3].line, 1);
        assert_eq!(layout.items[4].line, 1);
        assert_rect_approx_eq(layout.items[3].rect, Rect::new(0.0, 20.0, 155.0, 10.0));
        assert_rect_approx_eq(layout.items[4].rect, Rect::new(165.0, 20.0, 155.0, 10.0));
    }

    #[test]
    fn flex_layout_redistributes_grow_space_after_max_clamp() {
        let items = [FlexItem::fill().max_width(20.0), FlexItem::fill()];
        let layout = flex_layout(
            FlexStyle::horizontal(),
            &items,
            Constraints::tight(Size::new(100.0, 12.0)),
            |_, _| Size::new(0.0, 10.0),
        );

        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 20.0, 10.0));
        assert_rect_approx_eq(layout.items[1].rect, Rect::new(20.0, 0.0, 80.0, 10.0));
    }

    #[test]
    fn flex_layout_shrinks_items_when_space_is_tight() {
        let items = [
            FlexItem::new().min_width(20.0),
            FlexItem::new().min_width(20.0),
        ];
        let layout = flex_layout(
            FlexStyle::horizontal(),
            &items,
            Constraints::tight(Size::new(100.0, 12.0)),
            |index, _| {
                if index == 0 {
                    Size::new(80.0, 10.0)
                } else {
                    Size::new(40.0, 10.0)
                }
            },
        );

        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 66.666664, 10.0));
        assert_rect_approx_eq(
            layout.items[1].rect,
            Rect::new(66.666664, 0.0, 33.333332, 10.0),
        );
    }

    #[test]
    fn flex_layout_wraps_lines_with_cross_gap() {
        let items = [FlexItem::new(), FlexItem::new(), FlexItem::new()];
        let measured = [
            Size::new(30.0, 10.0),
            Size::new(30.0, 10.0),
            Size::new(30.0, 10.0),
        ];
        let layout = flex_layout(
            FlexStyle::horizontal().wrap(FlexWrap::Wrap).gap(5.0),
            &items,
            Constraints::tight(Size::new(50.0, 40.0)),
            |index, _| measured[index],
        );

        assert_eq!(layout.lines.len(), 3);
        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 30.0, 10.0));
        assert_rect_approx_eq(layout.items[1].rect, Rect::new(0.0, 15.0, 30.0, 10.0));
        assert_rect_approx_eq(layout.items[2].rect, Rect::new(0.0, 30.0, 30.0, 10.0));
    }

    #[test]
    fn flex_layout_justifies_remaining_space_between_items() {
        let items = [FlexItem::new(), FlexItem::new(), FlexItem::new()];
        let layout = flex_layout(
            FlexStyle::horizontal().justify(FlexJustify::SpaceBetween),
            &items,
            Constraints::tight(Size::new(100.0, 10.0)),
            |_, _| Size::new(10.0, 10.0),
        );

        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_rect_approx_eq(layout.items[1].rect, Rect::new(45.0, 0.0, 10.0, 10.0));
        assert_rect_approx_eq(layout.items[2].rect, Rect::new(90.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn flex_layout_stretches_items_and_lines_on_cross_axis() {
        let items = [FlexItem::new(), FlexItem::new()];
        let layout = flex_layout(
            FlexStyle::horizontal()
                .align_items(Alignment::Stretch)
                .align_content(FlexAlignContent::Stretch),
            &items,
            Constraints::tight(Size::new(60.0, 20.0)),
            |_, _| Size::new(20.0, 8.0),
        );

        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 20.0, 20.0));
        assert_rect_approx_eq(layout.items[1].rect, Rect::new(20.0, 0.0, 20.0, 20.0));
    }

    #[test]
    fn arrange_flex_uses_existing_measured_sizes() {
        let items = [FlexItem::new(), FlexItem::new().grow(1.0)];
        let layout = arrange_flex(
            FlexStyle::horizontal(),
            &items,
            Size::new(80.0, 12.0),
            &[Size::new(20.0, 8.0), Size::new(10.0, 8.0)],
        );

        assert_rect_approx_eq(layout.items[0].rect, Rect::new(0.0, 0.0, 20.0, 8.0));
        assert_rect_approx_eq(layout.items[1].rect, Rect::new(20.0, 0.0, 60.0, 8.0));
    }
}
