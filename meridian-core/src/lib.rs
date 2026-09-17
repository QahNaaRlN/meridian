#![forbid(unsafe_code)]

//! Meridian domain core.
//!
//! Package `rust-workspace-foundation` establishes only the crate boundary.
//! Domain types, the resolver, conflict detection, migration plans, evidence
//! and verdict/diagnostic structures are added by package `rust-domain-core`.

/// Identifies this crate in composition-root diagnostics until real domain
/// types exist.
pub const CRATE_NAME: &str = "meridian-core";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(CRATE_NAME, "meridian-core");
    }
}
