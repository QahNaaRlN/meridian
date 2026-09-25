//! The actual content of every record unit, reconstructed from the pinned
//! revision alone (`instance-data-migration.md` §10.5,
//! `resolveSourceContent`; `meridian-rust-migration-program-plan.md`
//! §5.22.4, item 6).
//!
//! A unit reference is `<carrier path>` (the whole file) or
//! `<carrier path>#<segment>/<segment>/…` (one fragment of a structured
//! carrier), each segment percent-encoded exactly as ECMAScript
//! `encodeURIComponent`. No carrier path, key name or product fact is known
//! here; the addressing is derived from the pinned document and the
//! declared references together, and must be unambiguous:
//!
//! - an object member is addressed by its own key;
//! - an array element is addressed by the ONE identity that maps the
//!   array's elements one-to-one onto the declared sibling segments — the
//!   0-based or 1-based position, or a scalar member every element carries
//!   (`id`, `alias`, …). No identity, or two that disagree, is a failure,
//!   never a guess;
//! - where every declared reference below a collection ends at its
//!   members, the members and the declared segments must correspond
//!   exactly — a source member no unit accounts for is a failure.
//!
//! Formats: `.yaml`/`.yml` through the strict Kernel YAML adapter, `.json`,
//! and `.jsonl` (the non-blank lines addressed as `line/<1-based n>`). A
//! fragment's content is the canonical JSON of the addressed value; a whole
//! carrier's content is its exact UTF-8 text with a media type by
//! extension. The first stage that cannot resolve a unit records why, per
//! unit; the source is never re-read to fill a gap.

use std::collections::BTreeMap;

use meridian_core::types::ContentDigest;
use serde_json::{Map, Value};

use super::source::SourceError;
use crate::operating_model::migration_boundary::canonical;
use crate::source_format::yaml;

/// One unit's reconstructed content envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnitContent {
    pub media_type: &'static str,
    pub content: String,
    pub digest: ContentDigest,
}

/// A structured carrier that declares itself an applicability register.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Register {
    pub carrier: String,
    pub document: Value,
    /// The unit references of the register's own `records` elements.
    pub record_refs: Vec<String>,
}

/// Every unit's content (or why it could not be resolved) and every
/// applicability register found among the carriers.
#[derive(Debug, Clone, Default)]
pub(crate) struct ResolvedUnits {
    pub contents: BTreeMap<String, Result<UnitContent, String>>,
    pub registers: Vec<Register>,
}

const REGISTER_SCHEMA_FILE: &str = "applicability.schema.json";

/// `encodeURIComponent` of one segment.
pub(crate) fn encode_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(char::from(byte)),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn media_type_for(path: &str) -> &'static str {
    if path.ends_with(".md") {
        "text/markdown"
    } else if path.ends_with(".yaml") || path.ends_with(".yml") {
        "text/yaml"
    } else {
        "text/plain"
    }
}

fn whole_file(path: &str, bytes: &[u8]) -> Result<UnitContent, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| format!("carrier \"{path}\" is not UTF-8 text"))?;
    Ok(UnitContent {
        media_type: media_type_for(path),
        content: text.to_string(),
        digest: ContentDigest::of_bytes(bytes),
    })
}

fn fragment_content(value: &Value) -> UnitContent {
    let content = canonical(value).canonical_text();
    let digest = ContentDigest::of_str(&content);
    UnitContent {
        media_type: "application/json",
        content,
        digest,
    }
}

fn parse_structured(path: &str, bytes: &[u8]) -> Result<Value, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| format!("carrier \"{path}\" is not UTF-8 text"))?;
    if path.ends_with(".yaml") || path.ends_with(".yml") {
        yaml::parse(text).map_err(|e| format!("carrier \"{path}\": {e}"))
    } else if path.ends_with(".jsonl") {
        let mut lines = Vec::new();
        for (i, line) in text.split('\n').enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line)
                .map_err(|e| format!("carrier \"{path}\" line {}: {e}", i + 1))?;
            lines.push(value);
        }
        let mut root = Map::new();
        root.insert("line".to_string(), Value::Array(lines));
        Ok(Value::Object(root))
    } else if path.ends_with(".json") {
        serde_json::from_str(text).map_err(|e| format!("carrier \"{path}\": {e}"))
    } else {
        Err(format!(
            "carrier \"{path}\" has no structured format a fragment can address"
        ))
    }
}

/// One declared reference below the current node.
struct Pending<'a> {
    unit_ref: &'a str,
    rest: &'a [String],
}

fn identity_keys(items: &[Value]) -> Vec<Vec<String>> {
    let mut candidates: Vec<Vec<String>> = vec![
        (0..items.len()).map(|i| i.to_string()).collect(),
        (1..=items.len()).map(|i| i.to_string()).collect(),
    ];
    let mut names: Vec<&String> = items
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|o| o.keys())
        .collect();
    names.sort();
    names.dedup();
    for name in names {
        let keys: Option<Vec<String>> = items
            .iter()
            .map(|item| match item.get(name.as_str()) {
                Some(Value::String(s)) => Some(encode_segment(s)),
                Some(Value::Number(n)) if n.is_i64() || n.is_u64() => {
                    Some(encode_segment(&n.to_string()))
                }
                _ => None,
            })
            .collect();
        if let Some(keys) = keys {
            candidates.push(keys);
        }
    }
    candidates
}

/// The element each declared segment names, when exactly one assignment is
/// admissible.
fn select_elements(
    items: &[Value],
    segments: &[&str],
    complete: bool,
) -> Result<Vec<usize>, String> {
    let mut chosen: Option<Vec<usize>> = None;
    for keys in identity_keys(items) {
        let mut unique = keys.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != keys.len() {
            continue;
        }
        let positions: Option<Vec<usize>> = segments
            .iter()
            .map(|s| keys.iter().position(|k| k == s))
            .collect();
        let Some(positions) = positions else {
            continue;
        };
        if complete && keys.len() != segments.len() {
            continue;
        }
        match &chosen {
            None => chosen = Some(positions),
            Some(previous) if *previous == positions => {}
            Some(_) => {
                return Err(
                    "two element identities address the declared fragments differently".to_string(),
                )
            }
        }
    }
    chosen.ok_or_else(|| {
        if complete {
            "no element identity addresses every element of the collection exactly once".to_string()
        } else {
            "no element identity addresses the declared fragments".to_string()
        }
    })
}

fn resolve_below(
    node: &Value,
    pending: Vec<Pending<'_>>,
    at: &str,
    out: &mut BTreeMap<String, Result<UnitContent, String>>,
) {
    let mut deeper: Vec<Pending<'_>> = Vec::new();
    for p in pending {
        if p.rest.is_empty() {
            out.insert(p.unit_ref.to_string(), Ok(fragment_content(node)));
        } else {
            deeper.push(p);
        }
    }
    if deeper.is_empty() {
        return;
    }
    let mut segments: Vec<&str> = deeper.iter().map(|p| p.rest[0].as_str()).collect();
    segments.sort();
    segments.dedup();
    let complete = deeper.iter().all(|p| p.rest.len() == 1);
    let fail_all = |deeper: Vec<Pending<'_>>,
                    out: &mut BTreeMap<String, Result<UnitContent, String>>,
                    reason: String| {
        for p in deeper {
            out.insert(p.unit_ref.to_string(), Err(format!("{at}: {reason}")));
        }
    };
    let children: Vec<(&str, &Value)> = match node {
        Value::Object(members) => {
            let keys: Vec<(String, &Value)> = members
                .iter()
                .map(|(k, v)| (encode_segment(k), v))
                .collect();
            if complete {
                let mut declared: Vec<&str> = segments.clone();
                declared.sort();
                let mut actual: Vec<&str> = keys.iter().map(|(k, _)| k.as_str()).collect();
                actual.sort();
                if declared != actual {
                    let missing: Vec<&str> = actual
                        .iter()
                        .filter(|k| !declared.contains(k))
                        .copied()
                        .collect();
                    let extra: Vec<&str> = declared
                        .iter()
                        .filter(|k| !actual.contains(k))
                        .copied()
                        .collect();
                    return fail_all(
                        deeper,
                        out,
                        format!(
                            "the declared fragments do not account for the members one-to-one (undeclared: {missing:?}, not in the source: {extra:?})"
                        ),
                    );
                }
            }
            let mut children = Vec::with_capacity(segments.len());
            for segment in &segments {
                match keys.iter().find(|(k, _)| k == segment) {
                    Some((_, v)) => children.push((*segment, *v)),
                    None => {
                        return fail_all(
                            deeper,
                            out,
                            format!("no member \"{segment}\" exists at the pinned revision"),
                        )
                    }
                }
            }
            children
        }
        Value::Array(items) => {
            let selected: Result<Vec<(&str, &Value)>, String> =
                select_elements(items, &segments, complete).and_then(|positions| {
                    segments
                        .iter()
                        .zip(positions)
                        .map(|(s, i)| {
                            items
                                .get(i)
                                .map(|item| (*s, item))
                                .ok_or_else(|| "an element identity named no element".to_string())
                        })
                        .collect()
                });
            match selected {
                Ok(children) => children,
                Err(reason) => return fail_all(deeper, out, reason),
            }
        }
        _ => {
            return fail_all(
                deeper,
                out,
                "a scalar value has no addressable members".to_string(),
            )
        }
    };
    for (segment, child) in children {
        let below: Vec<Pending<'_>> = deeper
            .iter()
            .filter(|p| p.rest[0] == segment)
            .map(|p| Pending {
                unit_ref: p.unit_ref,
                rest: &p.rest[1..],
            })
            .collect();
        resolve_below(child, below, &format!("{at}/{segment}"), out);
    }
}

fn register_of(carrier: &str, document: &Value, refs: &[(&str, Vec<String>)]) -> Option<Register> {
    let schema = document.get("$schema")?.as_str()?;
    let file = schema.rsplit('/').next()?;
    if file != REGISTER_SCHEMA_FILE || !document.get("records")?.is_array() {
        return None;
    }
    Some(Register {
        carrier: carrier.to_string(),
        document: document.clone(),
        record_refs: refs
            .iter()
            .filter(|(_, segments)| segments.len() == 2 && segments[0] == "records")
            .map(|(unit_ref, _)| (*unit_ref).to_string())
            .collect(),
    })
}

/// The units one carrier declares: each reference and its fragment
/// segments (`None` for the whole carrier).
type CarrierUnits<'a> = Vec<(&'a str, Option<Vec<String>>)>;

/// Resolves every unit reference in `unit_refs` through `read(carrier)`.
/// A carrier absent at the pinned revision is a per-unit failure; any other
/// read failure is returned as the input error it is.
pub(crate) fn resolve_units(
    unit_refs: &[&str],
    read: &dyn Fn(&str) -> Result<Vec<u8>, SourceError>,
) -> Result<ResolvedUnits, SourceError> {
    let mut by_carrier: BTreeMap<&str, CarrierUnits<'_>> = BTreeMap::new();
    for unit_ref in unit_refs {
        let (carrier, fragment) = match unit_ref.split_once('#') {
            Some((carrier, fragment)) => (
                carrier,
                Some(fragment.split('/').map(str::to_string).collect()),
            ),
            None => (*unit_ref, None),
        };
        by_carrier
            .entry(carrier)
            .or_default()
            .push((unit_ref, fragment));
    }

    let mut resolved = ResolvedUnits::default();
    for (carrier, units) in by_carrier {
        let fail = |out: &mut BTreeMap<String, Result<UnitContent, String>>, reason: &str| {
            for (unit_ref, _) in &units {
                out.insert((*unit_ref).to_string(), Err(reason.to_string()));
            }
        };
        let mut seen: Vec<&str> = units.iter().map(|(r, _)| *r).collect();
        seen.sort();
        let distinct = {
            let mut d = seen.clone();
            d.dedup();
            d.len()
        };
        if distinct != seen.len() {
            fail(
                &mut resolved.contents,
                &format!("carrier \"{carrier}\": a unit reference is declared more than once"),
            );
            continue;
        }
        let bytes = match read(carrier) {
            Ok(bytes) => bytes,
            Err(SourceError::MissingFile { revision, path }) => {
                fail(
                    &mut resolved.contents,
                    &format!("carrier \"{path}\" does not exist at revision {revision}"),
                );
                continue;
            }
            Err(other) => return Err(other),
        };
        let whole: Vec<&str> = units
            .iter()
            .filter(|(_, f)| f.is_none())
            .map(|(r, _)| *r)
            .collect();
        let fragments: Vec<(&str, Vec<String>)> = units
            .iter()
            .filter_map(|(r, f)| f.clone().map(|f| (*r, f)))
            .collect();
        if !whole.is_empty() && !fragments.is_empty() {
            fail(
                &mut resolved.contents,
                &format!(
                    "carrier \"{carrier}\" is declared both as a whole-file unit and as decomposed fragments"
                ),
            );
            continue;
        }
        let structured = parse_structured(carrier, &bytes);
        if let Ok(document) = &structured {
            if let Some(register) = register_of(carrier, document, &fragments) {
                resolved.registers.push(register);
            }
        }
        if !whole.is_empty() {
            let content = whole_file(carrier, &bytes);
            for unit_ref in whole {
                resolved
                    .contents
                    .insert(unit_ref.to_string(), content.clone());
            }
            continue;
        }
        if fragments
            .iter()
            .any(|(_, f)| f.iter().any(String::is_empty))
        {
            fail(
                &mut resolved.contents,
                &format!("carrier \"{carrier}\": a fragment carries an empty segment"),
            );
            continue;
        }
        let document = match structured {
            Ok(document) => document,
            Err(reason) => {
                fail(&mut resolved.contents, &reason);
                continue;
            }
        };
        let pending = fragments
            .iter()
            .map(|(unit_ref, segments)| Pending {
                unit_ref,
                rest: segments.as_slice(),
            })
            .collect();
        resolve_below(&document, pending, carrier, &mut resolved.contents);
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests;
