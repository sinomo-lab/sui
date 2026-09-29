// Exported from the SUI theme editor, based on the SUI dark preset.
use sui::{Color, ControlSize, DecorativeColors, DefaultTheme, NeutralRamp, ThemeColorScheme, ThemeColors, ThemeMotion, ThemeRadii, ThemeTextScale, ThemeTextToken};

pub fn custom_theme() -> DefaultTheme {
    let colors = ThemeColors {
        name: "custom",
        scheme: ThemeColorScheme::Dark,
        neutrals: NeutralRamp {
            window: Color::rgba(0.03529412, 0.050980393, 0.078431375, 1.0), // #090D14
            subtle: Color::rgba(0.050980393, 0.07058824, 0.09803922, 1.0), // #0D1219
            panel: Color::rgba(0.10980392, 0.12156863, 0.14901961, 1.0), // #1C1F26
            overlay: Color::rgba(0.09803922, 0.11764706, 0.14509805, 1.0), // #191E25
            control: Color::rgba(0.10980392, 0.12941177, 0.15686275, 1.0), // #1C2128
            control_hover: Color::rgba(0.14509805, 0.16470589, 0.19607843, 1.0), // #252A32
            control_active: Color::rgba(0.16862746, 0.19215687, 0.22352941, 1.0), // #2B3139
            button: Color::rgba(0.10980392, 0.12941177, 0.15686275, 1.0), // #1C2128
            button_hover: Color::rgba(0.14509805, 0.16470589, 0.19607843, 1.0), // #252A32
            button_active: Color::rgba(0.16862746, 0.19215687, 0.22352941, 1.0), // #2B3139
            field: Color::rgba(0.050980393, 0.07058824, 0.09803922, 1.0), // #0D1219
            border_subtle: Color::rgba(0.12156863, 0.14509805, 0.1764706, 1.0), // #1F252D
            border: Color::rgba(0.16862746, 0.19607843, 0.23529412, 1.0), // #2B323C
            border_strong: Color::rgba(0.23137255, 0.25882354, 0.29803923, 1.0), // #3B424C
            border_control: Color::rgba(0.42352942, 0.45490196, 0.49803922, 1.0), // #6C747F
            text: Color::rgba(0.93333334, 0.9411765, 0.9529412, 1.0), // #EEF0F3
            text_secondary: Color::rgba(0.7254902, 0.74509805, 0.7764706, 1.0), // #B9BEC6
            text_tertiary: Color::rgba(0.5568628, 0.5803922, 0.61960787, 1.0), // #8E949E
            text_disabled: Color::rgba(0.34117648, 0.3647059, 0.39607844, 1.0), // #575D65
        },
        primary: Color::rgba(0.05882353, 0.5411765, 0.37254903, 1.0), // #0F8A5F
        on_primary: Color::rgba(1.0, 1.0, 1.0, 1.0), // #FFFFFF
        secondary: Color::rgba(0.49019608, 0.3019608, 0.90588236, 1.0), // #7D4DE7
        on_secondary: Color::rgba(1.0, 1.0, 1.0, 1.0), // #FFFFFF
        info: Color::rgba(0.0, 0.6509804, 0.87058824, 1.0), // #00A6DE
        on_info: Color::rgba(0.007843138, 0.16470589, 0.23921569, 1.0), // #022A3D
        success: Color::rgba(0.023529412, 0.65882355, 0.30588236, 1.0), // #06A84E
        on_success: Color::rgba(0.047058824, 0.18039216, 0.08627451, 1.0), // #0C2E16
        warning: Color::rgba(0.95686275, 0.64705884, 0.0, 1.0), // #F4A500
        on_warning: Color::rgba(0.24705882, 0.14509805, 0.0, 1.0), // #3F2500
        danger: Color::rgba(0.8627451, 0.14901961, 0.15294118, 1.0), // #DC2627
        on_danger: Color::rgba(1.0, 1.0, 1.0, 1.0), // #FFFFFF
        decorative: DecorativeColors {
            red: Color::rgba(1.0, 0.44313726, 0.41960785, 1.0), // #FF716B
            orange: Color::rgba(1.0, 0.5411765, 0.21568628, 1.0), // #FF8A37
            amber: Color::rgba(0.9882353, 0.69411767, 0.0, 1.0), // #FCB100
            green: Color::rgba(0.21568628, 0.81960785, 0.42352942, 1.0), // #37D16C
            teal: Color::rgba(0.0, 0.8156863, 0.73333335, 1.0), // #00D0BB
            cyan: Color::rgba(0.0, 0.7882353, 0.91764706, 1.0), // #00C9EA
            blue: Color::rgba(0.40784314, 0.64705884, 1.0, 1.0), // #68A5FF
            violet: Color::rgba(0.65882355, 0.56078434, 1.0, 1.0), // #A88FFF
            magenta: Color::rgba(0.89411765, 0.44705883, 0.8627451, 1.0), // #E472DC
        },
    };
    let mut theme = DefaultTheme::from_colors(colors);
    theme.spacing = 5.0;
    theme.radius = ThemeRadii {
        xs: 3.0,
        sm: 6.0,
        md: 9.0,
        lg: 12.0,
        xl: 15.0,
        _2xl: 21.0,
        _3xl: 27.0,
        _4xl: 999.0,
    };
    theme.text = ThemeTextScale {
        xs: ThemeTextToken { size: 13.200001, line_height: 18.7 },
        sm: ThemeTextToken { size: 14.3, line_height: 19.800001 },
        base: ThemeTextToken { size: 16.5, line_height: 24.2 },
        lg: ThemeTextToken { size: 18.7, line_height: 27.5 },
        xl: ThemeTextToken { size: 20.9, line_height: 29.7 },
        _2xl: ThemeTextToken { size: 23.1, line_height: 31.900002 },
        _3xl: ThemeTextToken { size: 27.5, line_height: 36.3 },
        _4xl: ThemeTextToken { size: 34.100002, line_height: 42.9 },
        _5xl: ThemeTextToken { size: 40.7, line_height: 49.5 },
        _6xl: ThemeTextToken { size: 53.9, line_height: 60.5 },
        _7xl: ThemeTextToken { size: 67.1, line_height: 73.700005 },
        _8xl: ThemeTextToken { size: 80.3, line_height: 86.9 },
        _9xl: ThemeTextToken { size: 106.700005, line_height: 113.3 },
    };
    let mut motion = ThemeMotion::standard();
    motion.duration_fast *= 0.5;
    motion.duration_normal *= 0.5;
    motion.duration_slow *= 0.5;
    motion.duration_slower *= 0.5;
    theme.motion = motion;
    theme.sync_derived_fields();
    let mut theme = theme.with_size(ControlSize::Large);
    theme.palette.caret = Color::rgba(1.0, 0.3529412, 0.12156863, 1.0); // #FF5A1F
    theme
}
