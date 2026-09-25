//! Neutral owner for schema-reference resolution and free-text
//! portability checks (`rust-architecture-conformance-3` corrective round,
//! item 2: "TaskSpecification must be a genuinely valid type"). Both
//! functions are pure string/regex logic — no filesystem, no Git, no
//! process, no `serde_json::Value` — so they belong in `meridian-core`
//! rather than `meridian-app`: [`crate::task_contracts::specification::check_task_specification`]
//! needs [`non_portable_reason`] to gate its own [`crate::task_contracts::TaskSpecification`]
//! construction (a rooted path in a domain field must be exactly as
//! disqualifying as a missing acceptance criterion, checked by the SAME
//! public constructor, not by a second, optional pass a caller could skip).
//!
//! `meridian-app::operating_model::reference_portability` re-exports both
//! functions unchanged for its own `$schema`-envelope check and the five
//! other operating-model families that import them through that module's
//! own facade — moving the definitions here does not change any of those
//! call sites.

use fancy_regex::Regex;

/// A synthetic namespace root [`resolve_schema_ref`] resolves beneath, so
/// that a reference climbing past the Meridian namespace root is detectable
/// rather than silently clamped. It is a lexical anchor for address
/// normalisation, not a filesystem path.
const NAMESPACE_ROOT: &str = "/__meridian_namespace__";

/// The canonical logical base every operating-model record's `$schema`
/// reference is written relative to.
pub const CANONICAL_RECORD_BASE: &str = "records/task-specification";

fn is_drive_letter_prefix(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// A minimal `path.posix.normalize` for the one shape this module needs: an
/// always-absolute input (it always starts from [`NAMESPACE_ROOT`]),
/// collapsing `.`/empty segments and popping a real segment on `..`
/// (clamped at the root, exactly as Node's POSIX `normalize` does — never
/// erroring past it).
fn posix_normalize_absolute(p: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => continue,
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    format!("/{}", stack.join("/"))
}

/// Resolve a record's declared `$schema` as a logical address within the
/// Meridian namespace, from the canonical logical base. Returns the
/// namespace-root-relative address it resolves to, or `None` when it is not
/// a resolvable in-namespace relative reference (an absolute path, a
/// drive-letter path, a backslash path, or a reference that climbs out of
/// the namespace root). Port of `resolveSchemaRef`.
pub fn resolve_schema_ref(declared: Option<&str>) -> Option<String> {
    let declared = declared.filter(|s| !s.is_empty())?;
    if declared.contains('\\') {
        return None;
    }
    if declared.starts_with('/') || is_drive_letter_prefix(declared) {
        return None;
    }
    let from = format!("{NAMESPACE_ROOT}/{CANONICAL_RECORD_BASE}");
    let resolved = posix_normalize_absolute(&format!("{from}/{declared}"));
    let prefix = format!("{NAMESPACE_ROOT}/");
    if resolved != NAMESPACE_ROOT && !resolved.starts_with(&prefix) {
        return None;
    }
    if resolved == NAMESPACE_ROOT {
        Some(String::new())
    } else {
        Some(resolved[NAMESPACE_ROOT.len() + 1..].to_string())
    }
}

/// Any ROOTED machine path, a drive-letter path, a backslash-separated
/// path, a `~/` home reference or a `file://` URL anywhere in free text
/// breaks portability. Returns a short reason string or `None`. Port of
/// `nonPortableReason`.
static FILE_URL_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"(?i)\bfile://").expect("file url pattern compiles"));
static SCRUB_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"(?i)\b(?:https?|ftp|ftps|ssh|git|mailto):(?://)?[^\s"'()<>\[\]]+"#)
        .expect("scrub pattern compiles")
});
static HOME_REF_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"(?:^|[^\p{L}\p{N}._~-])~/"#).expect("home ref pattern compiles")
});
static DRIVE_LETTER_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"(?:^|[^A-Za-z0-9])[A-Za-z]:[\\/][^\s"')>\]]+"#)
        .expect("drive letter pattern compiles")
});
static ROOTED_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"(?:^|[^\p{L}\p{N}._~-])/[^\s/"'()<>\[\]][^\s"')>\]]*"#)
        .expect("rooted path pattern compiles")
});
static BACKSLASH_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"[\p{L}\p{N}_.-]+\\[\p{L}\p{N}_.-]+"#).expect("backslash pattern compiles")
});

pub fn non_portable_reason(s: Option<&str>) -> Option<String> {
    let s = s.filter(|s| !s.is_empty())?;

    if FILE_URL_RE.is_match(s).unwrap_or(false) {
        return Some("a file:// URL".to_string());
    }

    let scrubbed = SCRUB_RE.replace_all(s, " ");

    if HOME_REF_RE.is_match(scrubbed.as_ref()).unwrap_or(false) {
        return Some("a \"~/\" home-directory reference".to_string());
    }

    if DRIVE_LETTER_RE.is_match(scrubbed.as_ref()).unwrap_or(false) {
        return Some("a drive-letter path".to_string());
    }

    if ROOTED_RE.is_match(scrubbed.as_ref()).unwrap_or(false) {
        return Some("a rooted (absolute) POSIX path".to_string());
    }

    if BACKSLASH_RE.is_match(scrubbed.as_ref()).unwrap_or(false) {
        return Some("a backslash-separated path".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_schema_ref_accepts_the_canonical_relative_reference() {
        let resolved = resolve_schema_ref(Some(
            "../../registries/operating-model/task-specification.schema.json",
        ));
        assert_eq!(
            resolved.as_deref(),
            Some("registries/operating-model/task-specification.schema.json")
        );
    }

    #[test]
    fn resolve_schema_ref_rejects_an_absolute_path() {
        assert!(resolve_schema_ref(Some(
            "/registries/operating-model/task-specification.schema.json"
        ))
        .is_none());
    }

    #[test]
    fn resolve_schema_ref_rejects_climbing_out_of_the_namespace() {
        assert!(resolve_schema_ref(Some("../../../../../../etc/passwd")).is_none());
    }

    #[test]
    fn non_portable_reason_detects_file_url() {
        assert!(non_portable_reason(Some("file:///etc/passwd"))
            .unwrap()
            .contains("file://"));
    }

    #[test]
    fn non_portable_reason_detects_home_reference() {
        assert!(non_portable_reason(Some("go look at ~/secrets")).is_some());
    }

    #[test]
    fn non_portable_reason_detects_drive_letter_path() {
        assert!(non_portable_reason(Some("C:\\Users\\name"))
            .unwrap()
            .contains("drive-letter"));
    }

    #[test]
    fn non_portable_reason_detects_rooted_posix_path() {
        assert!(non_portable_reason(Some("see /etc/passwd for details"))
            .unwrap()
            .contains("rooted"));
    }

    #[test]
    fn non_portable_reason_is_silent_on_relative_paths() {
        assert!(non_portable_reason(Some("src/config/parser")).is_none());
        assert!(non_portable_reason(Some("n/a")).is_none());
        assert!(non_portable_reason(Some("24/7")).is_none());
        assert!(non_portable_reason(Some("owner-decision:2026-09-09:example")).is_none());
    }

    #[test]
    fn non_portable_reason_is_silent_on_ordinary_urls() {
        assert!(non_portable_reason(Some("see https://example.invalid/api for details")).is_none());
    }

    #[test]
    fn non_portable_reason_flags_unicode_rooted_paths_with_no_allow_list() {
        assert!(non_portable_reason(Some("см. /Проект/файл")).is_some());
        assert!(non_portable_reason(Some("see /数据/文件")).is_some());
    }
}
