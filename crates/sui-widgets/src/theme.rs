use palette::{FromColor, Oklch, Srgb};
use sui_core::Color;
use sui_layout::Padding as Insets;
use sui_text::TextStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeFontStack {
    pub primary: &'static str,
    pub fallbacks: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeFontFamilies {
    pub sans: ThemeFontStack,
    pub serif: ThemeFontStack,
    pub mono: ThemeFontStack,
}

impl Default for ThemeFontFamilies {
    fn default() -> Self {
        Self {
            sans: ThemeFontStack {
                primary: "ui-sans-serif",
                fallbacks: &[
                    "system-ui",
                    "sans-serif",
                    "Apple Color Emoji",
                    "Segoe UI Emoji",
                    "Segoe UI Symbol",
                    "Noto Color Emoji",
                ],
            },
            serif: ThemeFontStack {
                primary: "ui-serif",
                fallbacks: &[
                    "Georgia",
                    "Cambria",
                    "Times New Roman",
                    "Times",
                    "serif",
                ],
            },
            mono: ThemeFontStack {
                primary: "ui-monospace",
                fallbacks: &[
                    "SFMono-Regular",
                    "Menlo",
                    "Monaco",
                    "Consolas",
                    "Liberation Mono",
                    "Courier New",
                    "monospace",
                ],
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColorScale {
    pub _50: Color,
    pub _100: Color,
    pub _200: Color,
    pub _300: Color,
    pub _400: Color,
    pub _500: Color,
    pub _600: Color,
    pub _700: Color,
    pub _800: Color,
    pub _900: Color,
    pub _950: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    pub red: ThemeColorScale,
    pub orange: ThemeColorScale,
    pub amber: ThemeColorScale,
    pub yellow: ThemeColorScale,
    pub lime: ThemeColorScale,
    pub green: ThemeColorScale,
    pub emerald: ThemeColorScale,
    pub teal: ThemeColorScale,
    pub cyan: ThemeColorScale,
    pub sky: ThemeColorScale,
    pub blue: ThemeColorScale,
    pub indigo: ThemeColorScale,
    pub violet: ThemeColorScale,
    pub purple: ThemeColorScale,
    pub fuchsia: ThemeColorScale,
    pub pink: ThemeColorScale,
    pub rose: ThemeColorScale,
    pub slate: ThemeColorScale,
    pub gray: ThemeColorScale,
    pub zinc: ThemeColorScale,
    pub neutral: ThemeColorScale,
    pub stone: ThemeColorScale,
    pub mauve: ThemeColorScale,
    pub olive: ThemeColorScale,
    pub mist: ThemeColorScale,
    pub taupe: ThemeColorScale,
    pub black: Color,
    pub white: Color,
}

macro_rules! scale {
    ($($shade:ident => ($l:expr, $c:expr, $h:expr)),+ $(,)?) => {
        ThemeColorScale {
            $($shade: oklch($l, $c, $h)),+
        }
    };
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            red: scale!(
                _50 => (97.1, 0.013, 17.38),
                _100 => (93.6, 0.032, 17.717),
                _200 => (88.5, 0.062, 18.334),
                _300 => (80.8, 0.114, 19.571),
                _400 => (70.4, 0.191, 22.216),
                _500 => (63.7, 0.237, 25.331),
                _600 => (57.7, 0.245, 27.325),
                _700 => (50.5, 0.213, 27.518),
                _800 => (44.4, 0.177, 26.899),
                _900 => (39.6, 0.141, 25.723),
                _950 => (25.8, 0.092, 26.042)
            ),
            orange: scale!(
                _50 => (98.0, 0.016, 73.684),
                _100 => (95.4, 0.038, 75.164),
                _200 => (90.1, 0.076, 70.697),
                _300 => (83.7, 0.128, 66.29),
                _400 => (75.0, 0.183, 55.934),
                _500 => (70.5, 0.213, 47.604),
                _600 => (64.6, 0.222, 41.116),
                _700 => (55.3, 0.195, 38.402),
                _800 => (47.0, 0.157, 37.304),
                _900 => (40.8, 0.123, 38.172),
                _950 => (26.6, 0.079, 36.259)
            ),
            amber: scale!(
                _50 => (98.7, 0.022, 95.277),
                _100 => (96.2, 0.059, 95.617),
                _200 => (92.4, 0.12, 95.746),
                _300 => (87.9, 0.169, 91.605),
                _400 => (82.8, 0.189, 84.429),
                _500 => (76.9, 0.188, 70.08),
                _600 => (66.6, 0.179, 58.318),
                _700 => (55.5, 0.163, 48.998),
                _800 => (47.3, 0.137, 46.201),
                _900 => (41.4, 0.112, 45.904),
                _950 => (27.9, 0.077, 45.635)
            ),
            yellow: scale!(
                _50 => (98.7, 0.026, 102.212),
                _100 => (97.3, 0.071, 103.193),
                _200 => (94.5, 0.129, 101.54),
                _300 => (90.5, 0.182, 98.111),
                _400 => (85.2, 0.199, 91.936),
                _500 => (79.5, 0.184, 86.047),
                _600 => (68.1, 0.162, 75.834),
                _700 => (55.4, 0.135, 66.442),
                _800 => (47.6, 0.114, 61.907),
                _900 => (42.1, 0.095, 57.708),
                _950 => (28.6, 0.066, 53.813)
            ),
            lime: scale!(
                _50 => (98.6, 0.031, 120.757),
                _100 => (96.7, 0.067, 122.328),
                _200 => (93.8, 0.127, 124.321),
                _300 => (89.7, 0.196, 126.665),
                _400 => (84.1, 0.238, 128.85),
                _500 => (76.8, 0.233, 130.85),
                _600 => (64.8, 0.2, 131.684),
                _700 => (53.2, 0.157, 131.589),
                _800 => (45.3, 0.124, 130.933),
                _900 => (40.5, 0.101, 131.063),
                _950 => (27.4, 0.072, 132.109)
            ),
            green: scale!(
                _50 => (98.2, 0.018, 155.826),
                _100 => (96.2, 0.044, 156.743),
                _200 => (92.5, 0.084, 155.995),
                _300 => (87.1, 0.15, 154.449),
                _400 => (79.2, 0.209, 151.711),
                _500 => (72.3, 0.219, 149.579),
                _600 => (62.7, 0.194, 149.214),
                _700 => (52.7, 0.154, 150.069),
                _800 => (44.8, 0.119, 151.328),
                _900 => (39.3, 0.095, 152.535),
                _950 => (26.6, 0.065, 152.934)
            ),
            emerald: scale!(
                _50 => (97.9, 0.021, 166.113),
                _100 => (95.0, 0.052, 163.051),
                _200 => (90.5, 0.093, 164.15),
                _300 => (84.5, 0.143, 164.978),
                _400 => (76.5, 0.177, 163.223),
                _500 => (69.6, 0.17, 162.48),
                _600 => (59.6, 0.145, 163.225),
                _700 => (50.8, 0.118, 165.612),
                _800 => (43.2, 0.095, 166.913),
                _900 => (37.8, 0.077, 168.94),
                _950 => (26.2, 0.051, 172.552)
            ),
            teal: scale!(
                _50 => (98.4, 0.014, 180.72),
                _100 => (95.3, 0.051, 180.801),
                _200 => (91.0, 0.096, 180.426),
                _300 => (85.5, 0.138, 181.071),
                _400 => (77.7, 0.152, 181.912),
                _500 => (70.4, 0.14, 182.503),
                _600 => (60.0, 0.118, 184.704),
                _700 => (51.1, 0.096, 186.391),
                _800 => (43.7, 0.078, 188.216),
                _900 => (38.6, 0.063, 188.416),
                _950 => (27.7, 0.046, 192.524)
            ),
            cyan: scale!(
                _50 => (98.4, 0.019, 200.873),
                _100 => (95.6, 0.045, 203.388),
                _200 => (91.7, 0.08, 205.041),
                _300 => (86.5, 0.127, 207.078),
                _400 => (78.9, 0.154, 211.53),
                _500 => (71.5, 0.143, 215.221),
                _600 => (60.9, 0.126, 221.723),
                _700 => (52.0, 0.105, 223.128),
                _800 => (45.0, 0.085, 224.283),
                _900 => (39.8, 0.07, 227.392),
                _950 => (30.2, 0.056, 229.695)
            ),
            sky: scale!(
                _50 => (97.7, 0.013, 236.62),
                _100 => (95.1, 0.026, 236.824),
                _200 => (90.1, 0.058, 230.902),
                _300 => (82.8, 0.111, 230.318),
                _400 => (74.6, 0.16, 232.661),
                _500 => (68.5, 0.169, 237.323),
                _600 => (58.8, 0.158, 241.966),
                _700 => (50.0, 0.134, 242.749),
                _800 => (44.3, 0.11, 240.79),
                _900 => (39.1, 0.09, 240.876),
                _950 => (29.3, 0.066, 243.157)
            ),
            blue: scale!(
                _50 => (97.0, 0.014, 254.604),
                _100 => (93.2, 0.032, 255.585),
                _200 => (88.2, 0.059, 254.128),
                _300 => (80.9, 0.105, 251.813),
                _400 => (70.7, 0.165, 254.624),
                _500 => (62.3, 0.214, 259.815),
                _600 => (54.6, 0.245, 262.881),
                _700 => (48.8, 0.243, 264.376),
                _800 => (42.4, 0.199, 265.638),
                _900 => (37.9, 0.146, 265.522),
                _950 => (28.2, 0.091, 267.935)
            ),
            indigo: scale!(
                _50 => (96.2, 0.018, 272.314),
                _100 => (93.0, 0.034, 272.788),
                _200 => (87.0, 0.065, 274.039),
                _300 => (78.5, 0.115, 274.713),
                _400 => (67.3, 0.182, 276.935),
                _500 => (58.5, 0.233, 277.117),
                _600 => (51.1, 0.262, 276.966),
                _700 => (45.7, 0.24, 277.023),
                _800 => (39.8, 0.195, 277.366),
                _900 => (35.9, 0.144, 278.697),
                _950 => (25.7, 0.09, 281.288)
            ),
            violet: scale!(
                _50 => (96.9, 0.016, 293.756),
                _100 => (94.3, 0.029, 294.588),
                _200 => (89.4, 0.057, 293.283),
                _300 => (81.1, 0.111, 293.571),
                _400 => (70.2, 0.183, 293.541),
                _500 => (60.6, 0.25, 292.717),
                _600 => (54.1, 0.281, 293.009),
                _700 => (49.1, 0.27, 292.581),
                _800 => (43.2, 0.232, 292.759),
                _900 => (38.0, 0.189, 293.745),
                _950 => (28.3, 0.141, 291.089)
            ),
            purple: scale!(
                _50 => (97.7, 0.014, 308.299),
                _100 => (94.6, 0.033, 307.174),
                _200 => (90.2, 0.063, 306.703),
                _300 => (82.7, 0.119, 306.383),
                _400 => (71.4, 0.203, 305.504),
                _500 => (62.7, 0.265, 303.9),
                _600 => (55.8, 0.288, 302.321),
                _700 => (49.6, 0.265, 301.924),
                _800 => (43.8, 0.218, 303.724),
                _900 => (38.1, 0.176, 304.987),
                _950 => (29.1, 0.149, 302.717)
            ),
            fuchsia: scale!(
                _50 => (97.7, 0.017, 320.058),
                _100 => (95.2, 0.037, 318.852),
                _200 => (90.3, 0.076, 319.62),
                _300 => (83.3, 0.145, 321.434),
                _400 => (74.0, 0.238, 322.16),
                _500 => (66.7, 0.295, 322.15),
                _600 => (59.1, 0.293, 322.896),
                _700 => (51.8, 0.253, 323.949),
                _800 => (45.2, 0.211, 324.591),
                _900 => (40.1, 0.17, 325.612),
                _950 => (29.3, 0.136, 325.661)
            ),
            pink: scale!(
                _50 => (97.1, 0.014, 343.198),
                _100 => (94.8, 0.028, 342.258),
                _200 => (89.9, 0.061, 343.231),
                _300 => (82.3, 0.12, 346.018),
                _400 => (71.8, 0.202, 349.761),
                _500 => (65.6, 0.241, 354.308),
                _600 => (59.2, 0.249, 0.584),
                _700 => (52.5, 0.223, 3.958),
                _800 => (45.9, 0.187, 3.815),
                _900 => (40.8, 0.153, 2.432),
                _950 => (28.4, 0.109, 3.907)
            ),
            rose: scale!(
                _50 => (96.9, 0.015, 12.422),
                _100 => (94.1, 0.03, 12.58),
                _200 => (89.2, 0.058, 10.001),
                _300 => (81.0, 0.117, 11.638),
                _400 => (71.2, 0.194, 13.428),
                _500 => (64.5, 0.246, 16.439),
                _600 => (58.6, 0.253, 17.585),
                _700 => (51.4, 0.222, 16.935),
                _800 => (45.5, 0.188, 13.697),
                _900 => (41.0, 0.159, 10.272),
                _950 => (27.1, 0.105, 12.094)
            ),
            slate: scale!(
                _50 => (98.4, 0.003, 247.858),
                _100 => (96.8, 0.007, 247.896),
                _200 => (92.9, 0.013, 255.508),
                _300 => (86.9, 0.022, 252.894),
                _400 => (70.4, 0.04, 256.788),
                _500 => (55.4, 0.046, 257.417),
                _600 => (44.6, 0.043, 257.281),
                _700 => (37.2, 0.044, 257.287),
                _800 => (27.9, 0.041, 260.031),
                _900 => (20.8, 0.042, 265.755),
                _950 => (12.9, 0.042, 264.695)
            ),
            gray: scale!(
                _50 => (98.5, 0.002, 247.839),
                _100 => (96.7, 0.003, 264.542),
                _200 => (92.8, 0.006, 264.531),
                _300 => (87.2, 0.01, 258.338),
                _400 => (70.7, 0.022, 261.325),
                _500 => (55.1, 0.027, 264.364),
                _600 => (44.6, 0.03, 256.802),
                _700 => (37.3, 0.034, 259.733),
                _800 => (27.8, 0.033, 256.848),
                _900 => (21.0, 0.034, 264.665),
                _950 => (13.0, 0.028, 261.692)
            ),
            zinc: scale!(
                _50 => (98.5, 0.0, 0.0),
                _100 => (96.7, 0.001, 286.375),
                _200 => (92.0, 0.004, 286.32),
                _300 => (87.1, 0.006, 286.286),
                _400 => (70.5, 0.015, 286.067),
                _500 => (55.2, 0.016, 285.938),
                _600 => (44.2, 0.017, 285.786),
                _700 => (37.0, 0.013, 285.805),
                _800 => (27.4, 0.006, 286.033),
                _900 => (21.0, 0.006, 285.885),
                _950 => (14.1, 0.005, 285.823)
            ),
            neutral: scale!(
                _50 => (98.5, 0.0, 0.0),
                _100 => (97.0, 0.0, 0.0),
                _200 => (92.2, 0.0, 0.0),
                _300 => (87.0, 0.0, 0.0),
                _400 => (70.8, 0.0, 0.0),
                _500 => (55.6, 0.0, 0.0),
                _600 => (43.9, 0.0, 0.0),
                _700 => (37.1, 0.0, 0.0),
                _800 => (26.9, 0.0, 0.0),
                _900 => (20.5, 0.0, 0.0),
                _950 => (14.5, 0.0, 0.0)
            ),
            stone: scale!(
                _50 => (98.5, 0.001, 106.423),
                _100 => (97.0, 0.001, 106.424),
                _200 => (92.3, 0.003, 48.717),
                _300 => (86.9, 0.005, 56.366),
                _400 => (70.9, 0.01, 56.259),
                _500 => (55.3, 0.013, 58.071),
                _600 => (44.4, 0.011, 73.639),
                _700 => (37.4, 0.01, 67.558),
                _800 => (26.8, 0.007, 34.298),
                _900 => (21.6, 0.006, 56.043),
                _950 => (14.7, 0.004, 49.25)
            ),
            mauve: scale!(
                _50 => (98.5, 0.0, 0.0),
                _100 => (96.0, 0.003, 325.6),
                _200 => (92.2, 0.005, 325.62),
                _300 => (86.5, 0.012, 325.68),
                _400 => (71.1, 0.019, 323.02),
                _500 => (54.2, 0.034, 322.5),
                _600 => (43.5, 0.029, 321.78),
                _700 => (36.4, 0.029, 323.89),
                _800 => (26.3, 0.024, 320.12),
                _900 => (21.2, 0.019, 322.12),
                _950 => (14.5, 0.008, 326.0)
            ),
            olive: scale!(
                _50 => (98.8, 0.003, 106.5),
                _100 => (96.6, 0.005, 106.5),
                _200 => (93.0, 0.007, 106.5),
                _300 => (88.0, 0.011, 106.6),
                _400 => (73.7, 0.021, 106.9),
                _500 => (58.0, 0.031, 107.3),
                _600 => (46.6, 0.025, 107.3),
                _700 => (39.4, 0.023, 107.4),
                _800 => (28.6, 0.016, 107.4),
                _900 => (22.8, 0.013, 107.4),
                _950 => (15.3, 0.006, 107.1)
            ),
            mist: scale!(
                _50 => (98.7, 0.002, 197.1),
                _100 => (96.3, 0.002, 197.1),
                _200 => (92.5, 0.005, 214.3),
                _300 => (87.2, 0.007, 219.6),
                _400 => (72.3, 0.014, 214.4),
                _500 => (56.0, 0.021, 213.5),
                _600 => (45.0, 0.017, 213.2),
                _700 => (37.8, 0.015, 216.0),
                _800 => (27.5, 0.011, 216.9),
                _900 => (21.8, 0.008, 223.9),
                _950 => (14.8, 0.004, 228.8)
            ),
            taupe: scale!(
                _50 => (98.6, 0.002, 67.8),
                _100 => (96.0, 0.002, 17.2),
                _200 => (92.2, 0.005, 34.3),
                _300 => (86.8, 0.007, 39.5),
                _400 => (71.4, 0.014, 41.2),
                _500 => (54.7, 0.021, 43.1),
                _600 => (43.8, 0.017, 39.3),
                _700 => (36.7, 0.016, 35.7),
                _800 => (26.8, 0.011, 36.5),
                _900 => (21.4, 0.009, 43.1),
                _950 => (14.7, 0.004, 49.3)
            ),
            black: Color::BLACK,
            white: Color::WHITE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeBreakpoints {
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub _2xl: f32,
}

impl Default for ThemeBreakpoints {
    fn default() -> Self {
        Self {
            sm: 640.0,
            md: 768.0,
            lg: 1024.0,
            xl: 1280.0,
            _2xl: 1536.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeContainers {
    pub _3xs: f32,
    pub _2xs: f32,
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub _2xl: f32,
    pub _3xl: f32,
    pub _4xl: f32,
    pub _5xl: f32,
    pub _6xl: f32,
    pub _7xl: f32,
}

impl Default for ThemeContainers {
    fn default() -> Self {
        Self {
            _3xs: 256.0,
            _2xs: 288.0,
            xs: 320.0,
            sm: 384.0,
            md: 448.0,
            lg: 512.0,
            xl: 576.0,
            _2xl: 672.0,
            _3xl: 768.0,
            _4xl: 896.0,
            _5xl: 1024.0,
            _6xl: 1152.0,
            _7xl: 1280.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTextToken {
    pub size: f32,
    pub line_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTextScale {
    pub xs: ThemeTextToken,
    pub sm: ThemeTextToken,
    pub base: ThemeTextToken,
    pub lg: ThemeTextToken,
    pub xl: ThemeTextToken,
    pub _2xl: ThemeTextToken,
    pub _3xl: ThemeTextToken,
    pub _4xl: ThemeTextToken,
    pub _5xl: ThemeTextToken,
    pub _6xl: ThemeTextToken,
    pub _7xl: ThemeTextToken,
    pub _8xl: ThemeTextToken,
    pub _9xl: ThemeTextToken,
}

impl Default for ThemeTextScale {
    fn default() -> Self {
        Self {
            xs: ThemeTextToken {
                size: 12.0,
                line_height: 16.0,
            },
            sm: ThemeTextToken {
                size: 14.0,
                line_height: 20.0,
            },
            base: ThemeTextToken {
                size: 16.0,
                line_height: 24.0,
            },
            lg: ThemeTextToken {
                size: 18.0,
                line_height: 28.0,
            },
            xl: ThemeTextToken {
                size: 20.0,
                line_height: 28.0,
            },
            _2xl: ThemeTextToken {
                size: 24.0,
                line_height: 32.0,
            },
            _3xl: ThemeTextToken {
                size: 30.0,
                line_height: 36.0,
            },
            _4xl: ThemeTextToken {
                size: 36.0,
                line_height: 40.0,
            },
            _5xl: ThemeTextToken {
                size: 48.0,
                line_height: 48.0,
            },
            _6xl: ThemeTextToken {
                size: 60.0,
                line_height: 60.0,
            },
            _7xl: ThemeTextToken {
                size: 72.0,
                line_height: 72.0,
            },
            _8xl: ThemeTextToken {
                size: 96.0,
                line_height: 96.0,
            },
            _9xl: ThemeTextToken {
                size: 128.0,
                line_height: 128.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeFontWeights {
    pub thin: u16,
    pub extralight: u16,
    pub light: u16,
    pub normal: u16,
    pub medium: u16,
    pub semibold: u16,
    pub bold: u16,
    pub extrabold: u16,
    pub black: u16,
}

impl Default for ThemeFontWeights {
    fn default() -> Self {
        Self {
            thin: 100,
            extralight: 200,
            light: 300,
            normal: 400,
            medium: 500,
            semibold: 600,
            bold: 700,
            extrabold: 800,
            black: 900,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTracking {
    pub tighter: f32,
    pub tight: f32,
    pub normal: f32,
    pub wide: f32,
    pub wider: f32,
    pub widest: f32,
}

impl Default for ThemeTracking {
    fn default() -> Self {
        Self {
            tighter: -0.05,
            tight: -0.025,
            normal: 0.0,
            wide: 0.025,
            wider: 0.05,
            widest: 0.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeLeading {
    pub tight: f32,
    pub snug: f32,
    pub normal: f32,
    pub relaxed: f32,
    pub loose: f32,
}

impl Default for ThemeLeading {
    fn default() -> Self {
        Self {
            tight: 1.25,
            snug: 1.375,
            normal: 1.5,
            relaxed: 1.625,
            loose: 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeRadii {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub _2xl: f32,
    pub _3xl: f32,
    pub _4xl: f32,
}

impl Default for ThemeRadii {
    fn default() -> Self {
        Self {
            xs: 2.0,
            sm: 4.0,
            md: 6.0,
            lg: 8.0,
            xl: 12.0,
            _2xl: 16.0,
            _3xl: 24.0,
            _4xl: 32.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeShadowLayer {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
    pub inset: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeShadow {
    pub first: Option<ThemeShadowLayer>,
    pub second: Option<ThemeShadowLayer>,
}

impl ThemeShadow {
    pub const fn empty() -> Self {
        Self {
            first: None,
            second: None,
        }
    }

    pub const fn single(layer: ThemeShadowLayer) -> Self {
        Self {
            first: Some(layer),
            second: None,
        }
    }

    pub const fn double(first: ThemeShadowLayer, second: ThemeShadowLayer) -> Self {
        Self {
            first: Some(first),
            second: Some(second),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeBoxShadowScale {
    pub _2xs: ThemeShadow,
    pub xs: ThemeShadow,
    pub sm: ThemeShadow,
    pub md: ThemeShadow,
    pub lg: ThemeShadow,
    pub xl: ThemeShadow,
    pub _2xl: ThemeShadow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeInsetShadowScale {
    pub _2xs: ThemeShadow,
    pub xs: ThemeShadow,
    pub sm: ThemeShadow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeDropShadowScale {
    pub xs: ThemeShadow,
    pub sm: ThemeShadow,
    pub md: ThemeShadow,
    pub lg: ThemeShadow,
    pub xl: ThemeShadow,
    pub _2xl: ThemeShadow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTextShadowScale {
    pub _2xs: ThemeShadow,
    pub xs: ThemeShadow,
    pub sm: ThemeShadow,
    pub md: ThemeShadow,
    pub lg: ThemeShadow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeShadows {
    pub box_shadow: ThemeBoxShadowScale,
    pub inset: ThemeInsetShadowScale,
    pub drop: ThemeDropShadowScale,
    pub text: ThemeTextShadowScale,
}

impl Default for ThemeShadows {
    fn default() -> Self {
        let black_005 = Color::BLACK.with_alpha(0.05);
        let black_01 = Color::BLACK.with_alpha(0.1);
        let black_012 = Color::BLACK.with_alpha(0.12);
        let black_015 = Color::BLACK.with_alpha(0.15);
        let black_02 = Color::BLACK.with_alpha(0.2);
        let black_025 = Color::BLACK.with_alpha(0.25);
        let black_075 = Color::BLACK.with_alpha(0.075);

        Self {
            box_shadow: ThemeBoxShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 0.0, 0.0, black_005, false)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 2.0, 0.0, black_005, false)),
                sm: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 3.0, 0.0, black_01, false),
                    shadow_layer(0.0, 1.0, 2.0, -1.0, black_01, false),
                ),
                md: ThemeShadow::double(
                    shadow_layer(0.0, 4.0, 6.0, -1.0, black_01, false),
                    shadow_layer(0.0, 2.0, 4.0, -2.0, black_01, false),
                ),
                lg: ThemeShadow::double(
                    shadow_layer(0.0, 10.0, 15.0, -3.0, black_01, false),
                    shadow_layer(0.0, 4.0, 6.0, -4.0, black_01, false),
                ),
                xl: ThemeShadow::double(
                    shadow_layer(0.0, 20.0, 25.0, -5.0, black_01, false),
                    shadow_layer(0.0, 8.0, 10.0, -6.0, black_01, false),
                ),
                _2xl: ThemeShadow::single(shadow_layer(0.0, 25.0, 50.0, -12.0, black_025, false)),
            },
            inset: ThemeInsetShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 0.0, 0.0, black_005, true)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, black_005, true)),
                sm: ThemeShadow::single(shadow_layer(0.0, 2.0, 4.0, 0.0, black_005, true)),
            },
            drop: ThemeDropShadowScale {
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, black_005, false)),
                sm: ThemeShadow::single(shadow_layer(0.0, 1.0, 2.0, 0.0, black_015, false)),
                md: ThemeShadow::single(shadow_layer(0.0, 3.0, 3.0, 0.0, black_012, false)),
                lg: ThemeShadow::single(shadow_layer(0.0, 4.0, 4.0, 0.0, black_015, false)),
                xl: ThemeShadow::single(shadow_layer(0.0, 9.0, 7.0, 0.0, black_01, false)),
                _2xl: ThemeShadow::single(shadow_layer(0.0, 25.0, 25.0, 0.0, black_015, false)),
            },
            text: ThemeTextShadowScale {
                _2xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 0.0, 0.0, black_015, false)),
                xs: ThemeShadow::single(shadow_layer(0.0, 1.0, 1.0, 0.0, black_02, false)),
                sm: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 0.0, 0.0, black_075, false),
                    shadow_layer(0.0, 1.0, 1.0, 0.0, black_075, false),
                ),
                md: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 1.0, 0.0, black_01, false),
                    shadow_layer(0.0, 2.0, 4.0, 0.0, black_01, false),
                ),
                lg: ThemeShadow::double(
                    shadow_layer(0.0, 1.0, 2.0, 0.0, black_01, false),
                    shadow_layer(0.0, 4.0, 8.0, 0.0, black_01, false),
                ),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeBlurScale {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub _2xl: f32,
    pub _3xl: f32,
}

impl Default for ThemeBlurScale {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 12.0,
            lg: 16.0,
            xl: 24.0,
            _2xl: 40.0,
            _3xl: 64.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemePerspective {
    pub dramatic: f32,
    pub near: f32,
    pub normal: f32,
    pub midrange: f32,
    pub distant: f32,
}

impl Default for ThemePerspective {
    fn default() -> Self {
        Self {
            dramatic: 100.0,
            near: 300.0,
            normal: 500.0,
            midrange: 800.0,
            distant: 1200.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeAspectRatios {
    pub video: f32,
}

impl Default for ThemeAspectRatios {
    fn default() -> Self {
        Self { video: 16.0 / 9.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPalette {
    pub text: Color,
    pub placeholder: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub surface_pressed: Color,
    pub surface_focus: Color,
    pub border: Color,
    pub border_hover: Color,
    pub border_focus: Color,
    pub focus_ring: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_pressed: Color,
    pub accent_border: Color,
    pub accent_border_hover: Color,
    pub accent_border_focus: Color,
    pub accent_text: Color,
}

impl ControlPalette {
    pub fn from_colors(colors: &ThemeColors) -> Self {
        Self {
            text: colors.slate._900,
            placeholder: colors.slate._500,
            surface: colors.white,
            surface_hover: colors.sky._50,
            surface_pressed: colors.sky._100,
            surface_focus: colors.blue._50,
            border: colors.slate._300,
            border_hover: colors.slate._400,
            border_focus: colors.blue._600,
            focus_ring: colors.blue._600.with_alpha(0.28),
            accent: colors.blue._600,
            accent_hover: colors.blue._700,
            accent_pressed: colors.blue._800,
            accent_border: colors.blue._700,
            accent_border_hover: colors.blue._800,
            accent_border_focus: colors.blue._500,
            accent_text: colors.white,
        }
    }
}

impl Default for ControlPalette {
    fn default() -> Self {
        Self::from_colors(&ThemeColors::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlTypography {
    pub body_font_size: f32,
    pub body_line_height: f32,
}

impl ControlTypography {
    pub fn from_text_scale(text: &ThemeTextScale) -> Self {
        Self {
            body_font_size: text.sm.size,
            body_line_height: text.sm.line_height,
        }
    }
}

impl Default for ControlTypography {
    fn default() -> Self {
        Self::from_text_scale(&ThemeTextScale::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlMetrics {
    pub min_height: f32,
    pub button_min_width: f32,
    pub button_padding: Insets,
    pub checkbox_padding: Insets,
    pub checkbox_indicator_size: f32,
    pub checkbox_gap: f32,
    pub separator_thickness: f32,
    pub icon_size: f32,
    pub icon_button_size: f32,
    pub switch_track_width: f32,
    pub switch_track_height: f32,
    pub slider_min_width: f32,
    pub slider_track_height: f32,
    pub slider_thumb_size: f32,
    pub number_input_stepper_width: f32,
    pub text_input_min_width: f32,
    pub text_input_padding: Insets,
    pub text_area_min_height: f32,
    pub select_menu_max_height: f32,
    pub corner_radius: f32,
    pub indicator_corner_radius: f32,
    pub border_width: f32,
    pub focus_ring_width: f32,
    pub focus_ring_outset: f32,
    pub caret_width: f32,
}

impl ControlMetrics {
    pub fn from_tokens(spacing: f32, radius: ThemeRadii) -> Self {
        let unit = spacing.max(1.0);
        Self {
            min_height: 40.0,
            button_min_width: 88.0,
            button_padding: Insets {
                left: unit * 3.5,
                top: unit * 2.5,
                right: unit * 3.5,
                bottom: unit * 2.5,
            },
            checkbox_padding: Insets {
                left: unit * 2.5,
                top: unit * 2.0,
                right: unit * 2.5,
                bottom: unit * 2.0,
            },
            checkbox_indicator_size: 18.0,
            checkbox_gap: 10.0,
            separator_thickness: 1.0,
            icon_size: 18.0,
            icon_button_size: 40.0,
            switch_track_width: 38.0,
            switch_track_height: 22.0,
            slider_min_width: 180.0,
            slider_track_height: 4.0,
            slider_thumb_size: 18.0,
            number_input_stepper_width: 32.0,
            text_input_min_width: 240.0,
            text_input_padding: Insets {
                left: unit * 3.0,
                top: unit * 2.5,
                right: unit * 3.0,
                bottom: unit * 2.5,
            },
            text_area_min_height: 120.0,
            select_menu_max_height: 200.0,
            corner_radius: radius.lg,
            indicator_corner_radius: 5.0,
            border_width: 1.0,
            focus_ring_width: 2.0,
            focus_ring_outset: 2.0,
            caret_width: 2.0,
        }
    }
}

impl Default for ControlMetrics {
    fn default() -> Self {
        Self::from_tokens(4.0, ThemeRadii::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefaultTheme {
    pub fonts: ThemeFontFamilies,
    pub colors: ThemeColors,
    pub spacing: f32,
    pub breakpoints: ThemeBreakpoints,
    pub containers: ThemeContainers,
    pub text: ThemeTextScale,
    pub font_weights: ThemeFontWeights,
    pub tracking: ThemeTracking,
    pub leading: ThemeLeading,
    pub radius: ThemeRadii,
    pub shadows: ThemeShadows,
    pub blur: ThemeBlurScale,
    pub perspective: ThemePerspective,
    pub aspect: ThemeAspectRatios,
    pub palette: ControlPalette,
    pub typography: ControlTypography,
    pub metrics: ControlMetrics,
}

impl DefaultTheme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sync_derived_fields(&mut self) {
        self.palette = ControlPalette::from_colors(&self.colors);
        self.typography = ControlTypography::from_text_scale(&self.text);
        self.metrics = ControlMetrics::from_tokens(self.spacing, self.radius);
    }

    pub fn text_style(&self, color: Color) -> TextStyle {
        TextStyle {
            font_size: self.typography.body_font_size.max(1.0),
            line_height: self.typography.body_line_height.max(1.0),
            color,
            ..TextStyle::default()
        }
    }

    pub fn body_text_style(&self) -> TextStyle {
        self.text_style(self.palette.text)
    }

    pub fn placeholder_text_style(&self) -> TextStyle {
        self.text_style(self.palette.placeholder)
    }

    pub fn button_text_style(&self) -> TextStyle {
        self.text_style(self.palette.accent_text)
    }
}

impl Default for DefaultTheme {
    fn default() -> Self {
        let colors = ThemeColors::default();
        let text = ThemeTextScale::default();
        let radius = ThemeRadii::default();
        let spacing = 4.0;

        Self {
            fonts: ThemeFontFamilies::default(),
            colors,
            spacing,
            breakpoints: ThemeBreakpoints::default(),
            containers: ThemeContainers::default(),
            text,
            font_weights: ThemeFontWeights::default(),
            tracking: ThemeTracking::default(),
            leading: ThemeLeading::default(),
            radius,
            shadows: ThemeShadows::default(),
            blur: ThemeBlurScale::default(),
            perspective: ThemePerspective::default(),
            aspect: ThemeAspectRatios::default(),
            palette: ControlPalette::from_colors(&colors),
            typography: ControlTypography::from_text_scale(&text),
            metrics: ControlMetrics::from_tokens(spacing, radius),
        }
    }
}

fn shadow_layer(
    offset_x: f32,
    offset_y: f32,
    blur: f32,
    spread: f32,
    color: Color,
    inset: bool,
) -> ThemeShadowLayer {
    ThemeShadowLayer {
        offset_x,
        offset_y,
        blur,
        spread,
        color,
        inset,
    }
}

fn oklch(lightness_percent: f32, chroma: f32, hue: f32) -> Color {
    let rgb: Srgb = Srgb::from_color(Oklch::new(lightness_percent / 100.0, chroma, hue));
    Color::srgba(
        rgb.red.clamp(0.0, 1.0),
        rgb.green.clamp(0.0, 1.0),
        rgb.blue.clamp(0.0, 1.0),
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use super::{Color, DefaultTheme};

    #[test]
    fn default_theme_uses_tailwind_text_scale_for_body_typography() {
        let theme = DefaultTheme::default();

        assert_eq!(theme.typography.body_font_size, theme.text.sm.size);
        assert_eq!(theme.typography.body_line_height, theme.text.sm.line_height);
    }

    #[test]
    fn sync_derived_fields_updates_semantic_palette_and_typography() {
        let mut theme = DefaultTheme::default();
        theme.colors.blue._600 = Color::rgba(0.2, 0.3, 0.4, 1.0);
        theme.text.sm.size = 15.0;
        theme.text.sm.line_height = 22.0;
        theme.sync_derived_fields();

        assert_eq!(theme.palette.accent, Color::rgba(0.2, 0.3, 0.4, 1.0));
        assert_eq!(theme.typography.body_font_size, 15.0);
        assert_eq!(theme.typography.body_line_height, 22.0);
    }
}