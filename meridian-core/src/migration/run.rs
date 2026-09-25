//! One migration run — the recorded fact of applying one accepted plan —
//! and the pure decisions around it (`meridian-rust-migration-program-plan.md`
//! §5.22.4 items 9–10, §5.22.5): whether an apply proceeds, is a repeat or
//! conflicts, and whether a rollback of a run is admissible.
//!
//! The storage adapter gathers the facts (the stored run, the current
//! record-state digest, the newest run sequence) and this module decides.
//! Checkpoint files are the adapter's concern; their verified identity
//! arrives here as a digest.

use core::fmt;

use crate::types::ContentDigest;

/// The identity of one migration run: `mr-<sequence>-<12 hex of the plan
/// fingerprint>`. Deterministic for a given database state and plan —
/// never a clock or random value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MigrationRunId(String);

/// A value that is not a migration run id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRunIdError(pub String);

impl fmt::Display for MigrationRunIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" is not a migration run id (expected mr-<sequence>-<12 lowercase hex>)",
            self.0
        )
    }
}

impl std::error::Error for MigrationRunIdError {}

impl MigrationRunId {
    /// The id of run number `sequence` of the plan whose fingerprint is
    /// `fingerprint`.
    pub fn for_run(sequence: RunSequence, fingerprint: &ContentDigest) -> MigrationRunId {
        let short: String = fingerprint.value().chars().take(12).collect();
        MigrationRunId(format!("mr-{}-{short}", sequence.0))
    }

    /// Parses a run id exactly as [`MigrationRunId::for_run`] renders it.
    pub fn parse(value: &str) -> Result<MigrationRunId, MigrationRunIdError> {
        let invalid = || MigrationRunIdError(value.to_string());
        let rest = value.strip_prefix("mr-").ok_or_else(invalid)?;
        let (sequence, short) = rest.split_once('-').ok_or_else(invalid)?;
        let sequence_ok = !sequence.is_empty()
            && sequence.bytes().all(|b| b.is_ascii_digit())
            && !sequence.starts_with('0')
            && sequence.parse::<u64>().is_ok();
        let short_ok = short.len() == 12
            && short
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if sequence_ok && short_ok {
            Ok(MigrationRunId(value.to_string()))
        } else {
            Err(invalid())
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MigrationRunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The 1-based position of a run in one database's run journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunSequence(u64);

impl RunSequence {
    pub const FIRST: RunSequence = RunSequence(1);

    /// `None` for zero: sequences are 1-based.
    pub fn new(value: u64) -> Option<RunSequence> {
        (value > 0).then_some(RunSequence(value))
    }

    /// The sequence after the newest recorded one (`None` = no run yet).
    pub fn after(newest: Option<RunSequence>) -> RunSequence {
        newest.map_or(RunSequence::FIRST, |n| RunSequence(n.0 + 1))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

/// The status of a recorded run, DERIVED from the journal's facts: a run
/// is rolled back exactly when a [`RunRollback`] fact exists for it. The
/// status is never stored as an editable field of the applied fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Applied,
    RolledBack,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Applied => "applied",
            RunStatus::RolledBack => "rolled-back",
        }
    }
}

/// The record state of one database at one moment: the digest of its
/// canonical record export and the number of stored revisions (revisions
/// are append-only, so an equal count means no revision was added).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordState {
    pub digest: ContentDigest,
    pub revision_count: u64,
}

/// The append-only fact that a run was rolled back: recorded after the
/// run's checkpoint was restored and the reopened database was verified to
/// hold the run's pre-state. It never replaces or edits the applied fact it
/// names — the journal keeps both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRollback {
    /// The verified record state of the restored database — the run's
    /// pre-state.
    pub restored_state: RecordState,
}

/// A migration run as the journal records it: the immutable fact of the
/// apply, and — as a separate append-only fact — its rollback, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRun {
    pub id: MigrationRunId,
    pub sequence: RunSequence,
    pub plan_ref: String,
    pub plan_fingerprint: ContentDigest,
    pub idempotency_key: ContentDigest,
    pub source_repository_ref: String,
    pub source_revision: String,
    pub pre_state: RecordState,
    pub post_state: RecordState,
    pub checkpoint_digest: ContentDigest,
    pub written_records: u64,
    /// The separate rollback fact of this run, if one was recorded.
    pub rollback: Option<RunRollback>,
}

impl MigrationRun {
    /// `rolled-back` exactly when a rollback fact exists, else `applied`.
    pub fn status(&self) -> RunStatus {
        if self.rollback.is_some() {
            RunStatus::RolledBack
        } else {
            RunStatus::Applied
        }
    }
}

/// What an apply of a plan does, decided from the run (if any) already
/// recorded under the plan's idempotency key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyDecision {
    /// No run carries this key: the plan is applied.
    Proceed,
    /// The same plan content was already applied: nothing is written.
    AlreadyApplied(MigrationRun),
    /// The key was recorded for different plan content.
    Conflict {
        run: MigrationRunId,
        recorded_fingerprint: ContentDigest,
        requested_fingerprint: ContentDigest,
    },
    /// The same plan was applied and then rolled back; re-applying it is a
    /// separate decision this command never takes implicitly.
    RolledBack(MigrationRun),
}

pub fn decide_apply(existing: Option<&MigrationRun>, fingerprint: &ContentDigest) -> ApplyDecision {
    match existing {
        None => ApplyDecision::Proceed,
        Some(run) if run.plan_fingerprint != *fingerprint => ApplyDecision::Conflict {
            run: run.id.clone(),
            recorded_fingerprint: run.plan_fingerprint.clone(),
            requested_fingerprint: fingerprint.clone(),
        },
        Some(run) => match run.status() {
            RunStatus::Applied => ApplyDecision::AlreadyApplied(run.clone()),
            RunStatus::RolledBack => ApplyDecision::RolledBack(run.clone()),
        },
    }
}

/// Why a rollback is refused. Every refusal leaves storage untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackRefusal {
    UnknownRun {
        run: String,
    },
    AlreadyRolledBack {
        run: MigrationRunId,
    },
    /// The run belongs to a different plan than the one named by the source.
    PlanMismatch {
        run: MigrationRunId,
        run_fingerprint: ContentDigest,
        plan_fingerprint: ContentDigest,
    },
    /// A later run was recorded after this one.
    NewerRun {
        run: MigrationRunId,
        newer: MigrationRunId,
    },
    /// The current record state is not the state this run left behind: a
    /// record or revision changed since.
    StateChanged {
        run: MigrationRunId,
        expected: RecordState,
        found: RecordState,
    },
    CheckpointMissing {
        run: MigrationRunId,
    },
    CheckpointDigestMismatch {
        run: MigrationRunId,
        expected: ContentDigest,
        found: ContentDigest,
    },
    /// The checkpoint opens, but its role, edition or record state is not
    /// the run's recorded pre-state.
    CheckpointStateMismatch {
        run: MigrationRunId,
        reason: String,
    },
    /// After restore, the reopened database's record state is not the
    /// run's pre-state (the restore itself is kept and reported, but the
    /// rollback is not recorded as successful).
    RestoredStateMismatch {
        run: MigrationRunId,
        expected: ContentDigest,
        found: ContentDigest,
    },
}

impl RollbackRefusal {
    /// Whether the refusal stems from a damaged or missing checkpoint (an
    /// input/environment problem) rather than from the journal state.
    pub fn is_checkpoint_problem(&self) -> bool {
        matches!(
            self,
            RollbackRefusal::CheckpointMissing { .. }
                | RollbackRefusal::CheckpointDigestMismatch { .. }
                | RollbackRefusal::CheckpointStateMismatch { .. }
                | RollbackRefusal::RestoredStateMismatch { .. }
        )
    }
}

impl fmt::Display for RollbackRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RollbackRefusal::UnknownRun { run } => {
                write!(f, "migration run \"{run}\" is not recorded in this database")
            }
            RollbackRefusal::AlreadyRolledBack { run } => {
                write!(f, "migration run \"{run}\" was already rolled back")
            }
            RollbackRefusal::PlanMismatch {
                run,
                run_fingerprint,
                plan_fingerprint,
            } => write!(
                f,
                "migration run \"{run}\" applied plan fingerprint {}, but the named source's accepted plan has fingerprint {}",
                run_fingerprint.value(),
                plan_fingerprint.value()
            ),
            RollbackRefusal::NewerRun { run, newer } => write!(
                f,
                "migration run \"{run}\" is not the newest run: \"{newer}\" was recorded after it"
            ),
            RollbackRefusal::StateChanged {
                run,
                expected,
                found,
            } => write!(
                f,
                "the current record state (digest {}, {} revision(s)) is not the state migration run \"{run}\" left behind (digest {}, {} revision(s)); a record or revision changed since",
                found.digest.value(),
                found.revision_count,
                expected.digest.value(),
                expected.revision_count
            ),
            RollbackRefusal::CheckpointMissing { run } => {
                write!(f, "the checkpoint of migration run \"{run}\" is missing")
            }
            RollbackRefusal::CheckpointDigestMismatch {
                run,
                expected,
                found,
            } => write!(
                f,
                "the checkpoint of migration run \"{run}\" is damaged: digest {} differs from the recorded {}",
                found.value(),
                expected.value()
            ),
            RollbackRefusal::CheckpointStateMismatch { run, reason } => write!(
                f,
                "the checkpoint of migration run \"{run}\" does not hold the run's pre-state: {reason}"
            ),
            RollbackRefusal::RestoredStateMismatch {
                run,
                expected,
                found,
            } => write!(
                f,
                "after restoring migration run \"{run}\" the record state digest is {}, not the recorded pre-state {}",
                found.value(),
                expected.value()
            ),
        }
    }
}

/// The journal-level preconditions of a rollback, before any checkpoint is
/// touched: the run is applied, belongs to `plan_fingerprint`, is the
/// newest run, and the database is still exactly in the state it left.
pub fn check_rollback_preconditions(
    run: &MigrationRun,
    plan_fingerprint: &ContentDigest,
    newest: &MigrationRunId,
    current: &RecordState,
) -> Result<(), RollbackRefusal> {
    if run.status() == RunStatus::RolledBack {
        return Err(RollbackRefusal::AlreadyRolledBack {
            run: run.id.clone(),
        });
    }
    if run.plan_fingerprint != *plan_fingerprint {
        return Err(RollbackRefusal::PlanMismatch {
            run: run.id.clone(),
            run_fingerprint: run.plan_fingerprint.clone(),
            plan_fingerprint: plan_fingerprint.clone(),
        });
    }
    if *newest != run.id {
        return Err(RollbackRefusal::NewerRun {
            run: run.id.clone(),
            newer: newest.clone(),
        });
    }
    if *current != run.post_state {
        return Err(RollbackRefusal::StateChanged {
            run: run.id.clone(),
            expected: run.post_state.clone(),
            found: current.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: &str) -> ContentDigest {
        ContentDigest::of_str(byte)
    }

    fn run(status: RunStatus) -> MigrationRun {
        let pre_state = RecordState {
            digest: digest("pre"),
            revision_count: 0,
        };
        let fingerprint = digest("plan");
        MigrationRun {
            id: MigrationRunId::for_run(RunSequence::FIRST, &fingerprint),
            sequence: RunSequence::FIRST,
            plan_ref: "plan".to_string(),
            plan_fingerprint: fingerprint,
            idempotency_key: digest("key"),
            source_repository_ref: "repo".to_string(),
            source_revision: "rev".to_string(),
            pre_state: pre_state.clone(),
            post_state: RecordState {
                digest: digest("post"),
                revision_count: 3,
            },
            checkpoint_digest: digest("checkpoint"),
            written_records: 3,
            rollback: (status == RunStatus::RolledBack).then_some(RunRollback {
                restored_state: pre_state,
            }),
        }
    }

    #[test]
    fn migration_run_ids_round_trip_and_reject_foreign_shapes() {
        let id = MigrationRunId::for_run(RunSequence::FIRST, &digest("plan"));
        assert_eq!(MigrationRunId::parse(id.as_str()).unwrap(), id);
        for bad in [
            "",
            "mr-1",
            "mr-0-0123456789ab",
            "mr-01-0123456789ab",
            "mr-1-0123456789AB",
            "mr-1-0123456789a",
            "x-1-0123456789ab",
        ] {
            assert!(MigrationRunId::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn migration_apply_decision_distinguishes_repeat_conflict_and_rolled_back() {
        let applied = run(RunStatus::Applied);
        assert_eq!(decide_apply(None, &digest("plan")), ApplyDecision::Proceed);
        assert!(matches!(
            decide_apply(Some(&applied), &digest("plan")),
            ApplyDecision::AlreadyApplied(_)
        ));
        assert!(matches!(
            decide_apply(Some(&applied), &digest("other")),
            ApplyDecision::Conflict { .. }
        ));
        let rolled = run(RunStatus::RolledBack);
        assert!(matches!(
            decide_apply(Some(&rolled), &digest("plan")),
            ApplyDecision::RolledBack(_)
        ));
    }

    #[test]
    fn migration_rollback_preconditions_refuse_every_unsafe_state() {
        let applied = run(RunStatus::Applied);
        let fp = digest("plan");
        let post = applied.post_state.clone();
        assert_eq!(
            check_rollback_preconditions(&applied, &fp, &applied.id, &post),
            Ok(())
        );
        assert!(matches!(
            check_rollback_preconditions(&run(RunStatus::RolledBack), &fp, &applied.id, &post),
            Err(RollbackRefusal::AlreadyRolledBack { .. })
        ));
        assert!(matches!(
            check_rollback_preconditions(&applied, &digest("x"), &applied.id, &post),
            Err(RollbackRefusal::PlanMismatch { .. })
        ));
        let newer = MigrationRunId::for_run(RunSequence::new(2).unwrap(), &fp);
        assert!(matches!(
            check_rollback_preconditions(&applied, &fp, &newer, &post),
            Err(RollbackRefusal::NewerRun { .. })
        ));
        let moved = RecordState {
            digest: post.digest.clone(),
            revision_count: post.revision_count + 1,
        };
        assert!(matches!(
            check_rollback_preconditions(&applied, &fp, &applied.id, &moved),
            Err(RollbackRefusal::StateChanged { .. })
        ));
    }
}
