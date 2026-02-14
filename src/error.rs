use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum JwwError {
    Io(std::io::Error),
    InvalidSignature,
    UnexpectedEof(&'static str),
    EntityListNotFound,
    UnknownClassPid(u32),
    UnknownEntityClass(String),
}

impl Display for JwwError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::InvalidSignature => write!(f, "invalid JWW signature: expected \"JwwData.\""),
            Self::UnexpectedEof(ctx) => write!(f, "unexpected EOF while reading {ctx}"),
            Self::EntityListNotFound => write!(f, "could not find entity list in file"),
            Self::UnknownClassPid(pid) => write!(f, "unknown class PID: {pid}"),
            Self::UnknownEntityClass(name) => write!(f, "unknown entity class: {name}"),
        }
    }
}

impl Error for JwwError {}

impl From<std::io::Error> for JwwError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
