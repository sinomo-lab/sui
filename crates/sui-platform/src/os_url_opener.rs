use sui_core::UrlOpenerBackend;

/// Opens URLs with the program the system registers for their scheme, using
/// the `open` crate. The desktop platform installs it into the runtime.
///
/// Each URL launches on a background thread so a slow launcher never stalls
/// the UI. It reports whether that launch started; a launcher that fails
/// afterwards goes unreported.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsUrlOpenerBackend;

impl OsUrlOpenerBackend {
    pub fn new() -> Self {
        Self
    }
}

impl UrlOpenerBackend for OsUrlOpenerBackend {
    fn open(&mut self, url: &str) -> bool {
        let url = url.to_string();
        std::thread::Builder::new()
            .name("sui-open-url".to_string())
            .spawn(move || {
                let _ = open::that_detached(url);
            })
            .is_ok()
    }
}
