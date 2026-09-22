#![forbid(unsafe_code)]

//! Meridian application layer — pure source-format adapters, ports and
//! orchestration.
//!
//! The adapters in [`source_format`] accept in-memory text and values only.
//! They deliberately own no filesystem, Git, network, environment or CLI
//! input/output. [`rule_resolution`] adds strict in-memory composition over
//! `meridian-core` without widening that boundary. [`storage`] defines the
//! `RecordRepository`/`EvidenceRepository` ports a storage adapter
//! implements — this crate declares the contract, never SQLite, a file or a
//! connection itself. [`events`] defines the observed-event envelope and
//! sink port. [`knowledge`] reserves, without implementing, the future
//! Knowledge Resolver boundary.

pub mod events;
pub mod knowledge;
pub mod operating_model;
pub mod rule_resolution;
pub mod source_format;
pub mod storage;
pub mod validation;
pub mod workspace;

/// Identifies this crate in composition-root diagnostics until real ports
/// exist.
pub const CRATE_NAME: &str = "meridian-app";

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "meridian-app");
    }

    /// Test-only fixture used to prove that the selected serialization
    /// libraries round-trip a value. It is not a production domain type
    /// or an application port.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct ProbeRecord {
        id: String,
        scope: String,
    }

    #[test]
    fn probe_record_round_trips_through_json() {
        let record = ProbeRecord {
            id: "probe-1".to_string(),
            scope: "run-state".to_string(),
        };
        let json = serde_json::to_string(&record).expect("serialize probe record to json");
        let round_tripped: ProbeRecord =
            serde_json::from_str(&json).expect("deserialize probe record from json");
        assert_eq!(record, round_tripped);
    }

    /// Retains the package-1 serialization probe independently of the
    /// production strict adapter exercised in `source_format::yaml`.
    #[test]
    fn probe_record_round_trips_through_yaml() {
        let record = ProbeRecord {
            id: "probe-1".to_string(),
            scope: "run-state".to_string(),
        };
        let yaml = serde_saphyr::to_string(&record).expect("serialize probe record to yaml");
        let round_tripped: ProbeRecord =
            serde_saphyr::from_str(&yaml).expect("deserialize probe record from yaml");
        assert_eq!(record, round_tripped);
    }

    #[test]
    fn draft7_validator_accepts_a_conforming_instance() {
        let schema = json!({
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "string", "minLength": 1 } }
        });
        assert!(jsonschema::draft7::is_valid(
            &schema,
            &json!({ "id": "probe-1" })
        ));
    }

    #[test]
    fn draft7_validator_rejects_a_non_conforming_instance() {
        let schema = json!({
            "type": "object",
            "required": ["id"],
            "properties": { "id": { "type": "string", "minLength": 1 } }
        });
        assert!(!jsonschema::draft7::is_valid(&schema, &json!({ "id": "" })));
        assert!(!jsonschema::draft7::is_valid(&schema, &json!({})));
    }

    /// Structural regression (corrective round,
    /// `meridian-cli-foundation-architecture-remediation`, item 4): no `.rs`
    /// file under `meridian-app/src` performs concrete filesystem I/O in
    /// its PRODUCTION code — this crate's own module doc comment claims it
    /// (`workspace::WorkspaceReader` is a port; the one concrete adapter
    /// lives in `meridian-cli`), and this test makes the claim mechanically
    /// checked. Scoped to the text of each file BEFORE its first
    /// `#[cfg(test)]` (this codebase's own convention: production code
    /// first, `#[cfg(test)] mod tests { ... }` last — the same exemption
    /// `meridian-core`'s own equivalent structural test relies on), and
    /// comment-only lines are stripped first so a doc comment is free to
    /// NAME `std::fs` while explaining this very guarantee (as several
    /// modules' own doc comments do) without tripping the check.
    #[test]
    fn no_rs_file_performs_concrete_filesystem_io_in_production_code() {
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
            let production: String = text[..production_end]
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            for forbidden in [
                "std::fs::",
                "fs::File",
                "fs::read",
                "fs::write",
                "fs::remove",
                "fs::create_dir",
                "fs::OpenOptions",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{} uses \"{forbidden}\" outside #[cfg(test)] — meridian-app must never perform concrete filesystem I/O; the WorkspaceReader port exists exactly so it doesn't have to",
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

    /// Structural regression (item 4): production has exactly one concrete
    /// `WorkspaceReader` implementation IN THIS CRATE's own production
    /// code — none. Every `impl WorkspaceReader for` in `meridian-app`
    /// lives inside a `#[cfg(test)]` fake (`validation::mechanical_integrity::tests`).
    /// The one real production implementation is
    /// `meridian_cli::adapters::workspace_reader::FsWorkspaceReader`,
    /// checked by `meridian-cli`'s own equivalent structural test (which
    /// also scans this crate, to prove the total across BOTH crates is 1).
    #[test]
    fn no_workspace_reader_implementation_exists_in_this_crates_production_code() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&src, &mut files);
        for file in files {
            let text = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("{} is readable: {e}", file.display()));
            let production_end = text.find("#[cfg(test)]").unwrap_or(text.len());
            let production: String = text[..production_end]
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                !production.contains("impl WorkspaceReader for"),
                "{} implements WorkspaceReader outside #[cfg(test)] — meridian-app must stay a port-only crate; the one production adapter belongs in meridian-cli",
                file.display()
            );
        }
    }
}
