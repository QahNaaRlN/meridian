//! Port of `scripts/kernel-validate.mjs`'s `controlled-rule-intake` check
//! (package 7, subpackage 7c):
//! `registries/operating-model/controlled-rule-intake.schema.json` is the
//! specialised container/payload schema for a rule-candidate registry
//! document; each candidate's own envelope is validated separately against
//! `scoped-record.schema.json`. Registry DATA is Instance, like the source
//! registry and the intake register: the Kernel ships the schema and the
//! product-neutral fixtures, never a canonical data file. The composite
//! algorithm itself is split across two crates —
//! [`meridian_core::controlled_rule_intake`]'s domain types and pure
//! checks, orchestrated by
//! `meridian_app::operating_model::controlled_rule_intake::evaluate_controlled_rule_intake`
//! — neither owned by this file. The contract is a MANDATORY part of this
//! Kernel: a missing schema or missing fixtures is a FAIL, not an
//! informational skip. The fixtures bundle carries its own companion
//! `"resolution"` object — the external record-resolution boundary every
//! candidate's `source_ref` is checked against.
//!
//! [`FixtureSourceResolver`] is the concrete, fixtures-backed
//! `SourceResolver` (`governance/specifications/meridian-rust-target-architecture.md`
//! §3.1: `meridian-app` defines the port trait, `meridian-cli` owns every
//! local adapter — the ONLY `impl SourceResolver` in the whole workspace,
//! third corrective round items 1-2). It is still pure I/O + transport
//! parsing — a flat lookup into the bundle's `"resolution"` object plus a
//! `serde_json::from_value` into the app's closed DTO — no
//! `controlled-rule-intake` business rule lives here; this file remains a
//! presentation/I/O boundary, same as before.

use std::fs;
use std::path::Path;

use meridian_app::operating_model::controlled_rule_intake::{
    evaluate_controlled_rule_intake, EvalOpts, ResolvedInstructionSourceDto, SourceResolution,
    SourceResolver, SourceResolverError,
};
use meridian_app::source_format::json_schema;
use meridian_core::controlled_rule_intake::PinnedSourceRef;
use serde_json::Value;

const SCHEMA_NAME: &str = "controlled-rule-intake.schema.json";

/// The fixtures-backed resolver: looks a candidate's pinned `reference` up
/// in the fixtures bundle's `"resolution"` object (a flat
/// `reference -> resolved entry` map) and parses whatever it finds into the
/// closed transport DTO. `NotFound` for a missing key, `Failed` for a key
/// present but not shaped like a resolved instruction source — the two
/// resolver failures a real registry-backed adapter would also need to
/// distinguish.
struct FixtureSourceResolver<'a> {
    resolution_map: &'a Value,
}

impl SourceResolver for FixtureSourceResolver<'_> {
    fn resolve(
        &self,
        pinned: &PinnedSourceRef,
    ) -> Result<ResolvedInstructionSourceDto, SourceResolverError> {
        match self.resolution_map.get(pinned.reference()) {
            None => Err(SourceResolverError::NotFound),
            Some(entry) => serde_json::from_value(entry.clone())
                .map_err(|e| SourceResolverError::Failed(e.to_string())),
        }
    }
}

pub struct Outcome {
    pub failures: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let dir = kernel_root.join("registries").join("operating-model");
    let mut failures = Vec::new();

    let Some(schema_raw) = fs::read_to_string(dir.join(SCHEMA_NAME)).ok() else {
        failures.push(format!(
            "controlled-rule-intake: registries/operating-model/{SCHEMA_NAME} is missing; the controlled rule intake contract is a mandatory part of this Kernel, not an optional add-on"
        ));
        return Outcome { failures };
    };

    let mut ok = true;
    let mut schema: Option<Value> = None;
    match serde_json::from_str::<Value>(&schema_raw) {
        Ok(parsed) => schema = Some(parsed),
        Err(error) => {
            failures.push(format!(
                "controlled-rule-intake: {SCHEMA_NAME} is not valid JSON: {error}"
            ));
            ok = false;
        }
    }

    let mut envelope: Option<Value> = None;
    match fs::read_to_string(dir.join("scoped-record.schema.json")) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(parsed) => envelope = Some(parsed),
            Err(error) => {
                failures.push(format!(
                    "controlled-rule-intake: scoped-record.schema.json is not valid JSON: {error}"
                ));
                ok = false;
            }
        },
        Err(_) => {
            failures.push(
                "controlled-rule-intake: registries/operating-model/scoped-record.schema.json is missing; the candidate record composes with the record envelope and cannot be checked without it"
                    .to_string(),
            );
            ok = false;
        }
    }

    if let Some(schema) = &schema {
        if let Err(error) = json_schema::assert_supported_deep(schema, SCHEMA_NAME) {
            failures.push(format!(
                "controlled-rule-intake: the schema uses a construct this validator cannot check: {error}"
            ));
            ok = false;
        }
    }

    let fixtures_path = dir
        .join("fixtures")
        .join("controlled-rule-intake.fixtures.json");
    let Ok(fx_raw) = fs::read_to_string(&fixtures_path) else {
        failures.push(
            "controlled-rule-intake: the schema carries no fixtures (registries/operating-model/fixtures/controlled-rule-intake.fixtures.json); a schema no run exercises is not one this gate has reached"
                .to_string(),
        );
        return Outcome { failures };
    };
    if !ok {
        return Outcome { failures };
    }

    let bundle: Value = match serde_json::from_str(&fx_raw) {
        Ok(value) => value,
        Err(error) => {
            failures.push(format!(
                "controlled-rule-intake: the fixtures file is not valid JSON: {error}"
            ));
            return Outcome { failures };
        }
    };
    if !bundle.is_object() {
        failures.push(
            "controlled-rule-intake: the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays"
                .to_string(),
        );
        return Outcome { failures };
    }
    let mut bundle_ok = true;
    for key in ["valid", "invalid"] {
        let is_nonempty = bundle
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if !is_nonempty {
            bundle_ok = false;
            failures.push(format!(
                "controlled-rule-intake: the fixtures file has no non-empty \"{key}\" array"
            ));
        }
    }
    if !bundle.get("resolution").is_some_and(|r| r.is_object()) {
        bundle_ok = false;
        failures.push(
            "controlled-rule-intake: the fixtures file carries no \"resolution\" object; every candidate's source_ref is resolved OUTSIDE the record, and a bundle that resolves nothing cannot exercise the contract against an actual instruction source snapshot"
                .to_string(),
        );
    }
    if !bundle_ok {
        return Outcome { failures };
    }

    let resolver = FixtureSourceResolver {
        resolution_map: &bundle["resolution"],
    };
    let opts = EvalOpts {
        registry_schema: schema.as_ref().unwrap(),
        envelope_schema: envelope.as_ref().unwrap(),
        resolve_source: SourceResolution::Port(&resolver),
    };

    for case in bundle["valid"].as_array().unwrap() {
        let registry = case.get("registry").cloned().unwrap_or(Value::Null);
        let problems = evaluate_controlled_rule_intake(&registry, &opts);
        if !problems.is_empty() {
            failures.push(format!(
                "controlled-rule-intake: a fixture that must be a valid registry was rejected ({}): {}",
                case.get("note").and_then(Value::as_str).unwrap_or(""),
                problems[0].message()
            ));
        }
    }
    for case in bundle["invalid"].as_array().unwrap() {
        let registry = case.get("registry").cloned().unwrap_or(Value::Null);
        let problems = evaluate_controlled_rule_intake(&registry, &opts);
        if problems.is_empty() {
            failures.push(format!(
                "controlled-rule-intake: a fixture that must be rejected validated clean ({})",
                case.get("note").and_then(Value::as_str).unwrap_or("")
            ));
        }
    }

    Outcome { failures }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let kernel_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let outcome = run(kernel_root);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }

    /// Structural test (corrective round item 12): [`FixtureSourceResolver`]
    /// is pure I/O + transport parsing — a flat lookup plus a
    /// `serde_json::from_value` into the app's closed DTO. It carries no
    /// `controlled-rule-intake` business rule (no field-content validation,
    /// no cross-field check): a present-but-malformed entry becomes
    /// `Failed` from the `serde` deserialize error alone, and an absent key
    /// becomes `NotFound` from the plain `Option::None` alone — this file
    /// never inspects the shape further to decide which.
    #[test]
    fn fixture_source_resolver_distinguishes_not_found_from_a_malformed_entry_without_any_business_rule(
    ) {
        use meridian_core::types::SemanticId;
        use meridian_core::types::{ContentDigest, NonEmptyString, Revision};

        let pin = |reference: &str| {
            PinnedSourceRef::new(
                SemanticId::new("src-1").unwrap(),
                NonEmptyString::new(reference).unwrap(),
                Revision::new("a".repeat(40)).unwrap(),
                ContentDigest::from_hex("b".repeat(64)).unwrap(),
            )
        };

        let map = serde_json::json!({
            "sources/malformed": {"record_type": "instruction-source", "id": "src-1"},
        });
        let resolver = FixtureSourceResolver {
            resolution_map: &map,
        };

        assert!(matches!(
            resolver.resolve(&pin("sources/absent")),
            Err(SourceResolverError::NotFound)
        ));
        assert!(matches!(
            resolver.resolve(&pin("sources/malformed")),
            Err(SourceResolverError::Failed(_))
        ));
    }
}
