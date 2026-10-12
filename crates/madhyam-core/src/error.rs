//! One error type for the whole crate.
//!
//! Without `thiserror` we implement `Display` and `std::error::Error` by hand.
//! It is only a few lines and shows exactly what that crate generates for you.

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// Reading a file or stream failed.
    Io(std::io::Error),
    /// Malformed input. `line` is 1-based when known (0 = unknown).
    Parse {
        what: &'static str,
        line: usize,
        msg: String,
    },
    /// The server answered, but not with what we expected (e.g. bad login).
    Api(String),
    /// Caller asked for something that does not exist.
    NotFound(String),
    /// Caller passed an invalid argument.
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn parse(what: &'static str, line: usize, msg: impl Into<String>) -> Self {
        Error::Parse {
            what,
            line,
            msg: msg.into(),
        }
    }

    /// Short machine-readable code, handy for the FFI layer.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Io(_) => "io",
            Error::Parse { .. } => "parse",
            Error::Api(_) => "api",
            Error::NotFound(_) => "not_found",
            Error::Invalid(_) => "invalid",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Parse { what, line: 0, msg } => write!(f, "{what} parse error: {msg}"),
            Error::Parse { what, line, msg } => {
                write!(f, "{what} parse error on line {line}: {msg}")
            }
            Error::Api(m) => write!(f, "API error: {m}"),
            Error::NotFound(m) => write!(f, "not found: {m}"),
            Error::Invalid(m) => write!(f, "invalid argument: {m}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

// `?` on an io::Result inside our functions converts automatically thanks to this.
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
