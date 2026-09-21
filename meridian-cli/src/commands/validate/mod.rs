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
//! `agent-instruction-identity` — and, as of subpackage
//! `validate-operating-contracts` (package 7, subpackage 7b),
//! `functional-parity`, `task-pattern-registry`,
//! `instruction-source-registry`, `task-specification-contract`,
//! `execution-state-model`, `role-and-human-control` and
//! `bounded-context-manifest`. Each uses the exact ported strict adapters
//! (`meridian_app::source_format`, including the file-independent
//! marked-region adapter `meridian_app::source_format::regions` 7a added)
//! or, for `rule-resolution`, no composite algorithm beyond schema
//! validation at all (see `rule_resolution_fixtures`); 7b's seven families
//! each pair a JSON Schema with a bespoke composite-consistency algorithm
//! ported to `meridian_app::operating_model` — pure functions over
//! already-parsed values, with the file/YAML/JSON I/O and, for
//! `bounded-context-manifest`, the external pinned-record resolution
//! boundary, staying in this crate's own module for that family. The fixed
//! "no Instance configured" advisories every one of these checks' Node
//! counterpart already prints when `MERIDIAN_INSTANCE` is unset are
//! reproduced verbatim: this package wires no `--instance` flag (deferred
//! to package 8, `meridian-rust-migration-program-plan.md` §4).
//!
//! **Not yet ported — explicit, itemised `blocked` list, never silently
//! passed and never silently failed** ([`BLOCKED_CHECKS`]): the 8 remaining
//! operating-model composite contracts (`evidence-and-handoff-contract`,
//! `meridian-field-evaluation`, `controlled-rule-intake`,
//! `existing-project-compatibility-mode`, `instance-data-migration`,
//! `instance-canonical-export`, `workspace-compatibility-qualification`,
//! `upgrade-integration-qualification`) each pair a JSON Schema with a
//! bespoke composite-consistency algorithm from its own
//! `scripts/lib/*.mjs` module; two of those pure algorithms already exist in
//! `meridian-core` (`migration::checks`, `evidence::aggregate`) but are not
//! yet wired to real fixture files by this CLI, and the remaining six have
//! no Rust port at all. The owner has decided the split and order
//! (`governance/plans/meridian-rust-migration-program-plan.md` §5.5c):
//! subpackage 7b is implemented by this module (this doc comment's own
//! record); 7c (`evidence-and-handoff-contract`,
//! `meridian-field-evaluation`, `controlled-rule-intake`,
//! `existing-project-compatibility-mode`), then 7d
//! (`instance-data-migration`, `instance-canonical-export`,
//! `workspace-compatibility-qualification`,
//! `upgrade-integration-qualification`) — implemented and accepted in that
//! order, neither started by this module. Package 8
//! (`meridian-cli-migration`) stays blocked on the acceptance and
//! integration of all of 7a–7d, not only 7a/7b.

mod agent_instruction_identity;
mod bounded_context_manifest;
mod document_identity;
mod duplicate_fm;
mod execution_state;
mod functional_parity;
mod git_provenance;
mod instance_context;
mod instruction_source_registry;
mod instruction_topics;
mod kernel_purity;
mod link_check;
mod operating_foundation;
mod registry_schema;
mod role_and_human_control;
mod rule_resolution_fixtures;
mod sha_provenance;
mod stack_profiles;
mod task_pattern_registry;
mod task_specification;

use std::io::Write;
use std::path::Path;

use meridian_app::events::EventSink;
use meridian_core::types::NonEmptyString;
use serde_json::{json, Value};

use crate::cli::{OutputFormat, ParsedArgs};
use crate::exit_code;
use crate::kernel::{self, WalkError};

const COMMAND: &str = "validate";
pub const ALLOWED_FLAGS: &[&str] = &["kernel", "format"];

/// See the module documentation above for why each of these is not yet
/// implemented, and what porting it would require.
pub const BLOCKED_CHECKS: &[(&str, &str)] = &[
    ("evidence-and-handoff-contract", "meridian-core::evidence::aggregate ports the pure verdict algorithm, but the file-facing fixture harness is not yet wired into this CLI"),
    ("meridian-field-evaluation", "needs scripts/lib/field-evaluation.mjs's composite algorithm ported"),
    ("controlled-rule-intake", "needs scripts/lib/controlled-rule-intake.mjs's composite algorithm ported"),
    ("existing-project-compatibility-mode", "needs scripts/lib/existing-project-compatibility-mode.mjs's composite algorithm ported"),
    ("instance-data-migration", "meridian-core::migration::checks ports the pure plan-check algorithm, but the file-facing fixture harness is not yet wired into this CLI"),
    ("instance-canonical-export", "needs scripts/lib/instance-data-migration.mjs's export-side composite algorithm ported"),
    ("workspace-compatibility-qualification", "needs scripts/lib/workspace-compatibility-qualification.mjs's composite algorithm ported"),
    ("upgrade-integration-qualification", "needs scripts/lib/upgrade-integration-qualification.mjs's composite algorithm ported"),
];

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

fn collect(kernel_root: &Path) -> Result<Collected, WalkError> {
    let (files, used_git, fallback_warn) = match kernel::list_git_tracked_files(kernel_root) {
        Some(files) => (files, true, None),
        None => {
            let files = kernel::walk_all_files(kernel_root)?;
            (
                files,
                false,
                Some(
                    "kernel-purity: Git enumeration unavailable; scanning a filesystem walk instead (untracked files included)"
                        .to_string(),
                ),
            )
        }
    };

    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    if let Some(warning) = fallback_warn {
        warnings.push(warning);
    }

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

    let sha_provenance = sha_provenance::run(kernel_root);
    failures.extend(sha_provenance.failures);
    warnings.extend(sha_provenance.warnings);

    let topics = instruction_topics::run(kernel_root);
    failures.extend(topics.failures);

    let foundation = operating_foundation::run(kernel_root);
    failures.extend(foundation.failures);

    let profiles = stack_profiles::run(kernel_root);
    failures.extend(profiles.failures);
    let stack_profile_pool_loaded = profiles.pool_loaded;

    let identity_norms =
        agent_instruction_identity::run(kernel_root, &markdown_files, topics.topic_pool.as_ref());
    failures.extend(identity_norms.failures);

    let task_patterns = task_pattern_registry::run(kernel_root, &files);
    failures.extend(task_patterns.failures);

    let source_registry = instruction_source_registry::run(kernel_root);
    failures.extend(source_registry.failures);

    let task_specification = task_specification::run(kernel_root);
    failures.extend(task_specification.failures);

    let execution_state = execution_state::run(kernel_root);
    failures.extend(execution_state.failures);

    let role_and_human_control = role_and_human_control::run(kernel_root);
    failures.extend(role_and_human_control.failures);

    let bounded_context_manifest = bounded_context_manifest::run(kernel_root);
    failures.extend(bounded_context_manifest.failures);

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

    // A Kernel is never reported `ok` while any mandatory gate family is
    // blocked pending a port, even when every check this binary actually
    // ran came back clean: an unrun gate is not a passed gate
    // (`AGENTS.md`, `meridian-rust-migration-program-plan.md`, item 1 of the
    // second `CHANGES_REQUESTED` round on package `meridian-cli-foundation`
    // — "`meridian validate` не должен возвращать код 0/status ok, пока
    // часть обязательного гейта blocked или не исполнялась").
    let ok = collected.failures.is_empty() && BLOCKED_CHECKS.is_empty();
    let blocked: Vec<Value> = BLOCKED_CHECKS
        .iter()
        .map(|(check, reason)| json!({"check": check, "reason": reason}))
        .collect();
    let result = json!({
        "kernel": kernel_path,
        "checked_files": collected.checked_files,
        "failures": collected.failures,
        "warnings": collected.warnings,
        "blocked": blocked,
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
                "{} failure(s), {} warning(s), {} check(s) blocked pending a new architecture decision",
                collected.failures.len(),
                collected.warnings.len(),
                BLOCKED_CHECKS.len()
            );
            // Three distinct human verdicts, not two: a Kernel with zero
            // real failures but a non-empty `blocked` list is not "OK" (it
            // was not fully checked) and is not "FAIL" either (nothing this
            // binary actually ran found a real problem) — collapsing it
            // into either word would misreport which of the two is true.
            let verdict = if ok {
                "OK"
            } else if collected.failures.is_empty() {
                "BLOCKED"
            } else {
                "FAIL"
            };
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
pub fn collect_diagnostics(kernel_root: &Path) -> Result<(Vec<String>, Vec<String>), WalkError> {
    let collected = collect(kernel_root)?;
    Ok((collected.failures, collected.warnings))
}
