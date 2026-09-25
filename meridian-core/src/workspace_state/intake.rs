//! M-12/M-15 — instruction intake of a workspace.
//!
//! M-12: every intake record belongs to a repository the workspace inventory
//! names (the record's own `repository-scope`); an unknown repository is an
//! unresolvable reference, not an unreachable one.
//!
//! Topic existence (decision D3): a named topic is one the Kernel's agreed
//! topic pool holds; `unclassified` is a declared state, not a topic.
//!
//! M-15: a repository's intake is complete when every norm its tree tracks
//! carries a record — a whole file for an ordinary norm, each declared
//! region for a container (`AGENTS.md`, `CLAUDE.md`). Untracked or ignored
//! norms, text outside every region and a container that declares no
//! regions are named, never counted as confirmed; an unreachable repository,
//! and a tree fact whose port failed ([`TreeFact::Unavailable`]), is
//! `UNVERIFIED` and never counted as confirmed.

use std::collections::{BTreeMap, BTreeSet};

use crate::mechanical_integrity::instruction_topics::{TopicId, TopicPool};
use crate::types::{Diagnostic, DiagnosticLevel, NonEmptyString, RepositoryId, SemanticId};

use super::diagnostic;
use super::parentage::ParentReference;

/// Paths an intake register must cover: agent rules, skills and
/// instruction containers.
pub fn is_intake_candidate(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.ends_with(".mdc") || matches!(name, "SKILL.md" | "AGENTS.md" | "CLAUDE.md")
}

/// Candidates whose unit of intake is a declared region, not the file.
pub fn is_container(path: &str) -> bool {
    matches!(path.rsplit('/').next(), Some("AGENTS.md" | "CLAUDE.md"))
}

/// How the tool loads a norm (`delivery`): the closed vocabulary of
/// `agent-instruction-identity.md` §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Delivery {
    KernelDoc,
    SkillPackage,
    CursorRule,
    AgentsMdSection,
}

impl Delivery {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "kernel-doc" => Some(Self::KernelDoc),
            "skill-package" => Some(Self::SkillPackage),
            "cursor-rule" => Some(Self::CursorRule),
            "agents-md-section" => Some(Self::AgentsMdSection),
            _ => None,
        }
    }
}

/// The topic a record assigns: a name the topic pool must hold, or the
/// declared `unclassified` state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntakeTopic {
    Unclassified,
    Named(TopicId),
}

impl IntakeTopic {
    pub fn new(value: &str) -> Result<Self, String> {
        if value == "unclassified" {
            return Ok(Self::Unclassified);
        }
        TopicId::new(value)
            .map(Self::Named)
            .map_err(|e| e.to_string())
    }
}

/// The decision a record takes. An edition carries its parent, so an
/// `adopt-edition` without a qualified parent reference is unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntakeVerdict {
    AdoptCore,
    AdoptEdition(ParentReference),
    KeepLocal,
    MergeInto,
    Rename,
    Retire,
    Deferred,
}

impl IntakeVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AdoptCore => "adopt-core",
            Self::AdoptEdition(_) => "adopt-edition",
            Self::KeepLocal => "keep-local",
            Self::MergeInto => "merge-into",
            Self::Rename => "rename",
            Self::Retire => "retire",
            Self::Deferred => "deferred",
        }
    }
}

/// One intake record, as far as these checks read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeRecord {
    id: SemanticId,
    repository: RepositoryId,
    artifact: NonEmptyString,
    region: Option<NonEmptyString>,
    topic: IntakeTopic,
    delivery: Delivery,
    verdict: IntakeVerdict,
    recorded_at: NonEmptyString,
}

/// The fields of one [`IntakeRecord`], named.
pub struct IntakeFields {
    pub id: SemanticId,
    pub repository: RepositoryId,
    pub artifact: NonEmptyString,
    pub region: Option<NonEmptyString>,
    pub topic: IntakeTopic,
    pub delivery: Delivery,
    pub verdict: IntakeVerdict,
    pub recorded_at: NonEmptyString,
}

impl IntakeRecord {
    pub fn new(fields: IntakeFields) -> Self {
        let IntakeFields {
            id,
            repository,
            artifact,
            region,
            topic,
            delivery,
            verdict,
            recorded_at,
        } = fields;
        Self {
            id,
            repository,
            artifact,
            region,
            topic,
            delivery,
            verdict,
            recorded_at,
        }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }

    pub fn repository(&self) -> &RepositoryId {
        &self.repository
    }

    pub fn artifact(&self) -> &str {
        self.artifact.as_str()
    }

    pub fn topic(&self) -> &IntakeTopic {
        &self.topic
    }

    pub fn delivery(&self) -> Delivery {
        self.delivery
    }

    pub fn verdict(&self) -> &IntakeVerdict {
        &self.verdict
    }

    /// The parent an edition narrows, when the record is one.
    pub fn parent(&self) -> Option<&ParentReference> {
        match &self.verdict {
            IntakeVerdict::AdoptEdition(parent) => Some(parent),
            _ => None,
        }
    }
}

/// M-12: one failure per repository that intake records name but the
/// inventory does not, in repository order.
pub fn check_references(records: &[IntakeRecord], inventory: &BTreeSet<String>) -> Vec<Diagnostic> {
    let mut unknown: BTreeMap<&str, usize> = BTreeMap::new();
    for record in records {
        if !inventory.contains(record.repository.as_str()) {
            *unknown.entry(record.repository.as_str()).or_default() += 1;
        }
    }
    unknown
        .into_iter()
        .map(|(repository, count)| {
            diagnostic(
                DiagnosticLevel::Fail,
                format!(
                    "instruction-intake: {count} record(s) of repository \"{repository}\" name a repository the workspace inventory does not name; this is an unresolvable reference, not an unreachable repository"
                ),
            )
        })
        .collect()
}

/// Topic existence (not topic correctness, which no gate can see): one
/// failure per repository whose records name topics the Kernel's agreed
/// topic pool does not hold, the strays named in their sorted order.
pub fn check_topics(
    repository: &str,
    records: &[&IntakeRecord],
    pool: &TopicPool,
) -> Option<Diagnostic> {
    let strays: BTreeSet<&str> = records
        .iter()
        .filter_map(|r| match &r.topic {
            IntakeTopic::Named(topic) if !pool.contains(topic) => Some(topic.as_str()),
            _ => None,
        })
        .collect();
    if strays.is_empty() {
        return None;
    }
    let named: Vec<String> = strays.iter().map(|t| t.to_string()).collect();
    Some(diagnostic(
        DiagnosticLevel::Fail,
        format!(
            "instruction-intake: the register of repository \"{repository}\" names {} topic(s) outside the pool ({}); a new topic is a change to the registry, not to one record",
            named.len(),
            first_three(&named)
        ),
    ))
}

/// One declared region of a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredRegion {
    pub id: String,
    pub owner: String,
    pub generated: bool,
}

/// A container's parsed region structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerRegions {
    pub errors: Vec<String>,
    pub regions: Vec<DeclaredRegion>,
    pub uncovered_lines: usize,
}

/// One fact about a reachable repository's tree: observed, or not
/// observable because the port that answers it failed. An unavailable fact
/// is never read as an empty one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeFact<T> {
    Observed(T),
    Unavailable(String),
}

impl<T: Default> Default for TreeFact<T> {
    fn default() -> Self {
        Self::Observed(T::default())
    }
}

/// What the repository's tree says, as observed through the ports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepositoryTree {
    /// Tracked paths, in Git's order.
    pub tracked: Vec<String>,
    /// Untracked, non-ignored paths that match the intake masks.
    pub untracked_norms: TreeFact<Vec<String>>,
    /// Recorded artifacts that exist but Git ignores.
    pub ignored_norms: TreeFact<Vec<String>>,
    /// Each tracked container's regions; `None` when it could not be read.
    pub containers: BTreeMap<String, Option<ContainerRegions>>,
}

/// One repository's completeness verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletenessCheck {
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
}

fn first_three(items: &[String]) -> String {
    items.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
}

/// M-15 for one repository. `tree` is `None` when the repository is not
/// reachable from here.
pub fn check_completeness(
    repository: &str,
    records: &[&IntakeRecord],
    tree: Option<&RepositoryTree>,
) -> CompletenessCheck {
    let mut out = Vec::new();
    let Some(tree) = tree else {
        return CompletenessCheck {
            diagnostics: vec![diagnostic(
                DiagnosticLevel::Info,
                format!("instruction-intake: {repository} — repository not reachable from here; completeness is UNVERIFIED, not confirmed"),
            )],
            complete: false,
        };
    };
    let covered: BTreeSet<&str> = records.iter().map(|r| r.artifact()).collect();
    let covered_regions: BTreeSet<String> = records
        .iter()
        .filter_map(|r| {
            r.region
                .as_ref()
                .map(|g| format!("{}#{}", r.artifact(), g.as_str()))
        })
        .collect();
    let mut missing: Vec<String> = Vec::new();
    let mut regions_covered = 0usize;
    let mut uncovered_containers = 0usize;
    let mut unverified = false;
    for file in tree.tracked.iter().filter(|f| is_intake_candidate(f)) {
        if !is_container(file) {
            if !covered.contains(file.as_str()) {
                missing.push(file.clone());
            }
            continue;
        }
        let Some(Some(parsed)) = tree.containers.get(file) else {
            unverified = true;
            out.push(diagnostic(
                DiagnosticLevel::Info,
                format!("instruction-intake: {repository}/{file} could not be read; its regions are UNVERIFIED, not confirmed"),
            ));
            continue;
        };
        for error in &parsed.errors {
            out.push(diagnostic(
                DiagnosticLevel::Fail,
                format!("instruction-intake: {repository}/{file} — {error}"),
            ));
        }
        if !parsed.errors.is_empty() {
            continue;
        }
        let declared: BTreeSet<&str> = parsed.regions.iter().map(|r| r.id.as_str()).collect();
        for record in records.iter().filter(|r| r.artifact() == file) {
            if let Some(region) = &record.region {
                if !declared.contains(region.as_str()) {
                    out.push(diagnostic(
                        DiagnosticLevel::Fail,
                        format!(
                            "instruction-intake: {repository}/{file} — a record names region \"{}\", which the file does not declare",
                            region.as_str()
                        ),
                    ));
                }
            }
        }
        if parsed.regions.is_empty() {
            let mut whole: Vec<(usize, &&IntakeRecord)> = records
                .iter()
                .filter(|r| r.artifact() == file && r.region.is_none())
                .enumerate()
                .collect();
            whole.sort_by(|a, b| {
                a.1.recorded_at
                    .as_str()
                    .cmp(b.1.recorded_at.as_str())
                    .then(a.0.cmp(&b.0))
            });
            match whole.last() {
                None => missing.push(file.clone()),
                Some((_, current)) if current.verdict != IntakeVerdict::Deferred => out.push(diagnostic(
                    DiagnosticLevel::Fail,
                    format!(
                        "instruction-intake: {repository}/{file} declares no regions and its current record is \"{}\"; a container whose boundaries are not declared is itself the finding and is recorded as deferred with that reason (protocol §3.1)",
                        current.verdict.as_str()
                    ),
                )),
                Some(_) => out.push(diagnostic(
                    DiagnosticLevel::Info,
                    format!("instruction-intake: {repository}/{file} declares no regions and is recorded as deferred; the unit of intake for it is still undeclared"),
                )),
            }
            continue;
        }
        for region in &parsed.regions {
            let key = format!("{file}#{}", region.id);
            if region.owner.is_empty() {
                out.push(diagnostic(
                    DiagnosticLevel::Fail,
                    format!(
                        "instruction-intake: {repository}/{file} — region \"{}\" declares no owner; without one there is no answer to whether the next generation may overwrite it",
                        region.id
                    ),
                ));
            }
            if region.generated {
                if covered_regions.contains(&key) {
                    out.push(diagnostic(
                        DiagnosticLevel::Fail,
                        format!(
                            "instruction-intake: {repository}/{file} — region \"{}\" is declared generated and yet carries an intake record; a generated region is not the owner's norm and is not taken in",
                            region.id
                        ),
                    ));
                }
                continue;
            }
            if covered_regions.contains(&key) {
                regions_covered += 1;
            } else {
                missing.push(key);
            }
        }
        if parsed.uncovered_lines > 0 {
            uncovered_containers += 1;
            out.push(diagnostic(
                DiagnosticLevel::Warn,
                format!(
                    "instruction-intake: {repository}/{file} — {} line(s) lie outside every declared region and belong to no unit of intake; completeness below is claimed over the regions, not over the file",
                    parsed.uncovered_lines
                ),
            ));
        }
    }
    match &tree.ignored_norms {
        TreeFact::Observed(ignored) if !ignored.is_empty() => out.push(diagnostic(
            DiagnosticLevel::Warn,
            format!(
                "instruction-intake: {repository} — {} recorded norm(s) are excluded from version control by .gitignore ({}); a norm the repository deliberately keeps out of its history has no revision to cite, and this tree is NOT counted as confirmed complete",
                ignored.len(),
                first_three(ignored)
            ),
        )),
        TreeFact::Observed(_) => {}
        TreeFact::Unavailable(reason) => {
            unverified = true;
            out.push(diagnostic(
                DiagnosticLevel::Warn,
                format!(
                    "instruction-intake: {repository} — whether the recorded norms outside version control are ignored cannot be established ({reason}); such a norm is UNVERIFIED, and this tree is NOT counted as confirmed complete"
                ),
            ));
        }
    }
    match &tree.untracked_norms {
        TreeFact::Observed(untracked) if !untracked.is_empty() => out.push(diagnostic(
            DiagnosticLevel::Warn,
            format!(
                "instruction-intake: {repository} — {} norm(s) match the intake masks but are not under version control ({}); completeness is measured over tracked files, so this tree is NOT counted as confirmed complete",
                untracked.len(),
                first_three(untracked)
            ),
        )),
        TreeFact::Observed(_) => {}
        TreeFact::Unavailable(reason) => {
            unverified = true;
            out.push(diagnostic(
                DiagnosticLevel::Warn,
                format!(
                    "instruction-intake: {repository} — the untracked files of the tree cannot be listed ({reason}); norms outside version control are UNVERIFIED, and this tree is NOT counted as confirmed complete"
                ),
            ));
        }
    }
    let mut complete = false;
    if !missing.is_empty() {
        out.push(diagnostic(
            DiagnosticLevel::Fail,
            format!(
                "instruction-intake: {repository} — {} norm(s) in the tree have no record ({}); the register is complete or it proves nothing",
                missing.len(),
                first_three(&missing)
            ),
        ));
    } else if uncovered_containers > 0 {
        out.push(diagnostic(
            DiagnosticLevel::Info,
            format!("instruction-intake: {repository} — {regions_covered} declared region(s) carry a record, but {uncovered_containers} container(s) hold text outside every region; this tree is NOT counted as confirmed complete"),
        ));
    } else if !unverified
        && tree.untracked_norms == TreeFact::Observed(Vec::new())
        && tree.ignored_norms == TreeFact::Observed(Vec::new())
    {
        complete = true;
        if regions_covered > 0 {
            out.push(diagnostic(
                DiagnosticLevel::Info,
                format!("instruction-intake: {repository} — {regions_covered} declared region(s) of container files carry a record"),
            ));
        }
    }
    CompletenessCheck {
        diagnostics: out,
        complete,
    }
}

/// The run-level line.
pub fn summarize(records: usize, repositories: usize, complete: usize) -> Diagnostic {
    if records == 0 {
        return diagnostic(
            DiagnosticLevel::Info,
            "instruction-intake: no intake record in the workspace; intake has not started"
                .to_string(),
        );
    }
    diagnostic(
        DiagnosticLevel::Info,
        format!("instruction-intake: {records} record(s) across {repositories} repository register(s); {complete} repository tree(s) confirmed complete"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        repository: &str,
        artifact: &str,
        region: Option<&str>,
        verdict: &str,
        at: &str,
    ) -> IntakeRecord {
        let verdict = match verdict {
            "deferred" => IntakeVerdict::Deferred,
            "adopt-core" => IntakeVerdict::AdoptCore,
            other => panic!("no test verdict {other}"),
        };
        IntakeRecord::new(IntakeFields {
            id: SemanticId::new(format!("r-{}", artifact.len())).unwrap(),
            repository: RepositoryId::new(repository).unwrap(),
            artifact: NonEmptyString::new(artifact).unwrap(),
            region: region.map(|r| NonEmptyString::new(r).unwrap()),
            topic: IntakeTopic::Unclassified,
            delivery: Delivery::CursorRule,
            verdict,
            recorded_at: NonEmptyString::new(at).unwrap(),
        })
    }

    fn region(id: &str, owner: &str, generated: bool) -> DeclaredRegion {
        DeclaredRegion {
            id: id.to_string(),
            owner: owner.to_string(),
            generated,
        }
    }

    fn messages(check: &CompletenessCheck) -> Vec<(DiagnosticLevel, String)> {
        check
            .diagnostics
            .iter()
            .map(|d| (d.level(), d.message().to_string()))
            .collect()
    }

    #[test]
    fn workspace_state_intake_masks_match_the_reference() {
        for path in [
            ".cursor/rules/a.mdc",
            "skills/x/SKILL.md",
            "AGENTS.md",
            "docs/CLAUDE.md",
        ] {
            assert!(is_intake_candidate(path), "{path}");
        }
        for path in ["README.md", "AGENTS.md.bak", "skill.md"] {
            assert!(!is_intake_candidate(path), "{path}");
        }
        assert!(is_container("sub/AGENTS.md"));
        assert!(!is_container("skills/x/SKILL.md"));
    }

    #[test]
    fn workspace_state_intake_reference_to_an_unknown_repository_fails() {
        let records = [
            record("web", "AGENTS.md", None, "deferred", "2026-01-01"),
            record("ghost", "a.mdc", None, "adopt-core", "2026-01-01"),
            record("ghost", "b.mdc", None, "adopt-core", "2026-01-01"),
        ];
        let inventory: BTreeSet<String> = ["web".to_string()].into();
        let out = check_references(&records, &inventory);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].message(),
            "instruction-intake: 2 record(s) of repository \"ghost\" name a repository the workspace inventory does not name; this is an unresolvable reference, not an unreachable repository"
        );
        assert!(check_references(&records[..1], &inventory).is_empty());
    }

    #[test]
    fn workspace_state_intake_complete_tree_is_confirmed() {
        let records = [
            record(
                "web",
                ".cursor/rules/a.mdc",
                None,
                "adopt-core",
                "2026-01-01",
            ),
            record(
                "web",
                "AGENTS.md",
                Some("style"),
                "adopt-core",
                "2026-01-01",
            ),
        ];
        let refs: Vec<&IntakeRecord> = records.iter().collect();
        let tree = RepositoryTree {
            tracked: vec![
                ".cursor/rules/a.mdc".into(),
                "AGENTS.md".into(),
                "src/x.ts".into(),
            ],
            containers: [(
                "AGENTS.md".to_string(),
                Some(ContainerRegions {
                    errors: vec![],
                    regions: vec![region("style", "team", false), region("gen", "tool", true)],
                    uncovered_lines: 0,
                }),
            )]
            .into(),
            ..RepositoryTree::default()
        };
        let check = check_completeness("web", &refs, Some(&tree));
        assert!(check.complete, "{:?}", messages(&check));
        assert_eq!(
            messages(&check),
            [(
                DiagnosticLevel::Info,
                "instruction-intake: web — 1 declared region(s) of container files carry a record"
                    .to_string()
            )]
        );
    }

    #[test]
    fn workspace_state_intake_names_every_gap_of_an_incomplete_tree() {
        let records = [
            record("web", "AGENTS.md", Some("typo"), "adopt-core", "2026-01-01"),
            record("web", "AGENTS.md", Some("gen"), "adopt-core", "2026-01-01"),
            record("web", "docs/CLAUDE.md", None, "adopt-core", "2026-02-01"),
            record("web", "docs/CLAUDE.md", None, "deferred", "2026-01-01"),
        ];
        let refs: Vec<&IntakeRecord> = records.iter().collect();
        let tree = RepositoryTree {
            tracked: vec![
                "AGENTS.md".into(),
                "docs/CLAUDE.md".into(),
                "b.mdc".into(),
                "skills/s/SKILL.md".into(),
            ],
            untracked_norms: TreeFact::Observed(vec!["new.mdc".into()]),
            ignored_norms: TreeFact::Observed(vec!["local.mdc".into()]),
            containers: [
                (
                    "AGENTS.md".to_string(),
                    Some(ContainerRegions {
                        errors: vec![],
                        regions: vec![region("style", "", false), region("gen", "tool", true)],
                        uncovered_lines: 4,
                    }),
                ),
                (
                    "docs/CLAUDE.md".to_string(),
                    Some(ContainerRegions {
                        errors: vec![],
                        regions: vec![],
                        uncovered_lines: 0,
                    }),
                ),
            ]
            .into(),
        };
        let check = check_completeness("web", &refs, Some(&tree));
        assert!(!check.complete);
        let fails: Vec<String> = messages(&check)
            .into_iter()
            .filter(|(level, _)| *level == DiagnosticLevel::Fail)
            .map(|(_, m)| m)
            .collect();
        assert_eq!(
            fails,
            [
                "instruction-intake: web/AGENTS.md — a record names region \"typo\", which the file does not declare",
                "instruction-intake: web/AGENTS.md — region \"style\" declares no owner; without one there is no answer to whether the next generation may overwrite it",
                "instruction-intake: web/AGENTS.md — region \"gen\" is declared generated and yet carries an intake record; a generated region is not the owner's norm and is not taken in",
                "instruction-intake: web/docs/CLAUDE.md declares no regions and its current record is \"adopt-core\"; a container whose boundaries are not declared is itself the finding and is recorded as deferred with that reason (protocol §3.1)",
                "instruction-intake: web — 3 norm(s) in the tree have no record (AGENTS.md#style, b.mdc, skills/s/SKILL.md); the register is complete or it proves nothing",
            ]
        );
        let warns = messages(&check)
            .into_iter()
            .filter(|(level, _)| *level == DiagnosticLevel::Warn)
            .count();
        assert_eq!(warns, 3);
    }

    #[test]
    fn workspace_state_intake_unreachable_or_unreadable_is_unverified() {
        let check = check_completeness("web", &[], None);
        assert!(!check.complete);
        assert_eq!(
            check.diagnostics[0].message(),
            "instruction-intake: web — repository not reachable from here; completeness is UNVERIFIED, not confirmed"
        );
        let tree = RepositoryTree {
            tracked: vec!["AGENTS.md".into()],
            containers: [("AGENTS.md".to_string(), None)].into(),
            ..RepositoryTree::default()
        };
        let check = check_completeness("web", &[], Some(&tree));
        assert!(check.diagnostics[0]
            .message()
            .contains("could not be read; its regions are UNVERIFIED"));
        let parse_error = RepositoryTree {
            tracked: vec!["AGENTS.md".into()],
            containers: [(
                "AGENTS.md".to_string(),
                Some(ContainerRegions {
                    errors: vec!["a region ends where none is open".into()],
                    regions: vec![],
                    uncovered_lines: 0,
                }),
            )]
            .into(),
            ..RepositoryTree::default()
        };
        let check = check_completeness("web", &[], Some(&parse_error));
        assert_eq!(
            check.diagnostics[0].message(),
            "instruction-intake: web/AGENTS.md — a region ends where none is open"
        );
    }

    fn with_topic(repository: &str, topic: &str) -> IntakeRecord {
        let mut r = record(repository, "a.mdc", None, "deferred", "2026-01-01");
        r.topic = IntakeTopic::new(topic).unwrap();
        r
    }

    #[test]
    fn workspace_state_intake_topics_outside_the_pool_fail_per_repository() {
        let pool: TopicPool = ["naming", "error-handling"]
            .into_iter()
            .map(|t| TopicId::new(t).unwrap())
            .collect();
        let inside = [
            with_topic("web", "naming"),
            with_topic("web", "unclassified"),
        ];
        let refs: Vec<&IntakeRecord> = inside.iter().collect();
        assert_eq!(check_topics("web", &refs, &pool), None);

        let strays = [
            with_topic("web", "naming"),
            with_topic("web", "zeta"),
            with_topic("web", "alpha"),
            with_topic("web", "zeta"),
        ];
        let refs: Vec<&IntakeRecord> = strays.iter().collect();
        let out = check_topics("web", &refs, &pool).unwrap();
        assert_eq!(out.level(), DiagnosticLevel::Fail);
        assert_eq!(
            out.message(),
            "instruction-intake: the register of repository \"web\" names 2 topic(s) outside the pool (alpha, zeta); a new topic is a change to the registry, not to one record"
        );
    }

    fn complete_tree() -> RepositoryTree {
        RepositoryTree {
            tracked: vec![".cursor/rules/a.mdc".into()],
            ..RepositoryTree::default()
        }
    }

    #[test]
    fn workspace_state_intake_an_unavailable_tree_fact_is_unverified_never_complete() {
        let records = [record(
            "web",
            ".cursor/rules/a.mdc",
            None,
            "adopt-core",
            "2026-01-01",
        )];
        let refs: Vec<&IntakeRecord> = records.iter().collect();
        assert!(check_completeness("web", &refs, Some(&complete_tree())).complete);

        let untracked = RepositoryTree {
            untracked_norms: TreeFact::Unavailable("git ls-files failed".into()),
            ..complete_tree()
        };
        let check = check_completeness("web", &refs, Some(&untracked));
        assert!(!check.complete);
        assert_eq!(
            messages(&check),
            [(
                DiagnosticLevel::Warn,
                "instruction-intake: web — the untracked files of the tree cannot be listed (git ls-files failed); norms outside version control are UNVERIFIED, and this tree is NOT counted as confirmed complete".to_string()
            )]
        );

        let ignored = RepositoryTree {
            ignored_norms: TreeFact::Unavailable("git check-ignore failed".into()),
            ..complete_tree()
        };
        let check = check_completeness("web", &refs, Some(&ignored));
        assert!(!check.complete);
        assert_eq!(
            messages(&check),
            [(
                DiagnosticLevel::Warn,
                "instruction-intake: web — whether the recorded norms outside version control are ignored cannot be established (git check-ignore failed); such a norm is UNVERIFIED, and this tree is NOT counted as confirmed complete".to_string()
            )]
        );

        // A proven gap is still a failure when another fact is unavailable.
        let both = RepositoryTree {
            tracked: vec![".cursor/rules/a.mdc".into(), "b.mdc".into()],
            untracked_norms: TreeFact::Unavailable("x".into()),
            ..RepositoryTree::default()
        };
        let check = check_completeness("web", &refs, Some(&both));
        assert!(!check.complete);
        assert!(messages(&check)
            .iter()
            .any(|(level, m)| *level == DiagnosticLevel::Fail
                && m.contains("1 norm(s) in the tree have no record (b.mdc)")));
    }

    #[test]
    fn workspace_state_intake_an_unreadable_container_is_never_counted_complete() {
        let tree = RepositoryTree {
            tracked: vec!["AGENTS.md".into()],
            containers: [("AGENTS.md".to_string(), None)].into(),
            ..RepositoryTree::default()
        };
        let check = check_completeness("web", &[], Some(&tree));
        assert!(!check.complete, "{:?}", messages(&check));
    }
}
