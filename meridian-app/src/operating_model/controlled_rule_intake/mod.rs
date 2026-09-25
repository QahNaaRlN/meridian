//! `controlled-rule-intake` — the composite-consistency algorithm behind
//! the `meridian validate` check of the same name (package 7, subpackage
//! 7c; corrective pilot `rust-architecture-conformance-1`,
//! `governance/plans/meridian-rust-migration-program-plan.md` §5.13, and
//! its own corrective round documented in this module's history).
//!
//! `registries/operating-model/controlled-rule-intake.schema.json` is the
//! specialised payload schema for a rule candidate — text parsed out of a
//! registered instruction source (`instruction-source-registry.md`) and
//! proposed as a possible rule. Registry DATA is Instance data; the Kernel
//! ships the schema, the product-neutral fixtures and this module.
//!
//! # Architecture (as the code actually is)
//!
//! The boundary sequence, strictly separated across two crates:
//!
//! 1. **Syntax** — [`crate::source_format::json_schema::validate`] against
//!    the external, normative schema files. If EITHER the container OR any
//!    entry has a schema error, [`evaluate_controlled_rule_intake`] returns
//!    immediately with only those diagnostics: no transport parse, no
//!    domain construction is attempted at all (corrective round item 2).
//! 2. **Transport** — `dto`: closed `serde` DTOs,
//!    `#[serde(deny_unknown_fields)]` on every object shape (item 1). Only
//!    reached once step 1 found nothing.
//! 3. **Domain validation and construction** — `convert::build_candidate`
//!    walks each `dto::CandidateDto` into a
//!    [`meridian_core::controlled_rule_intake::RuleCandidate`], accumulating
//!    every independent field defect before deciding success (item 9), then
//!    calls [`meridian_core::controlled_rule_intake::RuleCandidate::try_new`]
//!    — the ONLY constructor, which itself enforces cross-field consistency
//!    (item 5). The three states are kept genuinely distinct: a
//!    `dto::CandidateDto` (transport), a `Result<RuleCandidate,
//!    Vec<Diagnostic>>` in progress (domain validation, not yet a type), and
//!    a [`meridian_core::controlled_rule_intake::RuleCandidate`] (valid
//!    domain type) — never conflated in one function (item 3).
//! 4. **Pure predicate checks** — entirely in `meridian-core`
//!    ([`meridian_core::controlled_rule_intake::checks`]): cluster,
//!    conflict-graph, and the resolved-`source_ref` comparison. None of
//!    these ever see a `Value` or a DTO (item 4).
//! 5. **External resolution** — [`source_resolver::SourceResolver`] is an
//!    app-defined port; its concrete adapter lives in `meridian-cli` (item
//!    7). [`evaluate_controlled_rule_intake`] calls it exactly once per
//!    candidate, outside any check. Its response is validated through
//!    `source_resolver::validate_resolved_response` (raw response ->
//!    closed DTO -> validated `ResolvedInstructionSource`, item 8) before
//!    the pure core comparison ever runs.
//!
//! Eleven properties this module enforces beyond the JSON Schema subset
//! (see the Node reference's own doc comment for the full statement of
//! each): a candidate is untrusted until an explicit owner decision;
//! `payload.source_ref` pins ONE instruction-source-registry entry by id,
//! an explicit revision AND a SHA-256 digest (both mandatory), resolved
//! through an external boundary and checked against that entry's own
//! `recorded_state`; `payload.boundary` preserves the candidate's exact
//! span; `scope` is an explicit classification, never derived from a
//! physical path; `semantic_key` clusters duplicate candidates found
//! through different origins, and every member of a cluster agrees on
//! scope, `applicability_state`/`owner_decision` and `conflicts_with`;
//! `conflicts_with` is declared SYMMETRICALLY and evaluated as a graph over
//! the WHOLE candidate set; `applicability_state` is a closed minimal
//! three-value state, unrepresentable in any other combination; `origin.kind`
//! is pinned to `"derived"` and `origin.source_ref` must name the SAME
//! source as `payload.source_ref.id`, enforced at construction, not
//! afterward; this module never reads, changes or reinterprets a source's
//! `read_channel`, reusing the SAME canonical
//! [`meridian_core::instruction_source::ReadChannel::try_new`] that
//! `super::instruction_source_registry`'s own `convert::build_read_channel`
//! also builds through, rather than a second copy of the coherence rule;
//! `raw_excerpt`/`normalized_text` are opaque strings,
//! never parsed here; `accepted`/`not-applicable` both require the pinned
//! source's CURRENT `recorded_state.currency` and the scope's own closed
//! human owner authority — `"delegated-run"` never itself decides
//! applicability.
//!
//! # Intentional differences from the Node reference
//!
//! - **Cluster scope agreement** uses full [`meridian_core::types::Scope`]
//!   identity instead of a `type::id`-only string key — documented in
//!   `COMPATIBILITY.md` and covered by a real Node/Rust divergence test in
//!   `test/conformance-harness.test.mjs` (corrective round item 10).
//! - **Schema errors stop domain construction entirely** (first corrective
//!   round item 2): `scripts/lib/controlled-rule-intake.mjs`'s
//!   `evaluateControlledRuleIntake` never stops at a schema violation
//!   (only at a schema-COMPILE exception) and keeps computing the full
//!   composite analysis regardless; this function stops immediately at
//!   ANY schema violation, by explicit corrective instruction — a
//!   schema-invalid document gets only its schema diagnostics, never a
//!   composite analysis over data that failed its own shape contract. This
//!   IS a real difference in how many diagnostics each side computes
//!   internally, but it is verified NOT to be observable through this
//!   Kernel's one current executable path for a controlled-rule-intake
//!   document (`meridian validate`'s and `scripts/kernel-validate.mjs`'s
//!   own self-test wrappers both report only the FIRST diagnostic for a
//!   wrongly-rejected fixture, and a schema diagnostic is always pushed
//!   before any composite one on both sides) — see the real Node/Rust
//!   "schema short-circuit" test in `test/conformance-harness.test.mjs`
//!   and `COMPATIBILITY.md` (second corrective round item 3, which
//!   replaced an earlier, untested overclaim here with this verified,
//!   narrower statement).
//! - **Resolution is called from the orchestration, never from inside a
//!   check** — a pure function only ever validates an already-obtained
//!   response, matching
//!   `meridian_core::migration::checks::check_reproducibility`'s own
//!   `resolved: Option<&ResolvedSourceSnapshot>` shape.

use std::collections::{BTreeMap, HashMap, HashSet};

use meridian_core::controlled_rule_intake::{PinnedSourceRef, RuleCandidate, SemanticKey};
use meridian_core::types::{Diagnostic, DiagnosticLevel};
use serde_json::Value;

use crate::source_format::json_schema;

mod convert;
mod dto;
mod source_resolver;

pub use source_resolver::{ResolvedInstructionSourceDto, SourceResolver, SourceResolverError};

/// How `evaluate_controlled_rule_intake` obtains a candidate's resolved
/// source, without ever requiring `meridian-app` itself to implement
/// [`SourceResolver`] (third corrective round, items 1-3):
///
/// - [`SourceResolution::Port`] calls a real, externally-implemented port
///   (`meridian-cli`'s `FixtureSourceResolver` today) — for when resolution
///   genuinely might need I/O.
/// - [`SourceResolution::Prefetched`] looks a candidate's pinned SOURCE ID
///   up (fourth corrective round, item 3.1 — NOT its `reference`: the same
///   id-first resolution `scripts/lib/context-manifest.mjs`'s
///   `buildSourceResolver` and this crate's own
///   `existing_project_compatibility_mode::build_source_resolver` already
///   use, so a candidate that pins the right source id but the wrong
///   `reference` still resolves — to a value whose `reference` then
///   genuinely disagrees with the pin — and gets the precise
///   reference-mismatch diagnostic [`meridian_core::controlled_rule_intake::checks::check_source_ref`]
///   already computes from that disagreement, not a blanket `NotFound`
///   that erases the distinction) in an already-typed table the caller
///   built once, up front, from data it already fully holds
///   (`operating_model::existing_project_compatibility_mode`'s own scan) —
///   a plain typed lookup, read directly by this orchestration, never
///   wrapped in a closure or a locally-defined adapter struct. Each table
///   entry is itself a `Result` (item 3.2): a source whose OWN data failed
///   to convert into the closed transport shape stays distinguishable from
///   a source that was never in the table at all — the caller must not
///   collapse that distinction with `.ok()` or an equivalent silent drop.
/// - [`SourceResolution::None`] resolves nothing (tests, and any caller
///   that only wants the non-resolution diagnostics).
pub enum SourceResolution<'a> {
    None,
    Port(&'a dyn SourceResolver),
    Prefetched(&'a HashMap<String, Result<ResolvedInstructionSourceDto, SourceResolverError>>),
}

impl SourceResolution<'_> {
    fn resolve(
        &self,
        pinned: &PinnedSourceRef,
    ) -> Option<Result<ResolvedInstructionSourceDto, SourceResolverError>> {
        match self {
            SourceResolution::None => None,
            SourceResolution::Port(resolver) => Some(resolver.resolve(pinned)),
            SourceResolution::Prefetched(table) => Some(match table.get(pinned.id().as_str()) {
                None => Err(SourceResolverError::NotFound),
                Some(result) => result.clone(),
            }),
        }
    }
}

pub(crate) fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

/// Runs `result`, pushing a `FAIL` diagnostic prefixed with `at` and
/// returning `None` on error rather than stopping the caller — the shared
/// accumulate-don't-fail-fast primitive every per-field conversion in
/// [`convert`] and [`source_resolver`] uses (corrective round item 9).
pub(crate) fn try_field<T, E: core::fmt::Display>(
    problems: &mut Vec<Diagnostic>,
    at: &str,
    result: Result<T, E>,
) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            problems.push(fail(format!("{at} {e}")));
            None
        }
    }
}

pub struct EvalOpts<'a> {
    pub registry_schema: &'a Value,
    pub envelope_schema: &'a Value,
    pub resolve_source: SourceResolution<'a>,
}

/// The whole composition pipeline for one controlled-rule-intake document.
/// See this module's own doc comment for the full boundary sequence.
pub fn evaluate_controlled_rule_intake(doc: &Value, opts: &EvalOpts) -> Vec<Diagnostic> {
    match json_schema::validate(doc, opts.registry_schema) {
        Ok(errors) if errors.is_empty() => {}
        Ok(errors) => return errors.into_iter().map(fail).collect(),
        Err(error) => {
            return vec![fail(format!(
                "container/payload schema could not be applied: {error}"
            ))]
        }
    }

    let empty = Vec::new();
    let entries = doc
        .get("rule_candidates")
        .and_then(Value::as_array)
        .unwrap_or(&empty);

    let mut entry_schema_errors = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        match json_schema::validate(entry, opts.envelope_schema) {
            Ok(errors) => entry_schema_errors.extend(
                errors
                    .into_iter()
                    .map(|m| fail(format!("entry {i} envelope {m}"))),
            ),
            Err(error) => entry_schema_errors.push(fail(format!(
                "entry {i} envelope could not be applied: {error}"
            ))),
        }
    }
    if !entry_schema_errors.is_empty() {
        return entry_schema_errors;
    }

    let registry: dto::RegistryDto = match serde_json::from_value(doc.clone()) {
        Ok(v) => v,
        Err(error) => {
            return vec![fail(format!(
                "registry document could not be parsed into the closed transport shape: {error}"
            ))]
        }
    };

    let mut problems = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut candidates: Vec<RuleCandidate> = Vec::new();
    for candidate_dto in &registry.rule_candidates {
        if !seen_ids.insert(candidate_dto.id.clone()) {
            problems.push(fail(format!(
                "rule candidate id \"{}\" is declared more than once",
                candidate_dto.id
            )));
        }
        match convert::build_candidate(candidate_dto) {
            Ok(candidate) => candidates.push(candidate),
            Err(errs) => problems.extend(errs),
        }
    }

    for candidate in &candidates {
        let candidate_id = candidate.id().as_str();
        let source_id = candidate.payload().source_ref().id().as_str();
        let applicability_state = candidate.payload().applicability().state_label();

        match opts.resolve_source.resolve(candidate.payload().source_ref()) {
            None => {
                problems.extend(meridian_core::controlled_rule_intake::checks::check_source_ref(candidate_id, candidate.payload().source_ref(), None, applicability_state));
            }
            Some(Ok(response_dto)) => match source_resolver::validate_resolved_response(candidate_id, source_id, response_dto) {
                Ok(resolved) => {
                    problems.extend(meridian_core::controlled_rule_intake::checks::check_source_ref(candidate_id, candidate.payload().source_ref(), Some(&resolved), applicability_state));
                }
                Err(errs) => problems.extend(errs),
            },
            Some(Err(SourceResolverError::NotFound)) => problems.push(fail(format!(
                "rule candidate \"{candidate_id}\" source_ref does not resolve to a registered instruction source through the external resolver; an unknown source is never accepted and the candidate fails closed"
            ))),
            Some(Err(SourceResolverError::Failed(reason))) => problems.push(fail(format!("rule candidate \"{candidate_id}\" source_ref resolution failed: {reason}"))),
        }
    }

    let mut by_semantic_key: BTreeMap<SemanticKey, Vec<&RuleCandidate>> = BTreeMap::new();
    for candidate in &candidates {
        by_semantic_key
            .entry(candidate.payload().semantic_key().clone())
            .or_default()
            .push(candidate);
    }
    for (key, members) in &by_semantic_key {
        problems.extend(
            meridian_core::controlled_rule_intake::checks::check_cluster(key.as_str(), members),
        );
    }
    problems.extend(
        meridian_core::controlled_rule_intake::checks::check_conflict_graph(&by_semantic_key),
    );

    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorded_state_ok() -> Value {
        serde_json::json!({
            "revision": "a".repeat(40),
            "revision_verified": true,
            "currency": "current",
            "digest": {"algorithm": "sha-256", "value": "b".repeat(64)},
        })
    }
    fn read_channel_ok() -> Value {
        serde_json::json!({"kind": "meridian-observed", "meridian_visibility": "full", "agent_auto_read": false})
    }
    fn resolution_map() -> Value {
        serde_json::json!({
            "sources/src-1": {
                "record_type": "instruction-source",
                "id": "src-1",
                "reference": "sources/src-1",
                "recorded_state": recorded_state_ok(),
                "read_channel": read_channel_ok(),
            }
        })
    }
    fn candidate_entry() -> Value {
        serde_json::json!({
            "id": "sample-candidate",
            "record_type": "rule-candidate",
            "scope": {"type": "repository-scope", "id": "sample-repository", "workspace_id": "sample-workspace"},
            "origin": {"kind": "derived", "source_ref": "instruction-source:src-1"},
            "authority": {"kind": "delegated-run", "authority_ref": "controlled-rule-intake-run:sample-pass-1"},
            "payload": {
                "source_ref": {"record_type": "instruction-source", "id": "src-1", "reference": "sources/src-1", "revision": "a".repeat(40), "sha256": "b".repeat(64)},
                "boundary": {"unit": "whole-source"},
                "raw_excerpt": "Branches follow feature/<slug>.",
                "normalized_text": "branches follow feature slug",
                "semantic_key": "branch-naming-feature-slug",
                "classification_basis": "the repository owns this rule",
                "applicability_state": "candidate",
            },
        })
    }
    fn permissive_schema() -> Value {
        serde_json::json!({"type": "object"})
    }
    fn registry_doc(candidates: Vec<Value>) -> Value {
        serde_json::json!({
            "schema_version": 1,
            "registry_id": "controlled-rule-intake",
            "title": "test registry",
            "rule_candidates": candidates,
        })
    }

    /// Counts how many times [`SourceResolver::resolve`] is called —
    /// corrective round item 12: resolution happens exactly once per
    /// candidate. A real trait impl: `#[cfg(test)]` test doubles implementing
    /// an app-owned port are ordinary test code, not the "production
    /// implementation" items 1-2 restrict to CLI/adapter crates.
    struct CountingResolver {
        calls: std::cell::Cell<usize>,
        map: Value,
    }
    impl SourceResolver for CountingResolver {
        fn resolve(
            &self,
            pinned: &meridian_core::controlled_rule_intake::PinnedSourceRef,
        ) -> Result<source_resolver::ResolvedInstructionSourceDto, SourceResolverError> {
            self.calls.set(self.calls.get() + 1);
            match self.map.get(pinned.reference()) {
                None => Err(SourceResolverError::NotFound),
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| SourceResolverError::Failed(e.to_string())),
            }
        }
    }

    #[test]
    fn on_a_null_document_fails_closed_without_panicking() {
        let opts = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::None,
        };
        let problems = evaluate_controlled_rule_intake(&Value::Null, &opts);
        assert!(!problems.is_empty());
    }

    #[test]
    fn a_well_formed_candidate_resolves_and_checks_clean() {
        let doc = registry_doc(vec![candidate_entry()]);
        let resolver = CountingResolver {
            calls: std::cell::Cell::new(0),
            map: resolution_map(),
        };
        let opts = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::Port(&resolver),
        };
        let problems = evaluate_controlled_rule_intake(&doc, &opts);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            resolver.calls.get(),
            1,
            "resolver must be called exactly once per candidate"
        );
    }

    #[test]
    fn resolver_not_found_and_resolver_failed_produce_distinguishable_diagnostics() {
        struct FailingResolver;
        impl SourceResolver for FailingResolver {
            fn resolve(
                &self,
                _pinned: &meridian_core::controlled_rule_intake::PinnedSourceRef,
            ) -> Result<source_resolver::ResolvedInstructionSourceDto, SourceResolverError>
            {
                Err(SourceResolverError::Failed("adapter exploded".to_string()))
            }
        }
        struct NotFoundResolver;
        impl SourceResolver for NotFoundResolver {
            fn resolve(
                &self,
                _pinned: &meridian_core::controlled_rule_intake::PinnedSourceRef,
            ) -> Result<source_resolver::ResolvedInstructionSourceDto, SourceResolverError>
            {
                Err(SourceResolverError::NotFound)
            }
        }

        let doc = registry_doc(vec![candidate_entry()]);
        let schema = permissive_schema();

        let failed_opts = EvalOpts {
            registry_schema: &schema,
            envelope_schema: &schema,
            resolve_source: SourceResolution::Port(&FailingResolver),
        };
        let failed_problems = evaluate_controlled_rule_intake(&doc, &failed_opts);
        assert!(
            failed_problems
                .iter()
                .any(|d| d.message().contains("resolution failed")
                    && d.message().contains("adapter exploded")),
            "{failed_problems:?}"
        );

        let not_found_opts = EvalOpts {
            registry_schema: &schema,
            envelope_schema: &schema,
            resolve_source: SourceResolution::Port(&NotFoundResolver),
        };
        let not_found_problems = evaluate_controlled_rule_intake(&doc, &not_found_opts);
        assert!(
            not_found_problems.iter().any(|d| d
                .message()
                .contains("does not resolve to a registered instruction source")),
            "{not_found_problems:?}"
        );

        assert_ne!(failed_problems, not_found_problems);
    }

    #[test]
    fn a_boundary_with_a_non_positive_span_is_rejected_without_masking_other_candidates() {
        let mut broken = candidate_entry();
        broken["id"] = Value::String("broken-candidate".to_string());
        broken["payload"]["boundary"] =
            serde_json::json!({"unit": "line-range", "start": 10, "end": 5});
        let doc = registry_doc(vec![candidate_entry(), broken]);
        let resolver = CountingResolver {
            calls: std::cell::Cell::new(0),
            map: resolution_map(),
        };
        let opts = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::Port(&resolver),
        };
        let problems = evaluate_controlled_rule_intake(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("broken-candidate") && p.message().contains("start")),
            "{problems:?}"
        );
        // the well-formed sibling candidate still resolves and checks clean
        assert_eq!(resolver.calls.get(), 1);
    }

    #[test]
    fn a_candidate_id_declared_twice_is_rejected() {
        let doc = registry_doc(vec![candidate_entry(), candidate_entry()]);
        let opts = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::None,
        };
        let problems = evaluate_controlled_rule_intake(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("declared more than once")),
            "{problems:?}"
        );
    }

    /// Corrective round item 2: a schema-invalid document produces ONLY
    /// schema diagnostics — no attempt at domain construction, so no
    /// cluster/authority/etc. diagnostics appear alongside them. A minimal
    /// synthetic input, cheap and fast, proving the mechanism in general —
    /// see `schema_and_composite_defect_together_...` below for the actual
    /// matched-pair proof against the real Node reference (fourth
    /// corrective round, item 3.3: the two prior tests used different,
    /// non-comparable inputs).
    #[test]
    fn a_schema_invalid_document_stops_before_any_domain_construction() {
        let strict_schema = serde_json::json!({"type": "object", "required": ["registry_id"]});
        let doc = serde_json::json!({"rule_candidates": []});
        let opts = EvalOpts {
            registry_schema: &strict_schema,
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::None,
        };
        let problems = evaluate_controlled_rule_intake(&doc, &opts);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].message().contains("registry_id"),
            "{problems:?}"
        );
    }

    /// The Rust half of the direct library-level Node/Rust matched pair
    /// (fourth corrective round, item 3.3; fifth corrective round, item 1):
    /// uses the SAME real fixture document
    /// (`registries/operating-model/fixtures/controlled-rule-intake.fixtures.json`'s
    /// `valid[0].registry`) and the SAME two mutations as the Node half in
    /// `test/conformance-harness.test.mjs` — not an "analogous shape"
    /// input, the identical document.
    ///
    /// The mutation is deliberately NOT "delete a required field" (fifth
    /// corrective round, item 1 — that was the previous mutation, and it
    /// had no teeth: `payload.classification_basis` is both schema-required
    /// AND a non-`Option` `String` field of `dto::PayloadDto`, so DTO
    /// deserialization alone would already produce a single diagnostic
    /// even with the schema gate entirely removed, and the test would stay
    /// green for the wrong reason). Instead: `registry_id` is changed to a
    /// wrong, but still non-empty, string. The schema's `registry_id: {const:
    /// "controlled-rule-intake"}` rejects it; `dto::RegistryDto.registry_id`
    /// is a plain, unvalidated `String` and would accept ANY string equally
    /// well. This mutation can therefore ONLY be caught by the schema gate
    /// — empirically confirmed: temporarily disabling the schema
    /// short-circuit in `evaluate_controlled_rule_intake` and rerunning
    /// this exact test makes it fail with a second, origin/source_ref
    /// diagnostic (see this round's transfer record for the verification
    /// log); the change was reverted immediately after.
    ///
    /// Node's half asserts AT LEAST 2 diagnostics (schema AND
    /// origin/source_ref, because `evaluateControlledRuleIntake` never
    /// stops at a schema violation); this asserts EXACTLY 1 (schema only,
    /// and explicitly NOT the origin/source_ref one) — together the direct
    /// proof, on both real language runtimes, of the library-level fact
    /// `COMPATIBILITY.md` records. Resolution is not wired here even
    /// though the real fixture bundle carries a `resolution` map: this
    /// input never reaches candidate resolution at all (it stops at the
    /// schema gate), so a resolver here would be dead setup, not part of
    /// what is actually exercised.
    #[test]
    fn schema_and_composite_defect_together_produce_exactly_one_schema_diagnostic_matching_the_node_reference(
    ) {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let read_json = |rel: &str| -> Value {
            serde_json::from_str(&std::fs::read_to_string(kernel_root.join(rel)).unwrap()).unwrap()
        };
        let registry_schema =
            read_json("registries/operating-model/controlled-rule-intake.schema.json");
        let envelope_schema = read_json("registries/operating-model/scoped-record.schema.json");
        let bundle =
            read_json("registries/operating-model/fixtures/controlled-rule-intake.fixtures.json");

        let mut registry = bundle["valid"][0]["registry"].clone();
        assert_eq!(
            registry["registry_id"].as_str(),
            Some("controlled-rule-intake")
        );
        registry["registry_id"] = Value::String("wrong-registry-id".to_string());
        let candidate = &mut registry["rule_candidates"][0];
        let original_source_ref = candidate["origin"]["source_ref"]
            .as_str()
            .unwrap()
            .to_string();
        candidate["origin"]["source_ref"] =
            Value::String(format!("{original_source_ref}-tampered"));

        // Confirms the mutation truly has teeth: the DTO itself parses this
        // registry cleanly (its own `registry_id` field carries no
        // validation), so ONLY the schema gate can be what rejects it below.
        let dto_parse_result: Result<dto::RegistryDto, _> =
            serde_json::from_value(registry.clone());
        assert!(dto_parse_result.is_ok(), "the mutated registry must still deserialize cleanly into the closed transport DTO: {dto_parse_result:?}");

        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            resolve_source: SourceResolution::None,
        };
        let problems = evaluate_controlled_rule_intake(&registry, &opts);

        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("registry_id") || p.message().contains("const")),
            "expected a schema diagnostic naming registry_id/const: {problems:?}"
        );
        assert!(
            !problems.iter().any(|p| p.message().contains("does not name the same source")),
            "expected NO origin/source_ref diagnostic (domain construction must never run past a schema violation): {problems:?}"
        );
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    /// Intentional difference from the Node reference (`COMPATIBILITY.md`):
    /// two cluster members that share `scope.type`/`scope.id` but disagree
    /// on `workspace_id` are flagged (full `Scope` identity), where the
    /// previous `type::id`-only key silently accepted them.
    #[test]
    fn a_cluster_whose_members_disagree_only_on_workspace_id_is_flagged() {
        let mut a = candidate_entry();
        a["id"] = Value::String("cand-a".to_string());
        a["payload"]["semantic_key"] = Value::String("shared-key".to_string());
        let mut b = candidate_entry();
        b["id"] = Value::String("cand-b".to_string());
        b["payload"]["semantic_key"] = Value::String("shared-key".to_string());
        b["scope"]["workspace_id"] = Value::String("a-different-workspace".to_string());
        let doc = registry_doc(vec![a, b]);
        let opts = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::None,
        };
        let problems = evaluate_controlled_rule_intake(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("more than one scope")),
            "{problems:?}"
        );
    }

    /// Corrective round item 3: `SourceResolution::Prefetched` resolves
    /// from plain typed data, with no port implementation and no resolver
    /// closure anywhere in this test.
    #[test]
    fn prefetched_source_resolution_works_from_typed_data_with_no_port_impl() {
        let mut table = HashMap::new();
        let resolution = resolution_map();
        let entry: ResolvedInstructionSourceDto =
            serde_json::from_value(resolution["sources/src-1"].clone()).unwrap();
        table.insert(entry.id.clone(), Ok(entry));

        let doc = registry_doc(vec![candidate_entry()]);
        let opts = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::Prefetched(&table),
        };
        let problems = evaluate_controlled_rule_intake(&doc, &opts);
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// Fourth corrective round, item 3.1 / test requirement 1: `Prefetched`
    /// resolves by SOURCE ID, not by `reference` — a candidate that pins
    /// the right id but a wrong `reference` still resolves (to the id's
    /// real entry), and the mismatch is reported as a precise
    /// reference-mismatch diagnostic by `check_source_ref`, never collapsed
    /// into `NotFound`.
    #[test]
    fn prefetched_resolution_is_id_first_and_reference_mismatch_is_diagnosed_precisely() {
        let mut table = HashMap::new();
        let resolution = resolution_map();
        let entry: ResolvedInstructionSourceDto =
            serde_json::from_value(resolution["sources/src-1"].clone()).unwrap();
        assert_eq!(entry.id, "src-1");
        table.insert(entry.id.clone(), Ok(entry));

        let mut candidate = candidate_entry();
        candidate["payload"]["source_ref"]["reference"] =
            Value::String("sources/src-1-WRONG-REFERENCE".to_string());
        let doc = registry_doc(vec![candidate]);
        let opts = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::Prefetched(&table),
        };
        let problems = evaluate_controlled_rule_intake(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("is not the resolved reference")),
            "expected a precise reference-mismatch diagnostic, not NotFound: {problems:?}"
        );
        assert!(
            !problems.iter().any(|p| p
                .message()
                .contains("does not resolve to a registered instruction source")),
            "a wrong reference with a correct id must not be reported as NotFound: {problems:?}"
        );
    }

    /// Test requirement 2: a source id genuinely absent from the table
    /// gives `NotFound`.
    #[test]
    fn prefetched_resolution_of_a_genuinely_absent_id_is_not_found() {
        let table: HashMap<String, Result<ResolvedInstructionSourceDto, SourceResolverError>> =
            HashMap::new();
        let doc = registry_doc(vec![candidate_entry()]);
        let opts = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::Prefetched(&table),
        };
        let problems = evaluate_controlled_rule_intake(&doc, &opts);
        assert!(
            problems.iter().any(|p| p
                .message()
                .contains("does not resolve to a registered instruction source")),
            "{problems:?}"
        );
    }

    /// Test requirement 3 / corrective round item 3.2: a source id PRESENT
    /// in the table but carrying a `Failed` entry (its own data did not
    /// convert into the closed transport shape) gives the resolution-failed
    /// diagnostic, never `NotFound` — the two states stay distinguishable
    /// through `SourceResolution::resolve` all the way to the diagnostic
    /// text. This exercises the ORCHESTRATION's own handling of a
    /// `Prefetched` table's `Err` entry, decoupled from how that entry got
    /// there — see
    /// `evaluate_controlled_rule_intake_reports_resolution_failed_for_a_malformed_prefetched_source_built_by_production_code`
    /// below for the matching test that goes through the REAL production
    /// builder instead of inserting the `Err` by hand (fifth corrective
    /// round, item 2).
    #[test]
    fn prefetched_resolution_of_a_malformed_entry_is_a_failure_not_a_not_found() {
        let mut table: HashMap<String, Result<ResolvedInstructionSourceDto, SourceResolverError>> =
            HashMap::new();
        table.insert(
            "src-1".to_string(),
            Err(SourceResolverError::Failed(
                "missing recorded_state".to_string(),
            )),
        );
        let doc = registry_doc(vec![candidate_entry()]);
        let opts = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::Prefetched(&table),
        };
        let problems = evaluate_controlled_rule_intake(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("resolution failed")
                    && p.message().contains("missing recorded_state")),
            "{problems:?}"
        );
        assert!(
            !problems.iter().any(|p| p
                .message()
                .contains("does not resolve to a registered instruction source")),
            "a malformed-but-present entry must not be reported as NotFound: {problems:?}"
        );
    }

    /// Builds a real, fully-valid
    /// [`meridian_core::instruction_source::InstructionSource`] — used by
    /// the two tests below to exercise the REAL production
    /// `existing_project_compatibility_mode::build_resolved_sources` against
    /// genuine typed data, not a `serde_json::Value` (`rust-architecture-conformance-2`,
    /// §4: that function no longer takes a `Value` at all).
    fn sample_instruction_source(id: &str) -> meridian_core::instruction_source::InstructionSource {
        use meridian_core::instruction_source::{
            Currency, Divergence, DivergenceStatus, InstructionSource, InstructionSourcePayload,
            Location, MeridianVisibility, OpaqueRef, ReadChannel, ReadChannelKind, RecordedState,
            RelativePath, SourceFormat,
        };
        use meridian_core::types::{ContentDigest, Revision, SemanticId};

        let recorded_state = RecordedState::new(
            Revision::new("a".repeat(40)).unwrap(),
            ContentDigest::from_hex("b".repeat(64)).unwrap(),
            true,
            Currency::Current,
        );
        let read_channel = ReadChannel::try_new(
            id,
            ReadChannelKind::MeridianObserved,
            MeridianVisibility::Full,
            false,
        )
        .unwrap();
        let divergence = Divergence::try_new(id, DivergenceStatus::Unknown, None, None).unwrap();
        let location = Location::file(
            RelativePath::new("a.md").unwrap(),
            OpaqueRef::new("container-1").unwrap(),
        );
        let payload = InstructionSourcePayload::new(
            location,
            SourceFormat::MarkdownSection,
            recorded_state,
            read_channel,
            divergence,
        );
        InstructionSource::try_new(SemanticId::new(id).unwrap(), payload).unwrap()
    }

    /// Fifth corrective round, item 2 (updated by
    /// `rust-architecture-conformance-2`, §4, for the typed
    /// `build_resolved_sources` signature): exercises the REAL production
    /// builder — no `Err` is inserted by hand anywhere in this test.
    /// `"malformed-src"` is present among the scan's discovered ids but has
    /// no corresponding entry in the typed `sources` map (exactly what
    /// `instruction_source_registry::convert_registry` produces for a
    /// discovered source that failed its own conversion).
    /// `candidate_entry()` is reused unmodified — it already pins source id
    /// `"src-1"`, so this pins `"malformed-src"` explicitly via a small
    /// override instead.
    #[test]
    fn evaluate_controlled_rule_intake_reports_resolution_failed_for_a_malformed_prefetched_source_built_by_production_code(
    ) {
        let discovered_ids: std::collections::BTreeSet<String> =
            ["malformed-src".to_string()].into_iter().collect();
        let sources: std::collections::BTreeMap<
            String,
            meridian_core::instruction_source::InstructionSource,
        > = std::collections::BTreeMap::new();
        let table =
            crate::operating_model::existing_project_compatibility_mode::build_resolved_sources(
                &discovered_ids,
                &sources,
            );
        assert!(
            matches!(table.get("malformed-src"), Some(Err(SourceResolverError::Failed(_)))),
            "the production builder must report the malformed source as Failed, not silently drop it: {table:?}"
        );

        let mut candidate = candidate_entry();
        candidate["origin"]["source_ref"] =
            Value::String("instruction-source:malformed-src".to_string());
        candidate["payload"]["source_ref"]["id"] = Value::String("malformed-src".to_string());
        let doc = registry_doc(vec![candidate]);
        let opts = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::Prefetched(&table),
        };
        let problems = evaluate_controlled_rule_intake(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("resolution failed")),
            "{problems:?}"
        );
        assert!(
            !problems.iter().any(|p| p.message().contains("does not resolve to a registered instruction source")),
            "a malformed-but-present source built by production code must not be reported as NotFound: {problems:?}"
        );
    }

    /// Fifth corrective round, item 2: determinism of the BUILDER itself
    /// (`build_resolved_sources`), not merely of looking a manually-built
    /// table up twice — the builder is called twice on the same
    /// `discovered_by_id`, and the FULL typed diagnostics `evaluate_controlled_rule_intake`
    /// produces through each of the two resulting tables are compared.
    #[test]
    fn build_resolved_sources_and_its_lookup_are_deterministic() {
        let discovered_ids: std::collections::BTreeSet<String> =
            ["src-1".to_string()].into_iter().collect();
        let mut sources = std::collections::BTreeMap::new();
        sources.insert("src-1".to_string(), sample_instruction_source("src-1"));
        let table_a =
            crate::operating_model::existing_project_compatibility_mode::build_resolved_sources(
                &discovered_ids,
                &sources,
            );
        let table_b =
            crate::operating_model::existing_project_compatibility_mode::build_resolved_sources(
                &discovered_ids,
                &sources,
            );

        let doc = registry_doc(vec![candidate_entry()]);
        let opts_a = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::Prefetched(&table_a),
        };
        let opts_b = EvalOpts {
            registry_schema: &permissive_schema(),
            envelope_schema: &permissive_schema(),
            resolve_source: SourceResolution::Prefetched(&table_b),
        };
        let problems_a = evaluate_controlled_rule_intake(&doc, &opts_a);
        let problems_b = evaluate_controlled_rule_intake(&doc, &opts_b);
        assert_eq!(problems_a, problems_b, "two independently-built tables from the same input must drive evaluate_controlled_rule_intake to the identical full diagnostics");
        assert!(
            !problems_a.iter().any(|p| p.message().contains("resolution failed")),
            "the well-formed discovered source must resolve, not fail, on both builds: {problems_a:?}"
        );
    }
}
