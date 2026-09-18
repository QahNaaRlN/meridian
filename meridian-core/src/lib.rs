#![forbid(unsafe_code)]

//! Meridian domain core.
//!
//! Synchronous, side-effect-free domain logic: strict domain types
//! ([`types`]), the rule resolver ([`resolver`]), instance-data-migration
//! plans and their checks ([`migration`]), and evidence/verdict structures
//! ([`evidence`]).
//!
//! This crate never opens a file, never talks to Git, a database or the
//! network, never reads an environment variable, and never produces output
//! (no `println!`/`eprintln!`/`dbg!`, no logging). Every function here
//! takes already-loaded, already-syntactically-checked structures as
//! parameters and returns a value — it does not know where its input came
//! from or where its output goes. Package `rust-domain-core`
//! (`meridian-rust-migration-program-plan.md` §4) establishes this crate's
//! content; adapters, ports and orchestration belong to `meridian-app` and
//! its own adapter crates, added by later packages of the same program.

pub mod evidence;
mod json;
pub mod migration;
pub mod resolver;
pub mod types;

/// Identifies this crate in composition-root diagnostics.
pub const CRATE_NAME: &str = "meridian-core";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(CRATE_NAME, "meridian-core");
    }
}
