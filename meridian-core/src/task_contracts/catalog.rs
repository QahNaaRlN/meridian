//! Domain types for `task-pattern-registry` — the built-in catalogue of the
//! seven universal task patterns, one per active `work_kind`/`change_class`
//! pair (`standards/workspace/task-pattern-registry.yaml`,
//! `registries/operating-model/task-pattern-registry.schema.json`).
//!
//! Reuses [`crate::resolver::WorkItemKind`] (and, through it,
//! [`crate::resolver::WorkKind`]/[`crate::resolver::ChangeClass`]) as the
//! ONE classification pool: a second string or enum pool for the same seven
//! pairs is never created here. A canonical link's path reuses
//! [`crate::types::WorkspaceRelativePath`] — the same portability shape
//! (non-empty, POSIX-relative, normalised, no backslash, no absolute/drive
//! form) a canonical link's `path` must already satisfy, so this module
//! adds no second path-portability rule.
//!
//! No filesystem, Git, process, env or `serde_json::Value` here — only
//! already-validated domain values in, [`crate::types::Diagnostic`]s and
//! typed domain values out.

use std::collections::{HashMap, HashSet};

use crate::resolver::{ChangeClass, WorkItemKind};
use crate::types::{
    Diagnostic, DiagnosticLevel, NonEmptyString, SemanticId, WorkspaceRelativePath,
};

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

/// One `applicable_protocols`/`applicable_skills`/`applicable_evidence_contracts`
/// entry: either a real, portable canonical reference, or an explicit,
/// reasoned absence. There is no third, implicit "unset" state — an absent
/// reference always carries the reason it is absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalLink {
    Present {
        id: SemanticId,
        path: WorkspaceRelativePath,
    },
    Absent {
        reason: NonEmptyString,
    },
}

impl CanonicalLink {
    pub fn path(&self) -> Option<&WorkspaceRelativePath> {
        match self {
            CanonicalLink::Present { path, .. } => Some(path),
            CanonicalLink::Absent { .. } => None,
        }
    }

    pub fn id(&self) -> Option<&SemanticId> {
        match self {
            CanonicalLink::Present { id, .. } => Some(id),
            CanonicalLink::Absent { .. } => None,
        }
    }

    pub fn is_present(&self) -> bool {
        matches!(self, CanonicalLink::Present { .. })
    }
}

/// Which of the three link arrays a [`CanonicalLink`] sits in — the
/// classification rule (skill-package shape vs. never-a-skill-package
/// shape) depends on which array holds the link, something a flat per-item
/// JSON Schema cannot express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanonicalLinkField {
    Protocols,
    Skills,
    EvidenceContracts,
}

impl CanonicalLinkField {
    pub fn as_str(self) -> &'static str {
        match self {
            CanonicalLinkField::Protocols => "applicable_protocols",
            CanonicalLinkField::Skills => "applicable_skills",
            CanonicalLinkField::EvidenceContracts => "applicable_evidence_contracts",
        }
    }
}

impl core::fmt::Display for CanonicalLinkField {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

fn is_skill_artifact_path(p: &str) -> bool {
    let Some(rest) = p.strip_prefix("skills/") else {
        return false;
    };
    // Exactly one non-empty segment, then "/SKILL.md" — mirrors
    // `^skills\/[^/]+\/SKILL\.md$`.
    match rest.strip_suffix("/SKILL.md") {
        Some(middle) => !middle.is_empty() && !middle.contains('/'),
        None => false,
    }
}

fn is_under_skills(p: &str) -> bool {
    p == "skills" || p.starts_with("skills/")
}

/// Validates ONE link against the classification rule its own field
/// carries: `applicable_skills` only ever holds a skill-package path
/// (`skills/<name>/SKILL.md`); `applicable_protocols`/
/// `applicable_evidence_contracts` never do.
fn check_link_field_classification(
    pattern_id: &SemanticId,
    field: CanonicalLinkField,
    link: &CanonicalLink,
) -> Option<Diagnostic> {
    let CanonicalLink::Present { path, .. } = link else {
        return None;
    };
    let p = path.as_str();
    if field == CanonicalLinkField::Skills {
        if !is_skill_artifact_path(p) {
            return Some(fail(format!(
                "pattern \"{pattern_id}\" applicable_skills link \"{p}\" is not a skill package (skills/<name>/SKILL.md); a protocol or contract is a different axis"
            )));
        }
        return None;
    }
    if is_under_skills(p) {
        return Some(fail(format!(
            "pattern \"{pattern_id}\" {field} link \"{p}\" points at a skill package; a way of carrying work out belongs in applicable_skills, not among protocols or evidence contracts"
        )));
    }
    None
}

/// One task-pattern entry: its identity, its closed classification
/// ([`WorkItemKind`] — reused, not a second pool) and its three link
/// arrays. The ONLY constructor is [`TaskPatternEntry::try_new`]: a link
/// whose classification is wrong for the field it sits in can never reach a
/// built entry, so a caller can never observe a "partially valid" one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPatternEntry {
    id: SemanticId,
    kind: WorkItemKind,
    protocols: Vec<CanonicalLink>,
    skills: Vec<CanonicalLink>,
    evidence_contracts: Vec<CanonicalLink>,
}

impl TaskPatternEntry {
    pub fn try_new(
        id: SemanticId,
        kind: WorkItemKind,
        protocols: Vec<CanonicalLink>,
        skills: Vec<CanonicalLink>,
        evidence_contracts: Vec<CanonicalLink>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut problems = Vec::new();
        for (field, links) in [
            (CanonicalLinkField::Protocols, &protocols),
            (CanonicalLinkField::Skills, &skills),
            (CanonicalLinkField::EvidenceContracts, &evidence_contracts),
        ] {
            for link in links {
                if let Some(d) = check_link_field_classification(&id, field, link) {
                    problems.push(d);
                }
            }
        }
        if problems.is_empty() {
            Ok(Self {
                id,
                kind,
                protocols,
                skills,
                evidence_contracts,
            })
        } else {
            Err(problems)
        }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }

    pub fn kind(&self) -> WorkItemKind {
        self.kind
    }

    pub fn protocols(&self) -> &[CanonicalLink] {
        &self.protocols
    }

    pub fn skills(&self) -> &[CanonicalLink] {
        &self.skills
    }

    pub fn evidence_contracts(&self) -> &[CanonicalLink] {
        &self.evidence_contracts
    }

    pub fn present_paths(links: &[CanonicalLink]) -> Vec<&WorkspaceRelativePath> {
        links.iter().filter_map(CanonicalLink::path).collect()
    }

    /// Every `present` link across all three fields, paired with the field
    /// it came from — the external per-link filesystem/tracked-set check
    /// (`meridian-app`'s own orchestration) walks this rather than
    /// re-deriving field membership itself.
    pub fn present_links(&self) -> Vec<(CanonicalLinkField, &SemanticId, &WorkspaceRelativePath)> {
        let mut out = Vec::new();
        for (field, links) in [
            (CanonicalLinkField::Protocols, &self.protocols),
            (CanonicalLinkField::Skills, &self.skills),
            (
                CanonicalLinkField::EvidenceContracts,
                &self.evidence_contracts,
            ),
        ] {
            for link in links {
                if let CanonicalLink::Present { id, path } = link {
                    out.push((field, id, path));
                }
            }
        }
        out
    }
}

pub const REFACTOR_PROTOCOL: &str = "verification/functional-parity/refactor-protocol.md";
pub const REFACTOR_EVIDENCE: &str =
    "verification/functional-parity/functional-parity-evidence-contract.md";
pub const BUGFIX_SKILL: &str = "skills/bugfix-protocol/SKILL.md";

fn pair_label(kind: WorkItemKind) -> String {
    match kind.change_class() {
        Some(cc) => format!("({}, {cc})", kind.work_kind()),
        None => format!("({})", kind.work_kind()),
    }
}

/// The seven mandatory classification pairs
/// (`standards/workspace/rule-resolution.md` §2-§3), in their canonically
/// declared order. Every legal [`WorkItemKind`] value appears exactly once:
/// unlike the Node reference (and the prior Rust port), this list does not
/// need its own "is this one of the seven" runtime check — [`WorkItemKind`]
/// is already closed to exactly these combinations, so an eighth,
/// structurally-invalid pair cannot exist to be checked against it.
pub const REQUIRED_KINDS: [WorkItemKind; 7] = [
    WorkItemKind::Assessment,
    WorkItemKind::Operation,
    WorkItemKind::Initiative,
    WorkItemKind::Change(ChangeClass::Bugfix),
    WorkItemKind::Change(ChangeClass::Feature),
    WorkItemKind::Change(ChangeClass::BehaviorChange),
    WorkItemKind::Change(ChangeClass::Refactor),
];

/// Why a [`TaskPatternCatalog::resolve`] lookup did not return exactly one
/// entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternRefError {
    /// No entry carries this id.
    Unknown,
    /// More than one entry carries this id — carries how many.
    Ambiguous(usize),
}

/// The built-in task-pattern catalogue: a typed, navigable set of
/// [`TaskPatternEntry`] values. Built only through [`TaskPatternCatalog::build`],
/// which also returns the catalogue-level diagnostics (duplicate ids,
/// pair-coverage, the REFACTOR/BUGFIX wiring rules) that need more than one
/// entry to state — a malformed entry never reaches this type at all
/// ([`TaskPatternEntry::try_new`] is its caller's own gate), so a catalogue
/// this type holds is never "partially built" in the sense of carrying an
/// internally inconsistent entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPatternCatalog {
    entries: Vec<TaskPatternEntry>,
}

impl TaskPatternCatalog {
    /// Builds a catalogue from already-individually-valid entries and
    /// reports every catalogue-level defect it finds (duplicate ids,
    /// missing/over-declared classification pairs, the REFACTOR/BUGFIX-
    /// specific wiring rules). **Fail-closed**
    /// (`rust-architecture-conformance-3` corrective round item 1): this,
    /// the only public constructor, returns `Some` catalogue if and only if
    /// `problems` is empty. A caller can never receive, and so can never
    /// resolve a `task_pattern` reference against, a catalogue this
    /// function itself already knows to be defective — the seven-pair
    /// coverage, uniqueness and wiring rules above are exactly the
    /// invariants a resolved reference implicitly trusts.
    pub fn build(entries: Vec<TaskPatternEntry>) -> (Option<Self>, Vec<Diagnostic>) {
        let mut problems = Vec::new();

        let mut seen_ids: HashSet<&str> = HashSet::new();
        for e in &entries {
            if !seen_ids.insert(e.id().as_str()) {
                problems.push(fail(format!(
                    "pattern id \"{}\" is declared more than once",
                    e.id()
                )));
            }
        }

        // `pair_order` preserves first-declaration order so that, when
        // several pairs are simultaneously over-declared, the diagnostics
        // below name them in the same order the document actually declares
        // them in, not an incidental hash order.
        let mut pair_count: HashMap<WorkItemKind, usize> = HashMap::new();
        let mut pair_order: Vec<WorkItemKind> = Vec::new();
        for e in &entries {
            let kind = e.kind();
            if !pair_count.contains_key(&kind) {
                pair_order.push(kind);
            }
            *pair_count.entry(kind).or_insert(0) += 1;
        }
        for kind in &pair_order {
            let n = pair_count[kind];
            if n > 1 {
                problems.push(fail(format!(
                    "classification pair {} is declared {n} times; each active pair appears exactly once",
                    pair_label(*kind)
                )));
            }
        }
        for kind in REQUIRED_KINDS {
            if !pair_count.contains_key(&kind) {
                problems.push(fail(format!(
                    "no pattern for classification pair {}; all seven are mandatory",
                    pair_label(kind)
                )));
            }
        }

        let find = |kind: WorkItemKind| entries.iter().find(|e| e.kind() == kind);

        if let Some(refactor) = find(WorkItemKind::Change(ChangeClass::Refactor)) {
            let protocols = TaskPatternEntry::present_paths(refactor.protocols());
            let evidence = TaskPatternEntry::present_paths(refactor.evidence_contracts());
            if !protocols.iter().any(|p| p.as_str() == REFACTOR_PROTOCOL) {
                problems.push(fail(format!(
                    "the REFACTOR pattern does not route to the functional-parity protocol \"{REFACTOR_PROTOCOL}\""
                )));
            }
            if !evidence.iter().any(|p| p.as_str() == REFACTOR_EVIDENCE) {
                problems.push(fail(format!(
                    "the REFACTOR pattern does not reference the functional-parity evidence contract \"{REFACTOR_EVIDENCE}\""
                )));
            }
        }

        if let Some(bugfix) = find(WorkItemKind::Change(ChangeClass::Bugfix)) {
            let bugfix_skills: HashSet<&str> = TaskPatternEntry::present_paths(bugfix.skills())
                .into_iter()
                .map(WorkspaceRelativePath::as_str)
                .collect();
            if !bugfix_skills.contains(BUGFIX_SKILL) {
                problems.push(fail(format!(
                    "the BUGFIX pattern does not reference the bugfix way of carrying work out \"{BUGFIX_SKILL}\" in applicable_skills"
                )));
            }
            for (label, other_kind) in [
                ("FEATURE", WorkItemKind::Change(ChangeClass::Feature)),
                (
                    "BEHAVIOR_CHANGE",
                    WorkItemKind::Change(ChangeClass::BehaviorChange),
                ),
            ] {
                let Some(other) = find(other_kind) else {
                    continue;
                };
                let shared = TaskPatternEntry::present_paths(other.skills())
                    .into_iter()
                    .chain(TaskPatternEntry::present_paths(other.protocols()))
                    .find(|p| bugfix_skills.contains(p.as_str()));
                if let Some(shared) = shared {
                    problems.push(fail(format!(
                        "the BUGFIX pattern shares \"{shared}\" with the {label} pattern; BUGFIX must not be conflated with FEATURE or BEHAVIOR_CHANGE"
                    )));
                }
            }
        }

        if problems.is_empty() {
            (Some(Self { entries }), problems)
        } else {
            (None, problems)
        }
    }

    pub fn entries(&self) -> &[TaskPatternEntry] {
        &self.entries
    }

    /// Test-only escape hatch around the fail-closed [`Self::build`] gate:
    /// assembles a catalogue directly from `entries` with NO catalogue-level
    /// checks, for tests elsewhere in this crate (`task_contracts::specification`)
    /// that need a minimal catalogue to exercise `task_pattern` RESOLUTION
    /// logic without also having to assemble all seven mandatory patterns.
    /// `#[cfg(test)]`-gated, so it never compiles into production code and
    /// is never a second production constructor; `pub(crate)` because it is
    /// only meaningful within this crate — a caller across a crate boundary
    /// (`meridian-app`'s own tests) has no such shortcut and must build a
    /// genuinely complete, defect-free catalogue through [`Self::build`].
    #[cfg(test)]
    pub(crate) fn test_only_unchecked(entries: Vec<TaskPatternEntry>) -> Self {
        Self { entries }
    }

    /// Resolves a `task_pattern.id` reference against this catalogue:
    /// `Unknown` when no entry carries it, `Ambiguous` when more than one
    /// does, `Ok` with the single matching entry otherwise.
    pub fn resolve(&self, id: &str) -> Result<&TaskPatternEntry, PatternRefError> {
        let mut matches = self.entries.iter().filter(|e| e.id().as_str() == id);
        let first = matches.next().ok_or(PatternRefError::Unknown)?;
        let rest = matches.count();
        if rest > 0 {
            Err(PatternRefError::Ambiguous(rest + 1))
        } else {
            Ok(first)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn present(id: &str, path: &str) -> CanonicalLink {
        CanonicalLink::Present {
            id: SemanticId::new(id).unwrap(),
            path: WorkspaceRelativePath::new(path).unwrap(),
        }
    }

    fn absent(reason: &str) -> CanonicalLink {
        CanonicalLink::Absent {
            reason: NonEmptyString::new(reason).unwrap(),
        }
    }

    fn entry(id: &str, kind: WorkItemKind) -> TaskPatternEntry {
        TaskPatternEntry::try_new(
            SemanticId::new(id).unwrap(),
            kind,
            vec![present(
                "standard-task-lifecycle",
                "workflows/task-lifecycle.md",
            )],
            vec![absent("no vendored way of carrying this out")],
            vec![absent("no canonical evidence contract yet")],
        )
        .unwrap()
    }

    fn seven_valid_entries() -> Vec<TaskPatternEntry> {
        vec![
            entry("assess-existing-state", WorkItemKind::Assessment),
            entry("operate-environment-action", WorkItemKind::Operation),
            entry("decompose-initiative", WorkItemKind::Initiative),
            TaskPatternEntry::try_new(
                SemanticId::new("fix-defect").unwrap(),
                WorkItemKind::Change(ChangeClass::Bugfix),
                vec![absent("no separate protocol document")],
                vec![present("bugfix-protocol", BUGFIX_SKILL)],
                vec![present(
                    "regression-evidence-record",
                    "verification/regression-testing/README.md",
                )],
            )
            .unwrap(),
            entry("add-capability", WorkItemKind::Change(ChangeClass::Feature)),
            entry(
                "change-behavior",
                WorkItemKind::Change(ChangeClass::BehaviorChange),
            ),
            TaskPatternEntry::try_new(
                SemanticId::new("refactor-preserving-behavior").unwrap(),
                WorkItemKind::Change(ChangeClass::Refactor),
                vec![present("refactor-execution-protocol", REFACTOR_PROTOCOL)],
                vec![absent("no separate vendored way of carrying this out")],
                vec![present(
                    "functional-parity-evidence-contract",
                    REFACTOR_EVIDENCE,
                )],
            )
            .unwrap(),
        ]
    }

    #[test]
    fn try_new_rejects_a_skill_package_placed_in_applicable_protocols() {
        let err = TaskPatternEntry::try_new(
            SemanticId::new("example").unwrap(),
            WorkItemKind::Assessment,
            vec![present(
                "bugfix-protocol",
                "skills/bugfix-protocol/SKILL.md",
            )],
            vec![absent("x")],
            vec![absent("x")],
        )
        .unwrap_err();
        assert!(
            err[0].message().contains("is not a skill package")
                || err[0].message().contains("points at a skill package")
        );
    }

    #[test]
    fn try_new_rejects_a_protocol_placed_in_applicable_skills() {
        let err = TaskPatternEntry::try_new(
            SemanticId::new("example").unwrap(),
            WorkItemKind::Assessment,
            vec![absent("x")],
            vec![present(
                "standard-task-lifecycle",
                "workflows/task-lifecycle.md",
            )],
            vec![absent("x")],
        )
        .unwrap_err();
        assert!(err[0].message().contains("is not a skill package"));
    }

    #[test]
    fn build_accepts_the_seven_canonical_patterns_with_no_diagnostics() {
        let (catalog, problems) = TaskPatternCatalog::build(seven_valid_entries());
        assert!(problems.is_empty(), "{problems:?}");
        let catalog = catalog.expect("no catalogue-level defects");
        assert_eq!(catalog.entries().len(), 7);
    }

    /// Fail-closed (`rust-architecture-conformance-3` corrective round item
    /// 1): a duplicate pattern id is a catalogue-level defect, so `build`
    /// must never hand the caller a `Some` catalogue for it — there is no
    /// partially-accepted catalogue to resolve a reference against.
    #[test]
    fn build_flags_a_duplicate_pattern_id_and_returns_no_catalog() {
        let mut entries = seven_valid_entries();
        let dup = entries[0].clone();
        entries.push(dup);
        let (catalog, problems) = TaskPatternCatalog::build(entries);
        assert!(catalog.is_none(), "a duplicate id must not build a catalog");
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("declared more than once")),
            "{problems:?}"
        );
    }

    #[test]
    fn build_flags_a_missing_required_pair_and_returns_no_catalog() {
        let mut entries = seven_valid_entries();
        entries.pop();
        let (catalog, problems) = TaskPatternCatalog::build(entries);
        assert!(
            catalog.is_none(),
            "a catalogue missing a mandatory pair must not build"
        );
        assert!(
            problems.iter().any(|p| p.message().contains("REFACTOR")),
            "{problems:?}"
        );
    }

    #[test]
    fn build_flags_bugfix_missing_its_skill_and_returns_no_catalog() {
        let mut entries = seven_valid_entries();
        let idx = entries
            .iter()
            .position(|e| e.kind() == WorkItemKind::Change(ChangeClass::Bugfix))
            .unwrap();
        entries[idx] = TaskPatternEntry::try_new(
            SemanticId::new("fix-defect").unwrap(),
            WorkItemKind::Change(ChangeClass::Bugfix),
            vec![absent("x")],
            vec![absent("no vendored way of carrying this out")],
            vec![absent("x")],
        )
        .unwrap();
        let (catalog, problems) = TaskPatternCatalog::build(entries);
        assert!(
            catalog.is_none(),
            "a BUGFIX pattern missing its wiring must not build a catalog"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("does not reference the bugfix")),
            "{problems:?}"
        );
    }

    #[test]
    fn resolve_finds_a_unique_pattern_by_id() {
        let (catalog, problems) = TaskPatternCatalog::build(seven_valid_entries());
        assert!(problems.is_empty(), "{problems:?}");
        let catalog = catalog.unwrap();
        let found = catalog.resolve("fix-defect").unwrap();
        assert_eq!(found.kind(), WorkItemKind::Change(ChangeClass::Bugfix));
    }

    #[test]
    fn resolve_reports_unknown_for_a_missing_id() {
        let (catalog, problems) = TaskPatternCatalog::build(seven_valid_entries());
        assert!(problems.is_empty(), "{problems:?}");
        let catalog = catalog.unwrap();
        assert_eq!(
            catalog.resolve("no-such-pattern"),
            Err(PatternRefError::Unknown)
        );
    }

    /// `build` can never itself produce a catalogue carrying a duplicate id
    /// (the test above proves it returns `None` for one instead) — the
    /// `Ambiguous` arm of [`TaskPatternCatalog::resolve`] is still exercised
    /// here, directly, as defense in depth for `resolve`'s own pure lookup
    /// logic. Constructing `TaskPatternCatalog { entries }` by hand (a
    /// private field, visible only within this module and its `tests`
    /// submodule) is a test-only shortcut around the fail-closed
    /// constructor, never a second production entry point.
    #[test]
    fn resolve_reports_ambiguous_for_a_duplicated_id() {
        let mut entries = seven_valid_entries();
        let dup = entries[0].clone();
        entries.push(dup);
        let catalog = TaskPatternCatalog { entries };
        assert_eq!(
            catalog.resolve("assess-existing-state"),
            Err(PatternRefError::Ambiguous(2))
        );
    }

    /// A malformed entry (bad link classification) never reaches
    /// `TaskPatternCatalog` at all — its caller (the orchestration layer)
    /// must drop it rather than force it through. This test exercises
    /// `resolve`'s own `Unknown` lookup behaviour directly against a
    /// hand-assembled, intentionally incomplete catalogue (the same
    /// test-only private-field shortcut as above) rather than through
    /// `build`, which would itself refuse to construct a catalogue missing
    /// six of the seven mandatory pairs.
    #[test]
    fn a_malformed_entry_that_never_entered_the_catalog_resolves_as_unknown() {
        let entries = vec![entry("assess-existing-state", WorkItemKind::Assessment)];
        let catalog = TaskPatternCatalog { entries };
        assert_eq!(catalog.resolve("fix-defect"), Err(PatternRefError::Unknown));
    }
}
