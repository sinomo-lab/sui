use std::error::Error as StdError;
use std::fmt;

/// A result whose error defaults to SUI's [`Error`]. The error type can be
/// given too, so a glob import of SUI's prelude leaves `Result<T, E>` working.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl StdError for Error {}
