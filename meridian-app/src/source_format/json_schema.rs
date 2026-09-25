//! Strict Draft 7 JSON Schema adapter.
//!
//! `jsonschema` compiles every accepted schema. An allowlist and compatibility
//! evaluator then preserve the narrower observable contract of
//! `scripts/lib/json-schema.mjs`, including its diagnostic text.

use std::{collections::HashSet, fmt};

use fancy_regex::Regex;
use serde_json::Value;

pub const SUPPORTED_KEYWORDS: &[&str] = &[
    "$schema",
    "$id",
    "id",
    "title",
    "description",
    "examples",
    "default",
    "comment",
    "$comment",
    "type",
    "properties",
    "required",
    "items",
    "additionalProperties",
    "enum",
    "const",
    "pattern",
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "anyOf",
    "oneOf",
    "allOf",
    "not",
    "$ref",
    "definitions",
    "$defs",
    "format",
    "if",
    "then",
    "else",
    "propertyNames",
    "contains",
];

pub const SUPPORTED_FORMATS: &[&str] = &["date", "date-time", "uri"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedSchema(pub String);

impl fmt::Display for UnsupportedSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UnsupportedSchema {}

/// Reject unsupported surface everywhere, compile as Draft 7, then return
/// diagnostics matching the Node.js reference evaluator.
pub fn validate(data: &Value, schema: &Value) -> Result<Vec<String>, UnsupportedSchema> {
    assert_supported_deep(schema, "/")?;
    validate_refs(schema, schema)?;
    let validator = jsonschema::draft7::new(schema)
        .map_err(|error| UnsupportedSchema(format!("invalid Draft 7 schema: {error}")))?;
    // Compilation is the stable library boundary. Running it also exercises
    // the compiled graph; compatibility diagnostics below remain the public
    // result because library wording is not cross-language stable.
    let _library_verdict = validator.is_valid(data);
    let mut errors = Vec::new();
    validate_node(data, schema, schema, "", &mut errors)?;
    Ok(errors)
}

pub fn assert_supported_deep(schema: &Value, where_: &str) -> Result<(), UnsupportedSchema> {
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    for key in object.keys() {
        if !SUPPORTED_KEYWORDS.contains(&key.as_str()) {
            return Err(UnsupportedSchema(format!(
                "keyword \"{key}\" at {where_} is not implemented by this validator"
            )));
        }
    }
    if let Some(format) = object.get("format").and_then(Value::as_str) {
        if !SUPPORTED_FORMATS.contains(&format) {
            return Err(UnsupportedSchema(format!(
                "format \"{format}\" at {where_} is not implemented by this validator"
            )));
        }
    }
    for keyword in ["properties", "definitions", "$defs"] {
        if let Some(children) = object.get(keyword).and_then(Value::as_object) {
            for (name, child) in children {
                assert_supported_deep(child, &format!("{where_}/{keyword}/{name}"))?;
            }
        }
    }
    for keyword in [
        "items",
        "additionalProperties",
        "not",
        "if",
        "then",
        "else",
        "propertyNames",
        "contains",
    ] {
        if let Some(child) = object.get(keyword).filter(|value| value.is_object()) {
            assert_supported_deep(child, &format!("{where_}/{keyword}"))?;
        }
    }
    for keyword in ["anyOf", "oneOf", "allOf"] {
        if let Some(children) = object.get(keyword).and_then(Value::as_array) {
            for (index, child) in children.iter().enumerate() {
                assert_supported_deep(child, &format!("{where_}/{keyword}/{index}"))?;
            }
        }
    }
    Ok(())
}

fn validate_refs(schema: &Value, root: &Value) -> Result<(), UnsupportedSchema> {
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
        resolve_ref(reference, root)?;
    }
    for keyword in ["properties", "definitions", "$defs"] {
        if let Some(children) = object.get(keyword).and_then(Value::as_object) {
            for child in children.values() {
                validate_refs(child, root)?;
            }
        }
    }
    for keyword in [
        "items",
        "additionalProperties",
        "not",
        "if",
        "then",
        "else",
        "propertyNames",
        "contains",
    ] {
        if let Some(child) = object.get(keyword).filter(|value| value.is_object()) {
            validate_refs(child, root)?;
        }
    }
    for keyword in ["anyOf", "oneOf", "allOf"] {
        if let Some(children) = object.get(keyword).and_then(Value::as_array) {
            for child in children {
                validate_refs(child, root)?;
            }
        }
    }
    Ok(())
}

fn resolve_ref<'a>(reference: &str, root: &'a Value) -> Result<&'a Value, UnsupportedSchema> {
    let Some(pointer) = reference.strip_prefix("#/") else {
        return Err(UnsupportedSchema(format!("external $ref \"{reference}\"")));
    };
    let mut node = root;
    for segment in pointer.split('/') {
        let key = segment.replace("~1", "/").replace("~0", "~");
        let Some(next) = node.get(&key) else {
            return Err(UnsupportedSchema(format!(
                "unresolvable $ref \"{reference}\""
            )));
        };
        node = next;
    }
    Ok(node)
}

fn validate_node(
    data: &Value,
    schema: &Value,
    root: &Value,
    pointer: &str,
    errors: &mut Vec<String>,
) -> Result<(), UnsupportedSchema> {
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
        return validate_node(data, resolve_ref(reference, root)?, root, pointer, errors);
    }
    if let Some(types) = object.get("type") {
        let expected: Vec<&str> = match types {
            Value::String(value) => vec![value],
            Value::Array(values) => values.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        let actual = type_of(data);
        let matches = expected
            .iter()
            .any(|kind| *kind == actual || (*kind == "number" && actual == "integer"));
        if !matches {
            errors.push(format!(
                "{}: expected {}, got {actual}",
                display_pointer(pointer),
                expected.join("|")
            ));
            return Ok(());
        }
    }
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        if !values.iter().any(|value| json_equal(value, data)) {
            errors.push(format!(
                "{}: value {} not in enum",
                display_pointer(pointer),
                json_text(data)
            ));
        }
    }
    if let Some(expected) = object.get("const") {
        if !json_equal(expected, data) {
            errors.push(format!(
                "{}: value must equal {}",
                display_pointer(pointer),
                json_text(expected)
            ));
        }
    }
    if let Some(text) = data.as_str() {
        validate_string(text, object, pointer, errors)?;
    }
    if let Some(number) = data.as_f64() {
        validate_number(number, object, pointer, errors);
    }
    if let Some(values) = data.as_array() {
        validate_array(values, object, root, pointer, errors)?;
    }
    if let Some(values) = data.as_object() {
        validate_object(values, object, root, pointer, errors)?;
    }

    if let Some(schemas) = object.get("allOf").and_then(Value::as_array) {
        for (index, child) in schemas.iter().enumerate() {
            validate_node(
                data,
                child,
                root,
                &format!("{pointer}/allOf/{index}"),
                errors,
            )?;
        }
    }
    for keyword in ["anyOf", "oneOf"] {
        let Some(schemas) = object.get(keyword).and_then(Value::as_array) else {
            continue;
        };
        let mut passing = 0;
        for child in schemas {
            let mut probe = Vec::new();
            validate_node(data, child, root, pointer, &mut probe)?;
            if probe.is_empty() {
                passing += 1;
            }
        }
        if keyword == "anyOf" && passing == 0 {
            errors.push(format!(
                "{}: matches none of anyOf",
                display_pointer(pointer)
            ));
        }
        if keyword == "oneOf" && passing != 1 {
            errors.push(format!(
                "{}: matches {passing} of oneOf (expected exactly 1)",
                display_pointer(pointer)
            ));
        }
    }
    if let Some(child) = object.get("not") {
        let mut probe = Vec::new();
        validate_node(data, child, root, pointer, &mut probe)?;
        if probe.is_empty() {
            errors.push(format!(
                "{}: must not match \"not\" schema",
                display_pointer(pointer)
            ));
        }
    }
    if let Some(condition) = object.get("if") {
        let mut probe = Vec::new();
        validate_node(data, condition, root, pointer, &mut probe)?;
        let branch = if probe.is_empty() {
            object.get("then")
        } else {
            object.get("else")
        };
        if let Some(branch) = branch {
            validate_node(data, branch, root, pointer, errors)?;
        }
    }
    Ok(())
}

fn validate_string(
    text: &str,
    schema: &serde_json::Map<String, Value>,
    pointer: &str,
    errors: &mut Vec<String>,
) -> Result<(), UnsupportedSchema> {
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        let regex = Regex::new(pattern).map_err(|error| {
            UnsupportedSchema(format!("invalid pattern \"{pattern}\": {error}"))
        })?;
        if !regex
            .is_match(text)
            .map_err(|error| UnsupportedSchema(format!("pattern evaluation failed: {error}")))?
        {
            errors.push(format!("{pointer}: does not match /{pattern}/"));
        }
    }
    let length = text.encode_utf16().count() as u64;
    if let Some(minimum) = schema.get("minLength").and_then(Value::as_u64) {
        if length < minimum {
            errors.push(format!("{pointer}: shorter than {minimum}"));
        }
    }
    if let Some(maximum) = schema.get("maxLength").and_then(Value::as_u64) {
        if length > maximum {
            errors.push(format!("{pointer}: longer than {maximum}"));
        }
    }
    if let Some(format) = schema.get("format").and_then(Value::as_str) {
        if !format_valid(format, text) {
            errors.push(format!("{pointer}: does not satisfy format \"{format}\""));
        }
    }
    Ok(())
}

fn validate_number(
    number: f64,
    schema: &serde_json::Map<String, Value>,
    pointer: &str,
    errors: &mut Vec<String>,
) {
    if schema
        .get("minimum")
        .and_then(Value::as_f64)
        .is_some_and(|limit| number < limit)
    {
        errors.push(format!("{pointer}: below minimum"));
    }
    if schema
        .get("maximum")
        .and_then(Value::as_f64)
        .is_some_and(|limit| number > limit)
    {
        errors.push(format!("{pointer}: above maximum"));
    }
    if schema
        .get("exclusiveMinimum")
        .and_then(Value::as_f64)
        .is_some_and(|limit| number <= limit)
    {
        errors.push(format!("{pointer}: at/below exclusiveMinimum"));
    }
    if schema
        .get("exclusiveMaximum")
        .and_then(Value::as_f64)
        .is_some_and(|limit| number >= limit)
    {
        errors.push(format!("{pointer}: at/above exclusiveMaximum"));
    }
}

fn validate_array(
    values: &[Value],
    schema: &serde_json::Map<String, Value>,
    root: &Value,
    pointer: &str,
    errors: &mut Vec<String>,
) -> Result<(), UnsupportedSchema> {
    if schema
        .get("minItems")
        .and_then(Value::as_u64)
        .is_some_and(|limit| values.len() < limit as usize)
    {
        errors.push(format!(
            "{pointer}: fewer than {} items",
            schema["minItems"]
        ));
    }
    if schema
        .get("maxItems")
        .and_then(Value::as_u64)
        .is_some_and(|limit| values.len() > limit as usize)
    {
        errors.push(format!("{pointer}: more than {} items", schema["maxItems"]));
    }
    if schema.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
        let unique: HashSet<String> = values.iter().map(json_text).collect();
        if unique.len() != values.len() {
            errors.push(format!("{pointer}: items are not unique"));
        }
    }
    if let Some(item_schema) = schema.get("items") {
        for (index, value) in values.iter().enumerate() {
            validate_node(
                value,
                item_schema,
                root,
                &format!("{pointer}/{index}"),
                errors,
            )?;
        }
    }
    if let Some(contains) = schema.get("contains") {
        let mut hit = false;
        for value in values {
            let mut probe = Vec::new();
            validate_node(value, contains, root, pointer, &mut probe)?;
            if probe.is_empty() {
                hit = true;
                break;
            }
        }
        if !hit {
            errors.push(format!(
                "{}: no item matches \"contains\"",
                display_pointer(pointer)
            ));
        }
    }
    Ok(())
}

fn validate_object(
    values: &serde_json::Map<String, Value>,
    schema: &serde_json::Map<String, Value>,
    root: &Value,
    pointer: &str,
    errors: &mut Vec<String>,
) -> Result<(), UnsupportedSchema> {
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            if !values.contains_key(name) {
                errors.push(format!("{pointer}/{name}: required property missing"));
            }
        }
    }
    if let Some(name_schema) = schema.get("propertyNames") {
        for name in values.keys() {
            validate_node(
                &Value::String(name.clone()),
                name_schema,
                root,
                &format!("{pointer}/{name}<name>"),
                errors,
            )?;
        }
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    for (name, value) in values {
        if let Some(child) = properties.and_then(|map| map.get(name)) {
            validate_node(value, child, root, &format!("{pointer}/{name}"), errors)?;
        } else if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
            errors.push(format!("{pointer}/{name}: additional property not allowed"));
        } else if let Some(child) = schema
            .get("additionalProperties")
            .filter(|value| value.is_object())
        {
            validate_node(value, child, root, &format!("{pointer}/{name}"), errors)?;
        }
    }
    Ok(())
}

fn format_valid(format: &str, value: &str) -> bool {
    match format {
        "date" => {
            let parts: Vec<_> = value.split('-').collect();
            parts.len() == 3
                && parts[0].len() == 4
                && parts[1].len() == 2
                && parts[2].len() == 2
                && parts
                    .iter()
                    .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
                && parts[1]
                    .parse::<u8>()
                    .is_ok_and(|month| (1..=12).contains(&month))
                && parts[2]
                    .parse::<u8>()
                    .is_ok_and(|day| (1..=31).contains(&day))
        }
        "date-time" => date_time_valid(value),
        "uri" => value.split_once(':').is_some_and(|(scheme, rest)| {
            !scheme.is_empty()
                && !rest.is_empty()
                && scheme.bytes().enumerate().all(|(index, byte)| {
                    if index == 0 {
                        byte.is_ascii_alphabetic()
                    } else {
                        byte.is_ascii_alphanumeric() || b"+.-".contains(&byte)
                    }
                })
        }),
        _ => false,
    }
}

fn date_time_valid(value: &str) -> bool {
    let regex = Regex::new(
        r"^\d{4}-(\d{2})-(\d{2})[Tt](\d{2}):(\d{2}):(\d{2})(\.\d+)?([Zz]|[+-](\d{2}):(\d{2}))$",
    )
    .expect("static date-time regex");
    let Ok(Some(captures)) = regex.captures(value) else {
        return false;
    };
    let number = |index| {
        captures
            .get(index)
            .and_then(|capture| capture.as_str().parse::<u8>().ok())
    };
    number(1).is_some_and(|month| (1..=12).contains(&month))
        && number(2).is_some_and(|day| (1..=31).contains(&day))
        && number(3).is_some_and(|hour| hour <= 23)
        && number(4).is_some_and(|minute| minute <= 59)
        && number(5).is_some_and(|second| second <= 59)
        && number(8).is_none_or(|hour| hour <= 23)
        && number(9).is_none_or(|minute| minute <= 59)
}

fn type_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn display_pointer(pointer: &str) -> &str {
    if pointer.is_empty() {
        "/"
    } else {
        pointer
    }
}
fn json_text(value: &Value) -> String {
    serde_json::to_string(value).expect("JSON value serializes")
}
fn json_equal(left: &Value, right: &Value) -> bool {
    json_text(left) == json_text(right)
}

#[cfg(test)]
mod tests {
    use super::{assert_supported_deep, validate};
    use serde_json::json;

    #[test]
    fn validates_supported_schema_with_reference_diagnostics() {
        let schema = json!({"type":"object","required":["id"],"properties":{"id":{"type":"string","minLength":2}},"additionalProperties":false});
        assert_eq!(
            validate(&json!({"id":"x","extra":true}), &schema).unwrap(),
            vec![
                "/extra: additional property not allowed",
                "/id: shorter than 2"
            ]
        );
    }

    #[test]
    fn rejects_unsupported_keywords_in_unvisited_branches() {
        let schema = json!({"properties":{"optional":{"type":"string","minProperties":1}}});
        assert!(assert_supported_deep(&schema, "/").is_err());
    }

    #[test]
    fn rejects_unknown_formats_and_external_references() {
        assert!(validate(&json!("x"), &json!({"type":"string","format":"email"})).is_err());
        assert!(validate(&json!(1), &json!({"$ref":"https://example.test/schema"})).is_err());
    }
}
