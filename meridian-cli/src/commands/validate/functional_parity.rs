//! Presentation/composition shim over the real port-based implementation:
//! `meridian_app::operating_model::functional_parity`, reading through
//! `crate::adapters::workspace_reader::FsWorkspaceReader`. No predicate
//! logic, `serde_json::Value`, schema navigation, fixture loop or domain
//! rules lives in this crate for this check any more
//! (`rust-architecture-conformance-4`).
//!
//! Every message [`app::evaluate`] produces is prefix-free; this is the ONE
//! place the `functional-parity: ` presentation prefix is applied
//! (`rust-architecture-conformance-3`/`-4` corrective-round convention,
//! "presentation ownership").

use std::path::Path;

use meridian_app::operating_model::functional_parity as app;

use super::{split_diagnostics, with_family_prefix};
use crate::adapters::workspace_reader::FsWorkspaceReader;

const FAMILY: &str = "functional-parity";

pub struct Outcome {
    pub failures: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let reader = FsWorkspaceReader::new(kernel_root);
    let outcome = app::evaluate(&reader);
    let (failures, _warnings) = split_diagnostics(outcome.diagnostics);
    Outcome {
        failures: with_family_prefix(FAMILY, failures),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let real_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let outcome = run(&real_root);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }

    /// A representative APP-generated failure carries EXACTLY ONE
    /// `functional-parity: ` prefix — an exact string match, so a doubled
    /// or missing prefix would fail this test either way.
    #[test]
    fn a_representative_app_generated_failure_carries_exactly_one_family_prefix() {
        let empty_root = std::env::temp_dir().join(format!(
            "functional-parity-cli-prefix-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&empty_root).unwrap();
        struct Guard(std::path::PathBuf);
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _guard = Guard(empty_root.clone());

        // No `verification/functional-parity/` at all under this empty
        // root: the schema is `NotFound`, so `app::evaluate` skips (no
        // diagnostics) — this test instead writes a real, VALID (but
        // permissive) schema with no fixtures beside it, so exactly one
        // production failure (the "carries no fixtures" gate) fires.
        let dir = empty_root.join("verification").join("functional-parity");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("functional-parity-evidence.schema.json"),
            r#"{"type": "object"}"#,
        )
        .unwrap();

        let outcome = run(&empty_root);
        assert_eq!(outcome.failures.len(), 1, "{:?}", outcome.failures);
        assert!(
            outcome.failures[0].starts_with("functional-parity: "),
            "{}",
            outcome.failures[0]
        );
        assert_eq!(
            outcome.failures[0].matches("functional-parity: ").count(),
            1,
            "{}",
            outcome.failures[0]
        );
    }
}
