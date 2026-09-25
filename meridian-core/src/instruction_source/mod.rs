//! The canonical, typed model of a registered instruction source
//! (`instruction-source-registry.md`): closed pools for its medium, format,
//! read channel and snapshot currency; a typed, confined [`Location`]; the
//! [`Divergence`] temporal model between a held snapshot and a fresh
//! observation; and the valid, constructed [`InstructionSource`] itself.
//!
//! Grown from the single-file `ReadChannel` module
//! `rust-architecture-conformance-1`'s corrective round extracted
//! (`rust-architecture-conformance-2`, §1): that pilot already established
//! this crate as the ONE place the `read_channel` coherence rule lives, and
//! this package generalises the same discipline to the REST of a
//! registered source — medium/location confinement, format, recorded-state
//! currency and the previous/current divergence comparison — so
//! `meridian-app`'s `operating_model::instruction_source_registry` (the
//! registry's own closed transport boundary) and
//! `operating_model::existing_project_compatibility_mode` (which composes a
//! discovered source through the SAME registry boundary before handing it,
//! as an already-resolved typed snapshot, to `controlled-rule-intake`) both
//! build and check ONE shared domain model instead of two divergent
//! Value-based copies.
//!
//! [`Currency`], [`RecordedState`] and [`ResolvedInstructionSource`] used to
//! live in `controlled_rule_intake::resolved`; they are canonical HERE now
//! (a resolved snapshot is a fact about an instruction SOURCE, not specific
//! to the rule-intake contract that happens to compare a pin against one),
//! and `controlled_rule_intake` re-exports them unchanged rather than
//! carrying a second copy — every existing `meridian_core::controlled_rule_intake::{Currency,
//! RecordedState, ResolvedInstructionSource}` import path keeps working.
//!
//! Like the rest of `meridian-core`, nothing here touches `serde`,
//! `serde_json::Value`, the filesystem, Git or process I/O; every
//! constructor takes already-syntax-checked scalars or nested domain values
//! and either builds a valid value or returns [`crate::types::Diagnostic`]s
//! explaining why it could not.

mod currency;
mod divergence;
mod location;
mod medium;
mod read_channel;
mod recorded_state;
mod resolved_instruction_source;
mod source;
mod source_format;

pub use currency::Currency;
pub use divergence::{Divergence, DivergenceStatus, ObservedState};
pub use location::{Location, OpaqueRef, OpaqueRefError, RelativePath, RelativePathError};
pub use medium::Medium;
pub use read_channel::{MeridianVisibility, ReadChannel, ReadChannelKind};
pub use recorded_state::RecordedState;
pub use resolved_instruction_source::ResolvedInstructionSource;
pub use source::{InstructionSource, InstructionSourcePayload, NORMATIVE_STATUS, RECORD_TYPE};
pub use source_format::SourceFormat;

use crate::types::{Diagnostic, DiagnosticLevel};

pub(crate) fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

#[cfg(test)]
mod structural_tests {
    /// Regression guard (`rust-architecture-conformance-2`, §1): exactly
    /// ONE definition of each of `Currency`/`RecordedState`/
    /// `ResolvedInstructionSource`/`ReadChannel` exists across the whole
    /// `meridian-core` crate — a future change re-introducing a second,
    /// divergent copy (in `controlled_rule_intake` or elsewhere) is caught
    /// here by name, not left to drift unnoticed.
    #[test]
    fn each_moved_canonical_type_is_defined_exactly_once_in_the_whole_crate() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let self_path = src.join("instruction_source").join("mod.rs");
        let mut files = Vec::new();
        collect_rs_files(&src, &mut files);
        // Excludes this very file: it quotes each search pattern as a
        // string literal in its own test body below, which would otherwise
        // self-match and inflate every count by one.
        files.retain(|f| f != &self_path);
        for definition in [
            "pub enum Currency",
            "pub struct RecordedState",
            "pub struct ResolvedInstructionSource",
            "pub struct ReadChannel",
        ] {
            let mut count = 0usize;
            for file in &files {
                let text = std::fs::read_to_string(file)
                    .unwrap_or_else(|e| panic!("{} is readable: {e}", file.display()));
                count += text.matches(definition).count();
            }
            assert_eq!(
                count, 1,
                "expected exactly one `{definition}` in meridian-core/src, found {count}"
            );
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
