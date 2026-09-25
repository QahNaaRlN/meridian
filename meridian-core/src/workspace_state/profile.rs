//! M-09 — the stack profile every inventory repository declares: it must
//! name a profile of the accepted Kernel pool (never `universal`, which is
//! the absence of one), and the repository's own manifest must support it.
//! A manifest that cannot be read leaves the declaration `UNVERIFIED`.

use std::collections::{BTreeMap, BTreeSet};

use crate::mechanical_integrity::stack_profiles::StackProfileName;
use crate::types::{Diagnostic, DiagnosticLevel, NonEmptyString};

use super::diagnostic;
use super::inventory::RepositoryIdentity;

/// Dependency names a profile expects (or excludes) in one manifest
/// section, in the pool's own order.
pub type SectionPredicate = Vec<(String, Vec<String>)>;

/// One profile of the Kernel pool with its manifest evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackProfileSpec {
    name: StackProfileName,
    manifest: NonEmptyString,
    requires: SectionPredicate,
    forbids: SectionPredicate,
    forbids_fields: Vec<String>,
}

impl StackProfileSpec {
    pub fn new(
        name: StackProfileName,
        manifest: NonEmptyString,
        requires: SectionPredicate,
        forbids: SectionPredicate,
        forbids_fields: Vec<String>,
    ) -> Self {
        Self {
            name,
            manifest,
            requires,
            forbids,
            forbids_fields,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// The manifest file name the profile's evidence lives in.
    pub fn manifest(&self) -> &str {
        self.manifest.as_str()
    }
}

/// A manifest reduced to what the predicates read: every top-level field,
/// and the key set of every top-level field whose value is an object.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    fields: BTreeSet<String>,
    sections: BTreeMap<String, BTreeSet<String>>,
}

impl Manifest {
    pub fn new(fields: BTreeSet<String>, sections: BTreeMap<String, BTreeSet<String>>) -> Self {
        Self { fields, sections }
    }
}

/// What a declaration needs next.
#[derive(Debug)]
pub enum Declaration<'a> {
    /// Refused without looking at any manifest.
    Refused(Diagnostic),
    /// The declared profile exists; its evidence is the manifest at
    /// `manifest_path` (as the inventory lists it).
    Evidence {
        spec: &'a StackProfileSpec,
        manifest_path: &'a str,
    },
}

/// The pool-and-listing half of the check.
pub fn declaration<'a>(
    identity: &'a RepositoryIdentity,
    pool: &'a [StackProfileSpec],
) -> Declaration<'a> {
    let id = identity.id().as_str();
    let name = identity.profile().trim();
    if name.is_empty() {
        return Declaration::Refused(diagnostic(
            DiagnosticLevel::Fail,
            format!("stack-profile: {id} declares no profile; a repository whose type is unstated cannot have its norms judged applicable or not"),
        ));
    }
    if name == "universal" {
        return Declaration::Refused(diagnostic(
            DiagnosticLevel::Fail,
            format!("stack-profile: {id} declares \"universal\", which is the absence of a profile, not the type of a project"),
        ));
    }
    let Some(spec) = pool.iter().find(|s| s.name() == name) else {
        return Declaration::Refused(diagnostic(
            DiagnosticLevel::Fail,
            format!("stack-profile: {id} declares \"{name}\", which is not in the pool; a profile is named from the pool or added to it by decision, never invented at the point of use"),
        ));
    };
    let listed = identity
        .manifests()
        .iter()
        .map(NonEmptyString::as_str)
        .find(|m| {
            let normalized = m.replace('\\', "/");
            normalized.rsplit('/').next() == Some(spec.manifest())
        });
    match listed {
        Some(manifest_path) => Declaration::Evidence {
            spec,
            manifest_path,
        },
        None => Declaration::Refused(diagnostic(
            DiagnosticLevel::Fail,
            format!(
                "stack-profile: {id} declares \"{name}\", whose evidence is {}, but the entry lists no such manifest",
                spec.manifest()
            ),
        )),
    }
}

/// The manifest could not be read from here.
pub fn unreadable_manifest(
    identity: &RepositoryIdentity,
    spec: &StackProfileSpec,
    manifest_path: &str,
) -> Diagnostic {
    diagnostic(
        DiagnosticLevel::Info,
        format!(
            "stack-profile: {} — {} at {manifest_path} is not readable from here; the declared \"{}\" is UNVERIFIED, not confirmed",
            identity.id().as_str(),
            spec.manifest(),
            spec.name()
        ),
    )
}

/// The manifest was read but is not JSON; `reason` is the parser's text.
pub fn invalid_manifest(
    identity: &RepositoryIdentity,
    manifest_path: &str,
    reason: &str,
) -> Diagnostic {
    diagnostic(
        DiagnosticLevel::Fail,
        format!(
            "stack-profile: {} — {manifest_path} is not valid JSON: {reason}",
            identity.id().as_str()
        ),
    )
}

/// The manifest half: `None` when the manifest supports the declaration.
pub fn judge_manifest(
    identity: &RepositoryIdentity,
    spec: &StackProfileSpec,
    manifest: &Manifest,
) -> Option<Diagnostic> {
    let mut problems = Vec::new();
    for (section, names) in &spec.requires {
        let Some(keys) = manifest.sections.get(section) else {
            problems.push(format!(
                "manifest has no \"{section}\" section, which the profile requires"
            ));
            continue;
        };
        let absent: Vec<String> = names
            .iter()
            .filter(|n| !keys.contains(*n))
            .map(|n| format!("\"{n}\""))
            .collect();
        if !absent.is_empty() {
            problems.push(format!(
                "\"{section}\" does not carry {}",
                absent.join(", ")
            ));
        }
    }
    for (section, names) in &spec.forbids {
        let Some(keys) = manifest.sections.get(section) else {
            continue;
        };
        let present: Vec<String> = names
            .iter()
            .filter(|n| keys.contains(*n))
            .map(|n| format!("\"{n}\""))
            .collect();
        if !present.is_empty() {
            problems.push(format!(
                "\"{section}\" carries {}, which this profile excludes",
                present.join(", ")
            ));
        }
    }
    for field in &spec.forbids_fields {
        if manifest.fields.contains(field) {
            problems.push(format!(
                "manifest declares \"{field}\", which this profile excludes"
            ));
        }
    }
    (!problems.is_empty()).then(|| {
        diagnostic(
            DiagnosticLevel::Fail,
            format!(
                "stack-profile: {} declares \"{}\" but its manifest does not support it — {}; correct the declaration or the pool, do not widen the predicate to fit",
                identity.id().as_str(),
                spec.name(),
                problems.join("; ")
            ),
        )
    })
}

/// The run-level lines.
pub fn summarize(total: usize, confirmed: usize, unconfirmed: usize) -> Vec<Diagnostic> {
    if total == 0 {
        return vec![diagnostic(
            DiagnosticLevel::Warn,
            "stack-profile: repository inventory not found; no declaration was checked".to_string(),
        )];
    }
    let mut lines = Vec::new();
    if confirmed > 0 {
        lines.push(diagnostic(
            DiagnosticLevel::Info,
            format!("stack-profile: {confirmed}/{total} declarations confirmed against the repository's own manifest"),
        ));
    }
    if unconfirmed > 0 {
        lines.push(diagnostic(
            DiagnosticLevel::Warn,
            format!("stack-profile: {unconfirmed}/{total} declarations could not be confirmed; the manifest was not reachable from here"),
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RepositoryId;
    use crate::workspace_state::inventory::{RecordedVcs, WorkingTree};

    fn spec() -> StackProfileSpec {
        StackProfileSpec::new(
            StackProfileName::new("vue-spa").unwrap(),
            NonEmptyString::new("package.json").unwrap(),
            vec![
                ("dependencies".to_string(), vec!["vue".to_string()]),
                ("devDependencies".to_string(), vec!["vite".to_string()]),
            ],
            vec![("dependencies".to_string(), vec!["react".to_string()])],
            vec!["workspaces".to_string()],
        )
    }

    fn identity(profile: &str, manifests: &[&str]) -> RepositoryIdentity {
        RepositoryIdentity::new(
            RepositoryId::new("web").unwrap(),
            NonEmptyString::new("/work/web").unwrap(),
            profile,
            RecordedVcs {
                revision: NonEmptyString::new("a".repeat(40)).unwrap(),
                git_ref: NonEmptyString::new("dev").unwrap(),
                working_tree: WorkingTree::Clean,
            },
            None,
            manifests
                .iter()
                .map(|m| NonEmptyString::new(*m).unwrap())
                .collect(),
        )
    }

    fn manifest(fields: &[&str], sections: &[(&str, &[&str])]) -> Manifest {
        Manifest::new(
            fields.iter().map(|f| f.to_string()).collect(),
            sections
                .iter()
                .map(|(s, keys)| (s.to_string(), keys.iter().map(|k| k.to_string()).collect()))
                .collect(),
        )
    }

    fn refused(identity: &RepositoryIdentity, pool: &[StackProfileSpec]) -> String {
        match declaration(identity, pool) {
            Declaration::Refused(d) => d.message().to_string(),
            Declaration::Evidence { .. } => panic!("expected a refusal"),
        }
    }

    #[test]
    fn workspace_state_profile_refuses_absent_universal_unknown_and_unlisted() {
        let pool = [spec()];
        assert!(refused(&identity(" ", &[]), &pool).ends_with("declares no profile; a repository whose type is unstated cannot have its norms judged applicable or not"));
        assert!(refused(&identity("universal", &[]), &pool)
            .contains("\"universal\", which is the absence of a profile"));
        assert_eq!(
            refused(&identity("vue-monorepo", &[]), &pool),
            "stack-profile: web declares \"vue-monorepo\", which is not in the pool; a profile is named from the pool or added to it by decision, never invented at the point of use"
        );
        assert_eq!(
            refused(&identity("vue-spa", &["/work/web/composer.json"]), &pool),
            "stack-profile: web declares \"vue-spa\", whose evidence is package.json, but the entry lists no such manifest"
        );
    }

    #[test]
    fn workspace_state_profile_confirms_a_supporting_manifest_and_names_every_mismatch() {
        let pool = [spec()];
        let web = identity("vue-spa", &["C:\\work\\web\\package.json"]);
        let Declaration::Evidence {
            spec,
            manifest_path,
        } = declaration(&web, &pool)
        else {
            panic!("expected evidence");
        };
        assert_eq!(manifest_path, "C:\\work\\web\\package.json");
        let good = manifest(
            &["dependencies", "devDependencies"],
            &[("dependencies", &["vue"]), ("devDependencies", &["vite"])],
        );
        assert_eq!(judge_manifest(&web, spec, &good), None);
        let bad = manifest(
            &["dependencies", "workspaces"],
            &[("dependencies", &["react"])],
        );
        assert_eq!(
            judge_manifest(&web, spec, &bad).unwrap().message(),
            "stack-profile: web declares \"vue-spa\" but its manifest does not support it — \"dependencies\" does not carry \"vue\"; manifest has no \"devDependencies\" section, which the profile requires; \"dependencies\" carries \"react\", which this profile excludes; manifest declares \"workspaces\", which this profile excludes; correct the declaration or the pool, do not widen the predicate to fit"
        );
    }

    #[test]
    fn workspace_state_profile_summarizes_confirmed_and_unreachable() {
        let lines: Vec<String> = summarize(3, 2, 1)
            .iter()
            .map(|d| d.message().to_string())
            .collect();
        assert_eq!(
            lines,
            [
                "stack-profile: 2/3 declarations confirmed against the repository's own manifest",
                "stack-profile: 1/3 declarations could not be confirmed; the manifest was not reachable from here",
            ]
        );
        assert_eq!(summarize(0, 0, 0)[0].level(), DiagnosticLevel::Warn);
    }
}
