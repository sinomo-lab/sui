use std::{cell::RefCell, fmt, rc::Rc};

/// Driver behind a [`UrlOpener`] handle that hands a URL to the platform.
///
/// The default [`LocalUrlOpenerBackend`] opens nothing, so links stay inert
/// in tests and headless runs. Platforms install a native implementation at
/// startup that opens the default browser, mail client, or whichever app the
/// system registers for the URL's scheme.
pub trait UrlOpenerBackend {
    /// Hand `url` to the platform, returning whether a handler accepted it.
    /// A handler can still fail after accepting it.
    fn open(&mut self, url: &str) -> bool;
}

/// Records URLs instead of opening them; used when no platform backend is
/// installed.
///
/// Clones share one record, so a test can install a clone and read back
/// what widgets asked to open.
#[derive(Debug, Default, Clone)]
pub struct LocalUrlOpenerBackend {
    opened: Rc<RefCell<Vec<String>>>,
}

impl LocalUrlOpenerBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// URLs handed to this backend, oldest first.
    pub fn opened(&self) -> Vec<String> {
        self.opened.borrow().clone()
    }
}

impl UrlOpenerBackend for LocalUrlOpenerBackend {
    fn open(&mut self, url: &str) -> bool {
        self.opened.borrow_mut().push(url.to_string());
        false
    }
}

/// Decides which URLs a [`UrlOpener`] hands to the platform.
///
/// The default, [`UrlPolicy::allows_by_default`], lets through only absolute
/// `http`, `https`, and `mailto` URLs without control characters, so links in
/// untrusted content, such as Markdown from the network or a model, cannot
/// launch local files, scripts, or arbitrary protocol handlers. An app that
/// needs more registers its own check with [`UrlPolicy::new`], or turns the
/// check off with [`UrlPolicy::allow_all`].
#[derive(Clone)]
pub struct UrlPolicy {
    allows: Rc<dyn Fn(&str) -> bool>,
}

impl UrlPolicy {
    /// A policy that lets through the URLs `allows` returns `true` for. Call
    /// [`Self::allows_by_default`] from it to extend the default instead of
    /// replacing it:
    ///
    /// ```ignore
    /// let policy = UrlPolicy::new(|url| {
    ///     url.starts_with("myapp:") || UrlPolicy::allows_by_default(url)
    /// });
    /// ```
    pub fn new(allows: impl Fn(&str) -> bool + 'static) -> Self {
        Self {
            allows: Rc::new(allows),
        }
    }

    /// A policy that lets every URL through. Use it only when the app trusts
    /// every URL it opens.
    pub fn allow_all() -> Self {
        Self::new(|_| true)
    }

    /// Whether this policy lets `url` through.
    pub fn allows(&self, url: &str) -> bool {
        (self.allows)(url)
    }

    /// The default check: whether `url` is an absolute `http`, `https`, or
    /// `mailto` URL without control characters. Relative URLs never pass.
    pub fn allows_by_default(url: &str) -> bool {
        !url.chars().any(char::is_control)
            && url_scheme(url).is_some_and(|scheme| {
                ["http", "https", "mailto"]
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(scheme))
            })
    }
}

impl Default for UrlPolicy {
    fn default() -> Self {
        Self::new(Self::allows_by_default)
    }
}

impl fmt::Debug for UrlPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UrlPolicy").finish_non_exhaustive()
    }
}

/// The scheme of an absolute URL, or `None` for a relative one.
fn url_scheme(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once(':')?;
    let mut chars = scheme.chars();
    let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    let valid = starts_with_letter
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    (valid && !rest.is_empty()).then_some(scheme)
}

/// Shared handle to the URL-opening service.
///
/// The handle trims a URL and hands it to its backend only when its
/// [`UrlPolicy`] allows it. Cloning is cheap and every clone refers to the
/// same backend and policy, so a backend or policy installed through one
/// handle applies to all of them.
#[derive(Clone)]
pub struct UrlOpener {
    inner: Rc<RefCell<UrlOpenerState>>,
}

struct UrlOpenerState {
    backend: Box<dyn UrlOpenerBackend>,
    policy: UrlPolicy,
}

impl UrlOpener {
    pub fn new() -> Self {
        Self::with_backend(LocalUrlOpenerBackend::new())
    }

    pub fn with_backend(backend: impl UrlOpenerBackend + 'static) -> Self {
        Self {
            inner: Rc::new(RefCell::new(UrlOpenerState {
                backend: Box::new(backend),
                policy: UrlPolicy::default(),
            })),
        }
    }

    /// Replace the driver while keeping every existing handle valid.
    pub fn set_backend(&self, backend: impl UrlOpenerBackend + 'static) {
        self.inner.borrow_mut().backend = Box::new(backend);
    }

    pub fn policy(&self) -> UrlPolicy {
        self.inner.borrow().policy.clone()
    }

    /// Decide which URLs open with `policy`; [`UrlPolicy::default`] restores
    /// the default.
    pub fn set_policy(&self, policy: UrlPolicy) {
        self.inner.borrow_mut().policy = policy;
    }

    /// Whether [`Self::open`] would hand `url` to the backend.
    pub fn allows(&self, url: &str) -> bool {
        // The policy runs unborrowed, so it may use this opener itself.
        self.policy().allows(url.trim())
    }

    /// Hand `url` to the platform when the policy allows it, returning
    /// whether a handler accepted it.
    pub fn open(&self, url: impl AsRef<str>) -> bool {
        let url = url.as_ref().trim();
        self.allows(url) && self.inner.borrow_mut().backend.open(url)
    }
}

impl Default for UrlOpener {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for UrlOpener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UrlOpener").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AcceptingBackend(LocalUrlOpenerBackend);

    impl UrlOpenerBackend for AcceptingBackend {
        fn open(&mut self, url: &str) -> bool {
            self.0.open(url);
            true
        }
    }

    #[test]
    fn default_policy_allows_web_and_mail_links_only() {
        let policy = UrlPolicy::default();
        assert!(policy.allows("https://example.com/docs"));
        assert!(policy.allows("HTTP://example.com"));
        assert!(policy.allows("mailto:team@example.com"));

        assert!(!policy.allows("file:///etc/passwd"));
        assert!(!policy.allows("javascript:alert(1)"));
        assert!(!policy.allows("ms-msdt:/id"));
        assert!(!policy.allows("C:\\Windows\\notepad.exe"));
        assert!(!policy.allows("/docs/intro"));
        assert!(!policy.allows("docs/intro.md"));
        assert!(!policy.allows("https:"));
        assert!(!policy.allows("https://example.com/\nmalicious"));
    }

    #[test]
    fn custom_policies_can_extend_or_turn_off_the_default() {
        let extended =
            UrlPolicy::new(|url| url.starts_with("myapp:") || UrlPolicy::allows_by_default(url));
        assert!(extended.allows("myapp://settings"));
        assert!(extended.allows("https://example.com"));
        assert!(!extended.allows("file:///etc/passwd"));

        let all = UrlPolicy::allow_all();
        assert!(all.allows("file:///etc/passwd"));
        assert!(all.allows("docs/intro.md"));
    }

    #[test]
    fn opener_hands_only_allowed_trimmed_urls_to_its_backend() {
        let record = LocalUrlOpenerBackend::new();
        let opener = UrlOpener::with_backend(AcceptingBackend(record.clone()));

        assert!(opener.open("  https://example.com  "));
        assert!(!opener.open("file:///etc/passwd"));
        assert_eq!(record.opened(), vec!["https://example.com".to_string()]);

        opener.set_policy(UrlPolicy::allow_all());
        assert!(opener.open("file:///etc/passwd"));
        opener.set_policy(UrlPolicy::default());
        assert!(!opener.open("file:///etc/passwd"));
        assert_eq!(
            record.opened(),
            vec![
                "https://example.com".to_string(),
                "file:///etc/passwd".to_string()
            ]
        );
    }

    #[test]
    fn local_backend_records_without_accepting() {
        let record = LocalUrlOpenerBackend::new();
        let opener = UrlOpener::new();
        opener.set_backend(record.clone());

        assert!(!opener.open("https://example.com"));
        assert_eq!(record.opened(), vec!["https://example.com".to_string()]);
    }

    #[test]
    fn clones_share_backend_and_policy() {
        let record = LocalUrlOpenerBackend::new();
        let opener = UrlOpener::new();
        let clone = opener.clone();
        clone.set_backend(record.clone());
        clone.set_policy(UrlPolicy::new(|url| url.starts_with("myapp:")));

        opener.open("myapp://settings");
        opener.open("https://example.com");
        assert_eq!(record.opened(), vec!["myapp://settings".to_string()]);
    }

    #[test]
    fn a_policy_may_change_its_own_opener() {
        let record = LocalUrlOpenerBackend::new();
        let opener = UrlOpener::with_backend(record.clone());
        let inner = opener.clone();
        // Lets one URL through, then restores the default.
        opener.set_policy(UrlPolicy::new(move |_| {
            inner.set_policy(UrlPolicy::default());
            true
        }));
        opener.open("file:///tmp/once");
        opener.open("file:///tmp/twice");
        assert_eq!(record.opened(), vec!["file:///tmp/once".to_string()]);
    }
}
