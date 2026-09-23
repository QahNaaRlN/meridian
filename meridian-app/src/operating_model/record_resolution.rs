//! The ONE transport-to-resolution conversion of every resolving family —
//! `bounded-context-manifest`, `evidence-and-handoff-contract`,
//! `meridian-field-evaluation`, and (through the field helpers below) the
//! resolver maps of the four migration/qualification families. A fixture bundle's `resolution` map is
//! TRANSPORT: this module turns it into the typed [`ResolutionCatalogue`]
//! `meridian-core` checks.
//!
//! A response is not schema-validated (its closed contract is the core's
//! domain rule), so each known field becomes a [`ResponseField`] — the
//! expected shape, or a [`ForeignValue`] carrying only its JSON kind and
//! rendered text for the diagnostic — and every other key is kept by name
//! only, in sorted order. A non-object entry is not a record and does not
//! resolve (the Node reference's resolver returns `null` for it).

use meridian_core::run_contracts::{
    EvidenceResultResponse, ForeignValue, JsonNumber, MeasurementFields, ObservationResponse,
    ResolutionCatalogue, ResolvedEntry, ResolvedStateField, ResolvedStateResponse, ResponseField,
    ResponseItem, ResponseValueKind, SpecificationResponse, WindowFields, KNOWN_RESPONSE_KEYS,
};
use serde_json::{Map, Value};

pub(crate) fn kind(value: &Value) -> ResponseValueKind {
    match value {
        Value::Null => ResponseValueKind::Null,
        Value::Bool(_) => ResponseValueKind::Boolean,
        Value::Number(_) => ResponseValueKind::Number,
        Value::String(_) => ResponseValueKind::String,
        Value::Array(_) => ResponseValueKind::Array,
        Value::Object(_) => ResponseValueKind::Object,
    }
}

/// `String(x)` for a JSON value, as a template literal interpolates it.
pub(crate) fn string_coercion(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Null => String::new(),
                other => string_coercion(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

pub(crate) fn foreign(value: &Value) -> ForeignValue {
    ForeignValue::new(kind(value), value.to_string(), string_coercion(value))
}

pub(crate) fn field<T>(
    value: Option<&Value>,
    expected: impl FnOnce(&Value) -> Option<T>,
) -> ResponseField<T> {
    match value {
        None => ResponseField::Absent,
        Some(v) => expected(v).map_or_else(
            || ResponseField::Foreign(foreign(v)),
            ResponseField::Present,
        ),
    }
}

/// A field whose JSON `null` is `Present(None)`.
pub(crate) fn nullable<T>(
    value: Option<&Value>,
    expected: impl FnOnce(&Value) -> Option<T>,
) -> ResponseField<Option<T>> {
    field(value, |v| match v {
        Value::Null => Some(None),
        other => expected(other).map(Some),
    })
}

pub(crate) fn text_field(value: Option<&Value>) -> ResponseField<String> {
    field(value, |v| v.as_str().map(str::to_string))
}

/// A JSON number as the core reads it: its value and its JSON text.
pub(crate) fn json_number(value: &Value) -> Option<JsonNumber> {
    let Value::Number(n) = value else {
        return None;
    };
    JsonNumber::new(n.as_f64()?, n.to_string())
}

fn number_field(value: Option<&Value>) -> ResponseField<JsonNumber> {
    field(value, json_number)
}

pub(crate) fn items_field(value: Option<&Value>) -> ResponseField<Vec<ResponseItem>> {
    field(value, |v| {
        v.as_array().map(|items| {
            items
                .iter()
                .map(|item| match item {
                    Value::String(s) => ResponseItem::Text(s.clone()),
                    other => ResponseItem::Foreign(foreign(other)),
                })
                .collect()
        })
    })
}

pub(crate) fn unknown_keys(object: &Map<String, Value>, known: &[&str]) -> Vec<String> {
    let mut keys: Vec<String> = object
        .keys()
        .filter(|k| !known.contains(&k.as_str()))
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn resolved_state(state: &Map<String, Value>) -> ResolvedStateResponse {
    ResolvedStateResponse {
        task_specification_ref: text_field(state.get("task_specification_ref")),
        scope_revision: field(state.get("scope_revision"), Value::as_u64),
        lifecycle_stage: text_field(state.get("lifecycle_stage")),
        work_status: text_field(state.get("work_status")),
        next_action: nullable(state.get("next_action"), |v| v.as_str().map(str::to_string)),
        next_gate: nullable(state.get("next_gate"), |v| v.as_str().map(str::to_string)),
        blocker_ids: items_field(state.get("blocker_ids")),
        resolved_norms: items_field(state.get("resolved_norms")),
        completed_checks: items_field(state.get("completed_checks")),
        unknown_fields: unknown_keys(state, &ResolvedStateField::NAMES),
    }
}

/// A measurement object's fields — also how a record's own schema-clean
/// measurement reaches the one measurement rule.
pub(crate) fn measurement_fields(m: &Map<String, Value>) -> MeasurementFields {
    MeasurementFields {
        kind: text_field(m.get("kind")),
        outcome: text_field(m.get("outcome")),
        basis: text_field(m.get("basis")),
        duration_kind: text_field(m.get("duration_kind")),
        unit: text_field(m.get("unit")),
        seconds: number_field(m.get("seconds")),
        paused_seconds: number_field(m.get("paused_seconds")),
        start_ts: text_field(m.get("start_ts")),
        end_ts: text_field(m.get("end_ts")),
        value: number_field(m.get("value")),
    }
}

fn window_fields(w: &Map<String, Value>) -> WindowFields {
    WindowFields {
        start_date: text_field(w.get("start_date")),
        end_date: text_field(w.get("end_date")),
    }
}

fn resolved_entry(entry: &Map<String, Value>) -> ResolvedEntry {
    let text = |key: &str| text_field(entry.get(key));
    ResolvedEntry {
        record_type: text("record_type"),
        id: text("id"),
        reference: text("reference"),
        revision: text("revision"),
        content_digest: text("content_digest"),
        source_bytes: text("source_bytes"),
        resolved_state: field(entry.get("resolved_state"), |v| {
            v.as_object().map(resolved_state)
        }),
        linked_run_ref: text("linked_run_ref"),
        specification: SpecificationResponse {
            acceptance_criteria: items_field(entry.get("acceptance_criteria")),
            mandatory_checks: items_field(entry.get("mandatory_checks")),
        },
        evidence_result: EvidenceResultResponse {
            observed_result: text("observed_result"),
            covers: items_field(entry.get("covers")),
            check_ref: text("check_ref"),
            specialised_contract: text("specialised_contract"),
            recorded_verdict: text("recorded_verdict"),
            metric_ref: text("metric_ref"),
        },
        observation: ObservationResponse {
            metric_id: text("metric_id"),
            status: text("status"),
            measurement: nullable(entry.get("measurement"), |v| {
                v.as_object().map(measurement_fields)
            }),
            workspace_id: text("workspace_id"),
            observed_at: text("observed_at"),
            supersedes_id: nullable(entry.get("supersedes_id"), |v| {
                v.as_str().map(str::to_string)
            }),
            observation_period: nullable(entry.get("observation_period"), |v| {
                v.as_object().map(window_fields)
            }),
            coverage: nullable(entry.get("coverage"), |v| v.as_str().map(str::to_string)),
        },
        other_fields: unknown_keys(entry, &KNOWN_RESPONSE_KEYS),
    }
}

/// The typed catalogue of one bundle's `resolution` object.
pub(crate) fn catalogue(resolution: &Map<String, Value>) -> ResolutionCatalogue {
    let mut catalogue = ResolutionCatalogue::new();
    for (reference, entry) in resolution {
        if let Some(entry) = entry.as_object() {
            catalogue.insert(reference.clone(), resolved_entry(entry));
        }
    }
    catalogue
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn one(entry: Value) -> ResolvedEntry {
        let map = json!({ "r": entry });
        let catalogue = catalogue(map.as_object().unwrap());
        catalogue.resolve("r").cloned().unwrap()
    }

    #[test]
    fn known_fields_are_typed_and_foreign_kinds_are_kept_for_the_diagnostic() {
        let entry = one(json!({
            "record_type": "execution-run",
            "revision": 7,
            "source_bytes": null,
            "zzzz": 1,
            "aaaa": 2,
            "resolved_state": { "scope_revision": -1, "next_action": null, "blocker_ids": ["a", 3], "extra": true }
        }));
        assert_eq!(
            entry.record_type,
            ResponseField::Present("execution-run".to_string())
        );
        assert!(
            matches!(&entry.revision, ResponseField::Foreign(f) if f.kind() == ResponseValueKind::Number)
        );
        assert!(
            matches!(&entry.source_bytes, ResponseField::Foreign(f) if f.kind() == ResponseValueKind::Null)
        );
        assert_eq!(entry.other_fields, ["aaaa", "zzzz"]);
        let ResponseField::Present(state) = &entry.resolved_state else {
            panic!("resolved_state is an object");
        };
        assert!(matches!(&state.scope_revision, ResponseField::Foreign(_)));
        assert_eq!(state.next_action, ResponseField::Present(None));
        assert_eq!(state.unknown_fields, ["extra"]);
        let ResponseField::Present(items) = &state.blocker_ids else {
            panic!("blocker_ids is an array");
        };
        assert_eq!(items[0], ResponseItem::Text("a".to_string()));
        assert!(matches!(items[1], ResponseItem::Foreign(_)));
    }

    #[test]
    fn evidence_and_observation_extensions_are_typed_and_null_stays_distinct() {
        let entry = one(json!({
            "record_type": "field-evaluation-observation",
            "observed_result": "confirmed",
            "covers": ["a-1"],
            "measurement": { "kind": "count", "unit": "count", "value": 2.0 },
            "supersedes_id": null,
            "observation_period": "not-an-object",
            "unexpected": true
        }));
        assert_eq!(
            entry.evidence_result.observed_result,
            ResponseField::Present("confirmed".to_string())
        );
        let ResponseField::Present(Some(m)) = &entry.observation.measurement else {
            panic!("measurement is an object");
        };
        assert!(matches!(&m.value, ResponseField::Present(n) if n.value() == 2.0));
        assert_eq!(
            entry.observation.supersedes_id,
            ResponseField::Present(None)
        );
        assert!(matches!(
            &entry.observation.observation_period,
            ResponseField::Foreign(_)
        ));
        assert_eq!(entry.other_fields, ["unexpected"]);
    }

    #[test]
    fn a_non_object_entry_does_not_resolve() {
        let map = json!({ "r": "not a record", "s": {} });
        let catalogue = catalogue(map.as_object().unwrap());
        assert!(catalogue.resolve("r").is_none());
        assert!(catalogue.resolve("s").is_some());
    }

    #[test]
    fn string_coercion_follows_template_interpolation() {
        assert_eq!(string_coercion(&json!([1, null, "a"])), "1,,a");
        assert_eq!(string_coercion(&json!({"a": 1})), "[object Object]");
    }
}
