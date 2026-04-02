use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use sui_core::{Color, Error, Rect, Result};
use sui_render_wgpu::RgbaImage;

use crate::snapshot::WindowSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screenshot {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactBundle {
    pub snapshot: WindowSnapshot,
    pub screenshot: Option<Screenshot>,
    pub semantics_overlay: Option<Screenshot>,
    pub widget_overlay: Option<Screenshot>,
}

impl Screenshot {
    pub(crate) fn from_rgba_image(image: RgbaImage) -> Self {
        Self {
            width: image.width(),
            height: image.height(),
            pixels: image.into_pixels(),
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn write_png(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }

        let file = File::create(path).map_err(io_error)?;
        let writer = BufWriter::new(file);
        let mut encoder = png::Encoder::new(writer, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(png_error)?;
        writer.write_image_data(&self.pixels).map_err(png_error)
    }

    pub fn crop(&self, bounds: Rect) -> Result<Self> {
        let left = bounds.x().floor().max(0.0) as u32;
        let top = bounds.y().floor().max(0.0) as u32;
        let right = bounds.max_x().ceil().max(0.0) as u32;
        let bottom = bounds.max_y().ceil().max(0.0) as u32;
        let clamped_right = right.min(self.width);
        let clamped_bottom = bottom.min(self.height);

        if left >= clamped_right || top >= clamped_bottom {
            return Err(Error::new(
                "cannot capture screenshot for an empty or out-of-bounds region",
            ));
        }

        let width = clamped_right - left;
        let height = clamped_bottom - top;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for y in top..clamped_bottom {
            let row_start = ((y * self.width + left) * 4) as usize;
            let row_end = row_start + (width * 4) as usize;
            pixels.extend_from_slice(&self.pixels[row_start..row_end]);
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }
}

impl ArtifactBundle {
    pub fn write_to_dir(&self, dir: impl AsRef<Path>) -> Result<()> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir).map_err(io_error)?;

        fs::write(dir.join("summary.txt"), format_summary(&self.snapshot)).map_err(io_error)?;
        fs::write(dir.join("semantics.txt"), format_semantics(&self.snapshot)).map_err(io_error)?;
        fs::write(
            dir.join("widget-graph.txt"),
            format_widget_graph(&self.snapshot),
        )
        .map_err(io_error)?;

        if let Some(scene) = &self.snapshot.scene_summary {
            fs::write(dir.join("scene.txt"), format_scene(scene)).map_err(io_error)?;
        }
        if let Some(screenshot) = &self.screenshot {
            screenshot.write_png(dir.join("screenshot.png"))?;
        }
        if let Some(overlay) = &self.semantics_overlay {
            overlay.write_png(dir.join("semantics-overlay.png"))?;
        }
        if let Some(overlay) = &self.widget_overlay {
            overlay.write_png(dir.join("widget-overlay.png"))?;
        }

        Ok(())
    }
}

pub(crate) fn read_png(path: &Path) -> Result<Screenshot> {
    let file = File::open(path).map_err(io_error)?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().map_err(png_error)?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).map_err(png_error)?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(Error::new("expected an RGBA8 PNG screenshot baseline"));
    }

    Ok(Screenshot {
        width: info.width,
        height: info.height,
        pixels: buffer[..info.buffer_size()].to_vec(),
    })
}

pub(crate) fn diff_screenshot(expected: &Screenshot, actual: &Screenshot) -> Result<Screenshot> {
    let width = expected.width.max(actual.width);
    let height = expected.height.max(actual.height);
    let mut pixels = vec![0; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let expected_px = pixel_at(expected, x, y).unwrap_or([0, 0, 0, 0]);
            let actual_px = pixel_at(actual, x, y).unwrap_or([0, 0, 0, 0]);
            let offset = ((y * width + x) * 4) as usize;
            let rgba = if expected_px == actual_px {
                [actual_px[0] / 2, actual_px[1] / 2, actual_px[2] / 2, 255]
            } else {
                [255, actual_px[1] / 3, actual_px[2] / 3, 255]
            };
            pixels[offset..offset + 4].copy_from_slice(&rgba);
        }
    }

    Ok(Screenshot {
        width,
        height,
        pixels,
    })
}

pub(crate) fn screenshot_mismatch_paths(path: &Path) -> (PathBuf, PathBuf) {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("screenshot");
    let actual = parent.join(format!("{stem}.actual.png"));
    let diff = parent.join(format!("{stem}.diff.png"));
    (actual, diff)
}

pub(crate) fn semantics_overlay(base: &Screenshot, snapshot: &WindowSnapshot) -> Screenshot {
    let mut overlay = base.clone();
    for node in &snapshot.accessibility.nodes {
        let color = if node.state.focused {
            Color::rgba(1.0, 0.2, 0.2, 1.0)
        } else {
            Color::rgba(0.2, 1.0, 0.3, 1.0)
        };
        draw_rect_outline(&mut overlay, node.bounds, color);
    }
    overlay
}

pub(crate) fn widget_overlay(base: &Screenshot, snapshot: &WindowSnapshot) -> Screenshot {
    let mut overlay = base.clone();
    for node in &snapshot.widget_graph.nodes {
        let color = if node.focused {
            Color::rgba(1.0, 0.6, 0.1, 1.0)
        } else {
            Color::rgba(0.2, 0.6, 1.0, 1.0)
        };
        draw_rect_outline(&mut overlay, node.bounds, color);
    }
    overlay
}

fn format_summary(snapshot: &WindowSnapshot) -> String {
    let mut lines = vec![
        format!("window: {} ({})", snapshot.title, snapshot.window_id.get()),
        format!(
            "focus: {:?}",
            snapshot.focus_state.focused_widget.map(|id| id.get())
        ),
        format!("semantics nodes: {}", snapshot.accessibility.nodes.len()),
        format!("widget graph nodes: {}", snapshot.widget_graph.nodes.len()),
    ];

    if let Some(scene) = &snapshot.scene_summary {
        lines.push(format!(
            "scene: viewport=({}, {}), dirty_regions={}, commands={}",
            scene.viewport.width,
            scene.viewport.height,
            scene.dirty_regions.len(),
            scene.command_count
        ));
    }

    lines.join("\n")
}

fn format_semantics(snapshot: &WindowSnapshot) -> String {
    snapshot
        .accessibility
        .nodes
        .iter()
        .map(|node| {
            format!(
                "id={} parent={:?} role={:?} name={:?} description={:?} value={:?} bounds=({}, {}, {}, {}) focused={} hidden={}",
                node.id.get(),
                node.parent.map(|id| id.get()),
                node.role,
                node.name,
                node.description,
                node.value,
                node.bounds.x(),
                node.bounds.y(),
                node.bounds.width(),
                node.bounds.height(),
                node.state.focused,
                node.state.hidden,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_widget_graph(snapshot: &WindowSnapshot) -> String {
    snapshot
        .widget_graph
        .nodes
        .iter()
        .map(|node| {
            format!(
                "id={} parent={:?} children={:?} bounds=({}, {}, {}, {}) focusable={} focused={}",
                node.id.get(),
                node.parent.map(|id| id.get()),
                node.children.iter().map(|id| id.get()).collect::<Vec<_>>(),
                node.bounds.x(),
                node.bounds.y(),
                node.bounds.width(),
                node.bounds.height(),
                node.accepts_focus,
                node.focused,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_scene(scene: &crate::snapshot::SceneSummary) -> String {
    format!(
        "viewport=({}, {})\ndirty_regions={:?}\ncommand_count={}\ncommand_breakdown={:?}\n",
        scene.viewport.width,
        scene.viewport.height,
        scene.dirty_regions,
        scene.command_count,
        scene.command_breakdown,
    )
}

fn draw_rect_outline(image: &mut Screenshot, bounds: Rect, color: Color) {
    let left = bounds.x().floor().max(0.0) as i32;
    let top = bounds.y().floor().max(0.0) as i32;
    let right = bounds.max_x().ceil().min(image.width as f32) as i32 - 1;
    let bottom = bounds.max_y().ceil().min(image.height as f32) as i32 - 1;

    if left > right || top > bottom {
        return;
    }

    for x in left..=right {
        blend_pixel(image, x, top, color);
        blend_pixel(image, x, bottom, color);
    }
    for y in top..=bottom {
        blend_pixel(image, left, y, color);
        blend_pixel(image, right, y, color);
    }
}

fn blend_pixel(image: &mut Screenshot, x: i32, y: i32, color: Color) {
    if x < 0 || y < 0 || x >= image.width as i32 || y >= image.height as i32 {
        return;
    }

    let offset = ((y as u32 * image.width + x as u32) * 4) as usize;
    let overlay = [
        (color.red * 255.0) as u8,
        (color.green * 255.0) as u8,
        (color.blue * 255.0) as u8,
        (color.alpha * 255.0) as u8,
    ];
    let alpha = overlay[3] as f32 / 255.0;

    for channel in 0..3 {
        let base = image.pixels[offset + channel] as f32;
        let over = overlay[channel] as f32;
        image.pixels[offset + channel] = ((base * (1.0 - alpha)) + (over * alpha)) as u8;
    }
    image.pixels[offset + 3] = 255;
}

fn pixel_at(image: &Screenshot, x: u32, y: u32) -> Option<[u8; 4]> {
    if x >= image.width || y >= image.height {
        return None;
    }

    let offset = ((y * image.width + x) * 4) as usize;
    Some([
        image.pixels[offset],
        image.pixels[offset + 1],
        image.pixels[offset + 2],
        image.pixels[offset + 3],
    ])
}

fn io_error(error: std::io::Error) -> Error {
    Error::new(format!("I/O error: {error}"))
}

fn png_error<E>(error: E) -> Error
where
    E: std::fmt::Display,
{
    Error::new(format!("PNG error: {error}"))
}
