//! Applicability equivalence of a migration: the controlled rule
//! applicability records of the pinned source against the records actually
//! read back from storage (`meridian-rust-migration-program-plan.md`
//! §5.22.4, item 12; §6.5a, item 2).
//!
//! Both sides arrive as typed [`ApplicabilityRecord`]s the caller built
//! through the one Kernel applicability contract — the "before" side from
//! the pinned source, the "after" side from the payload of the stored
//! records, never from a second read of the source. A record's identity is
//! its norm plus the intake decision that produced it plus its own
//! resolution date (the register is a temporal log, so the norm alone is not
//! unique); its outcome is everything that decides what the norm resolves
//! to — the full scope, the full activation (globs order-neutral) and the
//! status kind. Provenance bookkeeping (digest, `resume_condition` text,
//! `supersedes`) is not an outcome.

use crate::resolver::{Activation, ApplicabilityRecord, ApplicabilityScope, ApplicabilityStatus};

/// The identity of one controlled source context.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApplicabilityIdentity {
    repository: String,
    path: String,
    region: String,
    register: String,
    intake_recorded_at: String,
    verdict: String,
    recorded_at: String,
}

impl ApplicabilityIdentity {
    pub fn of(record: &ApplicabilityRecord) -> ApplicabilityIdentity {
        let intake = record.source().intake_record();
        ApplicabilityIdentity {
            repository: record.norm().repository().to_string(),
            path: record.norm().path().to_string(),
            region: record.norm().region().unwrap_or("").to_string(),
            register: intake.map_or(String::new(), |p| p.register().to_string()),
            intake_recorded_at: intake.map_or(String::new(), |p| p.recorded_at().to_string()),
            verdict: intake.map_or(String::new(), |p| p.verdict().as_str().to_string()),
            recorded_at: record.recorded_at().to_string(),
        }
    }

    /// A stable, human-readable rendering for diagnostics.
    pub fn label(&self) -> String {
        let region = if self.region.is_empty() {
            String::new()
        } else {
            format!(":{}", self.region)
        };
        format!(
            "{}/{}{region} [{}@{}/{}] recorded {}",
            self.repository,
            self.path,
            self.register,
            self.intake_recorded_at,
            self.verdict,
            self.recorded_at
        )
    }
}

/// What one record resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ApplicabilityOutcome {
    scope: ApplicabilityScope,
    activation: Activation,
    resolved: bool,
}

impl ApplicabilityOutcome {
    fn of(record: &ApplicabilityRecord) -> ApplicabilityOutcome {
        let activation = match record.activation() {
            Activation::PathGlob { globs } => {
                let mut globs = globs.clone();
                globs.sort();
                Activation::PathGlob { globs }
            }
            other => other.clone(),
        };
        ApplicabilityOutcome {
            scope: record.scope().clone(),
            activation,
            resolved: matches!(record.status(), ApplicabilityStatus::Resolved),
        }
    }
}

/// The verdict of one comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicabilityEquivalence {
    /// Distinct identities of the pinned source.
    pub controlled: usize,
    /// Records read back from storage.
    pub imported: usize,
    pub lost: Vec<String>,
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub duplicate: Vec<String>,
}

impl ApplicabilityEquivalence {
    /// Equivalent only when the pinned source controls at least one record
    /// and nothing was lost, added, changed or duplicated. Two empty sides
    /// prove nothing, so they are never equivalent: an absent or empty
    /// register cannot pass as a preserved one.
    pub fn is_equivalent(&self) -> bool {
        self.controlled > 0
            && self.lost.is_empty()
            && self.added.is_empty()
            && self.changed.is_empty()
            && self.duplicate.is_empty()
    }
}

fn index(
    records: &[ApplicabilityRecord],
    duplicate: &mut Vec<String>,
) -> Vec<(ApplicabilityIdentity, ApplicabilityOutcome)> {
    let mut out: Vec<(ApplicabilityIdentity, ApplicabilityOutcome)> = Vec::new();
    for record in records {
        let identity = ApplicabilityIdentity::of(record);
        if out.iter().any(|(i, _)| *i == identity) {
            duplicate.push(identity.label());
            continue;
        }
        out.push((identity, ApplicabilityOutcome::of(record)));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Compares the pinned source's records (`before`) with the stored
/// candidate's records (`after`). A duplicate identity on either side makes
/// the comparison non-equivalent on its own.
pub fn compare_applicability(
    before: &[ApplicabilityRecord],
    after: &[ApplicabilityRecord],
) -> ApplicabilityEquivalence {
    let mut duplicate = Vec::new();
    let before_index = index(before, &mut duplicate);
    let after_index = index(after, &mut duplicate);
    let mut lost = Vec::new();
    let mut changed = Vec::new();
    for (identity, outcome) in &before_index {
        match after_index.iter().find(|(i, _)| i == identity) {
            None => lost.push(identity.label()),
            Some((_, candidate)) if candidate != outcome => changed.push(identity.label()),
            Some(_) => {}
        }
    }
    let added = after_index
        .iter()
        .filter(|(i, _)| !before_index.iter().any(|(b, _)| b == i))
        .map(|(i, _)| i.label())
        .collect();
    duplicate.sort();
    ApplicabilityEquivalence {
        controlled: before_index.len(),
        imported: after.len(),
        lost,
        added,
        changed,
        duplicate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::{ApplicabilitySource, IntakePointer, IntakeVerdict, IsoDate, NormRef};
    use crate::types::ContentDigest;

    fn record(region: &str, scope: ApplicabilityScope, recorded: &str) -> ApplicabilityRecord {
        ApplicabilityRecord::new(
            NormRef::new("repo", "AGENTS.md", Some(region.to_string())).unwrap(),
            ContentDigest::of_str(region),
            scope,
            Activation::Always,
            ApplicabilitySource::Repository {
                intake_record: IntakePointer::new(
                    "intake/repo.yaml",
                    IsoDate::new("2026-08-26").unwrap(),
                    IntakeVerdict::KeepLocal,
                    None,
                )
                .unwrap(),
                supersedes: None,
            },
            ApplicabilityStatus::Resolved,
            IsoDate::new(recorded).unwrap(),
        )
    }

    fn repo_scope() -> ApplicabilityScope {
        ApplicabilityScope::Repository {
            repository: "repo".into(),
        }
    }

    #[test]
    fn migration_applicability_identical_sides_are_equivalent() {
        let before = vec![
            record("a", repo_scope(), "2026-08-29"),
            record("b", ApplicabilityScope::Universal, "2026-08-29"),
        ];
        let after = vec![before[1].clone(), before[0].clone()];
        let verdict = compare_applicability(&before, &after);
        assert!(verdict.is_equivalent(), "{verdict:?}");
        assert_eq!(verdict.controlled, 2);
    }

    #[test]
    fn migration_applicability_detects_lost_added_changed_and_duplicate() {
        let before = vec![
            record("a", repo_scope(), "2026-08-29"),
            record("b", repo_scope(), "2026-08-29"),
        ];
        let after = vec![
            record("a", ApplicabilityScope::Universal, "2026-08-29"),
            record("c", repo_scope(), "2026-08-29"),
            record("c", repo_scope(), "2026-08-29"),
        ];
        let verdict = compare_applicability(&before, &after);
        assert_eq!(verdict.changed.len(), 1);
        assert_eq!(verdict.lost.len(), 1);
        assert_eq!(verdict.added.len(), 1);
        assert_eq!(verdict.duplicate.len(), 1);
        assert!(!verdict.is_equivalent());
    }

    #[test]
    fn migration_applicability_empty_sides_are_never_equivalent() {
        let verdict = compare_applicability(&[], &[]);
        assert_eq!(verdict.controlled, 0);
        assert_eq!(verdict.imported, 0);
        assert!(verdict.lost.is_empty() && verdict.added.is_empty());
        assert!(!verdict.is_equivalent(), "{verdict:?}");

        // An empty source with a non-empty candidate is not equivalent
        // either: every candidate record is an addition.
        let verdict = compare_applicability(&[], &[record("a", repo_scope(), "2026-08-29")]);
        assert_eq!(verdict.added.len(), 1);
        assert!(!verdict.is_equivalent());
    }

    #[test]
    fn migration_applicability_a_re_resolution_on_another_date_is_its_own_identity() {
        let before = vec![record("a", repo_scope(), "2026-08-29")];
        let after = vec![record("a", repo_scope(), "2026-08-30")];
        let verdict = compare_applicability(&before, &after);
        assert_eq!(verdict.lost.len(), 1);
        assert_eq!(verdict.added.len(), 1);
    }
}
