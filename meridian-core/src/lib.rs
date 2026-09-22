#![forbid(unsafe_code)]

//! Meridian domain core.
//!
//! Synchronous, side-effect-free domain logic: strict domain types
//! ([`types`]), the rule resolver ([`resolver`]), instance-data-migration
//! plans and their checks ([`migration`]), evidence/verdict structures
//! ([`evidence`]), the `controlled-rule-intake` domain types and pure
//! checks ([`controlled_rule_intake`]), the canonical instruction-source
//! model ([`instruction_source`]), and the
//! `existing-project-compatibility-mode` domain types and pure checks
//! ([`existing_project_compatibility_mode`]).
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

pub mod controlled_rule_intake;
pub mod evidence;
pub mod existing_project_compatibility_mode;
pub mod instruction_source;
mod json;
pub mod mechanical_integrity;
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

    /// Structural test (`rust-architecture-conformance-1` corrective round
    /// item 12): this crate's own `Cargo.toml` carries no serde/JSON
    /// dependency at all — not a convention this test merely checks, but a
    /// fact that would make `use serde_json::Value` (or any serde derive)
    /// anywhere in `meridian-core/src` fail to compile. This test reads its
    /// own manifest (test-only I/O; the "never opens a file" guarantee in
    /// this module's own doc comment is about production code paths, which
    /// this `#[cfg(test)]` block is not) so a future accidental dependency
    /// addition is caught here, by name, rather than discovered later as a
    /// leaked `Value` deep in a domain module.
    #[test]
    fn crate_manifest_declares_no_serde_or_json_dependency() {
        let manifest_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        let manifest =
            std::fs::read_to_string(manifest_path).expect("meridian-core/Cargo.toml is readable");
        // Line-anchored, not a substring search over the whole file: a
        // doc-comment inside the manifest is allowed to MENTION
        // `serde_json` (as this one does, explaining a float-formatting
        // choice) without that counting as a dependency declaration. A real
        // dependency line looks like `serde_json = "..."` or
        // `serde_json = { ... }`, always at the start of a line inside
        // `[dependencies]`/`[dev-dependencies]`.
        for forbidden in ["serde", "serde_json", "jsonschema", "serde-saphyr"] {
            let declared = manifest.lines().map(str::trim).any(|line| {
                line.starts_with(forbidden) && line[forbidden.len()..].trim_start().starts_with('=')
            });
            assert!(
                !declared,
                "meridian-core/Cargo.toml must not depend on \"{forbidden}\" — domain checks must never see serde_json::Value"
            );
        }
    }

    /// Structural test (`rust-architecture-conformance-2` corrective round,
    /// item 6): no `.rs` file under `meridian-core/src` performs filesystem,
    /// process, or environment I/O in its PRODUCTION code — the crate's own
    /// doc comment claims this, and this test makes the claim mechanically
    /// checked rather than merely asserted in prose. Scoped to the text of
    /// each file BEFORE its first `#[cfg(test)]` (this codebase's own
    /// convention throughout is production code first, `#[cfg(test)] mod
    /// tests { ... }` last), matching the same test-only exemption
    /// `crate_manifest_declares_no_serde_or_json_dependency` and several
    /// per-module structural tests already rely on for their own
    /// `std::fs`/`std::path` use.
    #[test]
    fn no_rs_file_performs_filesystem_process_or_env_io_in_production_code() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&src, &mut files);
        assert!(
            files.len() > 20,
            "expected to scan the whole crate, found {}",
            files.len()
        );
        for file in files {
            let text = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("{} is readable: {e}", file.display()));
            let production_end = text.find("#[cfg(test)]").unwrap_or(text.len());
            // Comment-only lines (`//`/`///`/`//!`) are prose, not code —
            // this crate's own doc comments legitimately NAME these
            // patterns to explain the very guarantee this test checks.
            let production: String = text[..production_end]
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            for forbidden in [
                "std::fs",
                "std::net",
                "std::process",
                "std::env",
                "std::io::stdin",
                "std::io::stdout",
                "std::io::stderr",
                "println!",
                "eprintln!",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{} uses \"{forbidden}\" outside #[cfg(test)] — meridian-core must never perform filesystem, process, env or output I/O in production code",
                    file.display()
                );
            }
        }
    }

    fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let entries =
            std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{} is readable: {e}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|e| panic!("dir entry is readable: {e}"))
                .path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
}
