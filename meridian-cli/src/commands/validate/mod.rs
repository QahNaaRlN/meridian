//! `meridian validate` — a Rust port of the applicable, currently-portable
//! surface of `scripts/kernel-validate.mjs`, never running Node.js and never
//! wrapping it (`meridian-cli-rfc.md`, command `validate`).
//!
//! **Ported for real, in full, against real Kernel content** (not schema
//! self-checks): `kernel-purity` (personal-path leak half),
//! `document-identity`, `duplicate-fm`, the Markdown link check, generic
//! in-gate registry `$schema` validation, the `rule-resolution` PHASE B
//! fixture check, `git-provenance`, subpackage `validate-mechanical-integrity`
//! (package 7, subpackage 7a) — `sha-provenance`, `instruction-topics`,
//! `operating-foundation`, `stack-profiles` and
//! `agent-instruction-identity` — subpackage `validate-operating-contracts`
//! (package 7, subpackage 7b) — `functional-parity`, `task-pattern-registry`,
//! `instruction-source-registry`, `task-specification-contract`,
//! `execution-state-model`, `role-and-human-control` and
//! `bounded-context-manifest` — and, as of subpackage
//! `validate-evidence-and-intake` (package 7, subpackage 7c),
//! `evidence-and-handoff-contract`, `meridian-field-evaluation`,
//! `controlled-rule-intake` and `existing-project-compatibility-mode`. Each
//! uses the exact ported strict adapters (`meridian_app::source_format`,
//! including the file-independent marked-region adapter
//! `meridian_app::source_format::regions` 7a added) or, for
//! `rule-resolution`, no composite algorithm beyond schema validation at
//! all (see `rule_resolution_fixtures`); every operating-model family from
//! 7b onward pairs a JSON Schema with a bespoke composite-consistency
//! algorithm ported to `meridian_app::operating_model`. `functional-parity`,
//! `task-pattern-registry`, `task-specification-contract` and, since
//! `rust-architecture-conformance-5`, `execution-state-model`,
//! `role-and-human-control` and `bounded-context-manifest` run as app
//! operations over [`FsWorkspaceReader`](crate::adapters::workspace_reader::FsWorkspaceReader);
//! their command modules only compose and add the family prefix. For
//! `evidence-and-handoff-contract`/`meridian-field-evaluation`/
//! `controlled-rule-intake` the file/YAML/JSON I/O and the external
//! pinned-record resolution boundary still stay in this crate's own module
//! for that family;
//! `existing-project-compatibility-mode` composes the
//! REAL `instruction-source-registry`/`controlled-rule-intake` composite
//! algorithms directly rather than a second copy of either contract's
//! shape, and its rule-candidate resolver is built from that same scan's
//! own `discovered_sources`, never a second external wiring requirement.
//! The fixed "no Instance configured" advisories every one of these
//! checks' Node counterpart already prints when `MERIDIAN_INSTANCE` is
//! unset are reproduced verbatim: this package wires no `--instance` flag
//! (deferred to package 8, `meridian-rust-migration-program-plan.md` §4).
//!
//! As of `rust-architecture-conformance-7`, the four migration and
//! qualification families of historical subpackage 7d
//! (`instance-data-migration`, `instance-canonical-export`,
//! `workspace-compatibility-qualification`,
//! `upgrade-integration-qualification`) run as app operations over the same
//! reader, after `existing-project-compatibility-mode` — the Node
//! reference's own relative order. No mandatory gate family is blocked any
//! longer, so the former `BLOCKED_CHECKS` mechanism (and its `blocked`
//! result field and `BLOCKED` human verdict) is removed: `validate` reports
//! `ok` exactly when every family it runs is clean.

mod agent_instruction_identity;
mod bounded_context_manifest;
mod controlled_rule_intake;
mod document_identity;
mod duplicate_fm;
mod evidence_and_handoff;
mod execution_state;
mod existing_project_compatibility_mode;
mod field_evaluation;
mod functional_parity;
mod git_provenance;
mod instance_canonical_export;
mod instance_context;
mod instance_data_migration;
mod instruction_source_registry;
mod instruction_topics;
mod kernel_purity;
mod link_check;
mod mechanical_integrity_boundary;
mod operating_foundation;
mod registry_schema;
mod role_and_human_control;
mod rule_resolution_fixtures;
mod sha_provenance;
mod stack_profiles;
mod task_pattern_registry;
mod task_specification;
mod upgrade_integration_qualification;
mod workspace_compatibility_qualification;

use std::io::Write;
use std::path::{Path, PathBuf};

use meridian_app::events::EventSink;
use meridian_app::validation::mechanical_integrity::OperationError;
use meridian_app::workspace::{GitInspector, GitInspectorError};
use meridian_core::types::{Diagnostic, DiagnosticLevel, NonEmptyString, WorkspaceRelativePath};
use serde_json::json;

use crate::adapters::git_inspector::{CachedGitInspector, RealGitInspector};
use crate::cli::{OutputFormat, ParsedArgs};
use crate::exit_code;
use crate::kernel::{self, WalkError};

/// Everything that can stop `collect` before it produces a `Collected`
/// result: a fail-closed directory walk failure (unchanged, pre-existing —
/// [`WalkError`]), or a port-based `mechanical_integrity` check's own
/// [`OperationError`] — an access/encoding/walk failure reading the Kernel
/// workspace through `WorkspaceReader`, distinct from that check's ordinary
/// domain diagnostics, or an unreadable current directory while resolving a
/// schema path under a relative Kernel path (`registry_schema`). All are
/// environment problems from `validate`'s own point of view: none ever
/// becomes a false "clean" or "domain-negative" result.
#[derive(Debug)]
pub enum CollectError {
    Walk(WalkError),
    Workspace(OperationError),
    CurrentDirectory(String),
}

impl std::fmt::Display for CollectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CollectError::Walk(error) => write!(f, "{error}"),
            CollectError::Workspace(error) => write!(f, "{error}"),
            CollectError::CurrentDirectory(message) => write!(
                f,
                "cannot resolve a schema path under the relative Kernel path: the current directory is unreadable: {message}"
            ),
        }
    }
}

impl std::error::Error for CollectError {}

impl From<WalkError> for CollectError {
    fn from(error: WalkError) -> Self {
        CollectError::Walk(error)
    }
}

impl From<OperationError> for CollectError {
    fn from(error: OperationError) -> Self {
        CollectError::Workspace(error)
    }
}

/// Splits a batch of [`Diagnostic`]s produced by a port-based
/// `meridian_app::validation::mechanical_integrity` check into the
/// `(failures, warnings)` string lists this module's own JSON/human
/// presentation has always used — the observable shape is unchanged, only
/// the source of the messages is now a typed [`Diagnostic`] rather than a
/// pre-formatted `String`. An `Info`-level diagnostic (none of the five 7a
/// checks currently produces one) is treated as a warning rather than
/// silently dropped.
fn split_diagnostics(diagnostics: Vec<Diagnostic>) -> (Vec<String>, Vec<String>) {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    for diagnostic in diagnostics {
        match diagnostic.level() {
            DiagnosticLevel::Fail => failures.push(diagnostic.message().to_string()),
            DiagnosticLevel::Warn | DiagnosticLevel::Info => {
                warnings.push(diagnostic.message().to_string())
            }
        }
    }
    (failures, warnings)
}

/// The ONE place either family's CLI presentation prefix
/// (`task-pattern-registry: ` / `task-specification-contract: `) is ever
/// added to a message (`rust-architecture-conformance-3` corrective round:
/// "presentation ownership"). Core and app diagnostic messages for both
/// families are prefix-free by construction, so this is an unconditional
/// prepend — never a `starts_with`/`strip_prefix` check for an
/// already-present copy, because there is never one to find: a caller
/// passes every message a family's app operation produced exactly once,
/// from exactly one call site per family.
fn with_family_prefix(family: &str, messages: Vec<String>) -> Vec<String> {
    messages
        .into_iter()
        .map(|message| format!("{family}: {message}"))
        .collect()
}

const COMMAND: &str = "validate";
pub const ALLOWED_FLAGS: &[&str] = &["kernel", "format"];

struct Collected {
    failures: Vec<String>,
    warnings: Vec<String>,
    checked_files: usize,
    identity_checked: usize,
    identity_typed_docs: usize,
    duplicate_fm_checked: usize,
    link_check_checked: usize,
    schema_validated: usize,
    schema_attempted: usize,
    rule_resolution_satisfied: usize,
    rule_resolution_rejected: usize,
    agent_instruction_identity_declared_norms: usize,
    agent_instruction_identity_undeclared_prescriptive: usize,
    agent_instruction_identity_undeclared_other: usize,
}

fn is_markdown(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

/// Resolves the ONE real Git tracked-file snapshot for the whole `validate`
/// invocation (`rust-architecture-conformance-3` corrective round, "one Git
/// snapshot for the whole `validate`"): a single `git ls-files -z` process,
/// via the same production [`GitInspector`]
/// (`crate::adapters::git_inspector::RealGitInspector`) the
/// `task-pattern-registry` family uses. [`collect`] hands the RAW resulting
/// `Result` to [`collect_with_git_result`], which never spawns `git` itself
/// and normalizes it exactly once — every consumer (`kernel-purity`,
/// `document-identity`, `task-pattern-registry`) reads that same normalized
/// typed value.
fn collect(kernel_root: &Path) -> Result<Collected, CollectError> {
    let git_snapshot = RealGitInspector::new(kernel_root).tracked_files();
    collect_with_git_result(kernel_root, git_snapshot)
}

/// `Ok` with an EMPTY tracked-file list is folded into `Unavailable` here —
/// the ONE place in `validate`'s composition that decision is made (matches
/// the Node reference's own `gitTrackedFiles`, `scripts/kernel-validate.mjs`:
/// `return files.length ? files : null`). Every downstream consumer in
/// [`collect_with_git_result`] — the filesystem-universe fallback AND
/// `task-pattern-registry`'s own tracked-set resolution, via
/// [`CachedGitInspector`] — reads the RESULT of this normalization, never
/// the raw snapshot: an empty tracked list is never treated as a trustworthy
/// "nothing under this root is tracked" answer by anything downstream
/// (`rust-architecture-conformance-3` corrective round, "one normalized Git
/// snapshot, no text-based dedup"). `Unavailable` and `InvalidPath` pass
/// through unchanged.
fn normalize_git_snapshot(
    raw: Result<Vec<WorkspaceRelativePath>, GitInspectorError>,
) -> Result<Vec<WorkspaceRelativePath>, GitInspectorError> {
    match raw {
        Ok(tracked) if tracked.is_empty() => Err(GitInspectorError::Unavailable),
        other => other,
    }
}

/// The whole `validate` composition, minus resolving the Git snapshot
/// itself — split out from [`collect`] so tests can drive every consumer of
/// that snapshot from one caller-supplied RAW `Result`, without spawning a
/// real `git` process, and so this function's own production text can be
/// asserted (structural gate below) to construct no [`RealGitInspector`] of
/// its own. [`normalize_git_snapshot`] runs first, once, on that raw input;
/// everything below reads only its normalized result.
fn collect_with_git_result(
    kernel_root: &Path,
    raw_git_snapshot: Result<Vec<WorkspaceRelativePath>, GitInspectorError>,
) -> Result<Collected, CollectError> {
    let git_snapshot = normalize_git_snapshot(raw_git_snapshot);

    let mut failures = Vec::new();
    let mut warnings = Vec::new();

    // The file universe `kernel-purity` and `document-identity` scan.
    // `InvalidPath` is a distinct data-integrity problem, not mere absence:
    // it still falls back to a filesystem walk for the file universe (there
    // is no other file list to scan with), but reports a `Fail`, not a
    // `Warn` — a snapshot Git itself could not honestly produce is never
    // presented as an ordinary "Git unavailable" outcome.
    let (files, used_git): (Vec<PathBuf>, bool) = match &git_snapshot {
        Ok(tracked) => (
            tracked
                .iter()
                .map(|p| kernel_root.join(p.as_str()))
                .collect(),
            true,
        ),
        Err(GitInspectorError::Unavailable) => {
            warnings.push(
                "kernel-purity: Git enumeration unavailable; scanning a filesystem walk instead (untracked files included)"
                    .to_string(),
            );
            (kernel::walk_all_files(kernel_root)?, false)
        }
        Err(GitInspectorError::InvalidPath { raw, reason }) => {
            failures.push(format!(
                "kernel-purity: Git reported a workspace-invalid tracked path \"{raw}\" ({reason}); the tracked-file snapshot cannot be trusted as either the Kernel file universe or task-pattern-registry's canonical-link tracked-file set"
            ));
            (kernel::walk_all_files(kernel_root)?, false)
        }
    };

    let purity = kernel_purity::run(kernel_root, &files, used_git);
    failures.extend(purity.failures);
    warnings.extend(purity.warnings);

    let markdown_files: Vec<_> = files.iter().filter(|f| is_markdown(f)).cloned().collect();

    let identity = document_identity::run(kernel_root, &files);
    failures.extend(identity.failures);

    let dup = duplicate_fm::run(&markdown_files);
    failures.extend(dup.failures);

    let links = link_check::run(kernel_root, &markdown_files);
    failures.extend(links.failures);

    let schema = registry_schema::run(kernel_root)?;
    failures.extend(schema.failures);
    warnings.extend(schema.warnings);

    let rule_resolution = rule_resolution_fixtures::run(kernel_root);
    failures.extend(rule_resolution.failures);

    let functional_parity = functional_parity::run(kernel_root);
    failures.extend(functional_parity.failures);

    let provenance = git_provenance::run(kernel_root);
    failures.extend(provenance.failures);
    warnings.extend(provenance.warnings);

    warnings.extend(instance_context::run(kernel_root)?);

    let sha_provenance = sha_provenance::run(kernel_root)?;
    failures.extend(sha_provenance.failures);
    warnings.extend(sha_provenance.warnings);

    let topics = instruction_topics::run(kernel_root)?;
    failures.extend(topics.failures);

    let foundation = operating_foundation::run(kernel_root)?;
    failures.extend(foundation.failures);

    let profiles = stack_profiles::run(kernel_root)?;
    failures.extend(profiles.failures);
    let stack_profile_pool_loaded = profiles.pool_loaded;

    let identity_norms =
        agent_instruction_identity::run(kernel_root, &markdown_files, topics.topic_pool.as_ref())?;
    failures.extend(identity_norms.failures);

    // Reuses the exact NORMALIZED snapshot resolved once above — no second
    // `git ls-files` for this family (`CachedGitInspector` never spawns a
    // process), and no false "not in the tracked file set" failures from an
    // `Ok(empty)` raw answer, since normalization already turned that into
    // `Unavailable` before this point.
    let cached_git = CachedGitInspector::new(git_snapshot.clone());
    let task_patterns = task_pattern_registry::run(kernel_root, &cached_git)?;
    failures.extend(task_patterns.failures);
    // `task_patterns.warnings` structurally never contains a second copy of
    // the "Git enumeration unavailable" fact: `meridian_app`'s own
    // `task_pattern_registry::evaluate` carries that ONE fact on a separate
    // typed `Outcome::git_unavailable` field, kept out of its generic
    // `diagnostics`/`warnings` — nothing here has to scan message text to
    // find and drop a duplicate (`rust-architecture-conformance-3`
    // corrective round, "no text-based dedup").
    warnings.extend(task_patterns.warnings);

    let source_registry = instruction_source_registry::run(kernel_root);
    failures.extend(source_registry.failures);

    let task_specification = task_specification::run(kernel_root, task_patterns.catalog.as_ref())?;
    failures.extend(task_specification.failures);

    let execution_state = execution_state::run(kernel_root);
    failures.extend(execution_state.failures);

    let role_and_human_control = role_and_human_control::run(kernel_root);
    failures.extend(role_and_human_control.failures);

    let bounded_context_manifest = bounded_context_manifest::run(kernel_root);
    failures.extend(bounded_context_manifest.failures);

    let evidence_and_handoff = evidence_and_handoff::run(kernel_root);
    failures.extend(evidence_and_handoff.failures);

    let field_evaluation = field_evaluation::run(kernel_root);
    failures.extend(field_evaluation.failures);

    let controlled_rule_intake = controlled_rule_intake::run(kernel_root);
    failures.extend(controlled_rule_intake.failures);

    let existing_project_compatibility_mode = existing_project_compatibility_mode::run(kernel_root);
    failures.extend(existing_project_compatibility_mode.failures);

    let instance_data_migration = instance_data_migration::run(kernel_root);
    failures.extend(instance_data_migration.failures);

    let instance_canonical_export = instance_canonical_export::run(kernel_root);
    failures.extend(instance_canonical_export.failures);

    let workspace_compatibility_qualification =
        workspace_compatibility_qualification::run(kernel_root);
    failures.extend(workspace_compatibility_qualification.failures);

    let upgrade_integration_qualification =
        upgrade_integration_qualification::run(kernel_root, task_patterns.catalog.as_ref());
    failures.extend(upgrade_integration_qualification.failures);

    warnings.push(
        "front-matter/path-placement: no Instance root, working-memory artifacts were NOT checked"
            .to_string(),
    );
    warnings.push("ext-dependencies: no Instance root, not checked".to_string());
    warnings.push("inventory-git: no Instance root, not checked".to_string());
    // Mirrors the Node reference's `else if (STACK_PROFILES) warn(...)`
    // branch: the per-repository "declarations were not checked" advisory
    // is reached only when the pool itself parsed and agreed — a pool that
    // failed to load already reported its own failure above and must not
    // also claim declarations were merely "not checked".
    if stack_profile_pool_loaded {
        warnings.push("stack-profile: no Instance root, declarations were not checked".to_string());
    }
    warnings.push("instruction-intake: no Instance root, not checked".to_string());

    Ok(Collected {
        failures,
        warnings,
        checked_files: files.len(),
        identity_checked: identity.checked,
        identity_typed_docs: identity.typed_docs,
        duplicate_fm_checked: dup.checked,
        link_check_checked: links.checked,
        schema_validated: schema.validated,
        schema_attempted: schema.attempted,
        rule_resolution_satisfied: rule_resolution.satisfied,
        rule_resolution_rejected: rule_resolution.rejected,
        agent_instruction_identity_declared_norms: identity_norms.declared_norms,
        agent_instruction_identity_undeclared_prescriptive: identity_norms.undeclared_prescriptive,
        agent_instruction_identity_undeclared_other: identity_norms.undeclared_other,
    })
}

pub fn run(
    args: &ParsedArgs,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    let kernel_path = match args.require(COMMAND, "kernel") {
        Ok(value) => value,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    let kernel_root = Path::new(kernel_path);
    if !kernel_root.is_dir() {
        return crate::report_environment_error(
            err,
            &format!("Kernel path {kernel_path} is not a directory"),
        );
    }

    sink.record(&meridian_app::events::ObservedEvent::new(
        meridian_app::events::EventKind::InputContext,
        NonEmptyString::new(format!("validate: kernel={kernel_path}")).unwrap(),
    ));

    let collected = match collect(kernel_root) {
        Ok(collected) => collected,
        Err(error) => return crate::report_environment_error(err, &error),
    };

    let ok = collected.failures.is_empty();
    let result = json!({
        "kernel": kernel_path,
        "checked_files": collected.checked_files,
        "failures": collected.failures,
        "warnings": collected.warnings,
        "ok": ok,
        "stats": {
            "document_identity_checked": collected.identity_checked,
            "document_identity_typed_docs": collected.identity_typed_docs,
            "duplicate_fm_checked": collected.duplicate_fm_checked,
            "link_check_checked": collected.link_check_checked,
            "schema_validated": collected.schema_validated,
            "schema_attempted": collected.schema_attempted,
            "rule_resolution_satisfied": collected.rule_resolution_satisfied,
            "rule_resolution_rejected": collected.rule_resolution_rejected,
            "agent_instruction_identity_declared_norms": collected.agent_instruction_identity_declared_norms,
            "agent_instruction_identity_undeclared_prescriptive": collected.agent_instruction_identity_undeclared_prescriptive,
            "agent_instruction_identity_undeclared_other": collected.agent_instruction_identity_undeclared_other,
        },
    });

    sink.record(&meridian_app::events::ObservedEvent::new(
        meridian_app::events::EventKind::Outcome,
        NonEmptyString::new(if ok {
            "validate: pass"
        } else {
            "validate: fail"
        })
        .unwrap(),
    ));

    match args.format {
        OutputFormat::Json => crate::write_json_result(out, COMMAND, ok, &result),
        OutputFormat::Human => {
            let _ = writeln!(out, "Checked {} file(s)", collected.checked_files);
            for failure in &collected.failures {
                let _ = writeln!(out, "FAIL  {failure}");
            }
            for warning in &collected.warnings {
                let _ = writeln!(out, "WARN  {warning}");
            }
            let _ = writeln!(
                out,
                "{} failure(s), {} warning(s)",
                collected.failures.len(),
                collected.warnings.len()
            );
            let verdict = if ok { "OK" } else { "FAIL" };
            let _ = writeln!(out, "{verdict}");
        }
    }

    if ok {
        exit_code::OK
    } else {
        exit_code::DOMAIN_NEGATIVE
    }
}

/// Exposed for the real-producer conformance harness
/// (`verification/conformance-harness/`) and for tests: builds the same
/// failure/warning lists [`run`] does, without any CLI/JSON/human framing —
/// a real caller of the same logic the shipped command uses, not a second
/// implementation.
pub fn collect_diagnostics(kernel_root: &Path) -> Result<(Vec<String>, Vec<String>), CollectError> {
    let collected = collect(kernel_root)?;
    Ok((collected.failures, collected.warnings))
}

/// Structural gates for `rust-architecture-conformance-3`
/// (`governance/plans/meridian-rust-migration-program-plan.md` §5.17.3,
/// point 10): the two `task-pattern-registry`/`task-specification-contract`
/// CLI command modules stay thin composition/presentation shims — no
/// concrete filesystem I/O, no `serde_json::Value`, no schema navigation,
/// no domain rules and no reintroduced `evaluate_*(&Value, ...)`
/// production entrypoint for either family.
#[cfg(test)]
mod rust_architecture_conformance_3_structural_gates {
    fn read(rel: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
    }

    fn production_text(rel: &str) -> String {
        let text = read(rel);
        let production_end = text.find("#[cfg(test)]").unwrap_or(text.len());
        text[..production_end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_two_command_modules_carry_no_direct_io_value_or_domain_rules() {
        for rel in [
            "src/commands/validate/task_pattern_registry.rs",
            "src/commands/validate/task_specification.rs",
        ] {
            let production = production_text(rel);
            for forbidden in [
                "std::fs",
                "std::process",
                "serde_json::Value",
                "json_schema::",
                "parse_yaml(",
                "evaluate_task_pattern_registry(&",
                "evaluate_task_specification(&",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{rel} contains \"{forbidden}\" — this command module must stay a thin composition/presentation shim over meridian-app's own port-based orchestration"
                );
            }
        }
    }

    #[test]
    fn the_two_command_modules_only_compose_the_real_app_orchestration_and_adapters() {
        let expectations: &[(&str, &[&str])] = &[
            (
                "src/commands/validate/task_pattern_registry.rs",
                &[
                    "app::evaluate(",
                    "FsWorkspaceReader::new(",
                    "FsLinkTarget::new(",
                ],
            ),
            (
                "src/commands/validate/task_specification.rs",
                &["app::evaluate(", "FsWorkspaceReader::new("],
            ),
        ];
        for (rel, calls) in expectations {
            let text = read(rel);
            for call in *calls {
                assert!(
                    text.contains(call),
                    "{rel} must call `{call}` — it must delegate to the real production route, not reimplement it"
                );
            }
        }
    }

    /// One real Git snapshot for the whole `validate` invocation
    /// (corrective round item 3): `task_pattern_registry.rs` must receive
    /// its `GitInspector` from the caller — it must never construct its own
    /// `RealGitInspector` and spawn a second `git ls-files`.
    #[test]
    fn the_registry_command_module_never_constructs_its_own_git_inspector() {
        let production = production_text("src/commands/validate/task_pattern_registry.rs");
        assert!(
            !production.contains("RealGitInspector"),
            "task_pattern_registry.rs must receive `git: &dyn GitInspector` from its caller, not construct RealGitInspector itself — that would spawn a second real `git ls-files` for the same validate invocation"
        );
    }

    /// The composition root (`collect`) is the ONE place in this crate that
    /// resolves the real Git snapshot; `collect_with_git_result` — which
    /// does everything else `collect` does — must never construct its own
    /// `RealGitInspector` either, so every test driving it through a
    /// caller-supplied `Result` genuinely exercises "no second Git call".
    #[test]
    fn real_git_inspector_is_constructed_exactly_once_in_this_module() {
        let production = production_text("src/commands/validate/mod.rs");
        let occurrences = production.matches("RealGitInspector::new(").count();
        assert_eq!(
            occurrences, 1,
            "expected exactly one `RealGitInspector::new(` in validate/mod.rs (inside `collect`); found {occurrences}"
        );
    }
}

/// Composition-level coverage for `rust-architecture-conformance-3`
/// corrective round item 3: one Git snapshot, shared by `kernel-purity`
/// (via the filesystem-universe fallback) and `task-pattern-registry` (via
/// `CachedGitInspector`), for the whole `validate` invocation — driven
/// through [`collect_with_git_result`] so no test here spawns a real `git`
/// process of its own.
#[cfg(test)]
mod rust_architecture_conformance_3_single_git_snapshot {
    use super::*;

    fn kernel_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    /// A fake `GitInspector` that counts how many times it is asked for the
    /// tracked-file set — used to prove `CachedGitInspector` (the real
    /// production wrapper `collect` hands to `task_pattern_registry::run`)
    /// only ever reads the ALREADY-resolved snapshot, never triggers a
    /// second underlying lookup of its own.
    struct CountingGit {
        calls: std::cell::Cell<u32>,
        result: Result<Vec<WorkspaceRelativePath>, GitInspectorError>,
    }
    impl GitInspector for CountingGit {
        fn tracked_files(&self) -> Result<Vec<WorkspaceRelativePath>, GitInspectorError> {
            self.calls.set(self.calls.get() + 1);
            self.result.clone()
        }
    }

    /// Reuses ONE real snapshot (fetched here, directly, exactly once — not
    /// through `collect`) for both `collect_with_git_result`'s own file
    /// universe and, through `CachedGitInspector`, `task-pattern-registry`'s
    /// tracked-set resolution: proves the two no longer each spawn their own
    /// `git ls-files` for a real, valid snapshot.
    #[test]
    fn a_valid_real_snapshot_is_reused_by_every_consumer_without_a_second_lookup() {
        let root = kernel_root();
        let real_snapshot = RealGitInspector::new(&root).tracked_files();
        assert!(real_snapshot.is_ok(), "{real_snapshot:?}");

        let counting = CountingGit {
            calls: std::cell::Cell::new(0),
            result: real_snapshot.clone(),
        };
        // `task_pattern_registry::run` takes its `git` port from the
        // caller; feeding it a counting fake here (standing in for what
        // `collect_with_git_result` does with `CachedGitInspector` in
        // production) proves the registry route reads the snapshot exactly
        // once, matching `CachedGitInspector::tracked_files`'s own
        // zero-process-spawn behaviour.
        let outcome = super::task_pattern_registry::run(&root, &counting).unwrap();
        assert_eq!(
            counting.calls.get(),
            1,
            "task_pattern_registry::run must read the shared git port exactly once"
        );
        assert!(outcome.catalog.is_some(), "{:?}", outcome.failures);

        let collected = collect_with_git_result(&root, real_snapshot).unwrap();
        assert!(
            collected
                .warnings
                .iter()
                .all(|w| !w.contains("Git enumeration unavailable")),
            "a valid, non-empty real snapshot must never produce a Git-unavailable warning: {:?}",
            collected.warnings
        );
    }

    /// `Unavailable` produces exactly the one existing `kernel-purity`
    /// fallback warning — `task-pattern-registry`'s own copy of the same
    /// root cause is not additionally surfaced.
    #[test]
    fn unavailable_snapshot_yields_exactly_one_git_enumeration_warning() {
        let root = kernel_root();
        let collected =
            collect_with_git_result(&root, Err(GitInspectorError::Unavailable)).unwrap();
        let git_warnings: Vec<&String> = collected
            .warnings
            .iter()
            .filter(|w| w.contains("Git enumeration unavailable"))
            .collect();
        assert_eq!(
            git_warnings.len(),
            1,
            "expected exactly one Git-enumeration-unavailable warning, found {git_warnings:?}"
        );
        assert_eq!(
            git_warnings[0],
            "kernel-purity: Git enumeration unavailable; scanning a filesystem walk instead (untracked files included)"
        );
    }

    /// Corrective round item 1/4: `Ok(vec![])` — Git ran successfully but
    /// reports nothing tracked — is normalized to the SAME contract as
    /// `Unavailable`, not trusted as "nothing under this root is tracked":
    /// filesystem fallback, exactly the one existing warning, and no false
    /// "not in the tracked file set" rejection of `task-pattern-registry`'s
    /// own present canonical links (which an un-normalized `Ok(empty)`
    /// would previously have produced, since `resolve_tracked_set` treats
    /// any `Ok` — even empty — as a trustworthy, if empty, tracked set).
    #[test]
    fn an_empty_ok_snapshot_is_normalized_to_the_same_contract_as_unavailable() {
        let root = kernel_root();
        let collected = collect_with_git_result(&root, Ok(Vec::new())).unwrap();

        let git_warnings: Vec<&String> = collected
            .warnings
            .iter()
            .filter(|w| w.contains("Git enumeration unavailable"))
            .collect();
        assert_eq!(
            git_warnings.len(),
            1,
            "expected exactly one Git-enumeration-unavailable warning for Ok(empty), found {git_warnings:?}"
        );
        assert_eq!(
            git_warnings[0],
            "kernel-purity: Git enumeration unavailable; scanning a filesystem walk instead (untracked files included)"
        );

        // Filesystem fallback: `checked_files` must match a real filesystem
        // walk, not a (bogus) zero-tracked-file count.
        let walked = kernel::walk_all_files(&root).unwrap();
        assert_eq!(collected.checked_files, walked.len());

        // No false "not in the tracked file set" rejection, and the
        // registry must still build its catalog via the filesystem-
        // existence fallback, exactly as a genuine `Unavailable` does.
        assert!(
            !collected
                .failures
                .iter()
                .any(|f| f.contains("not in the Kernel's tracked file set")),
            "an Ok(empty) raw snapshot must never cause a false untracked-link failure: {:?}",
            collected.failures
        );
        assert!(
            !collected
                .failures
                .iter()
                .any(|f| f.contains("no task-pattern catalog is available")),
            "an Ok(empty) raw snapshot must still let task-pattern-registry build its catalog via the filesystem-existence fallback: {:?}",
            collected.failures
        );
    }

    /// `InvalidPath` fails closed: a `Fail` diagnostic is reported, and
    /// `task-pattern-registry` never treats the untrustworthy snapshot as a
    /// trusted tracked set — its catalog becomes `None`, which propagates
    /// as `task-specification-contract`'s own "no catalog available"
    /// failure, exactly like any other registry-side rejection.
    #[test]
    fn invalid_path_snapshot_fails_closed_with_no_catalog() {
        let root = kernel_root();
        let err = GitInspectorError::InvalidPath {
            raw: "../escape".to_string(),
            reason: "workspace path must be relative, not absolute or drive-prefixed".to_string(),
        };
        let collected = collect_with_git_result(&root, Err(err)).unwrap();
        assert!(
            collected
                .failures
                .iter()
                .any(|f| f.contains("workspace-invalid tracked path")),
            "{:?}",
            collected.failures
        );
        assert!(
            collected
                .failures
                .iter()
                .any(|f| f.contains("no task-pattern catalog is available")),
            "an untrustworthy Git snapshot must leave task-pattern-registry's catalog None, so task-specification-contract reports its own \"no catalog\" failure: {:?}",
            collected.failures
        );
        assert!(
            !collected
                .warnings
                .iter()
                .any(|w| w.contains("Git enumeration unavailable")),
            "InvalidPath must never be reported as mere unavailability: {:?}",
            collected.warnings
        );
    }
}

/// Gates for `rust-architecture-conformance-5`
/// (`governance/plans/meridian-rust-migration-program-plan.md` §5.19):
/// the three run-contract command modules are thin composition/presentation
/// shims over `meridian-app` and the old `Value` entrypoints are gone. The
/// temporary 7c facade this package introduced was deleted by
/// `rust-architecture-conformance-6`.
#[cfg(test)]
mod rust_architecture_conformance_5 {
    use std::path::{Path, PathBuf};

    const MODULES: [(&str, &str); 3] = [
        (
            "src/commands/validate/execution_state.rs",
            "execution-state-model",
        ),
        (
            "src/commands/validate/role_and_human_control.rs",
            "role-and-human-control",
        ),
        (
            "src/commands/validate/bounded_context_manifest.rs",
            "bounded-context-manifest",
        ),
    ];

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn production_text(path: &Path) -> String {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()));
        let end = text.find("#[cfg(test)]").unwrap_or(text.len());
        text[..end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    fn production_sources() -> Vec<(String, String)> {
        let root = workspace_root();
        let mut files = Vec::new();
        for krate in ["meridian-app/src", "meridian-cli/src", "meridian-core/src"] {
            rust_files(&root.join(krate), &mut files);
        }
        files
            .into_iter()
            .map(|p| {
                let rel = p
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                (rel, production_text(&p))
            })
            .collect()
    }

    #[test]
    fn the_three_command_modules_carry_no_direct_io_value_or_domain_rules() {
        for (rel, _) in MODULES {
            let production = production_text(&Path::new(env!("CARGO_MANIFEST_DIR")).join(rel));
            for forbidden in [
                "std::fs",
                "std::process",
                "serde_json",
                "json_schema",
                "parse_yaml",
                "make_record_resolver",
                "compat_7c",
                "run_contracts",
                "evaluate_",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{rel} contains \"{forbidden}\" — it must stay a thin composition/presentation shim"
                );
            }
            for required in [
                "app::evaluate(",
                "FsWorkspaceReader::new(",
                "with_family_prefix(FAMILY",
            ] {
                assert!(
                    production.contains(required),
                    "{rel} must call `{required}`"
                );
            }
        }
    }

    /// The pre-package `Value`/`Vec<String>` entrypoints of the three
    /// families no longer exist anywhere in production code.
    #[test]
    fn the_legacy_value_entrypoints_are_gone() {
        for (rel, text) in production_sources() {
            for legacy in [
                "evaluate_execution_state",
                "evaluate_role_registry",
                "evaluate_human_control",
                "evaluate_context_manifest",
                "RoleRegistrySchemas",
                "HumanControlSchemas",
            ] {
                assert!(
                    !text.contains(legacy),
                    "{rel} still names the legacy entrypoint `{legacy}`"
                );
            }
        }
    }

    /// `rust-architecture-conformance-6` deleted the temporary 7c facade
    /// this package introduced; it never comes back
    /// (`rust_architecture_conformance_6` holds the full gate).
    #[test]
    fn the_7c_facade_is_gone() {
        assert!(!workspace_root()
            .join("meridian-app/src/operating_model/bounded_context_manifest/compat_7c.rs")
            .exists());
        for (rel, text) in production_sources() {
            assert!(!text.contains("compat_7c"), "{rel} names compat_7c");
        }
    }

    #[test]
    fn the_real_adapter_route_is_clean_for_all_three_families() {
        let root = workspace_root();
        assert!(super::execution_state::run(&root).failures.is_empty());
        assert!(super::role_and_human_control::run(&root)
            .failures
            .is_empty());
        assert!(super::bounded_context_manifest::run(&root)
            .failures
            .is_empty());
    }

    struct TempRoot(PathBuf);
    impl TempRoot {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "rust-architecture-conformance-5-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn run_family(family: &str, root: &Path) -> Vec<String> {
        match family {
            "execution-state-model" => super::execution_state::run(root).failures,
            "role-and-human-control" => super::role_and_human_control::run(root).failures,
            _ => super::bounded_context_manifest::run(root).failures,
        }
    }

    /// An app-generated failure (every mandatory file absent) carries
    /// EXACTLY ONE family prefix.
    #[test]
    fn an_app_generated_failure_carries_exactly_one_family_prefix() {
        let empty = TempRoot::new("empty");
        for (_, family) in MODULES {
            let failures = run_family(family, &empty.0);
            assert_eq!(failures.len(), 1, "{failures:?}");
            let prefix = format!("{family}: ");
            assert!(failures[0].starts_with(&prefix), "{}", failures[0]);
            assert_eq!(failures[0].matches(&prefix).count(), 1, "{}", failures[0]);
            assert!(failures[0].contains(" is missing; "), "{}", failures[0]);
        }
    }

    /// Through the REAL `FsWorkspaceReader`, a directory standing where a
    /// mandatory file is expected is `ReadError::Io`, never "missing".
    #[test]
    fn a_directory_in_place_of_a_mandatory_file_is_unreadable_not_missing() {
        let cases = [
            (
                "execution-state-model",
                "registries/operating-model/execution-state.schema.json",
            ),
            (
                "role-and-human-control",
                "standards/workspace/role-registry.yaml",
            ),
            (
                "bounded-context-manifest",
                "registries/operating-model/fixtures/context-manifest.fixtures.json",
            ),
        ];
        let tree = TempRoot::new("directory");
        let real = workspace_root();
        for rel in [
            "registries/operating-model/execution-state.schema.json",
            "registries/operating-model/role-registry.schema.json",
            "registries/operating-model/human-control.schema.json",
            "registries/operating-model/context-manifest.schema.json",
            "registries/operating-model/scoped-record.schema.json",
            "registries/operating-model/fixtures/execution-state.fixtures.json",
            "registries/operating-model/fixtures/role-and-human-control.fixtures.json",
            "registries/operating-model/fixtures/context-manifest.fixtures.json",
            "standards/workspace/role-registry.yaml",
        ] {
            let target = tree.0.join(rel);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            if cases.iter().any(|(_, path)| *path == rel) {
                std::fs::create_dir_all(&target).unwrap();
            } else {
                std::fs::copy(real.join(rel), &target).unwrap();
            }
        }
        for (family, rel) in cases {
            let failures = run_family(family, &tree.0);
            assert_eq!(failures.len(), 1, "{family}: {failures:?}");
            assert!(
                failures[0].starts_with(&format!("{family}: {rel} could not be read: ")),
                "{}",
                failures[0]
            );
            assert!(!failures[0].contains("is missing"), "{}", failures[0]);
            assert!(
                !failures[0].contains("carries no fixtures"),
                "{}",
                failures[0]
            );
        }
    }
}

/// Gates for `rust-architecture-conformance-6`
/// (`governance/plans/meridian-rust-migration-program-plan.md` §5.20): the
/// evidence-and-handoff and field-evaluation command modules are thin
/// composition/presentation shims, the old `Value` entrypoints and both
/// temporary facades are gone for good, the core stays free of I/O and the
/// app free of the concrete adapter, and each family's prefix is applied
/// exactly once over the real adapter.
#[cfg(test)]
mod rust_architecture_conformance_6 {
    use std::path::{Path, PathBuf};

    const MODULES: [(&str, &str); 2] = [
        (
            "src/commands/validate/evidence_and_handoff.rs",
            "evidence-and-handoff-contract",
        ),
        (
            "src/commands/validate/field_evaluation.rs",
            "meridian-field-evaluation",
        ),
    ];

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn production_text(path: &Path) -> String {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()));
        // A module file that declares itself test-only (`#![cfg(test)]` as
        // its first code line) has no production part; nothing is exempted
        // by its file name.
        let first_code_line = text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty() && !line.starts_with("//"));
        if first_code_line == Some("#![cfg(test)]") {
            return String::new();
        }
        let end = text.find("#[cfg(test)]").unwrap_or(text.len());
        text[..end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    fn sources(krate: &str) -> Vec<(String, String)> {
        let root = workspace_root();
        let mut files = Vec::new();
        rust_files(&root.join(krate), &mut files);
        files
            .into_iter()
            .map(|p| {
                let rel = p
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                (rel, production_text(&p))
            })
            .collect()
    }

    #[test]
    fn evidence_and_handoff_and_field_evaluation_command_modules_are_thin_shims() {
        for (rel, _) in MODULES {
            let production = production_text(&Path::new(env!("CARGO_MANIFEST_DIR")).join(rel));
            for forbidden in [
                "std::fs",
                "std::process",
                "serde_json",
                "Value",
                "json_schema",
                "fixtures",
                "resolution",
                "make_record_resolver",
                "compat_7c",
                "meridian_core",
                "evaluate_",
                "unwrap",
                "expect(",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{rel} contains \"{forbidden}\" — it must stay a thin composition/presentation shim"
                );
            }
            for required in [
                "app::evaluate(",
                "FsWorkspaceReader::new(",
                "with_family_prefix(FAMILY",
            ] {
                assert!(
                    production.contains(required),
                    "{rel} must call `{required}`"
                );
            }
        }
    }

    /// The pre-package `Value` entrypoints, resolver closures and both
    /// temporary facades exist nowhere in production code.
    #[test]
    fn evidence_and_handoff_and_field_evaluation_legacy_entrypoints_and_facades_are_gone() {
        let mut all = sources("meridian-app/src");
        all.extend(sources("meridian-cli/src"));
        all.extend(sources("meridian-core/src"));
        for (rel, text) in all {
            for legacy in [
                "evaluate_evidence_and_handoff",
                "evaluate_field_evaluation",
                "evidence_and_handoff::EvalSchemas",
                "field_evaluation::EvalOpts",
                "RecordResolver",
                "make_record_resolver",
                "make_evidence_resolver",
                "compat_7c",
                "task_specification::{non_portable_reason",
                "task_specification::non_portable_reason",
                "task_specification::resolve_schema_ref",
            ] {
                assert!(
                    !text.contains(legacy),
                    "{rel} still names the legacy item `{legacy}`"
                );
            }
        }
    }

    /// The two families' core owners carry no transport or I/O; the app
    /// never names the concrete adapter.
    #[test]
    fn evidence_and_field_evaluation_core_is_pure_and_the_app_is_adapter_free() {
        for (rel, text) in sources("meridian-core/src")
            .into_iter()
            .filter(|(rel, _)| rel.contains("/evidence/") || rel.contains("/field_evaluation/"))
        {
            for forbidden in [
                "serde",
                "Value",
                "std::fs",
                "std::io",
                "std::env",
                "std::process",
                "println!",
            ] {
                assert!(!text.contains(forbidden), "{rel} contains \"{forbidden}\"");
            }
        }
        for (rel, text) in sources("meridian-app/src").into_iter().filter(|(rel, _)| {
            rel.contains("/evidence_and_handoff/")
                || rel.contains("/field_evaluation/")
                || rel.contains("/record_resolution")
                || rel.contains("/run_contract_boundary/")
        }) {
            for forbidden in [
                "FsWorkspaceReader",
                "std::fs",
                "std::process",
                "HashMap",
                "HashSet",
            ] {
                assert!(!text.contains(forbidden), "{rel} contains \"{forbidden}\"");
            }
        }
    }

    #[test]
    fn evidence_and_handoff_and_field_evaluation_real_adapter_route_is_clean() {
        let root = workspace_root();
        assert_eq!(
            super::evidence_and_handoff::run(&root).failures,
            Vec::<String>::new()
        );
        assert_eq!(
            super::field_evaluation::run(&root).failures,
            Vec::<String>::new()
        );
    }

    struct TempRoot(PathBuf);
    impl TempRoot {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "rust-architecture-conformance-6-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn run_family(family: &str, root: &Path) -> Vec<String> {
        match family {
            "evidence-and-handoff-contract" => super::evidence_and_handoff::run(root).failures,
            _ => super::field_evaluation::run(root).failures,
        }
    }

    /// An app-generated failure (every mandatory file absent) carries
    /// EXACTLY ONE family prefix.
    #[test]
    fn evidence_and_handoff_and_field_evaluation_failures_carry_exactly_one_prefix() {
        let empty = TempRoot::new("empty");
        for (_, family) in MODULES {
            let failures = run_family(family, &empty.0);
            assert_eq!(failures.len(), 1, "{failures:?}");
            let prefix = format!("{family}: ");
            assert!(failures[0].starts_with(&prefix), "{}", failures[0]);
            assert_eq!(failures[0].matches(&prefix).count(), 1, "{}", failures[0]);
            assert!(failures[0].contains(" is missing; "), "{}", failures[0]);
        }
    }

    /// Through the REAL `FsWorkspaceReader`, a directory standing where a
    /// mandatory file is expected is `ReadError::Io`, never "missing" —
    /// for the schema of one family and the fixture bundle of the other.
    #[test]
    fn evidence_and_handoff_and_field_evaluation_unreadable_is_not_missing() {
        let cases = [
            (
                "evidence-and-handoff-contract",
                "registries/operating-model/fixtures/evidence-and-handoff.fixtures.json",
            ),
            (
                "meridian-field-evaluation",
                "registries/operating-model/field-evaluation.schema.json",
            ),
        ];
        let tree = TempRoot::new("directory");
        let real = workspace_root();
        for rel in [
            "registries/operating-model/evidence-and-handoff.schema.json",
            "registries/operating-model/field-evaluation.schema.json",
            "registries/operating-model/scoped-record.schema.json",
            "registries/operating-model/fixtures/evidence-and-handoff.fixtures.json",
            "registries/operating-model/fixtures/field-evaluation.fixtures.json",
        ] {
            let target = tree.0.join(rel);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            if cases.iter().any(|(_, path)| *path == rel) {
                std::fs::create_dir_all(&target).unwrap();
            } else {
                std::fs::copy(real.join(rel), &target).unwrap();
            }
        }
        for (family, rel) in cases {
            let failures = run_family(family, &tree.0);
            assert_eq!(failures.len(), 1, "{family}: {failures:?}");
            assert!(
                failures[0].starts_with(&format!("{family}: {rel} could not be read: ")),
                "{}",
                failures[0]
            );
            assert!(!failures[0].contains("is missing"), "{}", failures[0]);
            assert!(
                !failures[0].contains("carries no fixtures"),
                "{}",
                failures[0]
            );
        }
    }
}

/// Structural and route gates of `rust-architecture-conformance-7`.
#[cfg(test)]
mod rust_architecture_conformance_7;
