use core::{error, fmt};
use std::fmt::Display;

#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
  /// `log` already initialized by another crate
  AlreadyInitialized,
}

impl error::Error for Error {}

impl Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::AlreadyInitialized => write!(f, "`log` has already been initialized by another crate"),
    }
  }
}
