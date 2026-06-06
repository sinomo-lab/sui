#![forbid(unsafe_code)]

pub const LUCIDE_VERSION: &str = "1.17.0";
pub const LUCIDE_IMAGE_HANDLE_BASE: u64 = 0x4c55_4349_0000_0000;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

pub fn icon_named(name: &str) -> Option<LucideIcon> {
    ALL_ICONS.iter().copied().find(|icon| icon.name() == name)
}

pub fn register_icon(
    application: &mut sui_runtime::Application,
    icon: LucideIcon,
) -> sui_core::Result<()> {
    application.register_embedded_svg_image(icon.resource())
}

pub fn register_icons(
    application: &mut sui_runtime::Application,
    icons: impl IntoIterator<Item = LucideIcon>,
) -> sui_core::Result<()> {
    application.register_embedded_svg_images(icons.into_iter().map(LucideIcon::resource))
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
