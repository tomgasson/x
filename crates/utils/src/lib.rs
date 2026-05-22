//! Utils crate — common helper functions and utilities.

#![deny(clippy::all)]
#![warn(clippy::perf)]

mod fmt;
mod str;

pub use fmt::format_bytes;
pub use str::trim_prefix;