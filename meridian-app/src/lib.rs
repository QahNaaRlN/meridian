#![forbid(unsafe_code)]

//! Meridian application layer — ports and orchestration.
//!
//! Package `rust-workspace-foundation` establishes only the crate boundary.
//! In `#[cfg(test)]` only, it proves that the chosen serialization
//! (`serde`/`serde_json`), YAML (`serde-saphyr`) and JSON Schema
//! (`jsonschema`, Draft 7) libraries compile, link and behave as expected.
//! Production code here carries no port trait, no adapter and none of the
//! `import`/`validate`/`resolve`/`migration`/`export` operations. Per the
//! active `meridian-rust-migration` roadmap: package `rust-domain-core`
//! introduces the domain core — domain types, the resolver, conflict
//! detection, migration plans, evidence and verdict/diagnostic structures —
//! in `meridian-core` only, not in this crate; package `rust-rule-resolution`
//! carries rule resolution and compositional orchestration into
//! `meridian-app`; packages `meridian-cli-foundation` and
//! `meridian-cli-migration` introduce the corresponding CLI commands. This
//! package does not anticipate the concrete types or interfaces those later
//! packages will settle on.

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

    /// Proves the chosen YAML crate (`serde-saphyr`) compiles, links and
    /// round-trips a value through serde. No strict-lint mode, no allowlist,
    /// no production adapter: those belong to package
    /// `rust-source-format-adapters`.
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
}
