//! Versioned identity keys for authorities.

use crate::model::Citation;

pub const KEY_VERSION: &str = "2";

pub fn key(_citation: &Citation) -> Option<String> {
    None
}
