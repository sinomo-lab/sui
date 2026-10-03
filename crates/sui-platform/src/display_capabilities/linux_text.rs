//! Linux text smoothing preferences. Fontconfig is optional at runtime; Xft
//! resources take precedence for X11 windows. Never guess a panel's RGB order.
use std::{
    ffi::{c_char, c_int, c_void},
    ptr::NonNull,
    sync::OnceLock,
};

use libloading::Library;
use sui_render_wgpu::TextSubpixelOrder;
use x11rb::resource_manager::Database;

#[derive(Default)]
struct Preference {
    antialias: Option<bool>,
    order: Option<TextSubpixelOrder>,
}

impl Preference {
    fn resolve(self, fallback: Self) -> TextSubpixelOrder {
        if self.antialias.or(fallback.antialias) == Some(false) {
            return TextSubpixelOrder::None;
        }
        self.order.or(fallback.order).unwrap_or_default()
    }
}

fn xft_preference(database: &Database) -> Preference {
    let antialias = database
        .get_string("Xft.antialias", "Xft.Antialias")
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        });
    let order = database
        .get_string("Xft.rgba", "Xft.Rgba")
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "rgb" => Some(TextSubpixelOrder::Rgb),
            "bgr" => Some(TextSubpixelOrder::Bgr),
            // SUI currently samples only horizontal subpixels.
            "none" | "vrgb" | "vbgr" => Some(TextSubpixelOrder::None),
            _ => None,
        });
    Preference { antialias, order }
}

fn fontconfig_order(rgba: c_int) -> Option<TextSubpixelOrder> {
    match rgba {
        1 => Some(TextSubpixelOrder::Rgb),
        2 => Some(TextSubpixelOrder::Bgr),
        3..=5 => Some(TextSubpixelOrder::None),
        _ => None,
    }
}

pub(super) fn query_text_subpixel_order(x11: bool) -> TextSubpixelOrder {
    static FONTCONFIG: OnceLock<Option<Fontconfig>> = OnceLock::new();
    let fallback = FONTCONFIG
        .get_or_init(Fontconfig::load)
        .as_ref()
        .map(Fontconfig::preference)
        .unwrap_or_default();
    let xft = if x11 {
        x11rb::connect(None).ok().and_then(|(connection, _)| {
            x11rb::resource_manager::new_from_default(&connection)
                .ok()
                .map(|database| xft_preference(&database))
        })
    } else {
        None
    };
    xft.unwrap_or_default().resolve(fallback)
}

// Opaque Fontconfig objects never escape this module. Keep the library loaded
// for the lifetime of its function pointers and destroy every owned pattern.
struct Fontconfig {
    _library: Library,
    name_parse: unsafe extern "C" fn(*const u8) -> *mut c_void,
    substitute: unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> c_int,
    defaults: unsafe extern "C" fn(*mut c_void),
    font_match: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_int) -> *mut c_void,
    get_integer: unsafe extern "C" fn(*const c_void, *const c_char, c_int, *mut c_int) -> c_int,
    get_bool: unsafe extern "C" fn(*const c_void, *const c_char, c_int, *mut c_int) -> c_int,
    destroy: unsafe extern "C" fn(*mut c_void),
}

struct Pattern<'a> {
    pointer: NonNull<c_void>,
    api: &'a Fontconfig,
}

impl Drop for Pattern<'_> {
    fn drop(&mut self) {
        // SAFETY: this pattern is uniquely owned and its library is still loaded.
        unsafe { (self.api.destroy)(self.pointer.as_ptr()) }
    }
}

impl Fontconfig {
    fn load() -> Option<Self> {
        // SAFETY: these are Fontconfig's documented C ABI signatures. Missing
        // libraries or symbols disable detection without adding a link dependency.
        unsafe {
            let library = Library::new("libfontconfig.so.1").ok()?;
            Some(Self {
                name_parse: *library.get(b"FcNameParse\0").ok()?,
                substitute: *library.get(b"FcConfigSubstitute\0").ok()?,
                defaults: *library.get(b"FcDefaultSubstitute\0").ok()?,
                font_match: *library.get(b"FcFontMatch\0").ok()?,
                get_integer: *library.get(b"FcPatternGetInteger\0").ok()?,
                get_bool: *library.get(b"FcPatternGetBool\0").ok()?,
                destroy: *library.get(b"FcPatternDestroy\0").ok()?,
                _library: library,
            })
        }
    }

    fn preference(&self) -> Preference {
        // SAFETY: null config selects Fontconfig's current configuration. Both
        // parsed and matched patterns are checked for null and owned by guards.
        unsafe {
            let Some(pointer) = NonNull::new((self.name_parse)(c"sans-serif".as_ptr().cast()))
            else {
                return Preference::default();
            };
            let pattern = Pattern { pointer, api: self };
            if (self.substitute)(std::ptr::null_mut(), pattern.pointer.as_ptr(), 0) == 0 {
                return Preference::default();
            }
            (self.defaults)(pattern.pointer.as_ptr());
            let mut result = 0;
            let Some(pointer) = NonNull::new((self.font_match)(
                std::ptr::null_mut(),
                pattern.pointer.as_ptr(),
                &mut result,
            )) else {
                return Preference::default();
            };
            let matched = Pattern { pointer, api: self };
            let mut rgba = 0;
            let order =
                ((self.get_integer)(matched.pointer.as_ptr(), c"rgba".as_ptr(), 0, &mut rgba) == 0)
                    .then(|| fontconfig_order(rgba))
                    .flatten();
            let mut enabled = 0;
            let antialias = ((self.get_bool)(
                matched.pointer.as_ptr(),
                c"antialias".as_ptr(),
                0,
                &mut enabled,
            ) == 0)
                .then_some(enabled != 0);
            Preference { antialias, order }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xft_resources_override_fontconfig_without_guessing_unsupported_orders() {
        for (resources, expected) in [
            ("Xft.rgba: rgb\n", TextSubpixelOrder::Rgb),
            ("Xft.rgba: BGR\n", TextSubpixelOrder::Bgr),
            ("Xft.rgba: none\n", TextSubpixelOrder::None),
            ("Xft.rgba: vrgb\n", TextSubpixelOrder::None),
            ("Xft.rgba: vbgr\n", TextSubpixelOrder::None),
            ("Xft.rgba: unknown\n", TextSubpixelOrder::Bgr),
            (
                "Xft.antialias: false\nXft.rgba: rgb\n",
                TextSubpixelOrder::None,
            ),
            ("Xft.antialias: 0\n", TextSubpixelOrder::None),
            ("Xft.antialias: true\n", TextSubpixelOrder::Bgr),
            ("", TextSubpixelOrder::Bgr),
        ] {
            let database = Database::new_from_data(resources.as_bytes());
            assert_eq!(
                xft_preference(&database).resolve(Preference {
                    antialias: Some(true),
                    order: Some(TextSubpixelOrder::Bgr),
                }),
                expected,
                "{resources}"
            );
        }
        assert_eq!(
            Preference::default().resolve(Preference::default()),
            TextSubpixelOrder::None
        );
        assert_eq!(
            xft_preference(&Database::new_from_data(b"Xft.rgba: rgb\n")).resolve(Preference {
                antialias: Some(false),
                order: None
            }),
            TextSubpixelOrder::None
        );
    }

    #[test]
    fn fontconfig_only_enables_supported_explicit_panel_orders() {
        assert_eq!(fontconfig_order(1), Some(TextSubpixelOrder::Rgb));
        assert_eq!(fontconfig_order(2), Some(TextSubpixelOrder::Bgr));
        for rgba in [3, 4, 5] {
            assert_eq!(fontconfig_order(rgba), Some(TextSubpixelOrder::None));
        }
        for rgba in [-1, 0, 6] {
            assert_eq!(fontconfig_order(rgba), None);
        }
    }
}
