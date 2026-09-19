#![forbid(unsafe_code)]

//! Meridian application layer — pure source-format adapters, ports and
//! orchestration.
//!
//! The adapters in [`source_format`] accept in-memory text and values only.
//! They deliberately own no filesystem, Git, network, environment or CLI
//! input/output. [`rule_resolution`] adds strict in-memory composition over
//! `meridian-core` without widening that boundary.

pub mod rule_resolution;
pub mod source_format;

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
}
