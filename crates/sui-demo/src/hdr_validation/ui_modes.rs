//! The same controls under each HDR theme mode, side by side, to show how
//! far each mode lets UI accents rise above SDR white.

use sui::prelude::*;
use sui::{GridTrack, HdrThemeMode};

use crate::app::{DemoTextRole, demo_text_style};
use crate::demo_support::NamedSection;
use crate::theme_demo::{
    EmissiveIndicator, ThemePreviewCardFrame, hdr_theme_lab_theme, hdr_theme_mode_title,
};

pub(crate) const UI_MODES_NAME: &str = "HDR theme mode columns";

const MODES: [HdrThemeMode; 4] = [
    HdrThemeMode::Disabled,
    HdrThemeMode::WideGamutOnly,
    HdrThemeMode::ConstrainedHdr,
    HdrThemeMode::FullHdr,
];

fn mode_card(mode: HdrThemeMode) -> impl Widget {
    let theme = hdr_theme_lab_theme(mode);
    let title = hdr_theme_mode_title(mode);
    ThemePreviewCardFrame::new(
        theme,
        Stack::vertical()
            .spacing(10.0)
            .alignment(Alignment::Start)
            .with_child(Label::new(title).style(demo_text_style(
                theme,
                DemoTextRole::Emphasis,
                theme.palette.text,
            )))
            .with_child(
                Label::new(match mode {
                    HdrThemeMode::Disabled | HdrThemeMode::WideGamutOnly => {
                        "Accents stay at SDR white.".to_string()
                    }
                    HdrThemeMode::ConstrainedHdr | HdrThemeMode::FullHdr => format!(
                        "Accent {:.2}× · indicator {:.2}× · alert {:.2}× SDR white",
                        theme.hdr.luminance.semantic_accent,
                        theme.hdr.luminance.emissive_indicator,
                        theme.hdr.luminance.alert_pulse,
                    ),
                })
                .style(demo_text_style(
                    theme,
                    DemoTextRole::Metadata,
                    theme.palette.placeholder,
                )),
            )
            .with_child(
                Button::primary(format!("{title} action"))
                    .min_width(180.0)
                    .theme(theme),
            )
            .with_child(Switch::new(format!("{title} live")).on(true).theme(theme))
            .with_child(
                Stack::horizontal()
                    .spacing(10.0)
                    .alignment(Alignment::Center)
                    .with_child(EmissiveIndicator::new(
                        format!("{title} emissive indicator"),
                        theme,
                    ))
                    .with_child(Label::new("Emissive indicator").style(demo_text_style(
                        theme,
                        DemoTextRole::Metadata,
                        theme.palette.placeholder,
                    ))),
            ),
    )
}

/// One card per HDR theme mode, two to a row.
pub(crate) fn ui_mode_columns() -> impl Widget {
    let mut grid = Grid::new([GridTrack::Fraction(1.0), GridTrack::Fraction(1.0)])
        .rows([GridTrack::Auto, GridTrack::Auto])
        .gap(14.0);
    for mode in MODES {
        grid.push(mode_card(mode));
    }
    NamedSection::new(UI_MODES_NAME, grid)
}
