use std::{
    collections::{BTreeMap, HashMap},
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
};

use cosmic_text::{Align, Buffer, Hinting, LayoutGlyph, Wrap};
use sui_core::{Error, Rect, Result, Size, Vector};

use crate::{
    flatten::{FlattenedParagraph, FlattenedTextDocument},
    font::{FontContext, ResolvedSpanInput, ResolvedTextFace},
    model::{
        ShapedGlyph, TextCluster, TextClusterGeometry, TextDirection, TextFlowDirection,
        TextLayout, TextLayoutData, TextLayoutId, TextLayoutMetadata, TextLayoutRun,
        TextLayoutVersion, TextLine, TextMeasurement, TextParagraphLayout, TextStyle, TextWrap,
    },
};

const DIRECTION_SENTINEL_METADATA: usize = usize::MAX;

#[derive(Debug, Clone)]
struct PreparedGlyph {
    glyph_id: u16,
    cluster_start: usize,
    span_metadata: usize,
    origin_x: f32,
    origin_y: f32,
    advance: Vector,
    scale: f32,
    bounds: Option<Rect>,
    face_index: usize,
    direction: TextFlowDirection,
}

#[derive(Debug, Clone)]
struct PreparedRunSegment {
    span_metadata: usize,
    byte_range: Range<usize>,
    face_index: usize,
    direction: TextFlowDirection,
    glyph_range: Range<usize>,
    rect: Rect,
}

#[derive(Debug, Clone)]
struct PreparedLine {
    paragraph_index: usize,
    byte_range: Range<usize>,
    rect: Rect,
    baseline: f32,
    ascent: f32,
    descent: f32,
    width: f32,
    direction: TextFlowDirection,
    clusters: Vec<TextClusterGeometry>,
    glyphs: Vec<PreparedGlyph>,
    runs: Vec<PreparedRunSegment>,
}

#[derive(Debug, Clone)]
struct PreparedParagraph {
    paragraph_index: usize,
    byte_range: Range<usize>,
    style: crate::model::TextParagraphStyle,
    rect: Rect,
    line_range: Range<usize>,
}

#[derive(Debug, Clone, Copy)]
struct FaceMetrics {
    units_per_em: f32,
    cap_height: Option<f32>,
}

#[derive(Debug)]
struct PreparedParagraphResult {
    paragraph_rect: Rect,
    lines: Vec<PreparedLine>,
}

pub(crate) fn layout_document(
    flattened: FlattenedTextDocument,
    resolved_spans: Vec<ResolvedSpanInput>,
    box_size: Option<Size>,
    mut font_context: FontContext,
    layout_id: TextLayoutId,
) -> Result<TextLayout> {
    let box_width = box_size.map(|size| size.width);
    let mut faces = vec![font_context.default_face().clone()];
    let mut face_slots: HashMap<cosmic_text::fontdb::ID, usize> = HashMap::new();
    let mut face_metrics: Vec<Option<FaceMetrics>> = vec![None];

    let mut paragraphs = Vec::with_capacity(flattened.paragraphs.len());
    let mut lines = Vec::new();
    let mut measured_width = 0.0_f32;
    let mut block_height = 0.0_f32;
    let mut max_ascent = 0.0_f32;
    let mut max_descent = 0.0_f32;
    let mut max_cap_height: Option<f32> = None;

    for paragraph in &flattened.paragraphs {
        let prepared = prepare_paragraph(
            paragraph,
            &resolved_spans,
            box_width,
            &mut font_context,
            &mut faces,
            &mut face_slots,
            &mut face_metrics,
            &mut max_cap_height,
        )?;

        for line in &prepared.lines {
            measured_width = measured_width.max(line.width);
            block_height = block_height.max(line.rect.max_y());
            max_ascent = max_ascent.max(line.ascent);
            max_descent = max_descent.max(line.descent);
        }

        paragraphs.push(PreparedParagraph {
            paragraph_index: paragraph.index,
            byte_range: paragraph.byte_range.clone(),
            style: paragraph.style.clone(),
            rect: prepared.paragraph_rect,
            line_range: lines.len()..(lines.len() + prepared.lines.len()),
        });
        lines.extend(prepared.lines);
    }

    if lines.is_empty() {
        let default_style = resolved_spans
            .first()
            .map(|span| span.style.clone())
            .unwrap_or_else(TextStyle::default);
        lines.push(PreparedLine {
            paragraph_index: 0,
            byte_range: 0..0,
            rect: Rect::new(0.0, 0.0, 0.0, default_style.line_height),
            baseline: default_style.font_size,
            ascent: default_style.font_size,
            descent: 0.0,
            width: 0.0,
            direction: TextFlowDirection::LeftToRight,
            clusters: Vec::new(),
            glyphs: Vec::new(),
            runs: Vec::new(),
        });
        paragraphs.push(PreparedParagraph {
            paragraph_index: 0,
            byte_range: 0..0,
            style: flattened
                .document
                .paragraphs
                .first()
                .map(|paragraph| paragraph.style.clone())
                .unwrap_or_default(),
            rect: lines[0].rect,
            line_range: 0..1,
        });
    }

    let natural_height = block_height.max(max_ascent + max_descent);
    let final_box_size = box_size.unwrap_or(Size::new(measured_width, natural_height));
    let block_top = ((final_box_size.height - block_height).max(0.0)) * 0.5;

    let mut shaped_glyphs = Vec::new();
    let mut layout_lines = Vec::with_capacity(lines.len());
    let mut layout_runs = Vec::new();
    let mut layout_clusters = Vec::new();
    let mut measured_bounds: Option<Rect> = None;

    for prepared_line in lines {
        let line_index = layout_lines.len();
        let glyph_start = shaped_glyphs.len();
        let run_start = layout_runs.len();
        let cluster_start = layout_clusters.len();

        for glyph in prepared_line.glyphs {
            let translated_bounds = glyph
                .bounds
                .map(|bounds| bounds.translate(Vector::new(0.0, block_top)));
            if let Some(bounds) = translated_bounds {
                measured_bounds = Some(match measured_bounds {
                    Some(current) => current.union(bounds),
                    None => bounds,
                });
            }

            shaped_glyphs.push(ShapedGlyph {
                glyph_id: glyph.glyph_id,
                cluster: glyph.cluster_start,
                span_id: resolved_spans[glyph.span_metadata].id.clone(),
                run_index: 0,
                line_index,
                face_index: glyph.face_index,
                origin_x: glyph.origin_x,
                origin_y: glyph.origin_y + block_top,
                advance: glyph.advance,
                scale: glyph.scale,
                bounds: translated_bounds,
            });
        }

        for run in prepared_line.runs {
            let run_index = layout_runs.len();
            for glyph in &mut shaped_glyphs[(glyph_start + run.glyph_range.start)..(glyph_start + run.glyph_range.end)] {
                glyph.run_index = run_index;
            }

            layout_runs.push(TextLayoutRun {
                paragraph_index: prepared_line.paragraph_index,
                line_index,
                span_id: resolved_spans[run.span_metadata].id.clone(),
                byte_range: run.byte_range,
                glyph_range: (glyph_start + run.glyph_range.start)..(glyph_start + run.glyph_range.end),
                cluster_range: cluster_start..cluster_start,
                rect: run.rect.translate(Vector::new(0.0, block_top)),
                baseline: prepared_line.baseline + block_top,
                face_index: run.face_index,
                direction: run.direction,
            });
        }

        for cluster in &prepared_line.clusters {
            let byte_range = cluster.range.clone();
            let glyph_range =
                (glyph_start + cluster.glyph_range.start)..(glyph_start + cluster.glyph_range.end);
            let run_range = overlapping_run_range(
                &layout_runs[run_start..],
                line_index,
                &byte_range,
            )
            .map(|range| (run_start + range.start)..(run_start + range.end))
            .unwrap_or(run_start..run_start);
            layout_clusters.push(TextCluster {
                paragraph_index: prepared_line.paragraph_index,
                line_index,
                byte_range: byte_range.clone(),
                glyph_range,
                run_range,
                rect: Rect::new(
                    cluster.x_start.min(cluster.x_end),
                    prepared_line.rect.y() + block_top,
                    (cluster.x_end - cluster.x_start).abs(),
                    prepared_line.rect.height(),
                ),
            });
        }

        for run in &mut layout_runs[run_start..] {
            let cluster_range = overlapping_cluster_range(
                &layout_clusters[cluster_start..],
                run.line_index,
                &run.byte_range,
            )
            .map(|range| (cluster_start + range.start)..(cluster_start + range.end))
            .unwrap_or(cluster_start..cluster_start);
            run.cluster_range = cluster_range;
        }

        layout_lines.push(TextLine {
            paragraph_index: prepared_line.paragraph_index,
            byte_range: prepared_line.byte_range,
            run_range: run_start..layout_runs.len(),
            cluster_range: cluster_start..layout_clusters.len(),
            glyph_range: glyph_start..shaped_glyphs.len(),
            rect: prepared_line.rect.translate(Vector::new(0.0, block_top)),
            baseline: prepared_line.baseline + block_top,
            ascent: prepared_line.ascent,
            descent: prepared_line.descent,
            width: prepared_line.width,
            direction: prepared_line.direction,
            clusters: prepared_line.clusters,
        });
    }

    let paragraph_layouts = paragraphs
        .into_iter()
        .map(|paragraph| {
            let (run_range, cluster_range, glyph_range) = collapse_line_component_ranges(
                &layout_lines[paragraph.line_range.clone()],
            );
            TextParagraphLayout {
                paragraph_index: paragraph.paragraph_index,
                byte_range: paragraph.byte_range,
                line_range: paragraph.line_range,
                run_range,
                cluster_range,
                glyph_range,
                rect: paragraph.rect.translate(Vector::new(0.0, block_top)),
                style: paragraph.style,
            }
        })
        .collect::<Vec<_>>();

    let bounds = measured_bounds.unwrap_or_else(|| Rect::new(0.0, block_top, measured_width, natural_height));
    let measurement = TextMeasurement {
        width: measured_width,
        height: natural_height,
        bounds,
        ascent: max_ascent,
        descent: max_descent,
        cap_height: max_cap_height,
    };
    let version = compute_layout_version(
        final_box_size,
        &faces,
        measurement,
        &paragraph_layouts,
        &layout_lines,
        &layout_runs,
        &layout_clusters,
        &shaped_glyphs,
    );

    Ok(TextLayout {
        primary_style: flattened.document.primary_style(),
        document: Arc::new(flattened.document),
        data: Arc::new(TextLayoutData {
            metadata: TextLayoutMetadata {
                id: layout_id,
                version,
            },
            text: flattened.text,
            box_size: final_box_size,
            faces,
            measurement,
            paragraphs: paragraph_layouts,
            lines: layout_lines,
            runs: layout_runs,
            clusters: layout_clusters,
            glyphs: shaped_glyphs,
        }),
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_paragraph(
    paragraph: &FlattenedParagraph,
    resolved_spans: &[ResolvedSpanInput],
    box_width: Option<f32>,
    font_context: &mut FontContext,
    faces: &mut Vec<ResolvedTextFace>,
    face_slots: &mut HashMap<cosmic_text::fontdb::ID, usize>,
    face_metrics: &mut Vec<Option<FaceMetrics>>,
    max_cap_height: &mut Option<f32>,
) -> Result<PreparedParagraphResult> {
    let paragraph_spans = paragraph
        .span_range
        .clone()
        .map(|index| &resolved_spans[index])
        .collect::<Vec<_>>();
    let primary_style = paragraph_spans
        .first()
        .map(|span| span.style.clone())
        .unwrap_or_else(TextStyle::default);

    let metrics = cosmic_text::Metrics::new(primary_style.font_size, primary_style.line_height);
    let mut buffer = Buffer::new(&mut font_context.font_system, metrics);
    buffer.set_wrap(&mut font_context.font_system, map_wrap(paragraph.style.wrap));
    buffer.set_hinting(&mut font_context.font_system, Hinting::Disabled);
    buffer.set_size(&mut font_context.font_system, box_width, None);

    let prefix = direction_prefix(paragraph.style.direction);
    let suffix = if prefix.is_empty() { "" } else { "\u{202C}" };
    let prefix_len = prefix.len();
    let paragraph_len = paragraph.byte_range.end.saturating_sub(paragraph.byte_range.start);
    let default_family_name = paragraph_spans.first().and_then(|span| span.family_name.clone());
    let default_attrs = default_attrs_for_style(
        &primary_style,
        default_family_name.as_deref(),
        DIRECTION_SENTINEL_METADATA,
    );

    let mut rich_spans = Vec::new();
    if !prefix.is_empty() {
        rich_spans.push((
            prefix.to_string(),
            default_attrs.clone().metadata(DIRECTION_SENTINEL_METADATA),
        ));
    }
    for (index, span) in paragraph.span_range.clone().zip(paragraph_spans.iter()) {
        rich_spans.push((span.text.clone(), FontContext::attrs_for_span(span, index)));
    }
    if !suffix.is_empty() {
        rich_spans.push((
            suffix.to_string(),
            default_attrs.clone().metadata(DIRECTION_SENTINEL_METADATA),
        ));
    }

    buffer.set_rich_text(
        &mut font_context.font_system,
        rich_spans.iter().map(|(text, attrs)| (text.as_str(), attrs.clone())),
        &default_attrs,
        cosmic_text::Shaping::Advanced,
        map_align(paragraph.style.align, paragraph.style.direction),
    );

    let buffer_line = buffer
        .lines
        .first()
        .ok_or_else(|| Error::new("cosmic-text paragraph buffer did not contain a line"))?;
    let paragraph_rtl = buffer_line
        .shape_opt()
        .map(|shape| shape.rtl)
        .unwrap_or(matches!(paragraph.style.direction, TextDirection::RightToLeft));
    let layout_lines = buffer_line
        .layout_opt()
        .ok_or_else(|| Error::new("cosmic-text paragraph buffer did not produce layout lines"))?;

    let mut prepared_lines = Vec::with_capacity(layout_lines.len());
    let mut line_top = 0.0_f32;
    let mut paragraph_rect: Option<Rect> = None;

    for layout_line in layout_lines {
        let line_height = layout_line.line_height_opt.unwrap_or(metrics.line_height);
        let glyph_height = layout_line.max_ascent + layout_line.max_descent;
        let centering_offset = (line_height - glyph_height) * 0.5;
        let baseline = line_top + centering_offset + layout_line.max_ascent;

        let visible_glyphs = layout_line
            .glyphs
            .iter()
            .filter_map(|glyph| {
                if glyph.metadata == DIRECTION_SENTINEL_METADATA {
                    return None;
                }
                let adjusted = adjust_working_range(glyph.start, glyph.end, prefix_len, paragraph_len);
                if adjusted.start >= adjusted.end {
                    return None;
                }
                Some((glyph, adjusted))
            })
            .collect::<Vec<_>>();

        let line_byte_range = visible_line_byte_range(&visible_glyphs, paragraph.byte_range.start);
        let line_direction = if paragraph_rtl {
            TextFlowDirection::RightToLeft
        } else {
            TextFlowDirection::LeftToRight
        };
        let line_left = visible_glyphs
                .iter()
            .map(|(glyph, _)| glyph.x)
            .reduce(f32::min)
            .unwrap_or_else(|| {
                line_origin_x(
                    paragraph.style.align,
                    line_direction,
                    box_width.unwrap_or(layout_line.w),
                    layout_line.w,
                )
            });
        let line_rect = Rect::new(line_left, line_top, layout_line.w.max(0.0), line_height);
        paragraph_rect = Some(match paragraph_rect {
            Some(rect) => rect.union(line_rect),
            None => line_rect,
        });

        let clusters = build_cluster_geometries(&visible_glyphs, paragraph.byte_range.start, line_byte_range.clone());
        let prepared_glyphs = build_prepared_glyphs(
            &visible_glyphs,
            paragraph.byte_range.start,
            baseline,
            font_context,
            faces,
            face_slots,
            face_metrics,
            max_cap_height,
        )?;
        let runs = build_run_segments(&visible_glyphs, &prepared_glyphs, paragraph.byte_range.start, line_top, line_height);

        prepared_lines.push(PreparedLine {
            paragraph_index: paragraph.index,
            byte_range: line_byte_range,
            rect: line_rect,
            baseline,
            ascent: layout_line.max_ascent,
            descent: layout_line.max_descent,
            width: layout_line.w,
            direction: line_direction,
            clusters,
            glyphs: prepared_glyphs,
            runs,
        });

        line_top += line_height;
    }

    Ok(PreparedParagraphResult {
        paragraph_rect: paragraph_rect.unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, metrics.line_height)),
        lines: prepared_lines,
    })
}

fn build_prepared_glyphs(
    visible_glyphs: &[(&LayoutGlyph, Range<usize>)],
    paragraph_start: usize,
    baseline: f32,
    font_context: &FontContext,
    faces: &mut Vec<ResolvedTextFace>,
    face_slots: &mut HashMap<cosmic_text::fontdb::ID, usize>,
    face_metrics: &mut Vec<Option<FaceMetrics>>,
    max_cap_height: &mut Option<f32>,
) -> Result<Vec<PreparedGlyph>> {
    let mut prepared = Vec::with_capacity(visible_glyphs.len());

    for (glyph, adjusted_range) in visible_glyphs {
        let face_index = font_context.resolve_face_index(face_slots, faces, glyph.font_id)?;
        ensure_face_metrics(face_index, faces, face_metrics)?;
        let metrics = face_metrics[face_index].expect("face metrics initialized");
        let scale = glyph.font_size / metrics.units_per_em;
        let origin_x = glyph.x + (glyph.font_size * glyph.x_offset);
        let origin_y = baseline - (glyph.font_size * glyph.y_offset);
        let bounds = faces[face_index].glyph_bounds(glyph.glyph_id, origin_x, origin_y, scale);
        if let Some(cap_height) = metrics.cap_height.map(|value| value * scale) {
            *max_cap_height = Some(max_cap_height.map_or(cap_height, |current| current.max(cap_height)));
        }

        prepared.push(PreparedGlyph {
            glyph_id: glyph.glyph_id,
            cluster_start: paragraph_start + adjusted_range.start,
            span_metadata: glyph.metadata,
            origin_x,
            origin_y,
            advance: Vector::new(glyph.w, 0.0),
            scale,
            bounds,
            face_index,
            direction: if glyph.level.is_rtl() {
                TextFlowDirection::RightToLeft
            } else {
                TextFlowDirection::LeftToRight
            },
        });
    }

    Ok(prepared)
}

fn build_run_segments(
    visible_glyphs: &[(&LayoutGlyph, Range<usize>)],
    prepared_glyphs: &[PreparedGlyph],
    paragraph_start: usize,
    line_top: f32,
    line_height: f32,
) -> Vec<PreparedRunSegment> {
    if visible_glyphs.is_empty() {
        return Vec::new();
    }

    let mut runs = Vec::new();
    let mut run_start = 0_usize;

    while run_start < visible_glyphs.len() {
        let first = visible_glyphs[run_start].0;
        let first_prepared = &prepared_glyphs[run_start];
        let mut run_end = run_start + 1;
        while run_end < visible_glyphs.len() {
            let next = visible_glyphs[run_end].0;
            let next_prepared = &prepared_glyphs[run_end];
            if next.metadata != first.metadata
                || next.font_id != first.font_id
                || next_prepared.direction != first_prepared.direction
            {
                break;
            }
            run_end += 1;
        }

        let local_range = visible_glyphs[run_start..run_end]
            .iter()
            .map(|(_, range)| range.clone())
            .fold(None, |current: Option<Range<usize>>, range| {
                Some(match current {
                    Some(existing) => existing.start.min(range.start)..existing.end.max(range.end),
                    None => range,
                })
            })
            .unwrap_or(0..0);
        let rect = visible_glyphs[run_start..run_end]
            .iter()
            .map(|(glyph, _)| Rect::new(glyph.x, line_top, glyph.w, line_height))
            .reduce(|bounds, rect| bounds.union(rect))
            .unwrap_or_else(|| Rect::new(0.0, line_top, 0.0, line_height));

        runs.push(PreparedRunSegment {
            span_metadata: first.metadata,
            byte_range: (paragraph_start + local_range.start)..(paragraph_start + local_range.end),
            face_index: first_prepared.face_index,
            direction: first_prepared.direction,
            glyph_range: run_start..run_end,
            rect,
        });

        run_start = run_end;
    }

    runs
}

fn build_cluster_geometries(
    visible_glyphs: &[(&LayoutGlyph, Range<usize>)],
    paragraph_start: usize,
    line_byte_range: Range<usize>,
) -> Vec<TextClusterGeometry> {
    if visible_glyphs.is_empty() {
        if line_byte_range.is_empty() {
            return Vec::new();
        }
        return vec![TextClusterGeometry {
            range: line_byte_range,
            x_start: 0.0,
            x_end: 0.0,
            glyph_range: 0..0,
        }];
    }

    let mut clusters = BTreeMap::<usize, (Range<usize>, f32, f32, bool, Range<usize>)>::new();
    for (glyph_index, (glyph, adjusted_range)) in visible_glyphs.iter().enumerate() {
        let range = (paragraph_start + adjusted_range.start)..(paragraph_start + adjusted_range.end);
        let key = range.start;
        let left = glyph.x;
        let right = glyph.x + glyph.w;
        let rtl = glyph.level.is_rtl();
        match clusters.get_mut(&key) {
            Some((stored_range, cluster_left, cluster_right, _, glyph_range)) => {
                stored_range.end = stored_range.end.max(range.end);
                *cluster_left = cluster_left.min(left);
                *cluster_right = cluster_right.max(right);
                glyph_range.end = glyph_index + 1;
            }
            None => {
                clusters.insert(key, (range, left, right, rtl, glyph_index..(glyph_index + 1)));
            }
        }
    }

    let mut geometries = Vec::with_capacity(clusters.len() + 2);
    let mut previous_end = line_byte_range.start;
    let mut previous_x = clusters
        .values()
        .next()
        .map(|(_, left, right, rtl, _)| if *rtl { *right } else { *left })
        .unwrap_or(0.0);
    let mut previous_glyph_end = 0_usize;

    for (_, (range, left, right, rtl, glyph_range)) in clusters {
        if range.start > previous_end {
            geometries.push(TextClusterGeometry {
                range: previous_end..range.start,
                x_start: previous_x,
                x_end: previous_x,
                glyph_range: previous_glyph_end..previous_glyph_end,
            });
        }
        let (x_start, x_end) = if rtl { (right, left) } else { (left, right) };
        geometries.push(TextClusterGeometry {
            range: range.clone(),
            x_start,
            x_end,
            glyph_range: glyph_range.clone(),
        });
        previous_end = range.end;
        previous_x = x_end;
        previous_glyph_end = glyph_range.end;
    }

    if previous_end < line_byte_range.end {
        geometries.push(TextClusterGeometry {
            range: previous_end..line_byte_range.end,
            x_start: previous_x,
            x_end: previous_x,
            glyph_range: previous_glyph_end..previous_glyph_end,
        });
    }

    geometries
}

fn collapse_line_component_ranges(lines: &[TextLine]) -> (Range<usize>, Range<usize>, Range<usize>) {
    let run_range = collapse_range(lines.iter().map(|line| line.run_range.clone())).unwrap_or(0..0);
    let cluster_range =
        collapse_range(lines.iter().map(|line| line.cluster_range.clone())).unwrap_or(0..0);
    let glyph_range =
        collapse_range(lines.iter().map(|line| line.glyph_range.clone())).unwrap_or(0..0);
    (run_range, cluster_range, glyph_range)
}

fn collapse_range(ranges: impl Iterator<Item = Range<usize>>) -> Option<Range<usize>> {
    ranges.fold(None, |current: Option<Range<usize>>, range| {
        Some(match current {
            Some(existing) => existing.start.min(range.start)..existing.end.max(range.end),
            None => range,
        })
    })
}

fn compute_layout_version(
    box_size: Size,
    faces: &[ResolvedTextFace],
    measurement: TextMeasurement,
    paragraphs: &[TextParagraphLayout],
    lines: &[TextLine],
    runs: &[TextLayoutRun],
    clusters: &[TextCluster],
    glyphs: &[ShapedGlyph],
) -> TextLayoutVersion {
    let mut state = DefaultHasher::new();
    hash_size(&mut state, box_size);
    hash_measurement(&mut state, measurement);
    hash_faces(&mut state, faces);
    hash_paragraphs(&mut state, paragraphs);
    hash_lines(&mut state, lines);
    hash_runs(&mut state, runs);
    hash_clusters(&mut state, clusters);
    hash_glyphs(&mut state, glyphs);
    TextLayoutVersion::new(state.finish())
}

fn hash_faces(state: &mut DefaultHasher, faces: &[ResolvedTextFace]) {
    faces.len().hash(state);
    for face in faces {
        face.data_ptr().hash(state);
        face.data_len().hash(state);
        face.face_index().hash(state);
    }
}

fn hash_paragraphs(state: &mut DefaultHasher, paragraphs: &[TextParagraphLayout]) {
    paragraphs.len().hash(state);
    for paragraph in paragraphs {
        paragraph.paragraph_index.hash(state);
        paragraph.byte_range.hash(state);
        paragraph.line_range.hash(state);
        paragraph.run_range.hash(state);
        paragraph.cluster_range.hash(state);
        paragraph.glyph_range.hash(state);
        paragraph.style.hash(state);
        hash_rect(state, paragraph.rect);
    }
}

fn hash_lines(state: &mut DefaultHasher, lines: &[TextLine]) {
    lines.len().hash(state);
    for line in lines {
        line.paragraph_index.hash(state);
        line.byte_range.hash(state);
        line.run_range.hash(state);
        line.cluster_range.hash(state);
        line.glyph_range.hash(state);
        hash_rect(state, line.rect);
        hash_f32(state, line.baseline);
        hash_f32(state, line.ascent);
        hash_f32(state, line.descent);
        hash_f32(state, line.width);
        line.direction.hash(state);
        line.clusters.len().hash(state);
        for cluster in &line.clusters {
            cluster.range.hash(state);
            cluster.glyph_range.hash(state);
            hash_f32(state, cluster.x_start);
            hash_f32(state, cluster.x_end);
        }
    }
}

fn hash_runs(state: &mut DefaultHasher, runs: &[TextLayoutRun]) {
    runs.len().hash(state);
    for run in runs {
        run.paragraph_index.hash(state);
        run.line_index.hash(state);
        run.span_id.hash(state);
        run.byte_range.hash(state);
        run.glyph_range.hash(state);
        run.cluster_range.hash(state);
        hash_rect(state, run.rect);
        hash_f32(state, run.baseline);
        run.face_index.hash(state);
        run.direction.hash(state);
    }
}

fn hash_clusters(state: &mut DefaultHasher, clusters: &[TextCluster]) {
    clusters.len().hash(state);
    for cluster in clusters {
        cluster.paragraph_index.hash(state);
        cluster.line_index.hash(state);
        cluster.byte_range.hash(state);
        cluster.glyph_range.hash(state);
        cluster.run_range.hash(state);
        hash_rect(state, cluster.rect);
    }
}

fn hash_glyphs(state: &mut DefaultHasher, glyphs: &[ShapedGlyph]) {
    glyphs.len().hash(state);
    for glyph in glyphs {
        glyph.glyph_id.hash(state);
        glyph.cluster.hash(state);
        glyph.span_id.hash(state);
        glyph.run_index.hash(state);
        glyph.line_index.hash(state);
        glyph.face_index.hash(state);
        hash_f32(state, glyph.origin_x);
        hash_f32(state, glyph.origin_y);
        hash_vector(state, glyph.advance);
        hash_f32(state, glyph.scale);
        match glyph.bounds {
            Some(bounds) => {
                true.hash(state);
                hash_rect(state, bounds);
            }
            None => false.hash(state),
        }
    }
}

fn hash_measurement(state: &mut DefaultHasher, measurement: TextMeasurement) {
    hash_f32(state, measurement.width);
    hash_f32(state, measurement.height);
    hash_rect(state, measurement.bounds);
    hash_f32(state, measurement.ascent);
    hash_f32(state, measurement.descent);
    match measurement.cap_height {
        Some(cap_height) => {
            true.hash(state);
            hash_f32(state, cap_height);
        }
        None => false.hash(state),
    }
}

fn hash_rect(state: &mut DefaultHasher, rect: Rect) {
    hash_f32(state, rect.x());
    hash_f32(state, rect.y());
    hash_f32(state, rect.width());
    hash_f32(state, rect.height());
}

fn hash_size(state: &mut DefaultHasher, size: Size) {
    hash_f32(state, size.width);
    hash_f32(state, size.height);
}

fn hash_vector(state: &mut DefaultHasher, vector: Vector) {
    hash_f32(state, vector.x);
    hash_f32(state, vector.y);
}

fn hash_f32(state: &mut DefaultHasher, value: f32) {
    value.to_bits().hash(state);
}

fn overlapping_run_range(
    runs: &[TextLayoutRun],
    line_index: usize,
    cluster_byte_range: &Range<usize>,
) -> Option<Range<usize>> {
    contiguous_overlap_range(runs.iter().enumerate().filter_map(|(index, run)| {
        if run.line_index == line_index && byte_ranges_overlap(cluster_byte_range, &run.byte_range) {
            Some(index)
        } else {
            None
        }
    }))
}

fn overlapping_cluster_range(
    clusters: &[TextCluster],
    line_index: usize,
    run_byte_range: &Range<usize>,
) -> Option<Range<usize>> {
    contiguous_overlap_range(clusters.iter().enumerate().filter_map(|(index, cluster)| {
        if cluster.line_index == line_index && byte_ranges_overlap(&cluster.byte_range, run_byte_range)
        {
            Some(index)
        } else {
            None
        }
    }))
}

fn contiguous_overlap_range(indices: impl Iterator<Item = usize>) -> Option<Range<usize>> {
    let mut indices = indices.peekable();
    let start = *indices.peek()?;
    let end = indices.last().map(|index| index + 1).unwrap_or(start + 1);
    Some(start..end)
}

fn byte_ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    if left.is_empty() {
        return right.start <= left.start && left.start <= right.end;
    }
    if right.is_empty() {
        return left.start <= right.start && right.start <= left.end;
    }
    left.start < right.end && right.start < left.end
}

fn visible_line_byte_range(
    visible_glyphs: &[(&LayoutGlyph, Range<usize>)],
    paragraph_start: usize,
) -> Range<usize> {
    visible_glyphs
        .iter()
        .map(|(_, range)| (paragraph_start + range.start)..(paragraph_start + range.end))
        .fold(None, |current: Option<Range<usize>>, range| {
            Some(match current {
                Some(existing) => existing.start.min(range.start)..existing.end.max(range.end),
                None => range,
            })
        })
        .unwrap_or(paragraph_start..paragraph_start)
}

fn adjust_working_range(
    start: usize,
    end: usize,
    prefix_len: usize,
    paragraph_len: usize,
) -> Range<usize> {
    let adjusted_start = start.saturating_sub(prefix_len).min(paragraph_len);
    let adjusted_end = end.saturating_sub(prefix_len).min(paragraph_len);
    adjusted_start..adjusted_end
}

fn ensure_face_metrics(
    face_index: usize,
    faces: &[ResolvedTextFace],
    face_metrics: &mut Vec<Option<FaceMetrics>>,
) -> Result<()> {
    while face_metrics.len() <= face_index {
        face_metrics.push(None);
    }
    if face_metrics[face_index].is_some() {
        return Ok(());
    }

    let face = ttf_parser::Face::parse(faces[face_index].bytes(), faces[face_index].face_index())
        .map_err(|_| Error::new("failed to parse text face metrics"))?;
    let units_per_em = face.units_per_em();
    if units_per_em == 0 {
        return Err(Error::new("text face reported an invalid units-per-em value"));
    }

    face_metrics[face_index] = Some(FaceMetrics {
        units_per_em: units_per_em as f32,
        cap_height: face.capital_height().map(f32::from),
    });
    Ok(())
}

fn direction_prefix(direction: TextDirection) -> &'static str {
    match direction {
        TextDirection::LeftToRight => "\u{202A}",
        TextDirection::RightToLeft => "\u{202B}",
        TextDirection::Auto => "",
    }
}

fn map_wrap(wrap: TextWrap) -> Wrap {
    match wrap {
        TextWrap::NoWrap => Wrap::None,
        TextWrap::Word => Wrap::WordOrGlyph,
        TextWrap::Character => Wrap::Glyph,
    }
}

fn map_align(align: crate::model::TextAlign, direction: TextDirection) -> Option<Align> {
    match align {
        crate::model::TextAlign::Start => match direction {
            TextDirection::LeftToRight => Some(Align::Left),
            TextDirection::RightToLeft => Some(Align::End),
            TextDirection::Auto => None,
        },
        crate::model::TextAlign::End => Some(Align::End),
        crate::model::TextAlign::Left => Some(Align::Left),
        crate::model::TextAlign::Right => Some(Align::Right),
        crate::model::TextAlign::Center => Some(Align::Center),
        crate::model::TextAlign::Justified => Some(Align::Justified),
    }
}

fn line_origin_x(
    align: crate::model::TextAlign,
    direction: TextFlowDirection,
    box_width: f32,
    line_width: f32,
) -> f32 {
    match align {
        crate::model::TextAlign::Center => (box_width - line_width) * 0.5,
        crate::model::TextAlign::End => match direction {
            TextFlowDirection::RightToLeft => 0.0,
            _ => box_width - line_width,
        },
        crate::model::TextAlign::Left => 0.0,
        crate::model::TextAlign::Right => box_width - line_width,
        crate::model::TextAlign::Start | crate::model::TextAlign::Justified => match direction {
            TextFlowDirection::RightToLeft => box_width - line_width,
            _ => 0.0,
        },
    }
}

fn default_attrs_for_style<'a>(
    style: &TextStyle,
    family_name: Option<&'a str>,
    metadata: usize,
) -> cosmic_text::Attrs<'a> {
    let attrs = cosmic_text::Attrs::new()
        .metrics(cosmic_text::Metrics::new(style.font_size, style.line_height))
        .metadata(metadata);
    match family_name {
        Some(name) => attrs.family(cosmic_text::Family::Name(name)),
        None => attrs,
    }
}