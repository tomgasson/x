//! Core crate — foundational types and utilities.

#![deny(clippy::all)]
#![warn(clippy::perf)]

mod error;
mod types;

pub use error::{Error, Result};
pub use types::*;