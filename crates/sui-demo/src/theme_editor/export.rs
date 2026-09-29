//! "Copy as Rust": a function that rebuilds the edited theme with the same
//! steps as [`ThemeRecipe::build`].

use std::collections::BTreeSet;
use std::fmt::Write;

use sui::ColorSpace;
use sui::prelude::*;

use super::state::{ThemeRecipe, scaled_radii, scaled_text_scale};
use super::tokens::{SourceContainer, SourceToken, TokenGroup};

/// Rust source for `fn custom_theme() -> DefaultTheme`.
pub(super) fn rust_source(recipe: &ThemeRecipe) -> String {
    let mut imports = BTreeSet::from([
        "Color",
        "ControlSize",
        "DecorativeColors",
        "DefaultTheme",
        "NeutralRamp",
        "ThemeColorScheme",
        "ThemeColors",
    ]);
    let mut body = String::new();
    let colors = &recipe.colors;

    writeln!(body, "    let colors = ThemeColors {{").unwrap();
    writeln!(body, "        name: \"custom\",").unwrap();
    writeln!(
        body,
        "        scheme: ThemeColorScheme::{},",
        scheme_variant(colors.scheme)
    )
    .unwrap();
    writeln!(body, "        neutrals: NeutralRamp {{").unwrap();
    for token in tokens_in(SourceContainer::Neutrals) {
        write_field(
            &mut body,
            12,
            token.field(),
            token.get(colors),
            &mut imports,
        );
    }
    writeln!(body, "        }},").unwrap();
    for token in tokens_in(SourceContainer::Colors) {
        write_field(&mut body, 8, token.field(), token.get(colors), &mut imports);
    }
    writeln!(body, "        decorative: DecorativeColors {{").unwrap();
    for token in tokens_in(SourceContainer::Decorative) {
        write_field(
            &mut body,
            12,
            token.field(),
            token.get(colors),
            &mut imports,
        );
    }
    writeln!(body, "        }},").unwrap();
    writeln!(body, "    }};").unwrap();

    writeln!(
        body,
        "    let mut theme = DefaultTheme::from_colors(colors);"
    )
    .unwrap();
    if recipe.shape_changed() {
        imports.extend(["ThemeMotion", "ThemeRadii", "ThemeTextScale"]);
        writeln!(body, "    theme.spacing = {};", number(recipe.spacing)).unwrap();
        write_radii(&mut body, recipe.radius_scale);
        write_text(&mut body, recipe.text_scale, &mut imports);
        write_motion(&mut body, recipe.motion_scale);
        writeln!(body, "    theme.sync_derived_fields();").unwrap();
    }
    writeln!(
        body,
        "    let mut theme = theme.with_size(ControlSize::{:?});",
        recipe.control_size
    )
    .unwrap();
    for (role, color) in &recipe.overrides {
        writeln!(
            body,
            "    theme.palette.{} = {};{}",
            role.field(),
            color_expression(*color, &mut imports),
            hex_comment(*color)
        )
        .unwrap();
    }
    writeln!(body, "    theme").unwrap();

    let mut source = String::new();
    writeln!(
        source,
        "// Exported from the SUI theme editor, based on the {} preset.",
        recipe.preset.label()
    )
    .unwrap();
    writeln!(
        source,
        "use sui::{{{}}};",
        imports.into_iter().collect::<Vec<_>>().join(", ")
    )
    .unwrap();
    writeln!(source).unwrap();
    writeln!(source, "pub fn custom_theme() -> DefaultTheme {{").unwrap();
    source.push_str(&body);
    source.push_str("}\n");
    source
}

fn tokens_in(container: SourceContainer) -> impl Iterator<Item = SourceToken> {
    TokenGroup::ALL
        .into_iter()
        .filter(move |group| group.container() == container)
        .flat_map(TokenGroup::tokens)
}

fn write_field(
    body: &mut String,
    indent: usize,
    field: &str,
    color: Color,
    imports: &mut BTreeSet<&'static str>,
) {
    writeln!(
        body,
        "{:indent$}{field}: {},{}",
        "",
        color_expression(color, imports),
        hex_comment(color)
    )
    .unwrap();
}

fn write_radii(body: &mut String, scale: f32) {
    if scale == 1.0 {
        writeln!(body, "    theme.radius = ThemeRadii::default();").unwrap();
        return;
    }
    let radii = scaled_radii(scale);
    writeln!(body, "    theme.radius = ThemeRadii {{").unwrap();
    for (name, value) in [
        ("xs", radii.xs),
        ("sm", radii.sm),
        ("md", radii.md),
        ("lg", radii.lg),
        ("xl", radii.xl),
        ("_2xl", radii._2xl),
        ("_3xl", radii._3xl),
        ("_4xl", radii._4xl),
    ] {
        writeln!(body, "        {name}: {},", number(value)).unwrap();
    }
    writeln!(body, "    }};").unwrap();
}

fn write_text(body: &mut String, scale: f32, imports: &mut BTreeSet<&'static str>) {
    if scale == 1.0 {
        writeln!(body, "    theme.text = ThemeTextScale::default();").unwrap();
        return;
    }
    imports.insert("ThemeTextToken");
    let text = scaled_text_scale(scale);
    writeln!(body, "    theme.text = ThemeTextScale {{").unwrap();
    for (name, token) in [
        ("xs", text.xs),
        ("sm", text.sm),
        ("base", text.base),
        ("lg", text.lg),
        ("xl", text.xl),
        ("_2xl", text._2xl),
        ("_3xl", text._3xl),
        ("_4xl", text._4xl),
        ("_5xl", text._5xl),
        ("_6xl", text._6xl),
        ("_7xl", text._7xl),
        ("_8xl", text._8xl),
        ("_9xl", text._9xl),
    ] {
        writeln!(
            body,
            "        {name}: ThemeTextToken {{ size: {}, line_height: {} }},",
            number(token.size),
            number(token.line_height)
        )
        .unwrap();
    }
    writeln!(body, "    }};").unwrap();
}

fn write_motion(body: &mut String, scale: f32) {
    if scale == 1.0 {
        writeln!(body, "    theme.motion = ThemeMotion::standard();").unwrap();
        return;
    }
    writeln!(body, "    let mut motion = ThemeMotion::standard();").unwrap();
    for field in [
        "duration_fast",
        "duration_normal",
        "duration_slow",
        "duration_slower",
    ] {
        writeln!(body, "    motion.{field} *= {};", number(scale)).unwrap();
    }
    writeln!(body, "    theme.motion = motion;").unwrap();
}

fn color_expression(color: Color, imports: &mut BTreeSet<&'static str>) -> String {
    let channels = format!(
        "{}, {}, {}, {}",
        number(color.red),
        number(color.green),
        number(color.blue),
        number(color.alpha)
    );
    match color.space {
        ColorSpace::Srgb => format!("Color::rgba({channels})"),
        space => {
            imports.insert("ColorSpace");
            format!("Color::new(ColorSpace::{space:?}, {channels})")
        }
    }
}

fn hex_comment(color: Color) -> String {
    if color.space == ColorSpace::Srgb {
        format!(" // {}", hex(color))
    } else {
        String::new()
    }
}

/// `#RRGGBB`, or `#RRGGBBAA` when translucent.
pub(super) fn hex(color: Color) -> String {
    let color = color.clamped();
    let byte = |channel: f32| (channel * 255.0).round() as u8;
    let rgb = format!(
        "#{:02X}{:02X}{:02X}",
        byte(color.red),
        byte(color.green),
        byte(color.blue)
    );
    if byte(color.alpha) == 255 {
        rgb
    } else {
        format!("{rgb}{:02X}", byte(color.alpha))
    }
}

/// Parses `#RGB`, `#RRGGBB`, or `#RRGGBBAA`, with or without `#`.
pub(super) fn parse_hex(text: &str) -> Option<Color> {
    let digits = text.trim().trim_start_matches('#');
    if !digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
        return None;
    }
    let pair = |index: usize| u8::from_str_radix(&digits[index..index + 2], 16).ok();
    let [red, green, blue, alpha] = match digits.len() {
        3 => {
            let nibble = |index: usize| {
                u8::from_str_radix(&digits[index..=index], 16)
                    .ok()
                    .map(|value| value * 17)
            };
            [nibble(0)?, nibble(1)?, nibble(2)?, 255]
        }
        6 => [pair(0)?, pair(2)?, pair(4)?, 255],
        8 => [pair(0)?, pair(2)?, pair(4)?, pair(6)?],
        _ => return None,
    };
    let unit = |byte: u8| f32::from(byte) / 255.0;
    Some(Color::rgba(unit(red), unit(green), unit(blue), unit(alpha)))
}

/// The shortest float literal that reads back as exactly `value`, so the
/// exported code rebuilds the same theme.
fn number(value: f32) -> String {
    format!("{value:?}")
}

fn scheme_variant(scheme: ThemeColorScheme) -> &'static str {
    match scheme {
        ThemeColorScheme::Light => "Light",
        ThemeColorScheme::Dark => "Dark",
        ThemeColorScheme::HighContrast => "HighContrast",
    }
}
