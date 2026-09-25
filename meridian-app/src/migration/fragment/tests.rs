use super::*;

fn files(path: &str) -> Result<Vec<u8>, SourceError> {
    let text = match path {
        "notes/readme.md" => "# Readme\r\nline\n",
        "inventory/items.yaml" => "items:\n  - id: alpha\n    size: 1\n  - id: beta gamma\n    size: 2\n",
        "log.jsonl" => "{\"b\":1,\"a\":2}\n\n{\"c\":3}\n",
        "map.yaml" => "things:\n  one:\n    v: 1\n  two:\n    v: 2\n",
        "register.yaml" => "$schema: ./applicability.schema.json\nschema_version: 1\nrecords:\n  - x: 1\n  - x: 2\n",
        "twins.yaml" => "items:\n  - id: a\n    alias: b\n  - id: b\n    alias: a\n",
        _ => {
            return Err(SourceError::MissingFile {
                revision: "r".to_string(),
                path: path.to_string(),
            })
        }
    };
    Ok(text.as_bytes().to_vec())
}

fn resolve(refs: &[&str]) -> ResolvedUnits {
    resolve_units(refs, &files).unwrap()
}

fn ok(units: &ResolvedUnits, unit_ref: &str) -> UnitContent {
    units.contents[unit_ref].clone().unwrap()
}

fn err(units: &ResolvedUnits, unit_ref: &str) -> String {
    units.contents[unit_ref].clone().unwrap_err()
}

#[test]
fn migration_fragment_segments_are_encoded_like_encode_uri_component() {
    assert_eq!(encode_segment("Sample Service"), "Sample%20Service");
    assert_eq!(encode_segment("a-b_c.d!e~f*g'h(i)j"), "a-b_c.d!e~f*g'h(i)j");
    assert_eq!(encode_segment("ж/#"), "%D0%B6%2F%23");
}

#[test]
fn migration_fragment_whole_carrier_keeps_its_exact_bytes() {
    let units = resolve(&["notes/readme.md"]);
    let content = ok(&units, "notes/readme.md");
    assert_eq!(content.media_type, "text/markdown");
    assert_eq!(content.content, "# Readme\r\nline\n");
    assert_eq!(content.digest, ContentDigest::of_str("# Readme\r\nline\n"));
}

#[test]
fn migration_fragment_elements_are_addressed_by_their_one_identity() {
    let units = resolve(&[
        "inventory/items.yaml#items/alpha",
        "inventory/items.yaml#items/beta%20gamma",
    ]);
    let alpha = ok(&units, "inventory/items.yaml#items/alpha");
    assert_eq!(alpha.media_type, "application/json");
    assert_eq!(alpha.content, "{\"id\":\"alpha\",\"size\":1}");
    assert_eq!(alpha.digest, ContentDigest::of_str(&alpha.content));
}

#[test]
fn migration_fragment_jsonl_lines_and_map_members_are_addressed() {
    let units = resolve(&["log.jsonl#line/1", "log.jsonl#line/2"]);
    assert_eq!(ok(&units, "log.jsonl#line/1").content, "{\"a\":2,\"b\":1}");
    let units = resolve(&["map.yaml#things/one", "map.yaml#things/two"]);
    assert_eq!(ok(&units, "map.yaml#things/two").content, "{\"v\":2}");
}

#[test]
fn migration_fragment_an_undeclared_member_fails_the_whole_collection() {
    let units = resolve(&["inventory/items.yaml#items/alpha"]);
    assert!(err(&units, "inventory/items.yaml#items/alpha").contains("exactly once"));
    let units = resolve(&["map.yaml#things/one"]);
    assert!(err(&units, "map.yaml#things/one").contains("one-to-one"));
}

#[test]
fn migration_fragment_ambiguous_identities_fail_closed() {
    let units = resolve(&["twins.yaml#items/a", "twins.yaml#items/b"]);
    assert!(err(&units, "twins.yaml#items/a").contains("differently"));
}

#[test]
fn migration_fragment_missing_carriers_and_mixed_declarations_fail_per_unit() {
    let units = resolve(&["gone.md"]);
    assert!(err(&units, "gone.md").contains("does not exist"));
    let units = resolve(&["map.yaml", "map.yaml#things/one"]);
    assert!(err(&units, "map.yaml").contains("both"));
}

#[test]
fn migration_fragment_an_applicability_register_is_recognised_by_its_own_schema() {
    let units = resolve(&["register.yaml#records/0", "register.yaml#records/1"]);
    assert_eq!(units.registers.len(), 1);
    assert_eq!(
        units.registers[0].record_refs,
        vec!["register.yaml#records/0", "register.yaml#records/1"]
    );
    assert_eq!(ok(&units, "register.yaml#records/1").content, "{\"x\":2}");
}
