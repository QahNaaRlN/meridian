//! M-08 — the repository inventory against the repositories themselves:
//! recorded revision, ref and working-tree state against what the local
//! repository reports now, and the age of each entry's `last_verified`.
//! A repository that cannot be observed is `UNVERIFIED`, never confirmed.

use crate::types::{Diagnostic, DiagnosticLevel, NonEmptyString, RepositoryId};

use super::diagnostic;
use super::timestamp::Timestamp;

/// An inventory entry older than this many days is due for a recheck.
pub const INVENTORY_TTL_DAYS: f64 = 3.0;

/// The recorded working-tree state of a repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkingTree {
    Clean,
    Dirty,
}

impl WorkingTree {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "clean" => Some(WorkingTree::Clean),
            "dirty" => Some(WorkingTree::Dirty),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            WorkingTree::Clean => "clean",
            WorkingTree::Dirty => "dirty",
        }
    }
}

/// The version-control facts an inventory entry records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedVcs {
    pub revision: NonEmptyString,
    pub git_ref: NonEmptyString,
    pub working_tree: WorkingTree,
}

/// What the local repository reports now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedVcs {
    pub revision: String,
    pub git_ref: String,
    pub dirty: bool,
}

/// One repository of the inventory, as far as this check needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryIdentity {
    id: RepositoryId,
    path: NonEmptyString,
    profile: String,
    recorded: RecordedVcs,
    last_verified: Option<Timestamp>,
    manifests: Vec<NonEmptyString>,
}

impl RepositoryIdentity {
    /// `last_verified` is `None` when the recorded value does not parse as
    /// an instant — a warning of its own, not a construction failure.
    pub fn new(
        id: RepositoryId,
        path: NonEmptyString,
        profile: impl Into<String>,
        recorded: RecordedVcs,
        last_verified: Option<Timestamp>,
        manifests: Vec<NonEmptyString>,
    ) -> Self {
        Self {
            id,
            path,
            profile: profile.into(),
            recorded,
            last_verified,
            manifests,
        }
    }

    pub fn id(&self) -> &RepositoryId {
        &self.id
    }

    /// The local path the inventory names for this repository.
    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    /// The declared stack profile, verbatim (may be empty or `universal`,
    /// which the profile check refuses).
    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub fn manifests(&self) -> &[NonEmptyString] {
        &self.manifests
    }
}

/// One entry's verdict: its diagnostics and whether its recorded state was
/// confirmed against the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryCheck {
    pub diagnostics: Vec<Diagnostic>,
    pub confirmed: bool,
}

/// Checks one entry. `observed` is `None` when the repository is not
/// reachable from here.
pub fn check_entry(
    identity: &RepositoryIdentity,
    observed: Option<&ObservedVcs>,
    now: Timestamp,
) -> EntryCheck {
    let id = identity.id.as_str();
    let Some(actual) = observed else {
        return EntryCheck {
            diagnostics: vec![diagnostic(
                DiagnosticLevel::Info,
                format!(
                    "inventory-git: {id} — repository at {} is not reachable from here; recorded revision is UNVERIFIED, not confirmed",
                    identity.path.as_str()
                ),
            )],
            confirmed: false,
        };
    };
    let mut diagnostics = Vec::new();
    let recorded = &identity.recorded;
    let mut problems = Vec::new();
    if recorded.revision.as_str() != actual.revision {
        problems.push(format!(
            "revision recorded {} but HEAD is {}",
            prefix(recorded.revision.as_str(), 7),
            prefix(&actual.revision, 7)
        ));
    }
    if recorded.git_ref.as_str() != actual.git_ref {
        problems.push(format!(
            "ref recorded \"{}\" but HEAD is on \"{}\"",
            recorded.git_ref.as_str(),
            actual.git_ref
        ));
    }
    if (recorded.working_tree == WorkingTree::Dirty) != actual.dirty {
        problems.push(format!(
            "working tree recorded \"{}\" but is now {}",
            recorded.working_tree.as_str(),
            if actual.dirty { "dirty" } else { "clean" }
        ));
    }
    let confirmed = problems.is_empty();
    if !confirmed {
        diagnostics.push(diagnostic(
            DiagnosticLevel::Fail,
            format!(
                "inventory-git: {id} — {}; revalidate the entry, do not merely re-date it",
                problems.join("; ")
            ),
        ));
    }
    match identity.last_verified {
        None => diagnostics.push(diagnostic(
            DiagnosticLevel::Warn,
            format!("inventory-git: {id} has no parseable last_verified"),
        )),
        Some(verified) => {
            let age = verified.days_until(now);
            if age > INVENTORY_TTL_DAYS {
                diagnostics.push(diagnostic(
                    DiagnosticLevel::Warn,
                    format!(
                        "inventory-git: {id} last_verified is {age:.1}d old (TTL {INVENTORY_TTL_DAYS}d) — recheck due"
                    ),
                ));
            }
        }
    }
    EntryCheck {
        diagnostics,
        confirmed,
    }
}

/// The run-level line: how many entries were confirmed, or a warning that
/// none could be.
pub fn summarize(total: usize, confirmed: usize) -> Diagnostic {
    if total == 0 {
        return diagnostic(
            DiagnosticLevel::Warn,
            "inventory-git: repository inventory not found".to_string(),
        );
    }
    if confirmed == 0 {
        diagnostic(
            DiagnosticLevel::Warn,
            format!("inventory-git: 0/{total} entries could be confirmed"),
        )
    } else {
        diagnostic(
            DiagnosticLevel::Info,
            format!(
                "inventory-git: {confirmed}/{total} entries confirmed against actual repository state"
            ),
        )
    }
}

fn prefix(text: &str, chars: usize) -> &str {
    text.char_indices()
        .nth(chars)
        .map(|(at, _)| &text[..at])
        .unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REV: &str = "aeb842f2134a4e06cc38d81f29d6b835ea26571b";

    fn identity(last_verified: Option<&str>) -> RepositoryIdentity {
        RepositoryIdentity::new(
            RepositoryId::new("sample-repo").unwrap(),
            NonEmptyString::new("/work/sample-repo").unwrap(),
            "vue-spa",
            RecordedVcs {
                revision: NonEmptyString::new(REV).unwrap(),
                git_ref: NonEmptyString::new("dev").unwrap(),
                working_tree: WorkingTree::Clean,
            },
            last_verified.and_then(Timestamp::parse_iso8601),
            Vec::new(),
        )
    }

    fn now() -> Timestamp {
        Timestamp::parse_iso8601("2026-09-02T00:00:00Z").unwrap()
    }

    fn messages(check: &EntryCheck) -> Vec<&str> {
        check.diagnostics.iter().map(Diagnostic::message).collect()
    }

    #[test]
    fn workspace_state_inventory_confirms_a_matching_fresh_entry() {
        let observed = ObservedVcs {
            revision: REV.to_string(),
            git_ref: "dev".to_string(),
            dirty: false,
        };
        let check = check_entry(
            &identity(Some("2026-09-01T00:00:00Z")),
            Some(&observed),
            now(),
        );
        assert!(check.confirmed);
        assert!(check.diagnostics.is_empty());
        assert_eq!(
            summarize(1, 1).message(),
            "inventory-git: 1/1 entries confirmed against actual repository state"
        );
    }

    #[test]
    fn workspace_state_inventory_fails_every_drifted_fact_and_warns_on_age() {
        let observed = ObservedVcs {
            revision: "0123456789abcdef0123456789abcdef01234567".to_string(),
            git_ref: "main".to_string(),
            dirty: true,
        };
        let check = check_entry(
            &identity(Some("2026-08-27T12:00:00Z")),
            Some(&observed),
            now(),
        );
        assert!(!check.confirmed);
        assert_eq!(
            messages(&check),
            [
                "inventory-git: sample-repo — revision recorded aeb842f but HEAD is 0123456; ref recorded \"dev\" but HEAD is on \"main\"; working tree recorded \"clean\" but is now dirty; revalidate the entry, do not merely re-date it",
                "inventory-git: sample-repo last_verified is 5.5d old (TTL 3d) — recheck due",
            ]
        );
        assert_eq!(
            summarize(1, 0).message(),
            "inventory-git: 0/1 entries could be confirmed"
        );
    }

    #[test]
    fn workspace_state_inventory_unreachable_is_unverified_not_confirmed() {
        let check = check_entry(&identity(None), None, now());
        assert!(!check.confirmed);
        assert_eq!(check.diagnostics[0].level(), DiagnosticLevel::Info);
        assert!(check.diagnostics[0]
            .message()
            .contains("UNVERIFIED, not confirmed"));
        let unparseable = check_entry(
            &identity(Some("not a date")),
            Some(&ObservedVcs {
                revision: REV.to_string(),
                git_ref: "dev".to_string(),
                dirty: false,
            }),
            now(),
        );
        assert_eq!(
            messages(&unparseable),
            ["inventory-git: sample-repo has no parseable last_verified"]
        );
        assert_eq!(
            summarize(0, 0).message(),
            "inventory-git: repository inventory not found"
        );
    }
}
