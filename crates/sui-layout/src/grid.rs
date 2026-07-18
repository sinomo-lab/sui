use sui_core::{Point, Rect, Size};

use crate::{Alignment, Constraints};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrackMax {
    Auto,
    Points(f32),
    Fraction(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrack {
    Fixed(f32),
    Auto,
    Fraction(f32),
    MinMax { min: f32, max: GridTrackMax },
}

impl GridTrack {
    pub const fn fixed(points: f32) -> Self {
        Self::Fixed(points)
    }

    pub const fn auto() -> Self {
        Self::Auto
    }

    pub const fn fraction(weight: f32) -> Self {
        Self::Fraction(weight)
    }

    pub const fn minmax(min: f32, max: GridTrackMax) -> Self {
        Self::MinMax { min, max }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPlacement {
    pub row: usize,
    pub column: usize,
    pub row_span: usize,
    pub column_span: usize,
}

impl GridPlacement {
    pub const fn new(row: usize, column: usize) -> Self {
        Self {
            row,
            column,
            row_span: 1,
            column_span: 1,
        }
    }

    pub const fn span(mut self, rows: usize, columns: usize) -> Self {
        self.row_span = if rows == 0 { 1 } else { rows };
        self.column_span = if columns == 0 { 1 } else { columns };
        self
    }
}

impl Default for GridPlacement {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridItem {
    pub placement: GridPlacement,
    pub minimum_size: Size,
    pub natural_size: Size,
    pub horizontal_alignment: Alignment,
    pub vertical_alignment: Alignment,
}

impl GridItem {
    pub fn new(placement: GridPlacement, natural_size: Size) -> Self {
        let natural_size = sanitize_size(natural_size);
        Self {
            placement,
            minimum_size: natural_size,
            natural_size,
            horizontal_alignment: Alignment::Stretch,
            vertical_alignment: Alignment::Stretch,
        }
    }

    pub fn minimum_size(mut self, minimum_size: Size) -> Self {
        self.minimum_size = sanitize_size(minimum_size);
        self
    }

    pub const fn align(mut self, horizontal: Alignment, vertical: Alignment) -> Self {
        self.horizontal_alignment = horizontal;
        self.vertical_alignment = vertical;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GridStyle {
    pub columns: Vec<GridTrack>,
    pub rows: Vec<GridTrack>,
    pub column_gap: f32,
    pub row_gap: f32,
}

impl GridStyle {
    pub fn new(columns: impl IntoIterator<Item = GridTrack>) -> Self {
        Self {
            columns: columns.into_iter().collect(),
            rows: Vec::new(),
            column_gap: 0.0,
            row_gap: 0.0,
        }
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = GridTrack>) -> Self {
        self.rows = rows.into_iter().collect();
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        let gap = finite_non_negative(gap);
        self.column_gap = gap;
        self.row_gap = gap;
        self
    }

    pub fn column_gap(mut self, gap: f32) -> Self {
        self.column_gap = finite_non_negative(gap);
        self
    }

    pub fn row_gap(mut self, gap: f32) -> Self {
        self.row_gap = finite_non_negative(gap);
        self
    }
}

impl Default for GridStyle {
    fn default() -> Self {
        Self::new([GridTrack::Auto])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridItemLayout {
    pub cell: Rect,
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GridLayout {
    pub size: Size,
    pub column_offsets: Vec<f32>,
    pub column_widths: Vec<f32>,
    pub row_offsets: Vec<f32>,
    pub row_heights: Vec<f32>,
    pub items: Vec<GridItemLayout>,
}

pub fn grid_layout(style: &GridStyle, items: &[GridItem], constraints: Constraints) -> GridLayout {
    let column_count = items
        .iter()
        .map(|item| item.placement.column + item.placement.column_span.max(1))
        .max()
        .unwrap_or(0)
        .max(style.columns.len())
        .max(1);
    let row_count = items
        .iter()
        .map(|item| item.placement.row + item.placement.row_span.max(1))
        .max()
        .unwrap_or(0)
        .max(style.rows.len())
        .max(1);

    let columns = padded_tracks(&style.columns, column_count);
    let rows = padded_tracks(&style.rows, row_count);
    let (column_widths, width) = resolve_tracks(
        &columns,
        style.column_gap,
        constraints.min.width,
        constraints.max.width,
        items.iter().map(|item| AxisItem {
            start: item.placement.column,
            span: item.placement.column_span.max(1),
            minimum: item.minimum_size.width,
            natural: item.natural_size.width,
        }),
    );
    let (row_heights, height) = resolve_tracks(
        &rows,
        style.row_gap,
        constraints.min.height,
        constraints.max.height,
        items.iter().map(|item| AxisItem {
            start: item.placement.row,
            span: item.placement.row_span.max(1),
            minimum: item.minimum_size.height,
            natural: item.natural_size.height,
        }),
    );
    let column_offsets = track_offsets(&column_widths, style.column_gap);
    let row_offsets = track_offsets(&row_heights, style.row_gap);
    let item_layouts = items
        .iter()
        .map(|item| {
            let cell = span_rect(
                item.placement,
                &column_offsets,
                &column_widths,
                &row_offsets,
                &row_heights,
                style.column_gap,
                style.row_gap,
            );
            let child_size = Size::new(
                aligned_extent(
                    item.horizontal_alignment,
                    cell.width(),
                    item.natural_size.width,
                ),
                aligned_extent(
                    item.vertical_alignment,
                    cell.height(),
                    item.natural_size.height,
                ),
            );
            let origin = Point::new(
                aligned_origin(
                    item.horizontal_alignment,
                    cell.x(),
                    cell.width(),
                    child_size.width,
                ),
                aligned_origin(
                    item.vertical_alignment,
                    cell.y(),
                    cell.height(),
                    child_size.height,
                ),
            );
            GridItemLayout {
                cell,
                rect: Rect::from_origin_size(origin, child_size),
            }
        })
        .collect();

    GridLayout {
        size: constraints.clamp(Size::new(width, height)),
        column_offsets,
        column_widths,
        row_offsets,
        row_heights,
        items: item_layouts,
    }
}

#[derive(Clone, Copy)]
struct AxisItem {
    start: usize,
    span: usize,
    minimum: f32,
    natural: f32,
}

fn padded_tracks(tracks: &[GridTrack], count: usize) -> Vec<GridTrack> {
    let mut result = tracks.to_vec();
    result.resize(count, GridTrack::Auto);
    result
}

fn resolve_tracks(
    tracks: &[GridTrack],
    gap: f32,
    minimum_container: f32,
    maximum_container: f32,
    items: impl Iterator<Item = AxisItem> + Clone,
) -> (Vec<f32>, f32) {
    let gap = finite_non_negative(gap);
    let gap_total = gap * tracks.len().saturating_sub(1) as f32;
    let mut minimums = tracks.iter().map(track_minimum).collect::<Vec<_>>();
    let mut sizes = minimums.clone();

    for item in items.clone().filter(|item| item.span == 1) {
        if let Some(track) = tracks.get(item.start) {
            let minimum = finite_non_negative(item.minimum);
            let natural = finite_non_negative(item.natural).max(minimum);
            if !matches!(track, GridTrack::Fixed(_)) {
                minimums[item.start] = minimums[item.start].max(minimum);
                sizes[item.start] = sizes[item.start].max(track_natural_target(*track, natural));
            }
        }
    }
    for item in items.filter(|item| item.span > 1) {
        distribute_span_shortfall(
            tracks,
            &mut minimums,
            item.start,
            item.span,
            finite_non_negative(item.minimum),
            gap,
        );
        distribute_span_shortfall(
            tracks,
            &mut sizes,
            item.start,
            item.span,
            finite_non_negative(item.natural).max(finite_non_negative(item.minimum)),
            gap,
        );
    }
    for (size, minimum) in sizes.iter_mut().zip(&minimums) {
        *size = size.max(*minimum);
    }
    for ((track, minimum), size) in tracks.iter().zip(&mut minimums).zip(&mut sizes) {
        let cap = track_maximum(*track);
        *minimum = minimum.min(cap);
        *size = size.min(cap).max(*minimum);
    }

    let target = if maximum_container.is_finite() {
        maximum_container.max(0.0)
    } else {
        finite_non_negative(minimum_container).max(sum_tracks(&sizes, gap_total))
    };
    let available_tracks = (target - gap_total).max(0.0);
    let current_tracks = sizes.iter().sum::<f32>();
    if current_tracks < available_tracks {
        distribute_fractional_space(tracks, &mut sizes, available_tracks - current_tracks);
    } else if current_tracks > available_tracks {
        shrink_to_fit(&mut sizes, &minimums, current_tracks - available_tracks);
    }

    let natural = sum_tracks(&sizes, gap_total);
    let resolved =
        natural
            .max(finite_non_negative(minimum_container))
            .min(if maximum_container.is_finite() {
                maximum_container.max(0.0)
            } else {
                f32::INFINITY
            });
    (sizes, resolved)
}

fn track_minimum(track: &GridTrack) -> f32 {
    match *track {
        GridTrack::Fixed(points) => finite_non_negative(points),
        GridTrack::MinMax { min, .. } => finite_non_negative(min),
        GridTrack::Auto | GridTrack::Fraction(_) => 0.0,
    }
}

fn track_natural_target(track: GridTrack, natural: f32) -> f32 {
    match track {
        GridTrack::Fixed(points) => finite_non_negative(points),
        GridTrack::Auto | GridTrack::Fraction(_) => natural,
        GridTrack::MinMax { min, max } => match max {
            GridTrackMax::Auto | GridTrackMax::Fraction(_) => natural.max(min),
            GridTrackMax::Points(max) => natural.clamp(min.max(0.0), max.max(min).max(0.0)),
        },
    }
}

fn track_maximum(track: GridTrack) -> f32 {
    match track {
        GridTrack::Fixed(points) => finite_non_negative(points),
        GridTrack::MinMax {
            min,
            max: GridTrackMax::Points(max),
        } => finite_non_negative(max).max(finite_non_negative(min)),
        GridTrack::Auto
        | GridTrack::Fraction(_)
        | GridTrack::MinMax {
            max: GridTrackMax::Auto | GridTrackMax::Fraction(_),
            ..
        } => f32::INFINITY,
    }
}

fn fractional_weight(track: GridTrack) -> f32 {
    match track {
        GridTrack::Fraction(weight) => finite_non_negative(weight),
        GridTrack::MinMax {
            max: GridTrackMax::Fraction(weight),
            ..
        } => finite_non_negative(weight),
        _ => 0.0,
    }
}

fn distribute_fractional_space(tracks: &[GridTrack], sizes: &mut [f32], extra: f32) {
    let weight = tracks.iter().copied().map(fractional_weight).sum::<f32>();
    if weight <= 0.0 || extra <= 0.0 {
        return;
    }
    for (track, size) in tracks.iter().copied().zip(sizes) {
        let track_weight = fractional_weight(track);
        if track_weight > 0.0 {
            *size += extra * track_weight / weight;
        }
    }
}

fn shrink_to_fit(sizes: &mut [f32], minimums: &[f32], mut overflow: f32) {
    while overflow > 0.001 {
        let shrinkable = sizes
            .iter()
            .zip(minimums)
            .filter(|(size, minimum)| **size > **minimum + 0.001)
            .count();
        if shrinkable == 0 {
            break;
        }
        let share = overflow / shrinkable as f32;
        let mut consumed = 0.0;
        for (size, minimum) in sizes.iter_mut().zip(minimums) {
            let amount = (*size - *minimum).max(0.0).min(share);
            *size -= amount;
            consumed += amount;
        }
        if consumed <= 0.001 {
            break;
        }
        overflow -= consumed;
    }
}

fn distribute_span_shortfall(
    tracks: &[GridTrack],
    sizes: &mut [f32],
    start: usize,
    span: usize,
    target: f32,
    gap: f32,
) {
    let end = (start + span).min(sizes.len());
    if start >= end {
        return;
    }
    let current = sizes[start..end].iter().sum::<f32>() + gap * (end - start - 1) as f32;
    let shortfall = (target - current).max(0.0);
    if shortfall <= 0.0 {
        return;
    }
    let eligible = (start..end)
        .filter(|index| !matches!(tracks[*index], GridTrack::Fixed(_)))
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return;
    }
    let share = shortfall / eligible.len() as f32;
    for index in eligible {
        sizes[index] += share;
    }
}

fn track_offsets(sizes: &[f32], gap: f32) -> Vec<f32> {
    let mut offset = 0.0;
    sizes
        .iter()
        .map(|size| {
            let current = offset;
            offset += *size + gap;
            current
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn span_rect(
    placement: GridPlacement,
    column_offsets: &[f32],
    column_widths: &[f32],
    row_offsets: &[f32],
    row_heights: &[f32],
    column_gap: f32,
    row_gap: f32,
) -> Rect {
    let column = placement.column.min(column_widths.len().saturating_sub(1));
    let row = placement.row.min(row_heights.len().saturating_sub(1));
    let column_end = (column + placement.column_span.max(1)).min(column_widths.len());
    let row_end = (row + placement.row_span.max(1)).min(row_heights.len());
    let width = column_widths[column..column_end].iter().sum::<f32>()
        + column_gap * column_end.saturating_sub(column + 1) as f32;
    let height = row_heights[row..row_end].iter().sum::<f32>()
        + row_gap * row_end.saturating_sub(row + 1) as f32;
    Rect::new(column_offsets[column], row_offsets[row], width, height)
}

fn aligned_extent(alignment: Alignment, available: f32, natural: f32) -> f32 {
    if alignment == Alignment::Stretch {
        available
    } else {
        finite_non_negative(natural).min(available)
    }
}

fn aligned_origin(alignment: Alignment, start: f32, available: f32, child_extent: f32) -> f32 {
    match alignment {
        Alignment::Start | Alignment::Stretch => start,
        Alignment::Center => start + (available - child_extent).max(0.0) * 0.5,
        Alignment::End => start + (available - child_extent).max(0.0),
    }
}

fn sum_tracks(sizes: &[f32], gap_total: f32) -> f32 {
    sizes.iter().sum::<f32>() + gap_total
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn sanitize_size(size: Size) -> Size {
    Size::new(
        finite_non_negative(size.width),
        finite_non_negative(size.height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_tracks_share_remaining_width_after_auto_content() {
        let style = GridStyle::new([
            GridTrack::Auto,
            GridTrack::Fraction(1.0),
            GridTrack::Fraction(2.0),
        ])
        .column_gap(10.0);
        let items = [GridItem::new(
            GridPlacement::new(0, 0),
            Size::new(80.0, 20.0),
        )];
        let layout = grid_layout(&style, &items, Constraints::tight(Size::new(380.0, 40.0)));

        assert_eq!(layout.column_widths[0], 80.0);
        assert!((layout.column_widths[1] - 93.333_336).abs() < 0.001);
        assert!((layout.column_widths[2] - 186.666_67).abs() < 0.001);
    }

    #[test]
    fn spanning_item_grows_auto_tracks_and_alignment_uses_cell() {
        let style = GridStyle::new([GridTrack::Auto, GridTrack::Auto]).gap(8.0);
        let item = GridItem::new(GridPlacement::new(0, 0).span(1, 2), Size::new(208.0, 24.0))
            .minimum_size(Size::new(120.0, 20.0))
            .align(Alignment::Center, Alignment::Center);
        let layout = grid_layout(
            &style,
            &[item],
            Constraints::new(Size::ZERO, Size::new(240.0, 80.0)),
        );

        assert_eq!(layout.column_widths, vec![100.0, 100.0]);
        assert_eq!(layout.items[0].cell.width(), 208.0);
        assert_eq!(layout.items[0].rect.width(), 208.0);
    }
}
