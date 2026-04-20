use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use sui_core::{Color, Error, Rect, Result};
use sui_render_wgpu::{HdrRgbaImage, RgbaImage};

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

pub fn write_hdr_exr(image: &HdrRgbaImage, path: impl AsRef<Path>) -> Result<()> {
    use exr::prelude::write_rgba_file;

    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }

    write_rgba_file(
        path,
        image.width() as usize,
        image.height() as usize,
        |x, y| {
            let offset = (y * image.width() as usize + x) * 4;
            let pixels = image.pixels();
            (
                pixels[offset],
                pixels[offset + 1],
                pixels[offset + 2],
                pixels[offset + 3],
            )
        },
    )
    .map_err(exr_error)
}

pub fn write_hdr_avif(
    image: &HdrRgbaImage,
    path: impl AsRef<Path>,
    sdr_white_level: f32,
) -> Result<()> {
    let reference_white = if sdr_white_level.is_finite() && sdr_white_level > 0.0 {
        sdr_white_level
    } else {
        1.0
    };
    let mut pixels = Vec::with_capacity((image.width() * image.height()) as usize);
    for rgba in image.pixels().chunks_exact(4) {
        pixels.push(ravif::RGBA8::new(
            linear_hdr_channel_to_avif_u8(rgba[0], reference_white),
            linear_hdr_channel_to_avif_u8(rgba[1], reference_white),
            linear_hdr_channel_to_avif_u8(rgba[2], reference_white),
            linear_hdr_alpha_to_u8(rgba[3]),
        ));
    }
    let encoded = ravif::Encoder::new()
        .with_quality(80.0)
        .with_alpha_quality(80.0)
        .with_speed(4)
        .with_bit_depth(ravif::BitDepth::Ten)
        .encode_rgba(ravif::Img::new(
            pixels.as_slice(),
            image.width() as usize,
            image.height() as usize,
        ))
        .map_err(avif_error)?;
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    fs::write(path, encoded.avif_file).map_err(io_error)
}

pub fn hdr_luminance_heatmap(image: &HdrRgbaImage) -> Result<Screenshot> {
    let mut pixels = Vec::with_capacity((image.width() * image.height() * 4) as usize);
    for rgba in image.pixels().chunks_exact(4) {
        let luminance = (rgba[0] * 0.2126 + rgba[1] * 0.7152 + rgba[2] * 0.0722).max(0.0);
        let normalized = (luminance / (1.0 + luminance)).clamp(0.0, 1.0);
        let value = (normalized * 255.0).round() as u8;
        pixels.extend_from_slice(&[value, value, value, 255]);
    }
    Screenshot::new(image.width(), image.height(), pixels)
}

pub fn hdr_headroom_heatmap(image: &HdrRgbaImage, sdr_white_level: f32) -> Result<Screenshot> {
    let reference_white = if sdr_white_level.is_finite() && sdr_white_level > 0.0 {
        sdr_white_level
    } else {
        1.0
    };
    let mut pixels = Vec::with_capacity((image.width() * image.height() * 4) as usize);
    for rgba in image.pixels().chunks_exact(4) {
        let headroom = (rgba[0].max(rgba[1]).max(rgba[2]) / reference_white).max(0.0);
        let normalized = (headroom / (1.0 + headroom)).clamp(0.0, 1.0);
        let red = (normalized * 255.0).round() as u8;
        let blue = ((1.0 - normalized) * 96.0).round() as u8;
        pixels.extend_from_slice(&[red, 32, blue, 255]);
    }
    Screenshot::new(image.width(), image.height(), pixels)
}

pub fn hdr_clip_mask(image: &HdrRgbaImage, threshold: f32) -> Result<Screenshot> {
    let clip_threshold = if threshold.is_finite() && threshold > 0.0 {
        threshold
    } else {
        1.0
    };
    let mut pixels = Vec::with_capacity((image.width() * image.height() * 4) as usize);
    for rgba in image.pixels().chunks_exact(4) {
        let clipped = rgba[0].max(rgba[1]).max(rgba[2]) > clip_threshold;
        if clipped {
            pixels.extend_from_slice(&[255, 64, 64, 255]);
        } else {
            pixels.extend_from_slice(&[0, 0, 0, 255]);
        }
    }
    Screenshot::new(image.width(), image.height(), pixels)
}

impl Screenshot {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self> {
        let expected_len = width as usize * height as usize * 4;
        if pixels.len() != expected_len {
            return Err(Error::new(format!(
                "screenshot pixel buffer length {} does not match {}x{} image size",
                pixels.len(),
                width,
                height
            )));
        }

        Ok(Self { width, height, pixels })
    }

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

    pub fn write_avif(&self, path: impl AsRef<Path>) -> Result<()> {
        let pixels = self
            .pixels
            .chunks_exact(4)
            .map(|rgba| ravif::RGBA8::new(rgba[0], rgba[1], rgba[2], rgba[3]))
            .collect::<Vec<_>>();
        let encoded = ravif::Encoder::new()
            .with_quality(80.0)
            .with_alpha_quality(80.0)
            .with_speed(4)
            .with_bit_depth(ravif::BitDepth::Ten)
            .encode_rgba(ravif::Img::new(
                pixels.as_slice(),
                self.width as usize,
                self.height as usize,
            ))
            .map_err(avif_error)?;
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        fs::write(path, encoded.avif_file).map_err(io_error)
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

fn avif_error<E>(error: E) -> Error
where
    E: std::fmt::Display,
{
    Error::new(format!("AVIF error: {error}"))
}

fn exr_error<E>(error: E) -> Error
where
    E: std::fmt::Display,
{
    Error::new(format!("EXR error: {error}"))
}

fn linear_hdr_channel_to_avif_u8(channel: f32, reference_white: f32) -> u8 {
    let normalized = (channel.max(0.0) / reference_white.max(f32::EPSILON)).max(0.0);
    let tone_mapped = normalized / (1.0 + normalized);
    linear_to_srgb_u8(tone_mapped)
}

fn linear_hdr_alpha_to_u8(alpha: f32) -> u8 {
    (alpha.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn linear_to_srgb_u8(linear: f32) -> u8 {
    let encoded = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        (1.055 * linear.powf(1.0 / 2.4)) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

    use super::{
        HdrRgbaImage, Screenshot, hdr_clip_mask, hdr_headroom_heatmap, hdr_luminance_heatmap,
        write_hdr_avif,
    };

    #[test]
    fn hdr_luminance_heatmap_returns_rgba8_image() {
        let image = HdrRgbaImage::new(1, 1, vec![4.0, 2.0, 1.0, 1.0]).unwrap();
        let heatmap = hdr_luminance_heatmap(&image).unwrap();
        assert_eq!(heatmap.width(), 1);
        assert_eq!(heatmap.height(), 1);
        assert_eq!(heatmap.pixels().len(), 4);
        assert_eq!(heatmap.pixels()[3], 255);
    }

    #[test]
    fn hdr_headroom_heatmap_highlights_overbright_pixels() {
        let image = HdrRgbaImage::new(1, 1, vec![2.0, 0.5, 0.5, 1.0]).unwrap();
        let heatmap = hdr_headroom_heatmap(&image, 1.0).unwrap();
        assert!(heatmap.pixels()[0] > heatmap.pixels()[2]);
    }

    #[test]
    fn hdr_clip_mask_marks_pixels_above_threshold() {
        let image = HdrRgbaImage::new(2, 1, vec![0.5, 0.5, 0.5, 1.0, 3.0, 0.0, 0.0, 1.0]).unwrap();
        let mask = hdr_clip_mask(&image, 1.0).unwrap();
        assert_eq!(&mask.pixels()[0..4], &[0, 0, 0, 255]);
        assert_eq!(&mask.pixels()[4..8], &[255, 64, 64, 255]);
    }

    #[test]
    fn screenshot_new_rejects_wrong_length() {
        assert!(Screenshot::new(2, 2, vec![0; 4]).is_err());
    }

    #[test]
    fn screenshot_write_avif_creates_nonempty_file() {
        let screenshot = Screenshot::new(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
        let path = unique_test_path("screenshot-avif");

        screenshot.write_avif(&path).unwrap();

        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[4..8], b"ftyp");
    }

    #[test]
    fn write_hdr_avif_creates_nonempty_file() {
        let image = HdrRgbaImage::new(2, 1, vec![4.0, 2.0, 1.0, 1.0, 0.5, 0.5, 0.5, 1.0]).unwrap();
        let path = unique_test_path("hdr-avif");

        write_hdr_avif(&image, &path, 1.0).unwrap();

        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[4..8], b"ftyp");
    }

    fn unique_test_path(stem: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{stem}-{nanos}.avif"))
    }
}
