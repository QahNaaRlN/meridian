//! Strict adapters for the source formats accepted by the frozen Node.js
//! implementation.

pub mod json_schema;
pub mod yaml;

pub use yaml::parse as parse_yaml;
