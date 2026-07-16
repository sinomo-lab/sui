#![forbid(unsafe_code)]

pub const LUCIDE_VERSION: &str = "1.17.0";
pub const LUCIDE_IMAGE_HANDLE_BASE: u64 = 0x4c55_4349_0000_0000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LucidePathCommand {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CubicTo(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LucidePathData {
    pub stroke: &'static [LucidePathCommand],
    pub fill: &'static [LucidePathCommand],
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

impl LucideIcon {
    pub fn paint(
        self,
        ctx: &mut sui_runtime::PaintCtx,
        bounds: sui_core::Rect,
        color: sui_core::Color,
    ) {
        if bounds.is_empty() {
            return;
        }

        let pixels_per_point = ctx.dpi().pixels_per_point().max(f32::EPSILON);
        let side = bounds.width().min(bounds.height());
        let side = (side * pixels_per_point).round().max(1.0) / pixels_per_point;
        let x = ((bounds.x() + (bounds.width() - side) * 0.5) * pixels_per_point).round()
            / pixels_per_point;
        let y = ((bounds.y() + (bounds.height() - side) * 0.5) * pixels_per_point).round()
            / pixels_per_point;
        let scale = side / 24.0;
        let data = self.path_data();

        if !data.fill.is_empty() {
            ctx.fill(build_path(data.fill, x, y, scale), color);
        }
        if !data.stroke.is_empty() {
            ctx.stroke(
                build_path(data.stroke, x, y, scale),
                color,
                sui_scene::StrokeStyle::new(2.0 * scale)
                    .with_cap(sui_scene::StrokeCap::Round)
                    .with_join(sui_scene::StrokeJoin::Round),
            );
        }
    }

    /// Rasterize a Lucide SVG as a white alpha mask.
    ///
    /// Lucide sources use `currentColor`, which `resvg` resolves to black by default. SUI's
    /// `ImageSource::with_tint` multiplies texture RGB by the tint color, so a black icon texture
    /// stays black no matter which color a widget requests. Using white RGB with the SVG alpha
    /// preserves normal image tint multiplication while making icons colorable.
    pub fn registered_mask_image(self) -> sui_core::Result<sui_scene::RegisteredImage> {
        let image = self.resource().registered_image()?;
        let mut pixels = Vec::with_capacity(image.bytes().len());
        for pixel in image.bytes().chunks_exact(4) {
            let alpha = pixel[3];
            if alpha == 0 {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                pixels.extend_from_slice(&[255, 255, 255, alpha]);
            }
        }
        sui_scene::RegisteredImage::from_rgba8(image.width(), image.height(), pixels)
    }
}

fn build_path(commands: &[LucidePathCommand], x: f32, y: f32, scale: f32) -> sui_core::Path {
    let point = |px: f32, py: f32| sui_core::Point::new(x + px * scale, y + py * scale);
    let mut builder = sui_core::PathBuilder::new();
    for command in commands {
        match *command {
            LucidePathCommand::MoveTo(px, py) => {
                builder.move_to(point(px, py));
            }
            LucidePathCommand::LineTo(px, py) => {
                builder.line_to(point(px, py));
            }
            LucidePathCommand::QuadTo(cx, cy, px, py) => {
                builder.quad_to(point(cx, cy), point(px, py));
            }
            LucidePathCommand::CubicTo(c1x, c1y, c2x, c2y, px, py) => {
                builder.cubic_to(point(c1x, c1y), point(c2x, c2y), point(px, py));
            }
            LucidePathCommand::Close => {
                builder.close();
            }
        }
    }
    builder.build()
}

pub fn icon_named(name: &str) -> Option<LucideIcon> {
    ALL_ICONS.iter().copied().find(|icon| icon.name() == name)
}

pub fn register_icon(
    application: &mut sui_runtime::Application,
    icon: LucideIcon,
) -> sui_core::Result<()> {
    application.register_image(icon.handle(), icon.registered_mask_image()?)
}

pub fn register_icons(
    application: &mut sui_runtime::Application,
    icons: impl IntoIterator<Item = LucideIcon>,
) -> sui_core::Result<()> {
    for icon in icons {
        register_icon(application, icon)?;
    }
    Ok(())
}

pub fn register_all(application: &mut sui_runtime::Application) -> sui_core::Result<()> {
    register_icons(application, ALL_ICONS.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_icons_expose_stable_handles_and_svg_data() {
        let plus = LucideIcon::Plus;
        assert_eq!(plus.name(), "plus");
        assert_eq!(plus.file_name(), "plus.svg");
        assert_eq!(plus.handle().get(), LUCIDE_IMAGE_HANDLE_BASE + plus as u64);
        assert!(plus.svg().starts_with(b"<svg"));
    }

    #[test]
    fn icon_lookup_finds_local_assets_by_kebab_name() {
        assert_eq!(icon_named("zoom-in"), Some(LucideIcon::ZoomIn));
        assert_eq!(icon_named("not-a-lucide-icon"), None);
    }

    #[test]
    fn generated_icons_expose_native_stroke_and_fill_paths() {
        let search = LucideIcon::Search.path_data();
        assert!(search.fill.is_empty());
        assert!(search.stroke.len() >= 8);
        assert!(matches!(
            search.stroke.first(),
            Some(LucidePathCommand::MoveTo(_, _))
        ));

        let scatter = LucideIcon::ChartScatter.path_data();
        assert!(!scatter.stroke.is_empty());
        assert!(!scatter.fill.is_empty());
    }

    #[test]
    fn register_subset_rasterizes_svg_resources() {
        let mut app = sui_runtime::Application::new();
        register_icons(&mut app, [LucideIcon::Plus, LucideIcon::Search]).unwrap();
        let runtime = app
            .window(sui_runtime::WindowBuilder::new().root(Empty))
            .build()
            .unwrap();

        let plus = runtime
            .image_registry()
            .get(LucideIcon::Plus.handle())
            .unwrap();
        let search = runtime
            .image_registry()
            .get(LucideIcon::Search.handle())
            .unwrap();
        assert_eq!(plus.width(), 24);
        assert_eq!(plus.height(), 24);
        assert_eq!(search.width(), 24);
        assert_eq!(search.height(), 24);
    }

    #[test]
    fn registered_mask_image_uses_white_rgb_for_tinting() {
        let image = LucideIcon::Sparkles.registered_mask_image().unwrap();
        let opaque_pixel = image
            .bytes()
            .chunks_exact(4)
            .find(|pixel| pixel[3] > 0)
            .expect("lucide icon should have visible pixels");

        assert_eq!(&opaque_pixel[0..3], &[255, 255, 255]);
    }

    struct Empty;

    impl sui_runtime::Widget for Empty {
        fn measure(
            &mut self,
            _ctx: &mut sui_runtime::MeasureCtx,
            _constraints: sui_layout::Constraints,
        ) -> sui_core::Size {
            sui_core::Size::new(1.0, 1.0)
        }
    }
}
