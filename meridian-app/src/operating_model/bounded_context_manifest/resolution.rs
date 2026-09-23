//! The fixture bundle's `resolution` map is TRANSPORT: this module turns it
//! into the typed [`ResolutionCatalogue`] `meridian-core` checks. A response
//! is not schema-validated (its closed contract is the core's domain rule),
//! so each known field becomes a [`ResponseField`] — the expected shape, or
//! a [`ForeignValue`] carrying only its JSON kind and rendered text for the
//! diagnostic — and every other key is kept by name only. A non-object
//! entry is not a record and does not resolve (the Node reference's
//! resolver returns `null` for it).

use meridian_core::run_contracts::{
    ForeignValue, ResolutionCatalogue, ResolvedEntry, ResolvedStateField, ResolvedStateResponse,
    ResponseField, ResponseItem, ResponseValueKind,
};
use serde_json::{Map, Value};

const KNOWN_ENTRY_FIELDS: [&str; 8] = [
    "record_type",
    "id",
    "reference",
    "revision",
    "content_digest",
    "source_bytes",
    "resolved_state",
    "linked_run_ref",
];

fn kind(value: &Value) -> ResponseValueKind {
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
fn string_coercion(value: &Value) -> String {
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

fn foreign(value: &Value) -> ForeignValue {
    ForeignValue::new(kind(value), value.to_string(), string_coercion(value))
}

fn field<T>(value: Option<&Value>, expected: impl FnOnce(&Value) -> Option<T>) -> ResponseField<T> {
    match value {
        None => ResponseField::Absent,
        Some(v) => expected(v).map_or_else(
            || ResponseField::Foreign(foreign(v)),
            ResponseField::Present,
        ),
    }
}

fn text_field(value: Option<&Value>) -> ResponseField<String> {
    field(value, |v| v.as_str().map(str::to_string))
}

fn step_field(value: Option<&Value>) -> ResponseField<Option<String>> {
    field(value, |v| match v {
        Value::Null => Some(None),
        Value::String(s) => Some(Some(s.clone())),
        _ => None,
    })
}

fn items_field(value: Option<&Value>) -> ResponseField<Vec<ResponseItem>> {
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

fn unknown_keys(object: &Map<String, Value>, known: &[&str]) -> Vec<String> {
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
        next_action: step_field(state.get("next_action")),
        next_gate: step_field(state.get("next_gate")),
        blocker_ids: items_field(state.get("blocker_ids")),
        resolved_norms: items_field(state.get("resolved_norms")),
        completed_checks: items_field(state.get("completed_checks")),
        unknown_fields: unknown_keys(state, &ResolvedStateField::NAMES),
    }
}

fn resolved_entry(entry: &Map<String, Value>) -> ResolvedEntry {
    ResolvedEntry {
        record_type: text_field(entry.get("record_type")),
        id: text_field(entry.get("id")),
        reference: text_field(entry.get("reference")),
        revision: text_field(entry.get("revision")),
        content_digest: text_field(entry.get("content_digest")),
        source_bytes: text_field(entry.get("source_bytes")),
        resolved_state: field(entry.get("resolved_state"), |v| {
            v.as_object().map(resolved_state)
        }),
        linked_run_ref: text_field(entry.get("linked_run_ref")),
        other_fields: unknown_keys(entry, &KNOWN_ENTRY_FIELDS),
    }
}

/// The typed catalogue of one bundle's `resolution` object.
pub(super) fn catalogue(resolution: &Map<String, Value>) -> ResolutionCatalogue {
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
