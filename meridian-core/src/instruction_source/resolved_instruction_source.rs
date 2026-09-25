//! [`ResolvedInstructionSource`] — the already-resolved external
//! instruction-source data a caller compares a pin against —
//! `meridian-core` never resolves a source itself; the caller resolves it
//! through whatever port it has and passes the closed result in. Mirrors
//! [`crate::migration::resolved`]'s own "closed transformer response"
//! discipline.
//!
//! Moved here from `controlled_rule_intake::resolved` by
//! `rust-architecture-conformance-2` (§1): resolving an instruction source
//! is not specific to `controlled-rule-intake` — `operating_model::existing_project_compatibility_mode`
//! (`meridian-app`) now builds this SAME type directly from an already-typed
//! [`super::InstructionSource`] it obtained through the registry's own
//! boundary, with no `controlled-rule-intake`-specific step in between.
//! `controlled_rule_intake` re-exports this type unchanged; its own
//! [`crate::controlled_rule_intake::checks::check_source_ref`] remains the
//! one function that compares a value of this type against a pin.

use super::RecordedState;
use crate::types::{NonEmptyString, SemanticId};

/// The already-resolved external instruction-source data a caller compares
/// a pin against. The ONLY way to construct a value of this type is
/// [`ResolvedInstructionSource::new`] — its fields are private and there is
/// no other public constructor, so external code cannot build one via a
/// struct literal and hand it to a check unchecked.
///
/// What this type's construction guarantees, precisely: `id`, `reference`
/// and `recorded_state` are each individually well-formed strict core
/// values (a [`SemanticId`], a [`NonEmptyString`], a [`RecordedState`]). It
/// does NOT by itself guarantee that those values came from an actual
/// resolver call or passed a resolver's own closed transport-shape
/// validation — that provenance is the job of whichever DTO -> domain
/// conversion produced it (`operating_model::controlled_rule_intake::source_resolver::validate_resolved_response`
/// or `operating_model::existing_project_compatibility_mode`'s own
/// composition, both in `meridian-app`), not an invariant this type can
/// check on its own, since it never sees a DTO or a resolver at all.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedInstructionSource {
    id: SemanticId,
    reference: NonEmptyString,
    recorded_state: RecordedState,
}

impl ResolvedInstructionSource {
    pub fn new(id: SemanticId, reference: NonEmptyString, recorded_state: RecordedState) -> Self {
        Self {
            id,
            reference,
            recorded_state,
        }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }

    pub fn reference(&self) -> &str {
        self.reference.as_str()
    }

    pub fn recorded_state(&self) -> &RecordedState {
        &self.recorded_state
    }
}

#[cfg(test)]
mod tests {
    use super::super::Currency;
    use super::*;
    use crate::types::{ContentDigest, Revision};

    fn sample() -> ResolvedInstructionSource {
        ResolvedInstructionSource::new(
            SemanticId::new("src-1").unwrap(),
            NonEmptyString::new("sources/src-1").unwrap(),
            RecordedState::new(
                Revision::new("a".repeat(40)).unwrap(),
                ContentDigest::from_hex("b".repeat(64)).unwrap(),
                true,
                Currency::Current,
            ),
        )
    }

    #[test]
    fn new_and_its_getters_round_trip_every_field() {
        let value = sample();
        assert_eq!(value.id().as_str(), "src-1");
        assert_eq!(value.reference(), "sources/src-1");
        assert_eq!(value.recorded_state().revision().as_str(), "a".repeat(40));
        assert_eq!(value.recorded_state().digest().value(), "b".repeat(64));
        assert!(value.recorded_state().revision_verified());
        assert_eq!(value.recorded_state().currency(), Currency::Current);
    }

    /// Structural (moved from `controlled_rule_intake::resolved` unchanged
    /// by `rust-architecture-conformance-2`, still the sole guarantee this
    /// type's own module offers): the whole `meridian-app` crate must build
    /// this type — and [`RecordedState`] — only through `::new(..)`, never
    /// a struct literal, grepped by name across the whole crate so a future
    /// regression is caught here, by a failing test, rather than relying on
    /// code review alone.
    #[test]
    fn meridian_app_never_constructs_these_types_with_a_struct_literal() {
        let app_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("meridian-core has a workspace parent directory")
            .join("meridian-app")
            .join("src");
        let mut checked = 0usize;
        for file in rs_files_under(&app_src) {
            let text = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("{} is readable: {e}", file.display()));
            for forbidden in ["ResolvedInstructionSource {", "RecordedState {"] {
                assert!(
                    !text.contains(forbidden),
                    "{} constructs `{forbidden}` directly via struct literal; the only allowed \
                     construction path is `::new(..)`",
                    file.display()
                );
            }
            checked += 1;
        }
        assert!(
            checked > 20,
            "expected to scan the whole meridian-app/src tree, only found {checked} .rs files"
        );
    }

    fn rs_files_under(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let entries =
            std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{} is readable: {e}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|e| panic!("dir entry is readable: {e}"))
                .path();
            if path.is_dir() {
                out.extend(rs_files_under(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
        out
    }
}
