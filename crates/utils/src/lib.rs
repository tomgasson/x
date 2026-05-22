//! Utils crate — common helper functions and utilities.

#![deny(clippy::all)]
#![warn(clippy::perf)]

mod fmt;
mod str;

pub use fmt::format_bytes;
pub use str::trim_prefix;

use core::Result;
use std::io;

/// Format a byte count into a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_idx])
}

/// Trim a prefix from a string, returning the original if it doesn't match.
pub fn trim_prefix(s: &str, prefix: &str) -> String {
    s.strip_prefix(prefix).map(String::from).unwrap_or_else(|| s.to_owned())
}