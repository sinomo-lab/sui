use sui_core::Result;
use sui_scene::RegisteredImage;

pub const DEFAULT_SUI_LOGO_SVG: &[u8] = include_bytes!("../assets/sui-logo.svg");

pub fn default_sui_logo_image(size: u32) -> Result<RegisteredImage> {
    RegisteredImage::from_svg_at_size(size, size, DEFAULT_SUI_LOGO_SVG)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_SUI_LOGO_SVG, default_sui_logo_image};

    #[test]
    fn default_sui_logo_asset_is_layered_wave_svg() {
        let svg = std::str::from_utf8(DEFAULT_SUI_LOGO_SVG).unwrap();

        assert!(svg.contains("SUI logo"));
        assert!(svg.contains("layered filled waves"));
        assert!(svg.contains("#27B7C8"));
        assert!(svg.contains("#0B355C"));
    }

    #[test]
    fn default_sui_logo_image_rasterizes_from_svg_asset() {
        let image = default_sui_logo_image(64).unwrap();

        assert_eq!(image.width(), 64);
        assert_eq!(image.height(), 64);
        assert!(image.bytes().chunks_exact(4).any(|pixel| pixel[3] > 0));
    }
}
