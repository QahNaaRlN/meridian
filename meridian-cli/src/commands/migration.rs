//! `meridian migration plan|apply|verify|rollback`
//! (`meridian-rust-migration-program-plan.md` §5.22.3).
//!
//! This module owns only the nested grammar, the explicit paths, the
//! composition of the concrete adapters (the Git source view, the SQLite
//! databases and their checkpoint location) and the presentation of the
//! typed outcomes `meridian_app::migration` returns. The frozen source is
//! named only by `--source`; nothing here reads `MERIDIAN_INSTANCE`.

use std::io::Write;
use std::path::{Path, PathBuf};

use meridian_app::events::{EventKind, EventSink, ObservedEvent};
use meridian_app::migration::{
    self, AcceptedBundle, ApplyIntent, ApplyOutcome, BundleRejection, BundleVerdict,
    DryRunDecision, EffectCounts, KernelSchemas, MigrationError, MigrationStore, PlanFacts,
    RollbackOutcome, StoreAccess, Verification,
};
use meridian_app::storage::{DatabaseMetadata, DatabaseRole};
use meridian_core::migration::run::{MigrationRun, MigrationRunId, RollbackRefusal};
use meridian_core::types::{ContentDigest, NonEmptyString, Revision};
use meridian_storage_sqlite::SqliteStorage;
use serde_json::{json, Value};

use crate::adapters::frozen_source::GitFrozenSource;
use crate::adapters::workspace_reader::FsWorkspaceReader;
use crate::cli::{parse_dry_run, parse_flags, CliError, OutputFormat, ParsedArgs};
use crate::{exit_code, kernel};

pub const PLAN_FLAGS: &[&str] = &["kernel", "source", "format"];
pub const APPLY_FLAGS: &[&str] = &[
    "kernel",
    "source",
    "workspace-db",
    "dry-run",
    "confirm",
    "format",
];
pub const VERIFY_FLAGS: &[&str] = &["kernel", "source", "workspace-db", "format"];
pub const ROLLBACK_FLAGS: &[&str] = &[
    "kernel",
    "source",
    "workspace-db",
    "run",
    "confirm",
    "format",
];

/// Records one observed event; a summary that is somehow empty is simply
/// not recorded (events never change the command's result).
pub(crate) fn observe(sink: &dyn EventSink, kind: EventKind, summary: String) {
    if let Ok(summary) = NonEmptyString::new(summary) {
        sink.record(&ObservedEvent::new(kind, summary));
    }
}

/// The directory a database's migration checkpoints live in: next to the
/// database file, never inside a Kernel or source repository.
pub(crate) fn checkpoint_dir_for(db: &Path) -> PathBuf {
    let mut name = db.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".checkpoints");
    db.with_file_name(name)
}

/// Opens one EXISTING database with the access an operation needs:
/// strictly read-only (never created, migrated or written), or read-write
/// with its checkpoint location.
pub(crate) fn open_store(
    path: &Path,
    role: DatabaseRole,
    edition: &Revision,
    access: StoreAccess,
) -> Result<Box<dyn MigrationStore>, MigrationError> {
    if !path.is_file() {
        return Err(MigrationError::Open(format!(
            "cannot open {role} database {}: no database file exists at this path",
            path.display()
        )));
    }
    let metadata = DatabaseMetadata::new(role, edition.clone());
    let opened = match access {
        StoreAccess::ReadOnly => SqliteStorage::open_read_only(path, metadata),
        StoreAccess::ReadWrite => SqliteStorage::open_path(path, metadata)
            .map(|s| s.with_checkpoint_dir(checkpoint_dir_for(path))),
    };
    opened
        .map(|s| Box::new(s) as Box<dyn MigrationStore>)
        .map_err(|e| {
            MigrationError::Open(format!(
                "cannot open {role} database {}: {e}",
                path.display()
            ))
        })
}

/// Everything a migration command needs before its operation runs.
pub(crate) struct Context {
    pub schemas: KernelSchemas,
    pub edition: Revision,
}

pub(crate) fn kernel_context(kernel_path: &str) -> Result<Context, String> {
    let root = Path::new(kernel_path);
    if !root.is_dir() {
        return Err(format!("Kernel path {kernel_path} is not a directory"));
    }
    let edition = kernel::read_kernel_edition(root).map_err(|e| e.to_string())?;
    let reader = FsWorkspaceReader::new(root);
    let schemas = KernelSchemas::load(&reader).map_err(|e| e.to_string())?;
    Ok(Context { schemas, edition })
}

pub(crate) fn open_source(source_path: &str) -> Result<GitFrozenSource, String> {
    GitFrozenSource::open(Path::new(source_path))
        .map_err(|e| format!("cannot open the migration source {source_path}: {e}"))
}

pub(crate) fn digest_json(digest: &ContentDigest) -> Value {
    json!({ "algorithm": digest.algorithm().as_str(), "value": digest.value() })
}

pub(crate) fn facts_json(facts: Option<&PlanFacts>) -> Value {
    match facts {
        None => Value::Null,
        Some(f) => json!({
            "plan_id": f.plan_id,
            "plan_fingerprint": digest_json(&f.fingerprint),
            "idempotency_key": digest_json(&f.idempotency_key),
            "counts": {
                "record_units": f.counts.record_units,
                "migrated": f.counts.migrated,
                "retained": f.counts.retained,
                "merged": f.counts.merged,
                "minted_targets": f.counts.minted_targets,
            },
            "verification_status": f.declared_status,
        }),
    }
}

fn accepted_json(bundle: &AcceptedBundle) -> Value {
    json!({
        "verdict": "accepted",
        "bundle_revision": bundle.bundle_revision,
        "source": {
            "repository_ref": bundle.source.repository_ref,
            "revision": bundle.source.revision,
            "digest": digest_json(&bundle.source.digest),
        },
        "plan": facts_json(Some(&bundle.facts)),
        "canonical_export": {
            "id": bundle.export.input().head.id.as_str(),
            "digest": digest_json(bundle.export.digest()),
            "records": bundle.export.input().payload.records.len(),
            "retained": bundle.export.input().payload.retained.len(),
        },
        "diagnostics": [],
    })
}

pub(crate) fn rejected_json(rejection: &BundleRejection) -> Value {
    json!({
        "verdict": "rejected",
        "bundle_revision": rejection.bundle_revision,
        "source": rejection.facts.as_ref().map(|f| json!({
            "repository_ref": f.source_repository_ref,
            "revision": f.source_revision,
            "digest": digest_json(&f.declared_source_digest),
        })),
        "plan": facts_json(rejection.facts.as_ref()),
        "diagnostics": rejection
            .diagnostics
            .iter()
            .map(|d| d.message().to_string())
            .collect::<Vec<_>>(),
    })
}

pub(crate) fn write_rejection_human(out: &mut dyn Write, rejection: &BundleRejection) {
    let _ = writeln!(
        out,
        "Bundle REJECTED (bundle commit {})",
        rejection.bundle_revision
    );
    if let Some(f) = &rejection.facts {
        let _ = writeln!(
            out,
            "Plan {} — recomputed fingerprint {}",
            f.plan_id,
            f.fingerprint.value()
        );
    }
    for d in &rejection.diagnostics {
        let _ = writeln!(out, "  FAIL {}", d.message());
    }
}

fn write_accepted_human(out: &mut dyn Write, bundle: &AcceptedBundle) {
    let f = &bundle.facts;
    let _ = writeln!(
        out,
        "Bundle ACCEPTED (bundle commit {})",
        bundle.bundle_revision
    );
    let _ = writeln!(
        out,
        "Source {} @ {} — tree digest {}",
        bundle.source.repository_ref,
        bundle.source.revision,
        bundle.source.digest.value()
    );
    let _ = writeln!(
        out,
        "Plan {} — fingerprint {}, idempotency key {}, declared {}",
        f.plan_id,
        f.fingerprint.value(),
        f.idempotency_key.value(),
        f.declared_status
    );
    let _ = writeln!(
        out,
        "Record units {}: {} migrated, {} retained-transitional, {} merged ({} target record(s))",
        f.counts.record_units,
        f.counts.migrated,
        f.counts.retained,
        f.counts.merged,
        f.counts.minted_targets
    );
}

pub(crate) fn report_error(err: &mut dyn Write, error: &dyn std::fmt::Display) -> i32 {
    let _ = writeln!(err, "error: {error}");
    exit_code::INPUT_OR_ENVIRONMENT
}

fn required<'a>(
    args: &'a ParsedArgs,
    command: &'static str,
    flag: &'static str,
) -> Result<&'a str, CliError> {
    args.require(command, flag)
}

/// Dispatches `migration <subcommand> ...`.
pub fn run(rest: &[String], out: &mut dyn Write, err: &mut dyn Write, sink: &dyn EventSink) -> i32 {
    let Some(subcommand) = rest.first() else {
        return crate::report_usage_error(
            err,
            &CliError::UnknownSubcommand {
                command: "migration",
                subcommand: None,
            },
        );
    };
    let flags = &rest[1..];
    let (name, allowed): (&'static str, &[&str]) = match subcommand.as_str() {
        "plan" => ("migration plan", PLAN_FLAGS),
        "apply" => ("migration apply", APPLY_FLAGS),
        "verify" => ("migration verify", VERIFY_FLAGS),
        "rollback" => ("migration rollback", ROLLBACK_FLAGS),
        other => {
            return crate::report_usage_error(
                err,
                &CliError::UnknownSubcommand {
                    command: "migration",
                    subcommand: Some(other.to_string()),
                },
            )
        }
    };
    let args = match parse_flags(name, flags, allowed) {
        Ok(args) => args,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    match subcommand.as_str() {
        "plan" => run_plan(&args, out, err, sink),
        "apply" => run_apply(&args, out, err, sink),
        "verify" => run_verify(&args, out, err, sink),
        _ => run_rollback(&args, out, err, sink),
    }
}

fn run_plan(
    args: &ParsedArgs,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    const COMMAND: &str = "migration plan";
    let (kernel_path, source_path) = match (
        required(args, COMMAND, "kernel"),
        required(args, COMMAND, "source"),
    ) {
        (Ok(k), Ok(s)) => (k, s),
        (Err(e), _) | (_, Err(e)) => return crate::report_usage_error(err, &e),
    };
    let context = match kernel_context(kernel_path) {
        Ok(context) => context,
        Err(message) => return report_error(err, &message),
    };
    let source = match open_source(source_path) {
        Ok(source) => source,
        Err(message) => return report_error(err, &message),
    };
    observe(
        sink,
        EventKind::InputContext,
        format!("{COMMAND}: kernel={kernel_path} source={source_path}"),
    );
    let verdict = match migration::plan(&context.schemas, &source, sink) {
        Ok(verdict) => verdict,
        Err(error) => return report_error(err, &error),
    };
    let (ok, result) = match &verdict {
        BundleVerdict::Accepted(bundle) => (true, accepted_json(bundle)),
        BundleVerdict::Rejected(rejection) => (false, rejected_json(rejection)),
    };
    observe(
        sink,
        EventKind::Outcome,
        format!("{COMMAND}: {}", if ok { "accepted" } else { "rejected" }),
    );
    match args.format {
        OutputFormat::Json => crate::write_json_result(out, COMMAND, ok, &result),
        OutputFormat::Human => match &verdict {
            BundleVerdict::Accepted(bundle) => write_accepted_human(out, bundle),
            BundleVerdict::Rejected(rejection) => write_rejection_human(out, rejection),
        },
    }
    if ok {
        exit_code::OK
    } else {
        exit_code::DOMAIN_NEGATIVE
    }
}

pub(crate) fn run_json(run: &MigrationRun) -> Value {
    json!({
        "id": run.id.as_str(),
        "sequence": run.sequence.get(),
        "status": run.status().as_str(),
        "plan_ref": run.plan_ref,
        "plan_fingerprint": digest_json(&run.plan_fingerprint),
        "pre_state_digest": digest_json(&run.pre_state.digest),
        "post_state_digest": digest_json(&run.post_state.digest),
        "checkpoint_digest": digest_json(&run.checkpoint_digest),
        "written_records": run.written_records,
    })
}

fn effects_json(effects: &EffectCounts) -> Value {
    json!({ "created": effects.created, "updated": effects.updated, "unchanged": effects.unchanged })
}

/// The `apply` result object, shared with `import --kind frozen-instance`.
pub(crate) fn apply_json(bundle: &AcceptedBundle, dry_run: bool, outcome: &ApplyOutcome) -> Value {
    let (status, run, effects, reason): (
        &str,
        Option<&MigrationRun>,
        EffectCounts,
        Option<String>,
    ) = match outcome {
        ApplyOutcome::ConfirmationMismatch { given, .. } => (
            "confirmation-mismatch",
            None,
            EffectCounts::default(),
            Some(format!(
                "--confirm \"{given}\" is not the recomputed plan fingerprint; nothing was changed"
            )),
        ),
        ApplyOutcome::DryRun { would, decision } => (
            match decision {
                DryRunDecision::WouldApply => "dry-run",
                DryRunDecision::AlreadyApplied(_) => "dry-run-already-applied",
                DryRunDecision::Conflict(_) => "dry-run-conflict",
                DryRunDecision::PreviouslyRolledBack(_) => "dry-run-previously-rolled-back",
            },
            None,
            *would,
            match decision {
                DryRunDecision::WouldApply => None,
                DryRunDecision::AlreadyApplied(id)
                | DryRunDecision::Conflict(id)
                | DryRunDecision::PreviouslyRolledBack(id) => Some(format!("run {id}")),
            },
        ),
        ApplyOutcome::Applied { run, effects } => ("applied", Some(run), *effects, None),
        ApplyOutcome::AlreadyApplied { run } => {
            ("already-applied", Some(run), EffectCounts::default(), None)
        }
        ApplyOutcome::Conflict {
            run,
            recorded,
            requested,
        } => (
            "conflict",
            None,
            EffectCounts::default(),
            Some(format!(
                "run {run} recorded this idempotency key for plan fingerprint {}, not {}",
                recorded.value(),
                requested.value()
            )),
        ),
        ApplyOutcome::PreviouslyRolledBack { run } => (
            "previously-rolled-back",
            Some(run),
            EffectCounts::default(),
            Some(format!(
                "run {} of this plan was rolled back; it is never re-applied implicitly",
                run.id
            )),
        ),
        ApplyOutcome::WriteFailed { reason } => (
            "write-failed",
            None,
            EffectCounts::default(),
            Some(reason.clone()),
        ),
    };
    json!({
        "dry_run": dry_run,
        "status": status,
        "plan_id": bundle.facts.plan_id,
        "plan_fingerprint": digest_json(&bundle.facts.fingerprint),
        "run_id": run.map(|r| r.id.as_str()),
        "already_applied": matches!(outcome, ApplyOutcome::AlreadyApplied { .. }),
        "effects": effects_json(&effects),
        "retained": bundle.write_set.retained().len(),
        "checkpoint_digest": run.map(|r| digest_json(&r.checkpoint_digest)),
        "run": run.map(run_json),
        "reason": reason,
    })
}

pub(crate) fn write_apply_human(out: &mut dyn Write, value: &Value) {
    let _ = writeln!(
        out,
        "Apply {}: plan {} (dry run: {})",
        value["status"].as_str().unwrap_or(""),
        value["plan_id"].as_str().unwrap_or(""),
        value["dry_run"]
    );
    let effects = &value["effects"];
    let _ = writeln!(
        out,
        "Records: {} created, {} updated, {} unchanged; {} retained unit(s) not written",
        effects["created"], effects["updated"], effects["unchanged"], value["retained"]
    );
    if let Some(run) = value["run_id"].as_str() {
        let _ = writeln!(out, "Run: {run}");
    }
    if let Some(reason) = value["reason"].as_str() {
        let _ = writeln!(out, "Reason: {reason}");
    }
}

fn run_apply(
    args: &ParsedArgs,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    const COMMAND: &str = "migration apply";
    let parsed = (|| {
        let kernel_path = required(args, COMMAND, "kernel")?;
        let source_path = required(args, COMMAND, "source")?;
        let db = required(args, COMMAND, "workspace-db")?;
        let dry_run = parse_dry_run(COMMAND, args.get("dry-run"))?;
        let intent = match (dry_run, args.get("confirm")) {
            (true, confirm) => ApplyIntent::DryRun {
                confirm: confirm.map(str::to_string),
            },
            (false, Some(confirm)) => ApplyIntent::Apply {
                confirm: confirm.to_string(),
            },
            (false, None) => {
                return Err(CliError::MissingRequiredFlag {
                    command: COMMAND,
                    flag: "confirm",
                })
            }
        };
        Ok::<_, CliError>((kernel_path, source_path, db, dry_run, intent))
    })();
    let (kernel_path, source_path, db, dry_run, intent) = match parsed {
        Ok(parsed) => parsed,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    let context = match kernel_context(kernel_path) {
        Ok(context) => context,
        Err(message) => return report_error(err, &message),
    };
    let source = match open_source(source_path) {
        Ok(source) => source,
        Err(message) => return report_error(err, &message),
    };
    observe(
        sink,
        EventKind::InputContext,
        format!("{COMMAND}: kernel={kernel_path} source={source_path} dry_run={dry_run}"),
    );
    let db_path = Path::new(db);
    let open = |access| open_store(db_path, DatabaseRole::Workspace, &context.edition, access);
    let result = match migration::apply(&context.schemas, &source, &open, &intent, sink) {
        Ok(result) => result,
        Err(error) => return report_error(err, &error),
    };
    let (ok, value) = match &result {
        Err(rejection) => (false, rejected_json(rejection)),
        Ok((bundle, outcome)) => (outcome.is_positive(), apply_json(bundle, dry_run, outcome)),
    };
    observe(
        sink,
        EventKind::Outcome,
        format!(
            "{COMMAND}: {}",
            value["status"].as_str().unwrap_or("rejected")
        ),
    );
    match args.format {
        OutputFormat::Json => crate::write_json_result(out, COMMAND, ok, &value),
        OutputFormat::Human => match &result {
            Err(rejection) => write_rejection_human(out, rejection),
            Ok(_) => write_apply_human(out, &value),
        },
    }
    if ok {
        exit_code::OK
    } else {
        exit_code::DOMAIN_NEGATIVE
    }
}

/// The `verify` result object, shared with `import --kind frozen-instance`.
pub(crate) fn verify_json(bundle: &AcceptedBundle, verification: &Verification) -> Value {
    let diff = &verification.diff;
    let a = &verification.applicability;
    json!({
        "status": if verification.verified { "verified" } else { "not-verified" },
        "plan_id": bundle.facts.plan_id,
        "plan_fingerprint": digest_json(&bundle.facts.fingerprint),
        "run": verification.run.as_ref().map(run_json),
        "expected": diff.expected,
        "imported": diff.imported,
        "missing": diff.missing.len(),
        "extra": diff.extra.len(),
        "changed": diff.changed.len(),
        "duplicate": diff.duplicate.len(),
        "retained": verification.retained,
        "missing_records": diff.missing.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        "extra_records": diff.extra,
        "changed_records": diff.changed.iter().map(|c| json!({
            "id": c.id.as_str(),
            "fields": c.fields.iter().map(|f| f.as_str()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "duplicate_provenance": diff.duplicate,
        "applicability": {
            "verdict": if a.is_equivalent() { "equivalent" } else { "not-equivalent" },
            "controlled": a.controlled,
            "imported": a.imported,
            "lost": a.lost.len(),
            "added": a.added.len(),
            "changed": a.changed.len(),
            "duplicate": a.duplicate.len(),
            "lost_identities": a.lost,
            "added_identities": a.added,
            "changed_identities": a.changed,
            "duplicate_identities": a.duplicate,
        },
    })
}

pub(crate) fn write_verify_human(out: &mut dyn Write, value: &Value) {
    let _ = writeln!(
        out,
        "Verify {}: plan {}",
        value["status"].as_str().unwrap_or(""),
        value["plan_id"].as_str().unwrap_or("")
    );
    let _ = writeln!(
        out,
        "Records: {} expected, {} imported, {} missing, {} extra, {} changed, {} duplicate; {} retained",
        value["expected"],
        value["imported"],
        value["missing"],
        value["extra"],
        value["changed"],
        value["duplicate"],
        value["retained"]
    );
    let a = &value["applicability"];
    let _ = writeln!(
        out,
        "Applicability {}: {} controlled, {} imported, {} lost, {} added, {} changed, {} duplicate",
        a["verdict"].as_str().unwrap_or(""),
        a["controlled"],
        a["imported"],
        a["lost"],
        a["added"],
        a["changed"],
        a["duplicate"]
    );
}

fn run_verify(
    args: &ParsedArgs,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    const COMMAND: &str = "migration verify";
    let parsed = (|| {
        Ok::<_, CliError>((
            required(args, COMMAND, "kernel")?,
            required(args, COMMAND, "source")?,
            required(args, COMMAND, "workspace-db")?,
        ))
    })();
    let (kernel_path, source_path, db) = match parsed {
        Ok(parsed) => parsed,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    let context = match kernel_context(kernel_path) {
        Ok(context) => context,
        Err(message) => return report_error(err, &message),
    };
    let source = match open_source(source_path) {
        Ok(source) => source,
        Err(message) => return report_error(err, &message),
    };
    observe(
        sink,
        EventKind::InputContext,
        format!("{COMMAND}: kernel={kernel_path} source={source_path}"),
    );
    let db_path = Path::new(db);
    let open = |access| open_store(db_path, DatabaseRole::Workspace, &context.edition, access);
    let result = match migration::verify(&context.schemas, &source, &open, sink) {
        Ok(result) => result,
        Err(error) => return report_error(err, &error),
    };
    let (ok, value) = match &result {
        Err(rejection) => (false, rejected_json(rejection)),
        Ok((bundle, verification)) => (verification.verified, verify_json(bundle, verification)),
    };
    observe(
        sink,
        EventKind::Outcome,
        format!(
            "{COMMAND}: {}",
            value["status"].as_str().unwrap_or("rejected")
        ),
    );
    match args.format {
        OutputFormat::Json => crate::write_json_result(out, COMMAND, ok, &value),
        OutputFormat::Human => match &result {
            Err(rejection) => write_rejection_human(out, rejection),
            Ok(_) => write_verify_human(out, &value),
        },
    }
    if ok {
        exit_code::OK
    } else {
        exit_code::DOMAIN_NEGATIVE
    }
}

fn refusal_code(refusal: &RollbackRefusal) -> &'static str {
    match refusal {
        RollbackRefusal::UnknownRun { .. } => "unknown-run",
        RollbackRefusal::AlreadyRolledBack { .. } => "already-rolled-back",
        RollbackRefusal::PlanMismatch { .. } => "plan-mismatch",
        RollbackRefusal::NewerRun { .. } => "newer-run",
        RollbackRefusal::StateChanged { .. } => "state-changed",
        RollbackRefusal::CheckpointMissing { .. } => "checkpoint-missing",
        RollbackRefusal::CheckpointDigestMismatch { .. } => "checkpoint-digest-mismatch",
        RollbackRefusal::CheckpointStateMismatch { .. } => "checkpoint-state-mismatch",
        RollbackRefusal::RestoredStateMismatch { .. } => "restored-state-mismatch",
    }
}

fn run_rollback(
    args: &ParsedArgs,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    const COMMAND: &str = "migration rollback";
    let parsed = (|| {
        let kernel_path = required(args, COMMAND, "kernel")?;
        let source_path = required(args, COMMAND, "source")?;
        let db = required(args, COMMAND, "workspace-db")?;
        let run_text = required(args, COMMAND, "run")?;
        let confirm = required(args, COMMAND, "confirm")?;
        let run = MigrationRunId::parse(run_text).map_err(|_| CliError::InvalidFlagValue {
            command: COMMAND,
            flag: "run",
            value: run_text.to_string(),
            expected: "a migration run id (mr-<sequence>-<12 lowercase hex>)",
        })?;
        Ok::<_, CliError>((kernel_path, source_path, db, run, confirm))
    })();
    let (kernel_path, source_path, db, run, confirm) = match parsed {
        Ok(parsed) => parsed,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    let context = match kernel_context(kernel_path) {
        Ok(context) => context,
        Err(message) => return report_error(err, &message),
    };
    let source = match open_source(source_path) {
        Ok(source) => source,
        Err(message) => return report_error(err, &message),
    };
    observe(
        sink,
        EventKind::InputContext,
        format!("{COMMAND}: kernel={kernel_path} source={source_path} run={run}"),
    );
    let db_path = Path::new(db);
    let open = |access| open_store(db_path, DatabaseRole::Workspace, &context.edition, access);
    let outcome = match migration::rollback(&context.schemas, &source, &open, &run, confirm, sink) {
        Ok(outcome) => outcome,
        Err(error) => return report_error(err, &error),
    };
    if let RollbackOutcome::Refused(refusal) = &outcome {
        if refusal.is_checkpoint_problem() {
            observe(
                sink,
                EventKind::Outcome,
                format!("{COMMAND}: refused ({})", refusal_code(refusal)),
            );
            return report_error(err, refusal);
        }
    }
    let (ok, value) = match &outcome {
        RollbackOutcome::RolledBack(rolled) => (
            true,
            json!({
                "status": "rolled-back",
                "run_id": rolled.run.id.as_str(),
                "checkpoint_digest": digest_json(&rolled.run.checkpoint_digest),
                "restored_record_state_digest": digest_json(&rolled.restored.digest),
                "restored_revision_count": rolled.restored.revision_count,
                "reason": Value::Null,
            }),
        ),
        RollbackOutcome::ConfirmationMismatch { run, confirm } => (
            false,
            json!({
                "status": "confirmation-mismatch",
                "run_id": run,
                "checkpoint_digest": Value::Null,
                "restored_record_state_digest": Value::Null,
                "reason": format!("--confirm \"{confirm}\" does not repeat --run \"{run}\"; nothing was changed"),
            }),
        ),
        RollbackOutcome::BundleRejected(rejection) => (false, rejected_json(rejection)),
        RollbackOutcome::Refused(refusal) => (
            false,
            json!({
                "status": "refused",
                "refusal": refusal_code(refusal),
                "run_id": run.as_str(),
                "checkpoint_digest": Value::Null,
                "restored_record_state_digest": Value::Null,
                "reason": refusal.to_string(),
            }),
        ),
    };
    observe(
        sink,
        EventKind::Outcome,
        format!(
            "{COMMAND}: {}",
            value["status"].as_str().unwrap_or("rejected")
        ),
    );
    match args.format {
        OutputFormat::Json => crate::write_json_result(out, COMMAND, ok, &value),
        OutputFormat::Human => match &outcome {
            RollbackOutcome::BundleRejected(rejection) => write_rejection_human(out, rejection),
            _ => {
                let _ = writeln!(
                    out,
                    "Rollback {}: run {}",
                    value["status"].as_str().unwrap_or(""),
                    value["run_id"].as_str().unwrap_or("")
                );
                if let Some(digest) = value["restored_record_state_digest"]["value"].as_str() {
                    let _ = writeln!(out, "Restored record-state digest: {digest}");
                }
                if let Some(reason) = value["reason"].as_str() {
                    let _ = writeln!(out, "Reason: {reason}");
                }
            }
        },
    }
    if ok {
        exit_code::OK
    } else {
        exit_code::DOMAIN_NEGATIVE
    }
}
