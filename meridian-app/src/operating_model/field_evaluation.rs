//! Business-contract migration of `scripts/lib/field-evaluation.mjs`: the
//! composite-consistency algorithm behind the `meridian-field-evaluation`
//! check (package 7, subpackage 7c).
//! `registries/operating-model/field-evaluation.schema.json` is the
//! COMPLETE schema for TWO record types that compose the practical-evaluation
//! contract:
//!   - `record_type: field-evaluation-observation` — one measured data point
//!     about ONE metric of ONE execution run, pinned to that run and
//!     grounded by externally-resolved evidence;
//!   - `record_type: field-evaluation-report` — a DETERMINISTIC aggregation,
//!     scoped to a project workspace over a stated period, built from a
//!     pinned set of included observations (and an explicit, reasoned set of
//!     excluded ones), never a single rolled-up score.
//!
//! Evaluation DATA is Instance data; the Kernel ships the schema, the
//! product-neutral fixtures and this module.
//!
//! The EIGHT characteristics of plan §10 are modelled SEPARATELY, never
//! folded into one score: mechanism-correctness, context-entry-time,
//! rework-returns, stop-correctness, missed-norms, resumption-success,
//! owner-cost, post-acceptance-defects. Each is a CLOSED measurement kind —
//! classification / duration / count — and a classification metric's
//! outcome pool is closed PER METRIC. An observation records ONE instance;
//! the REPORT aggregates many observations into per-metric counts and
//! rates, RECOMPUTED from the resolved observations, never taken on the
//! report's own say-so.
//!
//! The `$schema` resolution and rooted-path rules, and the CLOSED,
//! fail-closed exact-revision rule, are REUSED from
//! [`super::bounded_context_manifest`] and [`super::task_specification`] —
//! this module carries no divergent second copy of either. This module only
//! ports `evaluateFieldEvaluation` (the gate `scripts/kernel-validate.mjs`
//! calls); `buildFieldEvaluationReport`, the Node reference's deterministic
//! report BUILDER, is not part of the `validate` gate contract and is not
//! ported here.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::bounded_context_manifest::compat_7c::{
    classify_revision, RecordResolver, RESOLVED_ENTRY_COMMON_KEYS,
};
use super::task_specification::{non_portable_reason, resolve_schema_ref};
use crate::source_format::json_schema;

pub const OBSERVATION_RECORD_TYPE: &str = "field-evaluation-observation";
pub const REPORT_RECORD_TYPE: &str = "field-evaluation-report";
pub const RECORD_TYPES: [&str; 2] = [OBSERVATION_RECORD_TYPE, REPORT_RECORD_TYPE];

pub const CANONICAL_RECORD_BASE: &str = "records/field-evaluation";
pub const SCHEMA_NAMESPACE_DIR: &str = "registries/operating-model";
pub const EXPECTED_SCHEMA_BASENAME: &str = "field-evaluation.schema.json";
pub const ENVELOPE_SCHEMA_BASENAME: &str = "scoped-record.schema.json";

fn expected_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{EXPECTED_SCHEMA_BASENAME}")
}
fn envelope_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{ENVELOPE_SCHEMA_BASENAME}")
}

pub const OBSERVATION_SCOPE_TYPE: &str = "run-state";
pub const REPORT_SCOPE_TYPE: &str = "project-workspace";

fn scope_rejection_reason(scope_type: &str, want_scope_type: &str, label: &str) -> String {
    match scope_type {
        "built-in-methodology" => "built-in-methodology is Kernel methodology, not a place for practical-evaluation data".to_string(),
        "user-profile" => "user-profile holds a user's rules and settings, not practical-evaluation data".to_string(),
        "organization-profile" => "organization-profile holds an organisation's rules and settings, not practical-evaluation data".to_string(),
        "repository-scope" => "repository-scope holds facts true for one repository; evaluation data is scoped elsewhere".to_string(),
        _ => format!("a {label} lives in {want_scope_type} only"),
    }
}

pub const METRIC_IDS: [&str; 8] = [
    "mechanism-correctness",
    "context-entry-time",
    "rework-returns",
    "stop-correctness",
    "missed-norms",
    "resumption-success",
    "owner-cost",
    "post-acceptance-defects",
];

pub const MEASUREMENT_KINDS: [&str; 3] = ["classification", "duration", "count"];

fn metric_measurement_kind(metric_id: &str) -> Option<&'static str> {
    match metric_id {
        "mechanism-correctness" => Some("classification"),
        "context-entry-time" => Some("duration"),
        "rework-returns" => Some("count"),
        "stop-correctness" => Some("classification"),
        "missed-norms" => Some("classification"),
        "resumption-success" => Some("classification"),
        "owner-cost" => Some("duration"),
        "post-acceptance-defects" => Some("count"),
        _ => None,
    }
}

/// Only `post-acceptance-defects` names an explicit observation window and a
/// coverage completeness flag — the other count metric (`rework-returns`) is
/// a per-run tally and carries neither.
pub const METRICS_REQUIRING_WINDOW: [&str; 1] = ["post-acceptance-defects"];

fn classification_outcomes(metric_id: &str) -> &'static [&'static str] {
    match metric_id {
        "mechanism-correctness" => &["correct", "incorrect", "unknown"],
        "stop-correctness" => &["correct_stop", "false_stop", "unknown"],
        "missed-norms" => &["missed", "not_missed", "unknown"],
        "resumption-success" => &["successful", "unsuccessful", "unknown"],
        _ => &[],
    }
}

/// The single outcome each classification metric's primary rate tracks.
fn classification_tracked_outcome(metric_id: &str) -> Option<&'static str> {
    match metric_id {
        "mechanism-correctness" => Some("correct"),
        "stop-correctness" => Some("false_stop"),
        "missed-norms" => Some("missed"),
        "resumption-success" => Some("successful"),
        _ => None,
    }
}

pub const OBSERVATION_STATUSES: [&str; 4] =
    ["observed", "unknown", "not_applicable", "unmeasurable"];
pub const DURATION_KINDS: [&str; 2] = ["calendar", "active"];
pub const COVERAGE_VALUES: [&str; 2] = ["complete", "partial"];
pub const EVIDENCE_KINDS: [&str; 4] = [
    "check-run",
    "observation",
    "artifact-inspection",
    "external-confirmation",
];
pub const OBSERVED_RESULTS: [&str; 3] = ["confirmed", "contradicted", "inconclusive"];
pub const METRIC_REPORT_STATUSES: [&str; 3] = ["known", "unknown", "not_applicable"];

fn pinned_ref_slot_record_type(field: &str) -> Option<&'static str> {
    match field {
        "execution_run_ref" => Some("execution-run"),
        "supersedes" | "included_observation" | "excluded_observation" => {
            Some(OBSERVATION_RECORD_TYPE)
        }
        _ => None,
    }
}

fn allowed_entry_keys(wanted: &str) -> Vec<&'static str> {
    let mut keys = RESOLVED_ENTRY_COMMON_KEYS.to_vec();
    match wanted {
        t if t == OBSERVATION_RECORD_TYPE => {
            keys.extend([
                "metric_id",
                "status",
                "measurement",
                "workspace_id",
                "observed_at",
                "supersedes_id",
                "observation_period",
                "coverage",
            ]);
        }
        "evidence-result" => {
            keys.push("observed_result");
            keys.push("metric_ref");
        }
        _ => {}
    }
    keys
}

/// Fields that would turn a bounded evaluation record into an unbounded
/// archive, duplicate a later package's subject matter, or smuggle in a
/// single rolled-up score or an unwarranted release verdict this contract
/// forbids by design (plan §10: "единый итоговый балл не вводится").
pub const FORBIDDEN_PAYLOAD_FIELDS: [&str; 39] = [
    "file_contents",
    "full_text",
    "full_texts",
    "raw_context",
    "context_dump",
    "command_log",
    "commands",
    "transcript",
    "messages",
    "chat_history",
    "conversation",
    "directory_dump",
    "dir_listing",
    "tree_dump",
    "attachments",
    "score",
    "overall_score",
    "total_score",
    "composite_score",
    "grade",
    "readiness",
    "release_ready",
    "release_readiness",
    "pass_fail",
    "verdict",
    "go_no_go",
    "rating",
    "rank",
    "task_specification",
    "execution_run",
    "human_control",
    "context_manifest",
    "claimed_results",
    "verifiable_assertions",
    "mandatory_checks",
    "acceptance_criteria",
    "outcome",
    "next_step",
    "worktree_disposition",
];

static SEMANTIC_ID_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$")
        .expect("semantic id pattern compiles")
});
static SHA256_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(r"^[0-9a-fA-F]{64}$").expect("sha256 pattern compiles")
});
static DATE_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(r"^(\d{4})-(\d{2})-(\d{2})$").expect("date pattern compiles")
});
static DATETIME_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(
        r"^(\d{4})-(\d{2})-(\d{2})[Tt](\d{2}):(\d{2}):(\d{2})(?:\.(\d+))?(?:[Zz]|([+-])(\d{2}):(\d{2}))$",
    )
    .expect("datetime pattern compiles")
});

fn is_semantic_id(s: &str) -> bool {
    SEMANTIC_ID_RE.is_match(s).unwrap_or(false)
}
fn is_object(v: &Value) -> bool {
    v.is_object()
}
fn blank(v: &Value) -> bool {
    match v.as_str() {
        Some(s) => s.trim().is_empty(),
        None => true,
    }
}
fn has_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}
fn is_non_neg_int(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i >= 0
            } else {
                n.as_u64().is_some()
            }
        }
        _ => false,
    }
}
fn is_non_neg_number(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f.is_finite() && f >= 0.0),
        _ => false,
    }
}
fn is_valid_sha256(s: Option<&str>) -> bool {
    s.is_some_and(|s| SHA256_RE.is_match(s.trim()).unwrap_or(false))
}

fn json_stringify(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_string(),
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "undefined".to_string()),
    }
}
fn json_quote(s: Option<&str>) -> String {
    match s {
        None => "null".to_string(),
        Some(s) => serde_json::to_string(s).unwrap_or_default(),
    }
}
fn js_typeof(v: &Value) -> &'static str {
    match v {
        Value::Null => "object",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "object",
        Value::Object(_) => "object",
    }
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
const DAYS_IN_MONTH: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
fn is_valid_calendar_date(y: i64, mo: i64, d: i64) -> bool {
    if !(1..=12).contains(&mo) {
        return false;
    }
    let max = if mo == 2 && is_leap_year(y) {
        29
    } else {
        DAYS_IN_MONTH[(mo - 1) as usize]
    };
    (1..=max).contains(&d)
}

/// A date string is valid ONLY when it is well-formed AND names a calendar
/// date that actually exists. Port of `isValidDate`.
pub fn is_valid_date(s: Option<&str>) -> bool {
    let Some(s) = s else { return false };
    let Ok(Some(caps)) = DATE_RE.captures(s) else {
        return false;
    };
    let y: i64 = caps[1].parse().unwrap_or(0);
    let mo: i64 = caps[2].parse().unwrap_or(0);
    let d: i64 = caps[3].parse().unwrap_or(0);
    is_valid_calendar_date(y, mo, d)
}

/// The same calendar-existence rule for a date-TIME string. Port of
/// `isValidDateTime`.
pub fn is_valid_date_time(s: Option<&str>) -> bool {
    let Some(s) = s else { return false };
    let Ok(Some(caps)) = DATETIME_RE.captures(s) else {
        return false;
    };
    let y: i64 = caps[1].parse().unwrap_or(0);
    let mo: i64 = caps[2].parse().unwrap_or(0);
    let d: i64 = caps[3].parse().unwrap_or(0);
    let h: i64 = caps[4].parse().unwrap_or(0);
    let mi: i64 = caps[5].parse().unwrap_or(0);
    let se: i64 = caps[6].parse().unwrap_or(0);
    if !is_valid_calendar_date(y, mo, d) {
        return false;
    }
    h <= 23 && mi <= 59 && se <= 59
}

/// Days since the Unix epoch (1970-01-01) for a proleptic-Gregorian civil
/// date, by Howard Hinnant's `days_from_civil` algorithm — the same
/// proleptic-Gregorian rule `is_leap_year` above already uses, so this
/// agrees with it by construction. No external date/time crate: the two
/// call sites below need only millisecond-ORDER comparison of two already
/// format-and-range-validated timestamps, never calendar arithmetic
/// (adding days/months) or a caller-facing date type.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// The millisecond value `Date.parse` reads from an ISO fractional-seconds
/// string: the first three digits, taken as-is when there are fewer than
/// three (so `"1"` reads as `100`ms and `"12"` as `120`ms — a right-pad,
/// not a left-pad, matching the fraction's own place value) and TRUNCATED
/// — never rounded — to the first three when there are more (so `"1234"`
/// and `"123456"` both read as `123`ms). Confirmed against real
/// `Date.parse` output for 1/2/3/4/6-digit fractions.
fn fractional_seconds_to_millis(frac: &str) -> i64 {
    let mut digits = [b'0'; 3];
    for (i, b) in frac.as_bytes().iter().take(3).enumerate() {
        digits[i] = *b;
    }
    std::str::from_utf8(&digits)
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
}

/// Millisecond timestamp (UTC) for an already-validated
/// (`is_valid_date_time`) timestamp string, for interval ordering only.
/// Port of using `Date.parse` after validation has already ruled out
/// anything it would otherwise silently normalise. Fractional seconds are
/// read to millisecond precision (`Date.parse` carries no finer
/// resolution) via [`fractional_seconds_to_millis`] — dropping them
/// entirely, as an earlier version of this function did, silently
/// collapsed two genuinely different instants (e.g. `.100Z` and `.200Z`)
/// onto the same millisecond and made a real sub-second interval compare
/// as not-after.
fn parse_datetime_millis(s: &str) -> Option<i64> {
    let caps = DATETIME_RE.captures(s).ok()??;
    let y: i64 = caps[1].parse().ok()?;
    let mo: i64 = caps[2].parse().ok()?;
    let d: i64 = caps[3].parse().ok()?;
    let h: i64 = caps[4].parse().ok()?;
    let mi: i64 = caps[5].parse().ok()?;
    let se: i64 = caps[6].parse().ok()?;
    let frac_millis = caps
        .get(7)
        .map(|m| fractional_seconds_to_millis(m.as_str()))
        .unwrap_or(0);
    let days = days_from_civil(y, mo, d);
    let mut millis = days * 86_400_000 + (h * 3600 + mi * 60 + se) * 1000 + frac_millis;
    if let (Some(sign), Some(oh), Some(om)) = (caps.get(8), caps.get(9), caps.get(10)) {
        let offset_seconds: i64 =
            oh.as_str().parse::<i64>().ok()? * 3600 + om.as_str().parse::<i64>().ok()? * 60;
        // local = UTC + offset, so UTC = local - offset.
        millis -= if sign.as_str() == "-" {
            -offset_seconds
        } else {
            offset_seconds
        } * 1000;
    }
    Some(millis)
}

fn parse_date_millis(s: &str) -> Option<i64> {
    let caps = DATE_RE.captures(s).ok()??;
    let y: i64 = caps[1].parse().ok()?;
    let mo: i64 = caps[2].parse().ok()?;
    let d: i64 = caps[3].parse().ok()?;
    Some(days_from_civil(y, mo, d) * 86_400_000)
}

/// Is this `{ revision, sha256 }` pair a sufficient pin? Port of
/// `pinDefect`.
fn pin_defect(revision: Option<&str>, sha256: Option<&str>) -> Option<String> {
    if is_valid_sha256(sha256) {
        return None;
    }
    match classify_revision(revision) {
        "exact" => None,
        "floating" => Some(format!(
            "revision {} is a branch or channel reference; a branch or channel is never a pin, whatever digits it carries",
            json_quote(revision)
        )),
        "abbrev-sha" => Some(format!(
            "revision {} looks like an abbreviated or ambiguous Git SHA (not a full 40- or 64-hex object name); pin the full SHA or add a sha256 digest",
            json_quote(revision)
        )),
        "weak" => Some(format!(
            "revision {} is not a recognised exact revision (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); a provider-specific or unknown revision format requires a sha256 digest",
            json_quote(revision)
        )),
        _ => Some("it is pinned by neither an exact revision nor a SHA-256 digest".to_string()),
    }
}

/// Round to two decimal places the same way everywhere, so a recomputed
/// percentage can be compared by strict equality, never by float tolerance.
/// Port of `roundPercentage`.
pub fn round_percentage(numerator: f64, denominator: f64) -> Option<f64> {
    // `!(denominator > 0.0)`, spelled via `partial_cmp` per clippy's
    // `neg_cmp_op_on_partial_ord`: also `None` for a NaN denominator,
    // exactly like the JS `!(denominator > 0)` this ports.
    if denominator.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return None;
    }
    Some(((numerator / denominator) * 10000.0).round() / 100.0)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// `JSON.stringify` never distinguishes a whole-number float from an
/// integer (both print as `"600"`, never `"600.0"`) because JS has one
/// number type; `serde_json::Number` does distinguish its integer and
/// float representations, so a computed `f64` that happens to be whole
/// must be re-encoded as the integer variant before it is compared against
/// (or matches) a hand-authored fixture literal like `600`.
fn js_number(v: f64) -> Value {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 9e15 {
        Value::Number(serde_json::Number::from(v as i64))
    } else {
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// Validate one `pinned_ref` slot (no `run_id` semantics in this package).
/// Port of `checkPinnedRef`.
fn check_pinned_ref(
    problems: &mut Vec<String>,
    label: &str,
    id: &str,
    field: &str,
    ref_val: &Value,
) {
    let Some(want) = pinned_ref_slot_record_type(field) else {
        return;
    };
    if !is_object(ref_val) {
        problems.push(format!(
            "{label} \"{id}\" {field} is not a structured pinned reference; it is a closed {{ record_type, id, reference, revision?/sha256? }} object, never a plain string and never an embedded body"
        ));
        return;
    }
    let record_type = ref_val.get("record_type").and_then(Value::as_str);
    if record_type != Some(want) {
        problems.push(format!(
            "{label} \"{id}\" {field} names record_type \"{}\", not \"{want}\"",
            record_type.unwrap_or("undefined")
        ));
    }
    if !ref_val
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(is_semantic_id)
    {
        problems.push(format!(
            "{label} \"{id}\" {field} has no stable semantic id"
        ));
    }
    let reference = ref_val.get("reference");
    if reference.map(blank).unwrap_or(true) {
        problems.push(format!(
            "{label} \"{id}\" {field} has no portable reference"
        ));
    } else if let Some(r) = non_portable_reason(reference.and_then(Value::as_str)) {
        problems.push(format!(
            "{label} \"{id}\" {field} reference contains {r}; the reference is portable and is not an absolute machine path"
        ));
    }
    let revision = ref_val.get("revision").and_then(Value::as_str);
    let sha256 = ref_val.get("sha256").and_then(Value::as_str);
    if let Some(defect) = pin_defect(revision, sha256) {
        problems.push(format!(
            "{label} \"{id}\" {field} is not pinned to an exact edition: {defect}"
        ));
    }
    if let Some(rev) = revision.filter(|s| !s.is_empty()) {
        if let Some(rr) = non_portable_reason(Some(rev)) {
            problems.push(format!("{label} \"{id}\" {field} revision contains {rr}"));
        }
    }
}

struct ConfirmedDigest {
    digest: Option<String>,
    inconsistent: Option<String>,
}

/// Port of `confirmedDigest` (field-evaluation.mjs's own local copy).
fn confirmed_digest(entry: &Value) -> ConfirmedDigest {
    let cd = entry
        .get("content_digest")
        .and_then(Value::as_str)
        .filter(|s| SHA256_RE.is_match(s).unwrap_or(false));
    if let Some(source_bytes) = entry.get("source_bytes").and_then(Value::as_str) {
        let h = meridian_core::types::ContentDigest::of_str(source_bytes)
            .value()
            .to_string();
        if let Some(cd) = cd {
            if cd.to_lowercase() != h.to_lowercase() {
                return ConfirmedDigest {
                    digest: Some(h),
                    inconsistent: Some(cd.to_string()),
                };
            }
        }
        return ConfirmedDigest {
            digest: Some(h),
            inconsistent: None,
        };
    }
    if let Some(cd) = cd {
        return ConfirmedDigest {
            digest: Some(cd.to_string()),
            inconsistent: None,
        };
    }
    ConfirmedDigest {
        digest: None,
        inconsistent: None,
    }
}

/// Resolve ONE pinned reference through the boundary and check it against
/// the CLOSED transformer-response contract. Port of `resolvePinnedReference`
/// (this package's callers do their own type-specific completeness check
/// afterwards, using the returned entry — this package's own
/// `resolvePinnedReference` runs no default per-type dispatch, unlike the
/// bounded-context-manifest/evidence-and-handoff ports).
fn resolve_pinned_reference(
    problems: &mut Vec<String>,
    label: &str,
    id: &str,
    field: &str,
    ref_val: &Value,
    resolve: &RecordResolver,
    wanted: &str,
) -> Option<Value> {
    if !is_object(ref_val) {
        return None;
    }
    let entry = resolve(ref_val);
    let Some(entry) = entry.filter(is_object) else {
        problems.push(format!(
            "{label} \"{id}\" {field} does not resolve to an actual {wanted} through the external resolver; a pinned reference that resolves to nothing is not a verified pin and the record fails closed"
        ));
        return None;
    };

    let allowed_keys = allowed_entry_keys(wanted);
    if let Some(obj) = entry.as_object() {
        for k in obj.keys() {
            if !allowed_keys.contains(&k.as_str()) {
                problems.push(format!(
                    "{label} \"{id}\" {field}: the resolver returned a record with an unknown field \"{k}\"; the transformer response is closed to {{ {} }}",
                    allowed_keys.join(", ")
                ));
            }
        }
    }

    let record_type = entry.get("record_type").and_then(Value::as_str);
    match record_type.filter(|s| !s.is_empty()) {
        None => problems.push(format!(
            "{label} \"{id}\" {field}: the resolver returned a record with no record_type; the transformer response is a closed contract"
        )),
        Some(rt) => {
            if rt != wanted {
                problems.push(format!(
                    "{label} \"{id}\" {field} resolves to a \"{rt}\" record, not \"{wanted}\""
                ));
            }
        }
    }

    let entry_id = entry.get("id").and_then(Value::as_str);
    match entry_id.filter(|s| !s.trim().is_empty()) {
        None => problems.push(format!(
            "{label} \"{id}\" {field}: the resolver returned a {wanted} with no id; a resolved record without an identity cannot be checked against the pin"
        )),
        Some(eid) if !is_semantic_id(eid) => problems.push(format!(
            "{label} \"{id}\" {field}: the resolver returned a {wanted} whose id \"{eid}\" is not a stable semantic identifier"
        )),
        Some(eid) => {
            if let Some(pin_id) = ref_val.get("id").and_then(Value::as_str) {
                if eid != pin_id {
                    problems.push(format!(
                        "{label} \"{id}\" {field} pins id \"{pin_id}\" but the reference resolves to record id \"{eid}\"; a pinned reference that resolves to a DIFFERENT record is not the same record that was pinned"
                    ));
                }
            }
        }
    }

    if let Some(entry_ref) = entry.get("reference") {
        match entry_ref.as_str().filter(|s| !s.trim().is_empty()) {
            None => problems.push(format!(
                "{label} \"{id}\" {field}: the resolver's stated reference is present but not a non-empty string"
            )),
            Some(er) => {
                if let Some(r) = non_portable_reason(Some(er)) {
                    problems.push(format!(
                        "{label} \"{id}\" {field}: the resolver's stated reference contains {r}"
                    ));
                } else if let Some(pin_ref) = ref_val.get("reference").and_then(Value::as_str) {
                    if er != pin_ref {
                        problems.push(format!(
                            "{label} \"{id}\" {field}: the resolver's stated reference \"{er}\" is not the resolved reference \"{pin_ref}\""
                        ));
                    }
                }
            }
        }
    }

    if let Some(cd) = entry.get("content_digest") {
        let valid = cd
            .as_str()
            .is_some_and(|s| SHA256_RE.is_match(s).unwrap_or(false));
        if !valid {
            problems.push(format!(
                "{label} \"{id}\" {field}: the resolver's content_digest {} is not exactly 64 hexadecimal characters",
                json_stringify(Some(cd))
            ));
        }
    }
    if let Some(sb) = entry.get("source_bytes") {
        if sb.as_str().is_none() {
            let shown = if sb.is_null() {
                "null".to_string()
            } else {
                js_typeof(sb).to_string()
            };
            problems.push(format!(
                "{label} \"{id}\" {field}: the resolver's source_bytes is {shown}, not a string"
            ));
        }
    }

    let has_revision_string = entry
        .get("revision")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    let mut revision_exact = false;
    if let Some(rev) = entry.get("revision") {
        match rev.as_str() {
            None => {
                let shown = match rev {
                    Value::Null => "null".to_string(),
                    Value::Array(_) => "an array".to_string(),
                    Value::Number(_) => "a number".to_string(),
                    Value::Bool(_) => "a boolean".to_string(),
                    Value::Object(_) => "an object".to_string(),
                    Value::String(_) => unreachable!(),
                };
                problems.push(format!(
                    "{label} \"{id}\" {field}: the resolver's revision is {shown}, not a string"
                ));
            }
            Some(s) if s.trim().is_empty() => {
                problems.push(format!(
                    "{label} \"{id}\" {field}: the resolver's revision is present but empty"
                ));
            }
            Some(s) => {
                if classify_revision(Some(s)) != "exact" {
                    problems.push(format!(
                        "{label} \"{id}\" {field}: the resolver confirmed revision {}, which is not an exact edition (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); the transformer confirms a fixed edition, not a moving reference",
                        json_stringify(Some(rev))
                    ));
                } else {
                    revision_exact = true;
                }
            }
        }
    }
    let confirmed = confirmed_digest(&entry);
    if !revision_exact && confirmed.digest.is_none() {
        problems.push(format!(
            "{label} \"{id}\" {field}: the resolver confirmed neither an exact revision nor a content digest for this edition; the pinned edition is unverified"
        ));
    }
    if let Some(inconsistent) = &confirmed.inconsistent {
        problems.push(format!(
            "{label} \"{id}\" {field}: the resolver's content_digest \"{inconsistent}\" does not match the SHA-256 of the resolved source bytes \"{}\"",
            confirmed.digest.as_deref().unwrap_or("")
        ));
    }
    if let Some(pin_rev) = ref_val
        .get("revision")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
    {
        if !has_revision_string {
            problems.push(format!(
                "{label} \"{id}\" {field} pins revision \"{pin_rev}\" but the resolver confirmed no exact edition for this reference"
            ));
        } else {
            let entry_rev = entry.get("revision").and_then(Value::as_str).unwrap_or("");
            if entry_rev != pin_rev {
                problems.push(format!(
                    "{label} \"{id}\" {field} pins revision \"{pin_rev}\" but the resolver confirmed edition \"{entry_rev}\""
                ));
            }
        }
    }
    if let Some(pin_sha) = ref_val
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
    {
        match &confirmed.digest {
            None => problems.push(format!(
                "{label} \"{id}\" {field} pins sha256 \"{pin_sha}\" but the resolver confirmed no content digest for this edition; a digest is verified against resolved source, never against a second copy of itself"
            )),
            Some(d) => {
                if d.to_lowercase() != pin_sha.to_lowercase() {
                    problems.push(format!(
                        "{label} \"{id}\" {field} pins sha256 \"{pin_sha}\" but the SHA-256 of the resolved source is \"{d}\""
                    ));
                }
            }
        }
    }

    Some(entry)
}

/// Port of `resolveEvidenceEntry`'s return shape.
struct EvidenceResolution {
    clean: bool,
    observed_result: Option<String>,
}

/// Resolve ONE evidence entry as an `evidence-result` transformer response:
/// pinned by the closed rule, resolved to a closed evidence-result carrying
/// `observed_result` AND `metric_ref` — the SUBJECT it confirms. Port of
/// `resolveEvidenceEntry`.
fn resolve_evidence_entry(
    problems: &mut Vec<String>,
    label: &str,
    id: &str,
    at: &str,
    metric_id: &str,
    eo: &Value,
    resolve: &RecordResolver,
) -> EvidenceResolution {
    let mut synth_ref = serde_json::json!({
        "record_type": "evidence-result",
        "id": eo.get("id").cloned().unwrap_or(Value::Null),
        "reference": eo.get("reference").cloned().unwrap_or(Value::Null),
    });
    if let Some(rev) = eo
        .get("revision")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        synth_ref["revision"] = Value::String(rev.to_string());
    }
    if let Some(sha) = eo
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        synth_ref["sha256"] = Value::String(sha.to_string());
    }

    let before = problems.len();
    let field = format!("evidence entry {at}");
    let entry = resolve_pinned_reference(
        problems,
        label,
        id,
        &field,
        &synth_ref,
        resolve,
        "evidence-result",
    );
    if let Some(e) = &entry {
        if e.get("observed_result").is_none() {
            problems.push(format!(
                "{label} \"{id}\" evidence entry {at}: the resolved evidence-result confirms no observed_result; the transformer says whether the artefact confirmed, contradicted or was inconclusive about its subject"
            ));
        } else if !e
            .get("observed_result")
            .and_then(Value::as_str)
            .is_some_and(|s| OBSERVED_RESULTS.contains(&s))
        {
            problems.push(format!(
                "{label} \"{id}\" evidence entry {at}: the resolved evidence-result's observed_result {} is not one of {{ {} }}",
                json_stringify(e.get("observed_result")),
                OBSERVED_RESULTS.join(", ")
            ));
        }
        if e.get("metric_ref").map(blank).unwrap_or(true) {
            problems.push(format!(
                "{label} \"{id}\" evidence entry {at}: the resolved evidence-result confirms no metric_ref; which metric an evidence entry bears on is confirmed by the transformer, not asserted by the record"
            ));
        } else if e.get("metric_ref").and_then(Value::as_str) != Some(metric_id) {
            problems.push(format!(
                "{label} \"{id}\" evidence entry {at}: the resolved evidence-result's metric_ref \"{}\" is not this observation's own metric \"{metric_id}\"; evidence of a different indicator does not support this one",
                e.get("metric_ref").and_then(Value::as_str).unwrap_or("")
            ));
        }
    }
    let clean = problems.len() == before;
    let observed_result = entry
        .as_ref()
        .and_then(|e| e.get("observed_result"))
        .and_then(Value::as_str)
        .filter(|s| OBSERVED_RESULTS.contains(s))
        .map(str::to_string);
    EvidenceResolution {
        clean,
        observed_result,
    }
}

/// Measurement — the shape varies by kind, but the kind itself is fixed per
/// `metric_id`. A pure function returning bare issue strings (no label/id
/// prefix), so the exact same rule applies both to a record's own
/// `payload.measurement` and to the RESOLVED projection of an observation.
/// Port of `measurementIssues`.
fn measurement_issues(metric_id: &str, m: &Value) -> Vec<String> {
    let mut issues = Vec::new();
    let want_kind = metric_measurement_kind(metric_id);
    if !is_object(m) {
        issues.push("measurement is not an object".to_string());
        return issues;
    }
    let m_kind = m.get("kind").and_then(Value::as_str);
    if m_kind != want_kind {
        issues.push(format!(
            "measurement.kind is {}, but metric \"{metric_id}\" is measured by kind {}",
            json_stringify(m.get("kind")),
            want_kind
                .map(|k| format!("\"{k}\""))
                .unwrap_or_else(|| "undefined".to_string())
        ));
        return issues;
    }
    match want_kind {
        Some("classification") => {
            let pool = classification_outcomes(metric_id);
            let outcome = m.get("outcome").and_then(Value::as_str);
            if !outcome.is_some_and(|o| pool.contains(&o)) {
                issues.push(format!(
                    "measurement.outcome {} is not one of the closed pool for \"{metric_id}\" {{ {} }}",
                    json_stringify(m.get("outcome")),
                    pool.join(", ")
                ));
            }
            if m.get("basis").map(blank).unwrap_or(true) {
                issues.push("measurement has no classification basis; a correct/false or missed/not_missed classification needs a stated basis — the status or work_status alone is never proof, and an unknown classification is never auto-treated as the tracked outcome".to_string());
            } else if let Some(r) = non_portable_reason(m.get("basis").and_then(Value::as_str)) {
                issues.push(format!("measurement.basis contains {r}"));
            }
        }
        Some("duration") => {
            let dk = m.get("duration_kind").and_then(Value::as_str);
            if !dk.is_some_and(|k| DURATION_KINDS.contains(&k)) {
                issues.push(format!(
                    "measurement.duration_kind {} is not one of {{ {} }}; calendar length and active effort are distinguished, never summed together",
                    json_stringify(m.get("duration_kind")),
                    DURATION_KINDS.join(", ")
                ));
            }
            if m.get("unit").and_then(Value::as_str) != Some("seconds") {
                issues.push(format!(
                    "measurement.unit is {}, not \"seconds\"; a duration is always stated in seconds",
                    json_stringify(m.get("unit"))
                ));
            }
            if !is_non_neg_number(m.get("seconds")) {
                issues.push(format!(
                    "measurement.seconds {} is not a non-negative number; a negative duration is not measurable",
                    json_stringify(m.get("seconds"))
                ));
            }
            if m.get("paused_seconds").is_some() {
                if dk != Some("calendar") {
                    issues.push(format!(
                        "measurement carries paused_seconds but duration_kind is {}, not \"calendar\"; pause accounting subtracts paused time from a CALENDAR span to reach the active-effort figure, which is its own separate observation, not a second subtraction on top of it",
                        json_stringify(m.get("duration_kind"))
                    ));
                } else if !is_non_neg_number(m.get("paused_seconds")) {
                    issues.push(format!(
                        "measurement.paused_seconds {} is not a non-negative number",
                        json_stringify(m.get("paused_seconds"))
                    ));
                } else if is_non_neg_number(m.get("seconds")) {
                    let paused = m
                        .get("paused_seconds")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    let seconds = m.get("seconds").and_then(Value::as_f64).unwrap_or(0.0);
                    if paused > seconds {
                        issues.push(format!(
                            "measurement.paused_seconds ({}) exceeds the calendar seconds ({}); paused time cannot exceed the span it is paused within",
                            json_stringify(m.get("paused_seconds")),
                            json_stringify(m.get("seconds"))
                        ));
                    }
                }
            }
            let has_start = m.get("start_ts").is_some();
            let has_end = m.get("end_ts").is_some();
            if has_start != has_end {
                issues.push("measurement states only one of start_ts / end_ts; an interval names both ends or neither".to_string());
            } else if has_start && has_end {
                let start_ts = m.get("start_ts").and_then(Value::as_str);
                let end_ts = m.get("end_ts").and_then(Value::as_str);
                if !is_valid_date_time(start_ts) || !is_valid_date_time(end_ts) {
                    issues.push("measurement start_ts/end_ts is not a valid timestamp (a well-formed timestamp naming a calendar date and time that actually exist, not merely a digit pattern Date.parse would silently normalise)".to_string());
                } else {
                    let s = start_ts.and_then(parse_datetime_millis);
                    let e = end_ts.and_then(parse_datetime_millis);
                    if let (Some(s), Some(e)) = (s, e) {
                        if e <= s {
                            issues.push("measurement end_ts is not after start_ts; an interval that ends before (or exactly when) it starts is not a possible time interval".to_string());
                        }
                    }
                }
            }
        }
        Some("count") => {
            if m.get("unit").and_then(Value::as_str) != Some("count") {
                issues.push(format!(
                    "measurement.unit is {}, not \"count\"",
                    json_stringify(m.get("unit"))
                ));
            }
            if !is_non_neg_int(m.get("value")) {
                issues.push(format!(
                    "measurement.value {} is not a non-negative integer",
                    json_stringify(m.get("value"))
                ));
            }
        }
        _ => {}
    }
    issues
}

fn check_measurement(
    problems: &mut Vec<String>,
    label: &str,
    id: &str,
    metric_id: &str,
    m: &Value,
) {
    for issue in measurement_issues(metric_id, m) {
        problems.push(format!("{label} \"{id}\" {issue}"));
    }
}

/// Resolved observation projection — validated BEFORE it is ever used for
/// aggregation, counted, or trusted for anything. Port of
/// `resolvedObservationIssues`.
fn resolved_observation_issues(entry: &Value) -> Vec<String> {
    let mut issues = Vec::new();
    if !is_object(entry) {
        issues.push("did not resolve to an object".to_string());
        return issues;
    }
    let metric_id = entry.get("metric_id").and_then(Value::as_str);
    if !metric_id.is_some_and(|m| METRIC_IDS.contains(&m)) {
        issues.push(format!(
            "metric_id {} is not one of the eight closed characteristics",
            json_stringify(entry.get("metric_id"))
        ));
    }
    let status = entry.get("status").and_then(Value::as_str);
    if !status.is_some_and(|s| OBSERVATION_STATUSES.contains(&s)) {
        issues.push(format!(
            "status {} is not one of the closed pool {{ {} }}",
            json_stringify(entry.get("status")),
            OBSERVATION_STATUSES.join(", ")
        ));
    }
    if entry.get("workspace_id").map(blank).unwrap_or(true)
        || !entry
            .get("workspace_id")
            .and_then(Value::as_str)
            .is_some_and(is_semantic_id)
    {
        issues.push(format!(
            "workspace_id {} is missing or not a stable semantic id",
            json_stringify(entry.get("workspace_id"))
        ));
    }
    if entry.get("observed_at").map(blank).unwrap_or(true)
        || !is_valid_date(entry.get("observed_at").and_then(Value::as_str))
    {
        issues.push(format!(
            "observed_at {} is not a valid calendar date",
            json_stringify(entry.get("observed_at"))
        ));
    }
    if status == Some("observed") {
        if !entry.get("measurement").is_some_and(is_object) {
            issues.push("status is \"observed\" but measurement is not an object".to_string());
        } else if let Some(mid) = metric_id.filter(|m| METRIC_IDS.contains(m)) {
            for issue in measurement_issues(mid, entry.get("measurement").unwrap()) {
                issues.push(format!("measurement {issue}"));
            }
        }
    } else if entry.get("measurement").is_some_and(|v| !v.is_null()) {
        issues.push(format!(
            "status is {} but measurement is present",
            json_stringify(entry.get("status"))
        ));
    }
    if let Some(sid) = entry.get("supersedes_id") {
        if !sid.is_null() && !sid.as_str().is_some_and(is_semantic_id) {
            issues.push(format!(
                "supersedes_id {} is present but neither null nor a stable semantic id",
                json_stringify(Some(sid))
            ));
        }
    }
    let requires_window =
        metric_id.is_some_and(|m| METRIC_IDS.contains(&m) && METRICS_REQUIRING_WINDOW.contains(&m));
    let has_window = entry
        .get("observation_period")
        .is_some_and(|v| !v.is_null());
    if status == Some("observed") && requires_window {
        if !has_window {
            issues.push(format!(
                "metric \"{}\" requires an observation_period when observed",
                metric_id.unwrap_or("")
            ));
        } else {
            let empty = Value::Object(Default::default());
            let w = entry
                .get("observation_period")
                .filter(|v| is_object(v))
                .unwrap_or(&empty);
            let sd = w.get("start_date").and_then(Value::as_str);
            let ed = w.get("end_date").and_then(Value::as_str);
            if !is_valid_date(sd) || !is_valid_date(ed) {
                issues.push("observation_period does not carry two valid dates".to_string());
            } else if let (Some(s), Some(e)) = (
                sd.and_then(parse_date_millis),
                ed.and_then(parse_date_millis),
            ) {
                if e < s {
                    issues.push("observation_period end_date is before start_date".to_string());
                }
            }
            let coverage = entry.get("coverage").and_then(Value::as_str);
            if !coverage.is_some_and(|c| COVERAGE_VALUES.contains(&c)) {
                issues.push(format!(
                    "coverage {} is not one of the closed pool {{ {} }}",
                    json_stringify(entry.get("coverage")),
                    COVERAGE_VALUES.join(", ")
                ));
            }
        }
    } else if has_window || entry.get("coverage").is_some_and(|v| !v.is_null()) {
        issues.push(format!(
            "carries observation_period/coverage but metric \"{}\" at status {} does not use an observation window",
            metric_id.unwrap_or(""),
            json_stringify(entry.get("status"))
        ));
    }
    issues
}

/// Recompute the aggregate for ONE metric from its USABLE (status
/// "observed", resolved) contributing observations. Deterministic: sorted
/// by observation id before any accumulation, so permutation of the input
/// never changes the result. Port of `computeMetricAggregate`, returning
/// the same plain-object shape Node does, as a [`Value`] — `serde_json`'s
/// `Value::Object` is a `BTreeMap` (no `preserve_order` feature enabled
/// anywhere in this workspace), so structural [`Value`] equality against a
/// fixture's own stated aggregate is exactly [`Value`]'s `PartialEq`, the
/// same exactness Node's own recursively-key-sorted `canonicalJSON` string
/// comparison achieves (see `sameAggregate`) — never a divergent second
/// comparison rule.
pub fn compute_metric_aggregate(metric_id: &str, usable_entries: &[Value]) -> Value {
    let kind = metric_measurement_kind(metric_id).unwrap_or("count");
    let mut sorted: Vec<&Value> = usable_entries.iter().collect();
    sorted.sort_by(|a, b| {
        a.get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(b.get("id").and_then(Value::as_str).unwrap_or(""))
    });

    if kind == "classification" {
        let pool = classification_outcomes(metric_id);
        let mut counts: HashMap<&str, i64> = pool.iter().map(|o| (*o, 0)).collect();
        for e in &sorted {
            if let Some(outcome) = e
                .get("measurement")
                .and_then(|m| m.get("outcome"))
                .and_then(Value::as_str)
            {
                if let Some(c) = counts.get_mut(outcome) {
                    *c += 1;
                }
            }
        }
        let total: i64 = pool.iter().map(|o| counts[o]).sum();
        let tracked = classification_tracked_outcome(metric_id).unwrap_or("");
        let numerator = *counts.get(tracked).unwrap_or(&0);
        let percentage = if total > 0 {
            round_percentage(numerator as f64, total as f64)
        } else {
            None
        };
        let mut rate = serde_json::json!({
            "numerator": numerator,
            "denominator": total,
            "outcome": tracked,
            "note": format!("доля наблюдений с исходом \"{tracked}\" среди {total} классифицированных; ноль в знаменателе не даёт процента"),
        });
        if let Some(p) = percentage {
            rate["percentage"] = js_number(p);
        }
        let counts_obj: serde_json::Map<String, Value> = pool
            .iter()
            .map(|o| (o.to_string(), serde_json::json!(counts[o])))
            .collect();
        return serde_json::json!({
            "kind": "classification",
            "counts": Value::Object(counts_obj),
            "total": total,
            "rate": rate,
        });
    }
    if kind == "duration" {
        let mut by_kind: HashMap<&str, Vec<f64>> =
            HashMap::from([("calendar", Vec::new()), ("active", Vec::new())]);
        for e in &sorted {
            let m = e.get("measurement");
            let dk = m
                .and_then(|m| m.get("duration_kind"))
                .and_then(Value::as_str);
            let seconds = m.and_then(|m| m.get("seconds")).and_then(Value::as_f64);
            if let (Some(dk), Some(seconds)) = (dk, seconds) {
                if let Some(v) = by_kind.get_mut(dk) {
                    v.push(seconds);
                }
            }
        }
        let summarise = |arr: &[f64]| -> Value {
            let sample_count = arr.len();
            let total_seconds: f64 = arr.iter().sum();
            let mut out = serde_json::json!({
                "sample_count": sample_count,
                "total_seconds": js_number(total_seconds),
            });
            if sample_count > 0 {
                out["mean_seconds"] = js_number(round2(total_seconds / sample_count as f64));
            }
            out
        };
        return serde_json::json!({
            "kind": "duration",
            "calendar": summarise(&by_kind["calendar"]),
            "active": summarise(&by_kind["active"]),
        });
    }
    // count
    let values: Vec<f64> = sorted
        .iter()
        .filter_map(|e| {
            e.get("measurement")
                .and_then(|m| m.get("value"))
                .and_then(Value::as_f64)
        })
        .collect();
    let sample_count = values.len();
    let total: f64 = values.iter().sum();
    let mut out = serde_json::json!({
        "kind": "count",
        "sample_count": sample_count,
        "total": js_number(total),
    });
    if sample_count > 0 {
        out["mean"] = js_number(round2(total / sample_count as f64));
    }
    if METRICS_REQUIRING_WINDOW.contains(&metric_id) {
        let with_window: Vec<&&Value> = sorted
            .iter()
            .filter(|e| e.get("observation_period").is_some_and(is_object))
            .collect();
        if !with_window.is_empty() {
            let mut starts: Vec<&str> = with_window
                .iter()
                .filter_map(|e| {
                    e.get("observation_period")
                        .and_then(|w| w.get("start_date"))
                        .and_then(Value::as_str)
                })
                .collect();
            let mut ends: Vec<&str> = with_window
                .iter()
                .filter_map(|e| {
                    e.get("observation_period")
                        .and_then(|w| w.get("end_date"))
                        .and_then(Value::as_str)
                })
                .collect();
            starts.sort_unstable();
            ends.sort_unstable();
            if let (Some(s), Some(e)) = (starts.first(), ends.last()) {
                out["observation_window"] = serde_json::json!({ "start_date": s, "end_date": e });
            }
            let coverages: HashSet<&str> = with_window
                .iter()
                .filter_map(|e| e.get("coverage").and_then(Value::as_str))
                .collect();
            let coverage_val = if coverages.len() > 1 {
                "mixed".to_string()
            } else {
                coverages.into_iter().next().unwrap_or("").to_string()
            };
            out["coverage"] = serde_json::json!(coverage_val);
        }
    }
    out
}

fn resolved_is_usable(entry: &Value) -> bool {
    is_object(entry)
        && entry.get("status").and_then(Value::as_str) == Some("observed")
        && entry.get("measurement").is_some_and(is_object)
}

pub struct EvalOpts<'a> {
    pub record_schema: &'a Value,
    pub envelope_schema: &'a Value,
    pub resolve_records: Option<&'a RecordResolver<'a>>,
}

/// The whole composition pipeline, branching on `record_type`. Port of
/// `evaluateFieldEvaluation`.
pub fn evaluate_field_evaluation(doc: &Value, opts: &EvalOpts) -> Vec<String> {
    let mut problems = Vec::new();

    match json_schema::validate(doc, opts.envelope_schema) {
        Ok(errors) => problems.extend(errors.into_iter().map(|m| format!("envelope {m}"))),
        Err(error) => {
            return vec![format!(
                "record envelope schema could not be applied: {error}"
            )]
        }
    }
    match json_schema::validate(doc, opts.record_schema) {
        Ok(errors) => problems.extend(errors),
        Err(error) => {
            return vec![format!(
                "field-evaluation schema could not be applied: {error}"
            )]
        }
    }
    if !is_object(doc) {
        if problems.is_empty() {
            return vec!["the field-evaluation record is not an object".to_string()];
        }
        return problems;
    }

    let id = doc
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("(no id)")
        .to_string();
    let record_type = doc.get("record_type").and_then(Value::as_str).unwrap_or("");
    let label = if record_type == REPORT_RECORD_TYPE {
        "field evaluation report"
    } else {
        "field evaluation observation"
    };

    if !RECORD_TYPES.contains(&record_type) {
        problems.push(format!(
            "field evaluation record \"{id}\" declares record_type \"{record_type}\", not one of {{ {} }}",
            RECORD_TYPES.join(", ")
        ));
    }

    let declared = doc.get("$schema").and_then(Value::as_str);
    match declared {
        None => problems.push(format!(
            "{label} \"{id}\" declares no $schema; a record names {EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract — id, scope, origin, authority and body — not only the envelope"
        )),
        Some(declared) => {
            if let Some(portability) = non_portable_reason(Some(declared)) {
                problems.push(format!(
                    "{label} \"{id}\" $schema \"{declared}\" is not portable ({portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base {CANONICAL_RECORD_BASE})"
                ));
            } else {
                let resolved = resolve_schema_ref(Some(declared));
                if resolved.as_deref() == Some(envelope_schema_ref().as_str()) {
                    problems.push(format!(
                        "{label} \"{id}\" $schema \"{declared}\" resolves to the record envelope ({ENVELOPE_SCHEMA_BASENAME}); it must name {EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body"
                    ));
                } else if resolved.as_deref() != Some(expected_schema_ref().as_str()) {
                    let expected = expected_schema_ref();
                    problems.push(format!(
                        "{label} \"{id}\" $schema \"{declared}\" does not resolve to the logical address {expected} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)"
                    ));
                }
            }
        }
    }

    if !doc
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(is_semantic_id)
    {
        problems.push(format!(
            "{label} \"{id}\" has no stable semantic id on the record envelope"
        ));
    }
    let title = doc.get("title").and_then(Value::as_str);
    match title {
        None => problems.push(format!("{label} \"{id}\" has no human-readable title")),
        Some(t) if t.trim().is_empty() => problems.push(format!("{label} \"{id}\" has no human-readable title")),
        Some(t) if !has_cyrillic(t) => problems.push(format!(
            "{label} \"{id}\" title \"{t}\" carries no Russian (Cyrillic) text; the record name is stated in Russian for the human reader"
        )),
        _ => {}
    }
    if let Some(r) = non_portable_reason(title) {
        problems.push(format!("{label} \"{id}\" title contains {r}"));
    }

    let empty_obj = Value::Object(Default::default());
    let scope = doc
        .get("scope")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    let want_scope_type = if record_type == REPORT_RECORD_TYPE {
        REPORT_SCOPE_TYPE
    } else {
        OBSERVATION_SCOPE_TYPE
    };
    let scope_type = scope.get("type").and_then(Value::as_str);
    if let Some(st) = scope_type {
        if st != want_scope_type {
            let why = scope_rejection_reason(st, want_scope_type, label);
            problems.push(format!("{label} \"{id}\" is scoped to \"{st}\"; {why}"));
        }
    }

    let origin = doc
        .get("origin")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    if origin.get("kind").and_then(Value::as_str) == Some("built-in") {
        problems.push(format!(
            "{label} \"{id}\" declares origin.kind \"built-in\"; evaluation data is written in a workspace, not shipped with the methodology"
        ));
    }
    let authority = doc
        .get("authority")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    for (obj, field, flabel) in [
        (origin, "source_ref", "origin.source_ref"),
        (authority, "authority_ref", "authority.authority_ref"),
        (authority, "decision_ref", "authority.decision_ref"),
    ] {
        if let Some(val) = obj
            .get(field)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            if let Some(r) = non_portable_reason(Some(val)) {
                problems.push(format!(
                    "{label} \"{id}\" {flabel} contains {r}; a record is portable and carries no rooted machine path"
                ));
            }
        }
    }

    let payload = doc
        .get("payload")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    for f in FORBIDDEN_PAYLOAD_FIELDS {
        if payload.get(f).is_some() {
            problems.push(format!(
                "{label} \"{id}\" payload carries \"{f}\"; practical evaluation measures separately — it does not roll up into one score, grade or release verdict, and does not absorb another contract's body"
            ));
        }
    }
    if payload.get("record_type").is_some() {
        problems.push(format!(
            "{label} \"{id}\" repeats record_type inside the payload; the record type is declared once, on the envelope"
        ));
    }

    let resolve = opts.resolve_records;

    if record_type == OBSERVATION_RECORD_TYPE {
        evaluate_observation_body(&mut problems, label, &id, scope, payload, resolve);
    } else if record_type == REPORT_RECORD_TYPE {
        evaluate_report_body(&mut problems, label, &id, payload, resolve);
    }

    problems
}

fn evaluate_observation_body(
    problems: &mut Vec<String>,
    label: &str,
    id: &str,
    scope: &Value,
    payload: &Value,
    resolve: Option<&RecordResolver>,
) {
    let scope_type = scope.get("type").and_then(Value::as_str);
    if scope_type == Some(OBSERVATION_SCOPE_TYPE) {
        if !scope
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(is_semantic_id)
        {
            problems.push(format!(
                "{label} \"{id}\" run-state scope carries no stable scope.id identifying the run"
            ));
        }
        if !scope
            .get("workspace_id")
            .and_then(Value::as_str)
            .is_some_and(is_semantic_id)
        {
            problems.push(format!(
                "{label} \"{id}\" run-state scope carries no workspace_id; a run belongs to a project workspace (workspace-scope-model.md §1)"
            ));
        }
    }

    let run_ref = payload.get("execution_run_ref");
    if let Some(run_ref) = run_ref {
        check_pinned_ref(problems, label, id, "execution_run_ref", run_ref);
        match resolve {
            Some(resolve) => {
                resolve_pinned_reference(problems, label, id, "execution_run_ref", run_ref, resolve, "execution-run");
            }
            None => problems.push(format!(
                "{label} \"{id}\" cannot be verified: no external record resolver was supplied; the pinned run is resolved OUTSIDE the record and checked against it"
            )),
        }
    } else {
        problems.push(format!(
            "{label} \"{id}\" names no execution_run_ref; an observation carries a structured pinned reference to exactly one run"
        ));
    }
    if let Some(scope_id) = scope
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        if scope_type == Some(OBSERVATION_SCOPE_TYPE) {
            if let Some(run_id) = run_ref
                .filter(|v| is_object(v))
                .and_then(|v| v.get("id"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                if scope_id != run_id {
                    problems.push(format!(
                        "{label} \"{id}\" scope identifies run \"{scope_id}\" but execution_run_ref.id is \"{run_id}\"; the two must be the exact same string, not one a substring of the other"
                    ));
                }
            }
        }
    }

    let metric_id = payload.get("metric_id").and_then(Value::as_str);
    if !metric_id.is_some_and(|m| METRIC_IDS.contains(&m)) {
        problems.push(format!(
            "{label} \"{id}\" metric_id {} is not one of the eight closed characteristics {{ {} }}",
            json_stringify(payload.get("metric_id")),
            METRIC_IDS.join(", ")
        ));
    }

    if payload.get("observed_at").map(blank).unwrap_or(true)
        || !is_valid_date(payload.get("observed_at").and_then(Value::as_str))
    {
        problems.push(format!(
            "{label} \"{id}\" observed_at is not a valid date (YYYY-MM-DD); the record states when it was itself recorded"
        ));
    }

    let status = payload.get("status").and_then(Value::as_str);
    if !status.is_some_and(|s| OBSERVATION_STATUSES.contains(&s)) {
        problems.push(format!(
            "{label} \"{id}\" status {} is not one of the closed pool {{ {} }}; absence of observation is distinguished from a measured zero",
            json_stringify(payload.get("status")),
            OBSERVATION_STATUSES.join(", ")
        ));
    }

    if status == Some("observed") {
        if payload.get("status_reason").is_some() {
            problems.push(format!(
                "{label} \"{id}\" is \"observed\" but carries a status_reason; a reason is stated for unknown, not_applicable or unmeasurable, where the absence of a value needs explaining"
            ));
        }
        if payload.get("measurement").is_none() {
            problems.push(format!(
                "{label} \"{id}\" is \"observed\" but carries no measurement"
            ));
        } else if let Some(mid) = metric_id.filter(|m| METRIC_IDS.contains(m)) {
            check_measurement(
                problems,
                label,
                id,
                mid,
                payload.get("measurement").unwrap(),
            );
        }
    } else if status.is_some_and(|s| OBSERVATION_STATUSES.contains(&s)) {
        if payload.get("status_reason").map(blank).unwrap_or(true) {
            problems.push(format!(
                "{label} \"{id}\" is \"{}\" but states no status_reason; unknown, not_applicable and unmeasurable are distinct and each needs its own explanation, not a shared silence",
                status.unwrap()
            ));
        } else if let Some(r) =
            non_portable_reason(payload.get("status_reason").and_then(Value::as_str))
        {
            problems.push(format!("{label} \"{id}\" status_reason contains {r}"));
        }
        if payload.get("measurement").is_some() {
            problems.push(format!(
                "{label} \"{id}\" is \"{}\" but carries a measurement; a value that was not observed is not measured",
                status.unwrap()
            ));
        }
    }

    let requires_window = metric_id.is_some_and(|m| METRICS_REQUIRING_WINDOW.contains(&m));
    let has_window = payload.get("observation_period").is_some();
    if status == Some("observed") && requires_window {
        if !has_window {
            problems.push(format!(
                "{label} \"{id}\" measures \"{}\", which requires an observation_period (the window actually covered) and a coverage completeness flag",
                metric_id.unwrap_or("")
            ));
        } else {
            let empty = Value::Object(Default::default());
            let w = payload
                .get("observation_period")
                .filter(|v| is_object(v))
                .unwrap_or(&empty);
            let sd = w.get("start_date").and_then(Value::as_str);
            let ed = w.get("end_date").and_then(Value::as_str);
            if !is_valid_date(sd) || !is_valid_date(ed) {
                problems.push(format!(
                    "{label} \"{id}\" observation_period does not carry two valid dates"
                ));
            } else if let (Some(s), Some(e)) = (
                sd.and_then(parse_date_millis),
                ed.and_then(parse_date_millis),
            ) {
                if e < s {
                    problems.push(format!("{label} \"{id}\" observation_period end_date is before start_date; an impossible window"));
                }
            }
            let coverage = payload.get("coverage").and_then(Value::as_str);
            if !coverage.is_some_and(|c| COVERAGE_VALUES.contains(&c)) {
                problems.push(format!(
                    "{label} \"{id}\" measures \"{}\" but coverage {} is not one of {{ {} }}",
                    metric_id.unwrap_or(""),
                    json_stringify(payload.get("coverage")),
                    COVERAGE_VALUES.join(", ")
                ));
            } else if coverage == Some("partial")
                && payload.get("coverage_note").map(blank).unwrap_or(true)
            {
                problems.push(format!(
                    "{label} \"{id}\" coverage is \"partial\" but states no coverage_note; an incomplete window needs to say what is not covered"
                ));
            } else if coverage == Some("complete") && payload.get("coverage_note").is_some() {
                problems.push(format!(
                    "{label} \"{id}\" coverage is \"complete\" but carries a coverage_note; a note belongs to a partial window"
                ));
            }
        }
    } else if has_window
        || payload.get("coverage").is_some()
        || payload.get("coverage_note").is_some()
    {
        problems.push(format!(
            "{label} \"{id}\" carries observation_period/coverage, but metric \"{}\" at status {} does not use an observation window; only \"{}\" while \"observed\" does",
            metric_id.unwrap_or(""),
            json_stringify(payload.get("status")),
            METRICS_REQUIRING_WINDOW.join(", ")
        ));
    }

    // Evidence: required non-empty only when observed; forbidden otherwise.
    let empty_vec = Vec::new();
    let evidence = payload
        .get("evidence")
        .and_then(Value::as_array)
        .unwrap_or(&empty_vec);
    if status == Some("observed") && evidence.is_empty() {
        problems.push(format!(
            "{label} \"{id}\" is \"observed\" but carries no evidence; a measured value is grounded by at least one resolvable piece of evidence, not asserted on its own say-so"
        ));
    }
    if status != Some("observed") && payload.get("evidence").is_some() && !evidence.is_empty() {
        problems.push(format!("{label} \"{id}\" is \"{}\" but carries evidence; there is nothing to evidence when nothing was observed", status.unwrap_or("")));
    }
    let mut evidence_ids: HashSet<String> = HashSet::new();
    let mut any_confirmed = false;
    for (i, e) in evidence.iter().enumerate() {
        let eo = if is_object(e) { e.clone() } else { Value::Null };
        let eid = eo.get("id").and_then(Value::as_str);
        let at = eid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
        if !eid.is_some_and(is_semantic_id) {
            problems.push(format!(
                "{label} \"{id}\" evidence entry {at} has no stable semantic id"
            ));
        } else {
            let eid = eid.unwrap();
            if !evidence_ids.insert(eid.to_string()) {
                problems.push(format!(
                    "{label} \"{id}\" evidence id \"{eid}\" is used more than once"
                ));
            }
        }
        let kind = eo.get("kind").and_then(Value::as_str);
        if !kind.is_some_and(|k| EVIDENCE_KINDS.contains(&k)) {
            problems.push(format!(
                "{label} \"{id}\" evidence entry {at} kind {} is not one of {{ {} }}",
                json_stringify(eo.get("kind")),
                EVIDENCE_KINDS.join(", ")
            ));
        }
        for (f, human) in [("reference", "reference"), ("summary", "summary")] {
            if eo.get(f).map(blank).unwrap_or(true) {
                problems.push(format!(
                    "{label} \"{id}\" evidence entry {at} has no {human}"
                ));
            } else if let Some(r) = non_portable_reason(eo.get(f).and_then(Value::as_str)) {
                problems.push(format!(
                    "{label} \"{id}\" evidence entry {at} {f} contains {r}"
                ));
            }
        }
        if let Some(rev) = eo
            .get("revision")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            if let Some(rr) = non_portable_reason(Some(rev)) {
                problems.push(format!(
                    "{label} \"{id}\" evidence entry {at} revision contains {rr}"
                ));
            }
        }
        let pin_reason = pin_defect(
            eo.get("revision").and_then(Value::as_str),
            eo.get("sha256").and_then(Value::as_str),
        );
        if let Some(pin_reason) = pin_reason {
            problems.push(format!(
                "{label} \"{id}\" evidence entry {at} is not pinned to an exact edition: {pin_reason}; an unpinned reference is not verifiable evidence"
            ));
        }
        if let (Some(resolve), Some(mid)) = (resolve, metric_id.filter(|m| METRIC_IDS.contains(m)))
        {
            if status == Some("observed") {
                let res = resolve_evidence_entry(problems, label, id, &at, mid, &eo, resolve);
                if res.clean && res.observed_result.as_deref() == Some("confirmed") {
                    any_confirmed = true;
                }
            }
        }
    }
    if status == Some("observed") && resolve.is_some() && !evidence.is_empty() && !any_confirmed {
        problems.push(format!(
            "{label} \"{id}\" is \"observed\" but no evidence entry resolves cleanly with observed_result \"confirmed\" for this metric; an unresolved or contradicted/inconclusive entry does not ground a measured value"
        ));
    } else if status == Some("observed") && resolve.is_none() {
        problems.push(format!(
            "{label} \"{id}\" cannot be verified: no external record resolver was supplied; evidence is resolved OUTSIDE the record and checked against it — without that boundary the record is only an unanchored self-report"
        ));
    }

    // Correction: supersedes <-> correction_reason, both or neither.
    let has_supersedes = payload.get("supersedes").is_some();
    let has_reason = payload.get("correction_reason").is_some();
    if has_supersedes != has_reason {
        problems.push(format!(
            "{label} \"{id}\" states only one of supersedes / correction_reason; a correction names BOTH the observation it corrects and why"
        ));
    } else if has_supersedes {
        let supersedes = payload.get("supersedes").unwrap();
        check_pinned_ref(problems, label, id, "supersedes", supersedes);
        if payload.get("correction_reason").map(blank).unwrap_or(true) {
            problems.push(format!("{label} \"{id}\" correction_reason is blank"));
        } else if let Some(r) =
            non_portable_reason(payload.get("correction_reason").and_then(Value::as_str))
        {
            problems.push(format!("{label} \"{id}\" correction_reason contains {r}"));
        }
        if is_object(supersedes) && supersedes.get("id").and_then(Value::as_str) == Some(id) {
            problems.push(format!("{label} \"{id}\" supersedes itself; a correction names a DIFFERENT, earlier observation"));
        }
        if let Some(resolve) = resolve {
            resolve_pinned_reference(
                problems,
                label,
                id,
                "supersedes",
                supersedes,
                resolve,
                OBSERVATION_RECORD_TYPE,
            );
        }
    }
}

fn evaluate_report_body(
    problems: &mut Vec<String>,
    label: &str,
    id: &str,
    payload: &Value,
    resolve: Option<&RecordResolver>,
) {
    let workspace_id = payload.get("workspace_id").and_then(Value::as_str);
    if !workspace_id.is_some_and(is_semantic_id) {
        problems.push(format!("{label} \"{id}\" names no workspace_id; a report is scoped to the project workspace it covers"));
    }
    let empty_obj = Value::Object(Default::default());
    let period = payload
        .get("period")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    let mut period_ok = false;
    let sd = period.get("start_date").and_then(Value::as_str);
    let ed = period.get("end_date").and_then(Value::as_str);
    if !is_valid_date(sd) || !is_valid_date(ed) {
        problems.push(format!(
            "{label} \"{id}\" period does not carry two valid dates"
        ));
    } else if let (Some(s), Some(e)) = (
        sd.and_then(parse_date_millis),
        ed.and_then(parse_date_millis),
    ) {
        if e < s {
            problems.push(format!("{label} \"{id}\" period end_date is before start_date; an impossible reporting window"));
        } else {
            period_ok = true;
        }
    }

    let empty_vec = Vec::new();
    let included = payload
        .get("included_observations")
        .and_then(Value::as_array)
        .unwrap_or(&empty_vec);
    let excluded = payload
        .get("excluded_observations")
        .and_then(Value::as_array)
        .unwrap_or(&empty_vec);

    let mut seen_included_ref: HashSet<String> = HashSet::new();
    for (i, r) in included.iter().enumerate() {
        check_pinned_ref(problems, label, id, "included_observation", r);
        if let Some(reference) = r
            .get("reference")
            .and_then(Value::as_str)
            .filter(|_| is_object(r))
        {
            if !seen_included_ref.insert(reference.to_string()) {
                problems.push(format!(
                    "{label} \"{id}\" included_observations repeats reference \"{reference}\" at #{i}; the same observation is not counted twice"
                ));
            }
        }
    }
    let mut seen_excluded_ref: HashSet<String> = HashSet::new();
    for (i, x) in excluded.iter().enumerate() {
        let xo = if is_object(x) { x.clone() } else { Value::Null };
        let xref = xo.get("ref").cloned().unwrap_or(Value::Null);
        check_pinned_ref(problems, label, id, "excluded_observation", &xref);
        if xo.get("exclusion_reason").map(blank).unwrap_or(true) {
            problems.push(format!(
                "{label} \"{id}\" excluded_observations[{i}] has no exclusion_reason; an exclusion is explicit and reasoned, never a silent drop"
            ));
        } else if let Some(r) =
            non_portable_reason(xo.get("exclusion_reason").and_then(Value::as_str))
        {
            problems.push(format!(
                "{label} \"{id}\" excluded_observations[{i}] exclusion_reason contains {r}"
            ));
        }
        if let Some(reference) = xref
            .get("reference")
            .and_then(Value::as_str)
            .filter(|_| is_object(&xref))
        {
            if !seen_excluded_ref.insert(reference.to_string()) {
                problems.push(format!(
                    "{label} \"{id}\" excluded_observations repeats reference \"{reference}\" at #{i}; a repeated excluded reference is not legalised by a different exclusion_reason"
                ));
            }
            if seen_included_ref.contains(reference) {
                problems.push(format!(
                    "{label} \"{id}\" reference \"{reference}\" appears in both included_observations and excluded_observations; an observation is either included or excluded, never both"
                ));
            }
        }
    }

    struct Resolved {
        entry: Option<Value>,
    }
    let mut included_resolved: Vec<Resolved> = Vec::new();
    let mut excluded_resolved: Vec<Resolved> = Vec::new();
    if let Some(resolve) = resolve {
        for (i, r) in included.iter().enumerate() {
            let entry = resolve_pinned_reference(
                problems,
                label,
                id,
                "included_observation",
                r,
                resolve,
                OBSERVATION_RECORD_TYPE,
            );
            if let Some(e) = &entry {
                for issue in resolved_observation_issues(e) {
                    problems.push(format!(
                        "{label} \"{id}\" included_observations[{i}]: resolved observation {issue}"
                    ));
                }
            }
            included_resolved.push(Resolved { entry });
        }
        for (i, x) in excluded.iter().enumerate() {
            let xo = if is_object(x) { x.clone() } else { Value::Null };
            let xref = xo.get("ref").cloned().unwrap_or(Value::Null);
            let entry = if is_object(&xref) {
                resolve_pinned_reference(
                    problems,
                    label,
                    id,
                    "excluded_observation",
                    &xref,
                    resolve,
                    OBSERVATION_RECORD_TYPE,
                )
            } else {
                None
            };
            if let Some(e) = &entry {
                for issue in resolved_observation_issues(e) {
                    problems.push(format!(
                        "{label} \"{id}\" excluded_observations[{i}]: resolved observation {issue}"
                    ));
                }
            }
            excluded_resolved.push(Resolved { entry });
        }
    } else {
        problems.push(format!(
            "{label} \"{id}\" cannot be verified: no external record resolver was supplied; every included and excluded observation is resolved OUTSIDE the report and checked against it"
        ));
    }

    let mut by_id: HashMap<String, Value> = HashMap::new();
    for r in &included_resolved {
        if let Some(entry) = &r.entry {
            if let Some(eid) = entry.get("id").and_then(Value::as_str) {
                if by_id.contains_key(eid) {
                    problems.push(format!(
                        "{label} \"{id}\" included_observations resolves two entries to the SAME observation id \"{eid}\"; a repeated record is not counted twice"
                    ));
                }
                by_id.insert(eid.to_string(), entry.clone());
            }
        }
    }

    let mut excluded_id_seen: HashSet<String> = HashSet::new();
    for (at, r) in excluded_resolved.iter().enumerate() {
        let Some(entry) = &r.entry else { continue };
        let Some(eid) = entry.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !excluded_id_seen.insert(eid.to_string()) {
            problems.push(format!(
                "{label} \"{id}\" excluded_observations[{at}] resolves to observation \"{eid}\", which another excluded_observations entry already resolves to; two different references naming the same observation are not both excluded"
            ));
        }
        if by_id.contains_key(eid) {
            problems.push(format!(
                "{label} \"{id}\" excluded_observations[{at}] resolves to observation \"{eid}\", which is also an included observation; an observation is either included or excluded, never both"
            ));
        }
    }
    for (at, r) in included_resolved.iter().enumerate() {
        let Some(entry) = &r.entry else { continue };
        if let (Some(ews), Some(pws)) = (
            entry.get("workspace_id").and_then(Value::as_str),
            workspace_id,
        ) {
            if ews != pws {
                problems.push(format!(
                    "{label} \"{id}\" included_observations[{at}] belongs to workspace \"{ews}\", not this report's workspace \"{pws}\"; samples from different workspaces are not mixed without an explicit comparability rule"
                ));
            }
        }
        if period_ok {
            let observed_at = entry.get("observed_at").and_then(Value::as_str);
            if is_valid_date(observed_at) {
                if let (Some(t), Some(s), Some(e)) = (
                    observed_at.and_then(parse_date_millis),
                    sd.and_then(parse_date_millis),
                    ed.and_then(parse_date_millis),
                ) {
                    if t < s || t > e {
                        problems.push(format!(
                            "{label} \"{id}\" included_observations[{at}] was observed at \"{}\", outside the report period [{}, {}]; samples from different periods are not mixed without an explicit comparability rule",
                            observed_at.unwrap_or(""),
                            sd.unwrap_or(""),
                            ed.unwrap_or("")
                        ));
                    }
                }
            }
        }
        if let Some(sup) = entry.get("supersedes_id").and_then(Value::as_str) {
            if by_id.contains_key(sup) {
                problems.push(format!(
                    "{label} \"{id}\" includes both observation \"{}\" and the observation \"{sup}\" it supersedes; the superseded record's facts are replaced, not additionally counted",
                    entry.get("id").and_then(Value::as_str).unwrap_or("")
                ));
            }
        }
    }

    // per_metric: exactly the eight metric_ids, each exactly once.
    let per_metric = payload
        .get("per_metric")
        .and_then(Value::as_array)
        .unwrap_or(&empty_vec);
    let mut seen_metric: HashSet<String> = HashSet::new();
    let mut by_metric: HashMap<String, Value> = HashMap::new();
    for (i, pm) in per_metric.iter().enumerate() {
        let pmo = if is_object(pm) {
            pm.clone()
        } else {
            Value::Null
        };
        let mid = pmo.get("metric_id").and_then(Value::as_str);
        if !mid.is_some_and(|m| METRIC_IDS.contains(&m)) {
            problems.push(format!(
                "{label} \"{id}\" per_metric[{i}] metric_id {} is not one of the eight closed characteristics",
                json_stringify(pmo.get("metric_id"))
            ));
        } else {
            let mid = mid.unwrap();
            if !seen_metric.insert(mid.to_string()) {
                problems.push(format!(
                    "{label} \"{id}\" per_metric repeats metric_id \"{mid}\""
                ));
            }
            by_metric.insert(mid.to_string(), pmo.clone());
        }
    }
    for mid in METRIC_IDS {
        if !seen_metric.contains(mid) {
            problems.push(format!(
                "{label} \"{id}\" per_metric omits metric_id \"{mid}\"; all eight characteristics are represented, even when the result is \"unknown\""
            ));
        }
    }

    for (mid, pmo) in &by_metric {
        let status = pmo.get("status").and_then(Value::as_str);
        if !status.is_some_and(|s| METRIC_REPORT_STATUSES.contains(&s)) {
            problems.push(format!(
                "{label} \"{id}\" per_metric \"{mid}\" status {} is not one of {{ {} }}",
                json_stringify(pmo.get("status")),
                METRIC_REPORT_STATUSES.join(", ")
            ));
        }
        if status != Some("known") {
            if pmo.get("status_reason").map(blank).unwrap_or(true) {
                problems.push(format!(
                    "{label} \"{id}\" per_metric \"{mid}\" is {} but states no status_reason",
                    json_stringify(pmo.get("status"))
                ));
            }
        } else if pmo.get("status_reason").is_some() {
            problems.push(format!(
                "{label} \"{id}\" per_metric \"{mid}\" is \"known\" but carries a status_reason; a known value needs no absence explanation"
            ));
        }
        let sids = pmo
            .get("sample_observation_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut seen_s: HashSet<String> = HashSet::new();
        for sid in &sids {
            let Some(sid_str) = sid.as_str() else {
                continue;
            };
            if !seen_s.insert(sid_str.to_string()) {
                problems.push(format!("{label} \"{id}\" per_metric \"{mid}\" sample_observation_ids repeats \"{sid_str}\""));
            }
            match by_id.get(sid_str) {
                None => problems.push(format!(
                    "{label} \"{id}\" per_metric \"{mid}\" sample_observation_ids names \"{sid_str}\", which is not an included observation"
                )),
                Some(entry) => {
                    let entry_metric = entry.get("metric_id").and_then(Value::as_str).unwrap_or("");
                    if entry_metric != mid {
                        problems.push(format!(
                            "{label} \"{id}\" per_metric \"{mid}\" sample_observation_ids names \"{sid_str}\", which is an observation of metric \"{entry_metric}\", not \"{mid}\""
                        ));
                    }
                }
            }
        }
        let limitations = pmo
            .get("limitations")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (j, lm) in limitations.iter().enumerate() {
            if blank(lm) {
                problems.push(format!("{label} \"{id}\" per_metric \"{mid}\" limitations[{j}] is empty or whitespace-only"));
            } else if let Some(r) = non_portable_reason(lm.as_str()) {
                problems.push(format!(
                    "{label} \"{id}\" per_metric \"{mid}\" limitations[{j}] contains {r}"
                ));
            }
        }
    }

    if resolve.is_none() {
        return; // nothing further can be recomputed without the boundary.
    }

    // An observation superseded by another INCLUDED observation is not
    // counted at all.
    let mut superseded_ids: HashSet<String> = HashSet::new();
    for r in &included_resolved {
        if let Some(entry) = &r.entry {
            if let Some(sup) = entry.get("supersedes_id").and_then(Value::as_str) {
                if by_id.contains_key(sup) {
                    superseded_ids.insert(sup.to_string());
                }
            }
        }
    }

    for mid in METRIC_IDS {
        let Some(pmo) = by_metric.get(mid) else {
            continue;
        };
        let sids: HashSet<String> = pmo
            .get("sample_observation_ids")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        let mut actual_for_metric: HashSet<String> = HashSet::new();
        for (oid, entry) in &by_id {
            if entry.get("metric_id").and_then(Value::as_str) == Some(mid)
                && !superseded_ids.contains(oid)
            {
                actual_for_metric.insert(oid.clone());
            }
        }
        for oid in &actual_for_metric {
            if !sids.contains(oid) {
                problems.push(format!(
                    "{label} \"{id}\" per_metric \"{mid}\" sample_observation_ids omits included observation \"{oid}\"; an included observation is either counted in its metric's sample or formally excluded with a reason, never silently left out of the aggregate"
                ));
            }
        }
        for oid in &sids {
            if !actual_for_metric.contains(oid) {
                problems.push(format!(
                    "{label} \"{id}\" per_metric \"{mid}\" sample_observation_ids names \"{oid}\", which is not an included observation of this metric (or was superseded); a claimed sample member is not one of the report's own included observations"
                ));
            }
        }

        let mut usable: Vec<Value> = Vec::new();
        let mut any_not_applicable = false;
        let mut any_other = false;
        for (oid, entry) in &by_id {
            if entry.get("metric_id").and_then(Value::as_str) != Some(mid) {
                continue;
            }
            if !sids.contains(oid) {
                continue;
            }
            if resolved_is_usable(entry) {
                usable.push(entry.clone());
            } else if entry.get("status").and_then(Value::as_str) == Some("not_applicable") {
                any_not_applicable = true;
            } else {
                any_other = true;
            }
        }

        let expected_aggregate = compute_metric_aggregate(mid, &usable);
        let expected_status = if !usable.is_empty() {
            if metric_measurement_kind(mid) == Some("classification")
                && expected_aggregate.get("total").and_then(Value::as_i64) == Some(0)
            {
                "unknown"
            } else {
                "known"
            }
        } else if any_not_applicable && !any_other {
            "not_applicable"
        } else {
            "unknown"
        };
        if pmo.get("status").and_then(Value::as_str) != Some(expected_status) {
            problems.push(format!(
                "{label} \"{id}\" per_metric \"{mid}\" states status {}, but the resolved, named sample implies \"{expected_status}\"; the report's conclusion must agree with what its own observations show",
                json_stringify(pmo.get("status"))
            ));
        }
        let stated_aggregate = pmo.get("aggregate").cloned().unwrap_or(Value::Null);
        if stated_aggregate != expected_aggregate {
            problems.push(format!(
                "{label} \"{id}\" per_metric \"{mid}\" aggregate does not match the value recomputed from its resolved sample; a stated aggregate that diverges from the actual observations is rejected, whatever the status"
            ));
        }

        if mid == "post-acceptance-defects" {
            let total_zero = expected_aggregate.get("total").and_then(Value::as_f64) == Some(0.0);
            let coverage = expected_aggregate.get("coverage").and_then(Value::as_str);
            if total_zero && (coverage == Some("partial") || coverage == Some("mixed")) {
                let limitations = pmo
                    .get("limitations")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if limitations.is_empty() {
                    problems.push(format!(
                        "{label} \"{id}\" per_metric \"post-acceptance-defects\" shows zero defects over a \"{}\" observation window but carries no limitation; a zero count over incomplete coverage is not proof of no defects and must say so",
                        coverage.unwrap_or("")
                    ));
                }
            }
        }

        let excluded_for_metric = excluded_resolved.iter().any(|r| {
            r.entry
                .as_ref()
                .is_some_and(|e| e.get("metric_id").and_then(Value::as_str) == Some(mid))
        });
        if excluded_for_metric {
            let limitations = pmo
                .get("limitations")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if limitations.is_empty() {
                problems.push(format!(
                    "{label} \"{id}\" per_metric \"{mid}\" has an excluded observation of this metric but discloses no limitation; an exclusion that would have contributed to a metric is disclosed, not hidden"
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_validity_rejects_impossible_calendar_dates() {
        assert!(is_valid_date(Some("2026-01-15")));
        assert!(!is_valid_date(Some("2026-02-31")));
        assert!(!is_valid_date(Some("2026-02-29"))); // 2026 is not a leap year
        assert!(is_valid_date(Some("2024-02-29"))); // 2024 is a leap year
    }

    #[test]
    fn round_percentage_zero_denominator_is_none() {
        assert_eq!(round_percentage(1.0, 0.0), None);
        assert_eq!(round_percentage(1.0, 3.0), Some(33.33));
    }

    /// Audit: unknown-field reporting over a resolved entry iterates
    /// `serde_json::Map` (a `BTreeMap` in this workspace), never Node's
    /// `Object.keys()` source order — two fields named so their source
    /// order is the opposite of alphabetical order are both still
    /// reported, deterministically.
    #[test]
    fn resolve_pinned_reference_reports_every_unknown_field_deterministically_and_never_panics() {
        let resolver = |_r: &Value| {
            Some(serde_json::json!({
                "record_type": OBSERVATION_RECORD_TYPE,
                "id": "obs-1",
                "reference": "field-evaluation/observations/obs-1",
                "zzzz_extra_field": 1,
                "aaaa_extra_field": 2,
            }))
        };
        let ref_val = serde_json::json!({
            "record_type": OBSERVATION_RECORD_TYPE, "id": "obs-1",
            "reference": "field-evaluation/observations/obs-1",
        });
        for _ in 0..3 {
            let mut problems = Vec::new();
            resolve_pinned_reference(
                &mut problems,
                "field evaluation report",
                "h1",
                "included_observation",
                &ref_val,
                &resolver,
                OBSERVATION_RECORD_TYPE,
            );
            assert!(
                problems.iter().any(|p| p.contains("zzzz_extra_field")),
                "{problems:?}"
            );
            assert!(
                problems.iter().any(|p| p.contains("aaaa_extra_field")),
                "{problems:?}"
            );
        }
    }

    #[test]
    fn resolve_pinned_reference_on_a_resolver_that_finds_nothing_fails_closed_without_panicking() {
        let resolver = |_r: &Value| None;
        let ref_val = serde_json::json!({
            "record_type": "execution-run", "id": "run-1", "reference": "runs/run-1",
        });
        let mut problems = Vec::new();
        let entry = resolve_pinned_reference(
            &mut problems,
            "field evaluation observation",
            "h1",
            "execution_run_ref",
            &ref_val,
            &resolver,
            "execution-run",
        );
        assert!(entry.is_none());
        assert!(!problems.is_empty());
    }

    #[test]
    fn evaluate_field_evaluation_on_a_null_document_fails_closed_without_panicking() {
        let record_schema = serde_json::json!({"type": "object"});
        let envelope_schema = serde_json::json!({"type": "object"});
        let opts = EvalOpts {
            record_schema: &record_schema,
            envelope_schema: &envelope_schema,
            resolve_records: None,
        };
        let problems = evaluate_field_evaluation(&Value::Null, &opts);
        assert!(!problems.is_empty());
    }

    #[test]
    fn compute_metric_aggregate_on_no_usable_entries_never_panics() {
        for metric_id in METRIC_IDS {
            let aggregate = compute_metric_aggregate(metric_id, &[]);
            assert!(aggregate.is_object());
        }
    }

    /// Regression: an earlier version matched `(?:\.\d+)?` without
    /// capturing it, so `parse_datetime_millis` silently dropped
    /// fractional seconds entirely — two timestamps differing only in
    /// their fraction (e.g. `.100Z` vs `.200Z`) parsed to the SAME
    /// millisecond, and a genuinely later `end_ts` wrongly compared as
    /// not-after `start_ts`. Confirmed against real `Date.parse` output
    /// (1/2/3/4/6-digit fractions) before writing this fix.
    #[test]
    fn parse_datetime_millis_reads_fractional_seconds_to_millisecond_precision() {
        let base = parse_datetime_millis("2026-01-01T00:00:00Z").unwrap();
        assert_eq!(
            parse_datetime_millis("2026-01-01T00:00:00.1Z"),
            Some(base + 100)
        );
        assert_eq!(
            parse_datetime_millis("2026-01-01T00:00:00.12Z"),
            Some(base + 120)
        );
        assert_eq!(
            parse_datetime_millis("2026-01-01T00:00:00.123Z"),
            Some(base + 123)
        );
        // 4+ digit fractions are TRUNCATED to the first three, never rounded.
        assert_eq!(
            parse_datetime_millis("2026-01-01T00:00:00.1234Z"),
            Some(base + 123)
        );
        assert_eq!(
            parse_datetime_millis("2026-01-01T00:00:00.123456Z"),
            Some(base + 123)
        );
        assert_eq!(
            parse_datetime_millis("2026-01-01T00:00:00.999Z"),
            Some(base + 999)
        );

        let a = parse_datetime_millis("2026-01-01T00:00:00.100Z").unwrap();
        let b = parse_datetime_millis("2026-01-01T00:00:00.200Z").unwrap();
        assert_eq!(
            b - a,
            100,
            "a distinct sub-second fraction must move the millisecond value"
        );
    }

    fn duration_measurement(start_ts: &str, end_ts: &str) -> Value {
        serde_json::json!({
            "kind": "duration",
            "duration_kind": "calendar",
            "unit": "seconds",
            "seconds": 0.1,
            "start_ts": start_ts,
            "end_ts": end_ts,
        })
    }

    #[test]
    fn duration_interval_differing_only_in_sub_second_fraction_is_accepted() {
        let m = duration_measurement("2026-01-01T00:00:00.100Z", "2026-01-01T00:00:00.200Z");
        let issues = measurement_issues("context-entry-time", &m);
        assert!(
            !issues
                .iter()
                .any(|i| i.contains("end_ts is not after start_ts")),
            "{issues:?}"
        );
    }

    #[test]
    fn duration_interval_with_equal_timestamps_is_rejected() {
        let m = duration_measurement("2026-01-01T00:00:00.500Z", "2026-01-01T00:00:00.500Z");
        let issues = measurement_issues("context-entry-time", &m);
        assert!(
            issues
                .iter()
                .any(|i| i.contains("end_ts is not after start_ts")),
            "{issues:?}"
        );
    }

    #[test]
    fn duration_interval_with_offset_and_sub_second_difference_is_accepted() {
        // start_ts is UTC 00:00:00.100Z under its own +02:00 offset; end_ts
        // is UTC 00:00:00.200Z — 100ms strictly later once the offset and
        // the fraction are both accounted for.
        let m = duration_measurement("2026-01-01T02:00:00.100+02:00", "2026-01-01T00:00:00.200Z");
        let issues = measurement_issues("context-entry-time", &m);
        assert!(
            !issues
                .iter()
                .any(|i| i.contains("end_ts is not after start_ts")),
            "{issues:?}"
        );
    }
}
