//! Strict adapters for the source formats accepted by the frozen Node.js
//! implementation.

pub mod json_schema;
pub mod markdown_identity;
pub mod regions;
pub mod yaml;

pub use markdown_identity::carries_own_front_matter;
pub use yaml::parse as parse_yaml;
