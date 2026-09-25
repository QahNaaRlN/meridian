//! Pure domain types and checks for the historical `validate-mechanical-integrity`
//! subpackage (package 7, subpackage 7a): `sha-provenance`,
//! `instruction-topics`, `operating-foundation`, `stack-profiles` and
//! `agent-instruction-identity`. Moved here from `meridian-cli` by the
//! `meridian-cli-foundation-architecture-remediation` package
//! (`meridian-rust-migration-program-plan.md` §5.16): file discovery and
//! reading stay in `meridian-cli` (behind the `meridian-app`-owned
//! `WorkspaceReader` port), transport parsing (YAML, marked regions, table
//! rows) stays in `meridian-app`, and this module owns exactly the
//! predicate algorithms and the types whose invalid states are made
//! unrepresentable by construction — no file, Git, environment or process
//! I/O, and no `serde`/`serde_json` (enforced by
//! `crate::tests::crate_manifest_declares_no_serde_or_json_dependency` and
//! `crate::tests::no_rs_file_performs_filesystem_process_or_env_io_in_production_code`).

pub mod agent_instruction_identity;
pub mod instruction_topics;
pub mod operating_foundation;
mod pool;
pub mod sha_provenance;
pub mod stack_profiles;

use crate::types::{Diagnostic, DiagnosticLevel};

pub(crate) fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

pub(crate) fn warn(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Warn, message).expect("message is non-empty")
}

pub use pool::{dedupe_preserve_order, duplicates_after_first, ordered_pool_agreement};

/// Structural regressions (corrective round,
/// `meridian-cli-foundation-architecture-remediation`, second round, item
/// 5): prove, mechanically rather than by prose claim, that every valid
/// domain type this module (and [`crate::types::entry_name`]) introduces has
/// private fields and is created only through a validating constructor, and
/// that no raw, app-owned transport projection (`RawFrontMatter`,
/// `RawFoundationEntry`, `RawFoundationRow`, `RawPin`) is even NAMED here —
/// this crate does not depend on `meridian-app` at all, so that boundary is
/// also enforced at compile time, but a text-level check catches the
/// specific named types item 1 introduced, not merely "any raw type".
#[cfg(test)]
mod structural_tests {
    fn production_text(path: &std::path::Path) -> String {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()));
        let production_end = text.find("#[cfg(test)]").unwrap_or(text.len());
        text[..production_end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Recognizes a `pub name: Type` STRUCT FIELD declaration line, as
    /// opposed to `pub fn`/`pub struct`/`pub enum`/`pub mod`/`pub use`/
    /// `pub type`/`pub const`/`pub trait`/`pub impl`, none of which are a
    /// field.
    fn is_public_field_line(trimmed: &str) -> bool {
        let Some(rest) = trimmed.strip_prefix("pub ") else {
            return false;
        };
        let rest = rest.trim_start();
        for keyword in [
            "fn ", "struct ", "enum ", "mod ", "use ", "type ", "const ", "static ", "trait ",
            "impl ", "(",
        ] {
            if rest.starts_with(keyword) {
                return false;
            }
        }
        let ident_end = rest
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        if ident_end == 0 {
            return false;
        }
        let after = rest[ident_end..].trim_start();
        after.starts_with(':') && !after.starts_with("::")
    }

    /// No struct field declaration in any of the five families' own module
    /// is `pub` — a "valid domain value" here is always built through its
    /// own `new`/`build_*` constructor, never by a caller writing a struct
    /// literal that could skip validation entirely.
    #[test]
    fn no_domain_struct_in_the_five_families_declares_a_public_field() {
        let base =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mechanical_integrity");
        for file in [
            "agent_instruction_identity.rs",
            "sha_provenance.rs",
            "operating_foundation.rs",
            "instruction_topics.rs",
            "stack_profiles.rs",
        ] {
            let production = production_text(&base.join(file));
            for line in production.lines() {
                let trimmed = line.trim();
                assert!(
                    !is_public_field_line(trimmed),
                    "{file} declares a public struct field: {line:?} — a valid domain value must only be constructible through its own validating constructor"
                );
            }
        }
    }

    /// Neither `meridian-app`'s own raw transport projections, nor
    /// `serde_json`, are ever named in this module's five family files —
    /// core receives only already-extracted scalars through a constructor,
    /// never a raw, unvalidated app-owned struct or an untyped JSON value.
    #[test]
    fn no_family_file_names_an_app_owned_raw_transport_type_or_json_value() {
        let base =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mechanical_integrity");
        for file in [
            "agent_instruction_identity.rs",
            "sha_provenance.rs",
            "operating_foundation.rs",
            "instruction_topics.rs",
            "stack_profiles.rs",
        ] {
            let production = production_text(&base.join(file));
            for forbidden in [
                "RawFrontMatter",
                "RawFoundationEntry",
                "RawFoundationRow",
                "RawPin",
                "serde_json",
                "meridian_app",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{file} names \"{forbidden}\" — meridian-core must never see an app-owned raw transport type or a raw JSON value"
                );
            }
        }
    }

    /// `CompleteIdentity`'s `delivery`/`activation` are the closed
    /// [`super::agent_instruction_identity::Delivery`]/
    /// [`super::agent_instruction_identity::Activation`] enums themselves —
    /// never a bare `Option<String>`, and never an `Absent`/`Unknown`
    /// placeholder variant an unvalidated value could sit in: those states
    /// are [`super::agent_instruction_identity::IdentityConstructionIssue`]s,
    /// which can never be part of a successfully constructed
    /// `CompleteIdentity` at all.
    #[test]
    fn complete_identity_delivery_and_activation_are_the_closed_enum_fields_not_raw_strings() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/mechanical_integrity/agent_instruction_identity.rs");
        let production = production_text(&path);
        assert!(production.contains("struct CompleteIdentity"));
        assert!(production.contains("delivery: Delivery"));
        assert!(production.contains("activation: Activation"));
        assert!(!production.contains("delivery: Option<String>"));
        assert!(!production.contains("activation: Option<String>"));
        assert!(!production.contains("enum DeliveryField"));
        assert!(!production.contains("enum ActivationField"));
    }

    /// `SourceArchive::new` returns `Result<Self, SourceArchiveError>` — a
    /// complete, valid source archive is the only value the type can ever
    /// hold; there is no `SourceArchiveDeclaration`/`build_source_archive`
    /// domain enum with an `Incomplete` variant, and no infallible
    /// constructor that could accept a missing/invalid field.
    #[test]
    fn source_archive_incomplete_is_a_constructor_error_not_a_domain_enum_variant() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/mechanical_integrity/sha_provenance.rs");
        let production = production_text(&path);
        assert!(production.contains("enum SourceArchiveError"));
        assert!(production.contains("pub fn new(path: Option<&str>, sha256: Option<&str>) -> Result<Self, SourceArchiveError>"));
        assert!(!production.contains("SourceArchiveDeclaration"));
        assert!(!production.contains("fn build_source_archive"));
    }

    /// `SkillPin::new` returns `Result<Self, SkillPinError>` — an
    /// infallible constructor that could accept a missing/empty/invalid
    /// artifact or sha256 no longer exists.
    #[test]
    fn skill_pin_new_is_fallible_and_rejects_a_missing_or_invalid_field() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/mechanical_integrity/sha_provenance.rs");
        let production = production_text(&path);
        assert!(production.contains("enum SkillPinError"));
        assert!(production.contains("pub fn new(artifact: Option<&str>, sha256: Option<&str>) -> Result<Self, SkillPinError>"));
        assert!(!production
            .contains("pub fn new(artifact: Option<&str>, sha256: Option<&str>) -> Self"));
    }
}
