#[cfg(test)]
use crate::draw::ClipState;
#[cfg(test)]
use crate::draw::DrawOp;
use crate::draw::DrawOpArena;
#[cfg(test)]
use crate::draw::DrawOpKind;
#[cfg(test)]
use crate::draw::PreparedVertices;
use crate::draw::SceneRasterState;
use crate::geometry::CachedGlyphMesh;
#[cfg(test)]
use crate::geometry::analytic_path_transform_without_translation;
use crate::geometry::append_analytic_path_quad;
use crate::geometry::build_analytic_fill_path_data;
use crate::geometry::build_analytic_stroke_path_data;
use crate::geometry::build_lyon_path;
use crate::geometry::normalized_analytic_path_transform;
use crate::gpu::Vertex;
use crate::output::shader_color;
use crate::path_cache::PathMeshCache;
use crate::primitives::to_ndc;
use crate::scene::FillPathRenderMode;
use crate::scene::analytic_coverage_outset;
#[cfg(test)]
use crate::submission::cache_draw_ops_internal;
#[cfg(test)]
use crate::submission::stamp_draw_op_analytic_path_slots;
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;
use sui_core::Color;
use sui_core::Path as ScenePath;
#[cfg(test)]
use sui_core::Point;
#[cfg(test)]
use sui_core::Rect;
use sui_core::Result;
use sui_core::Size;
#[cfg(test)]
use sui_core::Transform;
#[cfg(test)]
use sui_core::Vector;
use sui_scene::StrokeStyle;

pub(crate) fn append_cached_path_mesh(
    vertices: &mut Vec<Vertex>,
    mesh: &CachedGlyphMesh,
    color: Color,
    viewport: Size,
) {
    if viewport.is_empty() {
        return;
    }

    let rgba = shader_color(color);
    for index in &mesh.indices {
        let vertex = mesh.vertices[*index as usize];
        let ndc = to_ndc(vertex.position.x, vertex.position.y, viewport);
        vertices.push(Vertex::basic(
            ndc,
            [rgba[0], rgba[1], rgba[2], rgba[3] * vertex.coverage],
            [0.0, 0.0],
            [0.0; 4],
        ));
    }
}
pub(crate) fn append_painted_path(
    vertices: &mut Vec<Vertex>,
    overlay_vertices: &mut Vec<Vertex>,
    draw_ops: &mut DrawOpArena,
    state: &SceneRasterState,
    path: &ScenePath,
    color: Color,
    path_cache: &mut PathMeshCache,
    viewport: Size,
    feather_width: f32,
) -> Result<FillPathRenderMode> {
    if path.is_empty() || viewport.is_empty() {
        return Ok(FillPathRenderMode::SolidOnly);
    }

    let coverage_outset = analytic_coverage_outset(feather_width);
    if state
        .visible_rect(path.bounds().inflate(coverage_outset, coverage_outset))
        .is_none()
    {
        return Ok(FillPathRenderMode::SolidOnly);
    }

    let transformed_bounds = state.current_transform.transform_rect_bbox(path.bounds());
    let (analytic_transform, path_origin) =
        normalized_analytic_path_transform(path, state.current_transform);
    if let Some(data) =
        path_cache.cached_analytic_fill(path, state.current_transform, feather_width, || {
            let lyon_path = build_lyon_path(path, analytic_transform);
            build_analytic_fill_path_data(&lyon_path, feather_width)
        })
    {
        append_analytic_path_quad(
            overlay_vertices,
            transformed_bounds.inflate(coverage_outset, coverage_outset),
            color,
            viewport,
            path_origin,
        );
        let id = draw_ops.insert_analytic_path_arc(data);
        return Ok(FillPathRenderMode::SolidPlusAnalytic { id });
    }

    let mesh = path_cache.cached_fill_mesh(path, state.current_transform, feather_width)?;
    append_cached_path_mesh(vertices, &mesh, color, viewport);
    Ok(FillPathRenderMode::SolidOnly)
}

pub(crate) fn append_stroked_path(
    vertices: &mut Vec<Vertex>,
    overlay_vertices: &mut Vec<Vertex>,
    draw_ops: &mut DrawOpArena,
    state: &SceneRasterState,
    path: &ScenePath,
    color: Color,
    stroke: StrokeStyle,
    path_cache: &mut PathMeshCache,
    viewport: Size,
    feather_width: f32,
) -> Result<Option<u64>> {
    if path.is_empty() || viewport.is_empty() {
        return Ok(None);
    }

    let stroke = StrokeStyle {
        width: stroke.width.max(1.0),
        ..stroke
    };
    let line_width = stroke.width;
    let coverage_outset = analytic_coverage_outset(feather_width);
    let stroke_outset = (line_width * 0.5) + coverage_outset;
    if state
        .visible_rect(path.bounds().inflate(stroke_outset, stroke_outset))
        .is_none()
    {
        return Ok(None);
    }

    let analytic_stroke_supported = matches!(
        (stroke.cap, stroke.join),
        (sui_scene::StrokeCap::Butt, sui_scene::StrokeJoin::Miter)
            | (sui_scene::StrokeCap::Round, sui_scene::StrokeJoin::Round)
    );
    if analytic_stroke_supported {
        let transformed_bounds = state.current_transform.transform_rect_bbox(path.bounds());
        let (analytic_transform, path_origin) =
            normalized_analytic_path_transform(path, state.current_transform);
        if let Some(data) = path_cache.cached_analytic_stroke(
            path,
            state.current_transform,
            stroke,
            feather_width,
            || {
                let lyon_path = build_lyon_path(path, analytic_transform);
                build_analytic_stroke_path_data(&lyon_path, line_width, feather_width)
            },
        ) {
            append_analytic_path_quad(
                overlay_vertices,
                transformed_bounds.inflate(stroke_outset, stroke_outset),
                color,
                viewport,
                path_origin,
            );
            let id = draw_ops.insert_analytic_path_arc(data);
            return Ok(Some(id));
        }
    }

    let mesh =
        path_cache.cached_stroke_mesh(path, state.current_transform, stroke, feather_width)?;
    append_cached_path_mesh(vertices, &mesh, color, viewport);
    Ok(None)
}

#[cfg(test)]
pub(crate) mod analytic_path_cache_tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn analytic_path_resource_identity_ignores_translation() {
        let path = ScenePath::circle(Point::ZERO, 18.0);
        let (first_transform, first_origin) =
            analytic_path_transform_without_translation(Transform::translation(32.0, 48.0));
        let (translated_transform, translated_origin) =
            analytic_path_transform_without_translation(Transform::translation(312.0, 196.0));
        let first =
            build_analytic_stroke_path_data(&build_lyon_path(&path, first_transform), 2.0, 1.0)
                .expect("circle should use analytic path rendering");
        let translated = build_analytic_stroke_path_data(
            &build_lyon_path(&path, translated_transform),
            2.0,
            1.0,
        )
        .expect("translated circle should use analytic path rendering");

        assert_eq!(first.contours, translated.contours);
        assert_eq!(first.points, translated.points);
        assert_eq!(first.resource_signature, translated.resource_signature);
        assert_eq!(translated_origin - first_origin, Vector::new(280.0, 148.0));
    }

    #[test]
    fn analytic_path_resource_identity_ignores_path_coordinate_translation() {
        let first_path = ScenePath::circle(Point::new(18.0, 18.0), 18.0);
        let translated_path = ScenePath::circle(Point::new(298.0, 166.0), 18.0);
        let (first_transform, first_origin) =
            normalized_analytic_path_transform(&first_path, Transform::IDENTITY);
        let (translated_transform, translated_origin) =
            normalized_analytic_path_transform(&translated_path, Transform::IDENTITY);
        let first = build_analytic_stroke_path_data(
            &build_lyon_path(&first_path, first_transform),
            2.0,
            1.0,
        )
        .expect("circle should use analytic path rendering");
        let translated = build_analytic_stroke_path_data(
            &build_lyon_path(&translated_path, translated_transform),
            2.0,
            1.0,
        )
        .expect("translated circle should use analytic path rendering");

        assert_eq!(first.resource_signature, translated.resource_signature);
        assert_eq!(translated_origin - first_origin, Vector::new(280.0, 148.0));
    }

    #[test]
    fn stamped_analytic_paths_with_distinct_resources_share_one_draw() {
        let first = build_analytic_stroke_path_data(
            &build_lyon_path(
                &ScenePath::circle(Point::new(18.0, 18.0), 18.0),
                Transform::IDENTITY,
            ),
            2.0,
            1.0,
        )
        .expect("circle should use analytic path rendering");
        let second = build_analytic_stroke_path_data(
            &build_lyon_path(
                &ScenePath::rounded_rect(Rect::new(0.0, 0.0, 80.0, 40.0), 8.0),
                Transform::IDENTITY,
            ),
            2.0,
            1.0,
        )
        .expect("rounded rect should use analytic path rendering");
        let first_signature = first.resource_signature;
        let second_signature = second.resource_signature;
        let mut draw_ops = DrawOpArena::default();
        draw_ops.clip_states.push(ClipState {
            clip_paths: Vec::new(),
        });
        let first_id = draw_ops.insert_analytic_path(first);
        let second_id = draw_ops.insert_analytic_path(second);
        draw_ops.scene_vertices =
            vec![Vertex::basic([0.0, 0.0], [1.0; 4], [0.0; 2], [0.0; 4],); 12];
        draw_ops.draw_ops = vec![
            DrawOp {
                kind: DrawOpKind::AnalyticPath { id: first_id },
                vertices: PreparedVertices { start: 0, len: 6 },
                clip_rect: None,
                clip_state_index: 0,
                image: None,
            },
            DrawOp {
                kind: DrawOpKind::AnalyticPath { id: second_id },
                vertices: PreparedVertices { start: 6, len: 6 },
                clip_rect: None,
                clip_state_index: 0,
                image: None,
            },
        ];
        let slots = HashMap::from([(first_signature, 3), (second_signature, 7)]);

        stamp_draw_op_analytic_path_slots(&mut draw_ops, &slots);
        let passes = cache_draw_ops_internal(&draw_ops, true);

        assert_eq!(passes.len(), 1);
        assert_eq!(passes[0].draws.len(), 1);
        assert_eq!(passes[0].draws[0].vertices.len, 12);
        assert_eq!(draw_ops.scene_vertices[0].shader_params[0], 3.0);
        assert_eq!(draw_ops.scene_vertices[6].shader_params[0], 7.0);
    }

    #[test]
    fn cpu_analytic_path_cache_reuses_translated_geometry() {
        let first_path = ScenePath::circle(Point::new(18.0, 18.0), 18.0);
        let translated_path = ScenePath::circle(Point::new(298.0, 166.0), 18.0);
        let stroke = StrokeStyle::new(2.0);
        let builds = Cell::new(0_u32);
        let mut cache = PathMeshCache::default();
        let build = |path: &ScenePath| {
            builds.set(builds.get() + 1);
            let (transform, _) = normalized_analytic_path_transform(path, Transform::IDENTITY);
            build_analytic_stroke_path_data(&build_lyon_path(path, transform), 2.0, 1.0)
        };

        let first = cache
            .cached_analytic_stroke(&first_path, Transform::IDENTITY, stroke, 1.0, || {
                build(&first_path)
            })
            .expect("first circle should populate the CPU cache");
        let translated = cache
            .cached_analytic_stroke(&translated_path, Transform::IDENTITY, stroke, 1.0, || {
                build(&translated_path)
            })
            .expect("translated circle should reuse the CPU cache");

        assert_eq!(builds.get(), 1);
        assert!(Arc::ptr_eq(&first, &translated));
    }

    #[test]
    fn retired_mesh_and_analytic_geometry_are_evicted_without_invalidating_draws() {
        let path = ScenePath::circle(Point::new(18.0, 18.0), 18.0);
        let mut cache = PathMeshCache::default();
        cache.begin_frame(1);
        let mesh = cache
            .cached_stroke_mesh(&path, Transform::IDENTITY, StrokeStyle::new(2.0), 1.0)
            .unwrap();
        let analytic = cache
            .cached_analytic_fill(&path, Transform::IDENTITY, 1.0, || {
                build_analytic_fill_path_data(&build_lyon_path(&path, Transform::IDENTITY), 1.0)
            })
            .unwrap();
        assert_eq!(cache.snapshot().entries, 2);
        assert_eq!(cache.snapshot().misses, 2);
        cache.begin_frame(121);
        assert_eq!(cache.snapshot().entries, 2);
        cache.begin_frame(122);
        assert_eq!(cache.snapshot().entries, 0);
        // Retained draws own their geometry independently of cache ownership.
        assert!(!mesh.vertices.is_empty());
        assert!(!analytic.points.is_empty());
        let rebuilt = cache
            .cached_stroke_mesh(&path, Transform::IDENTITY, StrokeStyle::new(2.0), 1.0)
            .unwrap();
        assert_eq!(rebuilt.vertices.len(), mesh.vertices.len());
        assert!(!Arc::ptr_eq(&rebuilt, &mesh));
    }
}
