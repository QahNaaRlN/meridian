//! Port of `scripts/kernel-validate.mjs`'s `document-identity` check
//! (`standards/workspace/document-identity.md`): file/directory naming
//! (lower kebab-case, no smell of date/version/draft state in the name) and,
//! for every Markdown document that carries its own Front Matter, the six
//! required fields plus a `document_type` from the closed pool.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use fancy_regex::Regex;
use meridian_app::source_format::carries_own_front_matter;

/// `"skill"` is deliberately absent: delivery is a field of its own now
/// (`agent-instruction-identity.md`), not a type.
fn document_types() -> HashSet<&'static str> {
    [
        "standard",
        "contract",
        "protocol",
        "template",
        "readme",
        "reference",
        "tutorial",
        "how-to",
        "explanation",
        "rfc",
        "adr",
        "concept",
        "problem",
        "incident",
        "analysis",
        "plan",
        "report",
        "technical-specification",
        "changelog",
        "unclassified",
    ]
    .into_iter()
    .collect()
}

fn external_names() -> HashSet<&'static str> {
    [
        "README.md",
        "SKILL.md",
        "AGENTS.md",
        "PIN.yaml",
        "LICENSE",
        "VERSION",
        "Cargo.toml",
        "Cargo.lock",
    ]
    .into_iter()
    .collect()
}

fn root_uppercase() -> HashSet<&'static str> {
    [
        "README.md",
        "LICENSE",
        "VERSION",
        "CHANGELOG.md",
        "COMPATIBILITY.md",
        "MANUAL.md",
    ]
    .into_iter()
    .collect()
}

pub struct Outcome {
    pub failures: Vec<String>,
    pub checked: usize,
    pub typed_docs: usize,
}

pub fn run(kernel_root: &Path, files: &[PathBuf]) -> Outcome {
    let lower_segment = Regex::new(r"^\.?[a-z0-9]+([._-][a-z0-9]+)*$").unwrap();
    let name_smell = Regex::new(
        r"(^|-)(final|new|old|copy|tmp|draft)(-|\.|$)|\d{4}-\d{2}-\d{2}|(^|-)v?\d+\.\d+(\.\d+)?(-|\.|$)",
    )
    .unwrap();

    let document_types = document_types();
    let external_names = external_names();
    let root_uppercase = root_uppercase();

    let mut rels: Vec<String> = files
        .iter()
        .map(|f| relative_slash(kernel_root, f))
        .collect();
    rels.sort();
    rels.dedup();

    let mut release_unit_roots: HashSet<String> = HashSet::new();
    release_unit_roots.insert(String::new());
    for rel in &rels {
        if Path::new(rel).file_name().and_then(|n| n.to_str()) == Some("VERSION") {
            let dir = match rel.rfind('/') {
                Some(idx) => rel[..idx].to_string(),
                None => String::new(),
            };
            release_unit_roots.insert(dir);
        }
    }

    let mut failures = Vec::new();

    for rel in &rels {
        let segs: Vec<&str> = rel.split('/').collect();
        let base = segs[segs.len() - 1];
        for seg in &segs[..segs.len() - 1] {
            if !matches(&lower_segment, seg) {
                failures.push(format!(
                    "document-identity: directory \"{seg}\" in \"{rel}\" is not lower kebab-case"
                ));
            }
        }
        let dir = if segs.len() == 1 {
            String::new()
        } else {
            segs[..segs.len() - 1].join("/")
        };
        let root_allowed = root_uppercase.contains(base) && release_unit_roots.contains(&dir);
        if !external_names.contains(base) && !root_allowed && !matches(&lower_segment, base) {
            failures.push(format!(
                "document-identity: \"{rel}\" is not lower kebab-case; upper case is reserved for the release unit's root set"
            ));
        }
        if matches(&name_smell, base) {
            failures.push(format!(
                "document-identity: \"{rel}\" carries a date, a version or one of final/new/old/copy/tmp/draft in its name; that state belongs in Front Matter"
            ));
        }
    }

    let mut typed_docs = 0usize;
    for rel in rels
        .iter()
        .filter(|r| r.ends_with(".md") && carries_own_front_matter(r))
    {
        let Ok(text) = fs::read_to_string(kernel_root.join(rel)) else {
            continue;
        };
        if !text.starts_with("---") {
            failures.push(format!(
                "document-identity: {rel} has no Front Matter; a Kernel document declares its type rather than leaving it to be guessed"
            ));
            continue;
        }
        let end = text[3..].find("\n---").map(|i| i + 3);
        let fm = match end {
            Some(end) => &text[4..end],
            None => "",
        };
        for field in ["title", "status", "scope", "owner", "created", "updated"] {
            let pattern = format!(r"(?m)^{field}:[^\S\r\n]*\S");
            let re = Regex::new(&pattern).unwrap();
            if !matches(&re, fm) {
                failures.push(format!(
                    "document-identity: {rel} is missing required Front Matter field \"{field}\""
                ));
            }
        }
        let type_re = Regex::new(r"(?m)^document_type:\s*(\S+)").unwrap();
        let doc_type = type_re
            .captures(fm)
            .ok()
            .flatten()
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());
        match doc_type {
            None => {
                failures.push(format!(
                    "document-identity: {rel} declares no document_type"
                ));
            }
            Some(ref t) if !document_types.contains(t.as_str()) => {
                failures.push(format!(
                    "document-identity: {rel} declares document_type \"{t}\", which is not in the pool; a new type is a change to the standard, not to one file"
                ));
            }
            Some(ref t) if t == "unclassified" => {
                let reason_re = Regex::new(r"(?m)^unclassified_reason:[^\S\r\n]*\S").unwrap();
                if !matches(&reason_re, fm) {
                    failures.push(format!(
                        "document-identity: {rel} is unclassified with no unclassified_reason; the state is legal, an unexplained one is not"
                    ));
                } else {
                    typed_docs += 1;
                }
            }
            Some(_) => {
                typed_docs += 1;
            }
        }
    }

    Outcome {
        failures,
        checked: rels.len(),
        typed_docs,
    }
}

fn matches(re: &Regex, text: &str) -> bool {
    re.is_match(text).unwrap_or(false)
}

fn relative_slash(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
