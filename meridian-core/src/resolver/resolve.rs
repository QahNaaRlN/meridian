//! `resolve_rules` — the pure core of the resolver
//! (`rule-resolver.mjs`'s `resolveRules`, lines 447–778, ported without its
//! file I/O, argument parsing, environment variables, JSON/YAML and
//! `main()`).

use core::fmt;
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::types::ContentDigest;

use super::applicability::{
    Activation, ApplicabilityRecord, ApplicabilityScope, ApplicabilitySource, ApplicabilityStatus,
};
use super::glob::{CompiledGlob, GlobError};
use super::output::{
    ApplicableNorm, ApplicableProtocol, Conflict, DecompositionBlocker, ResolverOutput, RouteKey,
    Unresolved,
};
use super::sources::{
    IntakeRecordEntry, ProtocolRoute, ProtocolRouteScope, ResolverSources, VerificationTarget,
};
use super::work_item::{ChangeClass, WorkItem, WorkItemKind, WorkKind};

/// A value that stops resolution rather than letting a guess through
/// (`rule-resolver.mjs`'s `ResolverError`/`UnsupportedGlob`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolverError {
    /// `work_item.repository_id` names no entry in `repository_inventory`.
    UnknownRepository { repository_id: String },
    /// A path-glob mask uses syntax outside the documented grammar.
    UnsupportedGlob(GlobError),
    /// Every other fail-closed condition `rule-resolver.mjs` reports by
    /// message alone, carried verbatim here rather than split into a large
    /// number of single-use variants.
    Other(String),
}

impl ResolverError {
    fn other(message: impl Into<String>) -> Self {
        ResolverError::Other(message.into())
    }
}

impl fmt::Display for ResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolverError::UnknownRepository { repository_id } => write!(
                f,
                "unknown repository_id \"{repository_id}\": no entry with that exact id in repository_inventory (no fuzzy, path or basename fallback)"
            ),
            ResolverError::UnsupportedGlob(e) => write!(f, "{e}"),
            ResolverError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ResolverError {}

impl From<GlobError> for ResolverError {
    fn from(e: GlobError) -> Self {
        ResolverError::UnsupportedGlob(e)
    }
}

/// The work-item context a matching decision is made against
/// (`rule-resolver.mjs`'s `ctx`).
struct Ctx {
    repository_id: String,
    technology_profile: Option<String>,
    product_domains: Vec<String>,
    work_kind: WorkKind,
    change_class: Option<ChangeClass>,
    candidate_paths: Vec<String>,
}

fn unique_sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v.dedup();
    v
}

/// The pure core: `work_item` + `sources` → a deterministic
/// [`ResolverOutput`]. The same input on the same source revision returns a
/// byte-identical, identically ordered result (`rule-resolver.mjs`'s own
/// documented guarantee).
pub fn resolve_rules(
    work_item: &WorkItem,
    sources: &ResolverSources,
) -> Result<ResolverOutput, ResolverError> {
    let repo = sources
        .repository_inventory
        .iter()
        .find(|r| r.id == work_item.repository_id())
        .ok_or_else(|| ResolverError::UnknownRepository {
            repository_id: work_item.repository_id().to_string(),
        })?;

    let technology_profile = work_item
        .declared_profiles()
        .and_then(|d| d.technology_profile.clone())
        .or_else(|| repo.profile.clone());

    let ctx = Ctx {
        repository_id: work_item.repository_id().to_string(),
        technology_profile,
        product_domains: repo.semantic_areas.clone(),
        work_kind: work_item.kind().work_kind(),
        change_class: work_item.kind().change_class(),
        candidate_paths: work_item.candidate_paths().to_vec(),
    };

    let mut out = ResolverOutput::default();

    let ref_paths: Vec<String> = match &sources.prior_state.previous_resolution {
        Some(p) => p.candidate_paths.clone(),
        None => ctx.candidate_paths.clone(),
    };
    out.requires_reresolution = work_item
        .changed_paths()
        .iter()
        .any(|p| !ref_paths.contains(p));

    if matches!(work_item.kind(), WorkItemKind::Initiative) {
        let decomposed = sources
            .prior_state
            .decomposition
            .as_ref()
            .is_some_and(|d| !d.child_work_items.is_empty());
        out.decomposition_required = !decomposed;
        out.applicable_initiative_protocol = sources
            .prior_state
            .initiative_protocol
            .clone()
            .filter(|s| !s.is_empty());
        out.pre_decomposition_norms =
            unique_sorted(sources.prior_state.pre_decomposition_norms.clone());
        if !decomposed {
            let enough = sources
                .prior_state
                .source_repository_ids
                .as_ref()
                .is_some_and(|v| v.len() >= 2);
            if !enough {
                out.unresolved_items.push(DecompositionBlocker {
                    item: "complete list of source repositories".to_string(),
                    reason: "the request carries a single repository_id and no complete source-repository list was supplied; a consolidation initiative cannot be decomposed into child work items without it (rule-resolution.md §6)".to_string(),
                });
            }
        }
        return Ok(finalize(out));
    }

    let route_key = match work_item.kind() {
        WorkItemKind::Change(c) => RouteKey::ChangeClass(c),
        other => RouteKey::WorkKind(other.work_kind()),
    };
    route_protocols(route_key, sources, &mut out, &ctx);

    if matches!(route_key, RouteKey::ChangeClass(ChangeClass::Refactor))
        && !sources.prior_state.refactor_findings.is_empty()
    {
        out.unresolved_applicability.push(Unresolved {
            subject: "refactor-in-progress-finding".to_string(),
            reason: "a behavior change was found inside this REFACTOR work item; it is not fixed silently — a separate BUGFIX child work item must be created or the owner must decide, and the original REFACTOR work item is not reclassified (rule-resolution.md §3.2)".to_string(),
        });
    }

    resolve_norms(sources, &ctx, &mut out)?;

    let applicable_norm_ids: HashSet<&str> = out
        .applicable_norms
        .iter()
        .map(|n| n.norm.as_str())
        .collect();
    let mut verif = Vec::new();
    for vr in &sources.verification_routes {
        if vr.path.is_empty() {
            continue;
        }
        let hit = match &vr.applies_to {
            VerificationTarget::All => true,
            VerificationTarget::Norm(id) => applicable_norm_ids.contains(id.as_str()),
        };
        if hit {
            verif.push(vr.path.clone());
        }
    }
    out.required_verification = unique_sorted(verif);

    Ok(finalize(out))
}

fn route_in_scope(route: &ProtocolRoute, ctx: &Ctx) -> bool {
    match route.scope() {
        ProtocolRouteScope::Universal => true,
        ProtocolRouteScope::Repository { repository } => repository == &ctx.repository_id,
        ProtocolRouteScope::ProductDomain { product_domain } => {
            ctx.product_domains.contains(product_domain)
        }
    }
}

fn route_protocols(
    route_key: RouteKey,
    sources: &ResolverSources,
    out: &mut ResolverOutput,
    ctx: &Ctx,
) {
    let routes: Vec<&ProtocolRoute> = sources
        .protocol_routes
        .iter()
        .filter(|r| r.routed_from() == route_key)
        .filter(|r| route_in_scope(r, ctx))
        .collect();

    let mandatory: Vec<&&ProtocolRoute> = routes.iter().filter(|r| r.mandatory()).collect();
    let distinct_mandatory =
        unique_sorted(mandatory.iter().map(|r| r.protocol().to_string()).collect());
    // `distinctMandatory.length` (`rule-resolver.mjs`) — the count of
    // DISTINCT mandatory protocol names, not the number of mandatory route
    // RECORDS: two mandatory routes both citing protocol "a" plus one
    // citing "b" is 2 incompatible protocols, not 3.
    let distinct_mandatory_count = distinct_mandatory.len();

    let routes: Vec<&ProtocolRoute> = if distinct_mandatory_count > 1 {
        out.conflicts.push(Conflict {
            norms: distinct_mandatory,
            reason: format!(
                "{distinct_mandatory_count} incompatible mandatory routes for \"{route_key}\"; a local protocol route does not silently override a universal one (rule-resolution.md §7)",
            ),
        });
        routes.into_iter().filter(|r| !r.mandatory()).collect()
    } else {
        routes
    };

    // `byProtocol` (`rule-resolver.mjs`): among routes tied on
    // `source.rank()` for the same protocol key, the one with the
    // lexicographically smaller `JSON.stringify(route)` wins — reproduced
    // here via each route's own COMPUTED `tiebreak_key()`
    // (`ProtocolRoute::new` derives it from the route's own field values;
    // it is never an independently-suppliable opaque string).
    let mut by_protocol: HashMap<&str, &ProtocolRoute> = HashMap::new();
    for r in &routes {
        match by_protocol.get(r.protocol()) {
            None => {
                by_protocol.insert(r.protocol(), r);
            }
            Some(prev) => {
                let replace = r.source().rank() < prev.source().rank()
                    || (r.source().rank() == prev.source().rank()
                        && r.tiebreak_key() < prev.tiebreak_key());
                if replace {
                    by_protocol.insert(r.protocol(), r);
                }
            }
        }
    }

    let mut keys: Vec<&str> = by_protocol.keys().copied().collect();
    keys.sort();
    for key in keys {
        let r = by_protocol[key];
        out.applicable_protocols.push(ApplicableProtocol {
            protocol: key.to_string(),
            routed_from: route_key,
            source: r.source(),
            scope: r.scope().route_scope(),
            provenance: r.provenance().clone(),
        });
    }
}

enum RecordClass {
    NoMatch,
    Applies,
    ActivationUndetermined,
}

fn classify_record(rec: &ApplicabilityRecord, ctx: &Ctx) -> Result<RecordClass, ResolverError> {
    let scope_ok = match rec.scope() {
        ApplicabilityScope::Universal => true,
        ApplicabilityScope::Repository { repository } => repository == &ctx.repository_id,
        ApplicabilityScope::Profile { technology_profile } => {
            ctx.technology_profile.as_deref() == Some(technology_profile.as_str())
        }
        ApplicabilityScope::ProductDomain { product_domain } => {
            ctx.product_domains.contains(product_domain)
        }
    };
    if !scope_ok {
        return Ok(RecordClass::NoMatch);
    }

    match rec.activation() {
        Activation::Always => Ok(RecordClass::Applies),
        Activation::PathGlob { globs } => {
            for g in globs {
                let compiled = CompiledGlob::compile(g)?;
                if ctx.candidate_paths.iter().any(|p| compiled.is_match(p)) {
                    return Ok(RecordClass::Applies);
                }
            }
            Ok(RecordClass::NoMatch)
        }
        Activation::TaskClass { task_class } => {
            if !task_class.work_kind().contains(&ctx.work_kind) {
                return Ok(RecordClass::NoMatch);
            }
            if let Some(cc) = task_class.change_class() {
                if ctx.work_kind != WorkKind::Change {
                    return Ok(RecordClass::NoMatch);
                }
                match ctx.change_class {
                    Some(actual) if cc.contains(&actual) => {}
                    _ => return Ok(RecordClass::NoMatch),
                }
            }
            Ok(RecordClass::Applies)
        }
        Activation::Explicit => Ok(RecordClass::NoMatch),
        Activation::Undetermined => Ok(RecordClass::ActivationUndetermined),
    }
}

fn resolve_intake_pointer<'a>(
    rec: &ApplicabilityRecord,
    sources: &'a ResolverSources,
) -> Result<&'a IntakeRecordEntry, ResolverError> {
    let ptr = rec.source().intake_record().expect(
        "resolve_intake_pointer is only called for a non-kernel source, which always carries one",
    );
    let register = sources
        .intake_registers
        .iter()
        .find(|r| r.register == ptr.register())
        .ok_or_else(|| {
            ResolverError::other(format!(
                "intake pointer of \"{}\" names register \"{}\", which is not among the supplied intake registers",
                rec.norm_id(),
                ptr.register()
            ))
        })?;
    let region = rec.norm().region();
    let matches: Vec<&IntakeRecordEntry> = register
        .records
        .iter()
        .filter(|ir| {
            ir.artifact == rec.norm().path()
                && ir.region.as_deref() == region
                && &ir.recorded_at == ptr.recorded_at()
                && ir.verdict == ptr.verdict()
        })
        .collect();
    if matches.is_empty() {
        return Err(ResolverError::other(format!(
            "intake pointer of \"{}\" resolves to no record in {} (artifact {}{} @ {} → {})",
            rec.norm_id(),
            ptr.register(),
            rec.norm().path(),
            region.map(|r| format!("#{r}")).unwrap_or_default(),
            ptr.recorded_at(),
            ptr.verdict(),
        )));
    }
    if matches.len() > 1 {
        return Err(ResolverError::other(format!(
            "intake pointer of \"{}\" is ambiguous: {} records in {} match on artifact, region, date and verdict",
            rec.norm_id(),
            matches.len(),
            ptr.register()
        )));
    }
    Ok(matches[0])
}

fn verify_provenance(
    rec: &ApplicabilityRecord,
    sources: &ResolverSources,
) -> Result<super::applicability::Delivery, ResolverError> {
    let text = sources.norm_texts.get(&rec.norm_text_key()).ok_or_else(|| {
        ResolverError::other(format!(
            "no current text supplied for norm \"{}\"; its digest cannot be recomputed, so it is fail-closed rather than treated as applicable (rule-resolution.md §9)",
            rec.norm_id()
        ))
    })?;
    let computed = ContentDigest::of_str(text);
    if computed.value() != rec.digest().value() {
        return Err(ResolverError::other(format!(
            "stale digest for norm \"{}\": the supplied text hashes to {}… but the record records {}…; a norm that has moved since its record was written is not treated as applicable",
            rec.norm_id(),
            &computed.value()[..12.min(computed.value().len())],
            &rec.digest().value()[..12.min(rec.digest().value().len())],
        )));
    }
    match rec.source() {
        ApplicabilitySource::Kernel => Ok(super::applicability::Delivery::KernelDoc),
        _ => Ok(resolve_intake_pointer(rec, sources)?.delivery),
    }
}

fn matches_intake_pointer(
    rec: &ApplicabilityRecord,
    p: &super::applicability::SupersedesPointer,
) -> bool {
    let Some(ir) = rec.source().intake_record() else {
        return false;
    };
    if ir.register() != p.register() {
        return false;
    }
    if ir.recorded_at() != p.recorded_at() {
        return false;
    }
    if ir.verdict() != p.verdict() {
        return false;
    }
    if rec.norm().path() != p.path() {
        return false;
    }
    if rec.norm().region() != p.region() {
        return false;
    }
    if let Some(rev) = p.revision() {
        if ir.revision() != Some(rev) {
            return false;
        }
    }
    true
}

fn resolve_supersedes_edge<'a>(
    r: &'a ApplicabilityRecord,
    cohort: &[&'a ApplicabilityRecord],
    group: &[&'a ApplicabilityRecord],
) -> Result<Option<usize>, ResolverError> {
    let Some(p) = r.source().supersedes() else {
        return Ok(None);
    };
    if p.path() != r.norm().path() || p.region() != r.norm().region() {
        return Err(ResolverError::other(format!(
            "applicability record \"{}\" carries a \"supersedes\" pointer to {}{}, a different norm identity than its own {}{}; supersession is only between records of one norm identity (rule-resolution.md §9)",
            r.norm_id(),
            p.path(),
            p.region().map(|x| format!("#{x}")).unwrap_or_default(),
            r.norm().path(),
            r.norm().region().map(|x| format!("#{x}")).unwrap_or_default(),
        )));
    }
    let hits: Vec<usize> = cohort
        .iter()
        .enumerate()
        .filter(|(_, t)| matches_intake_pointer(t, p))
        .map(|(i, _)| i)
        .collect();
    if hits.len() > 1 {
        return Err(ResolverError::other(format!(
            "the \"supersedes\" pointer of \"{}\" is ambiguous: {} records sharing applicability recorded_at {} match register+path+region+recorded_at+verdict; fail-closed (rule-resolution.md §9)",
            r.norm_id(),
            hits.len(),
            r.recorded_at(),
        )));
    }
    if hits.is_empty() {
        let matches_elsewhere = group
            .iter()
            .any(|&t| !std::ptr::eq(t, r) && matches_intake_pointer(t, p));
        if matches_elsewhere {
            return Err(ResolverError::other(format!(
                "the \"supersedes\" pointer of \"{}\" names a record that does not share this record's applicability recorded_at {}; a \"supersedes\" relationship orders only records of one applicability date, so it is fail-closed (rule-resolution.md §9)",
                r.norm_id(),
                r.recorded_at(),
            )));
        }
        return Err(ResolverError::other(format!(
            "the \"supersedes\" pointer of \"{}\" resolves to no applicability record (register {}, {}{} @ {} → {}); fail-closed (rule-resolution.md §9)",
            r.norm_id(),
            p.register(),
            p.path(),
            p.region().map(|x| format!("#{x}")).unwrap_or_default(),
            p.recorded_at(),
            p.verdict(),
        )));
    }
    let idx = hits[0];
    if std::ptr::eq(cohort[idx], r) {
        return Err(ResolverError::other(format!(
            "applicability record \"{}\" declares that it supersedes itself; a \"supersedes\" pointer must name a different record (rule-resolution.md §9)",
            r.norm_id()
        )));
    }
    Ok(Some(idx))
}

fn assert_no_supersedes_cycle(
    cohort: &[&ApplicabilityRecord],
    edges: &[(usize, usize)],
) -> Result<(), ResolverError> {
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let n = cohort.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(from, to) in edges {
        adj[from].push(to);
    }
    let mut color = vec![Color::White; n];
    let mut stack: Vec<usize> = Vec::new();

    fn visit(
        u: usize,
        adj: &[Vec<usize>],
        color: &mut [Color],
        stack: &mut Vec<usize>,
        cohort: &[&ApplicabilityRecord],
    ) -> Result<(), ResolverError> {
        color[u] = Color::Gray;
        stack.push(u);
        for &v in &adj[u] {
            if color[v] == Color::Gray {
                let pos = stack.iter().position(|&x| x == v).unwrap();
                let loop_ids: Vec<String> = stack[pos..]
                    .iter()
                    .chain(std::iter::once(&v))
                    .map(|&i| cohort[i].norm_id())
                    .collect();
                return Err(ResolverError::other(format!(
                    "the \"supersedes\" relationship among the applicability records sharing recorded_at {} forms a cycle ({}); an append-only supersession chain cannot be cyclic, so it is fail-closed (rule-resolution.md §9)",
                    cohort[0].recorded_at(),
                    loop_ids.join(" → "),
                )));
            }
            if color[v] == Color::White {
                visit(v, adj, color, stack, cohort)?;
            }
        }
        color[u] = Color::Black;
        stack.pop();
        Ok(())
    }

    for i in 0..n {
        if color[i] == Color::White {
            visit(i, &adj, &mut color, &mut stack, cohort)?;
        }
    }
    Ok(())
}

struct CohortValidation<'a> {
    declares_ordering: bool,
    head: Option<&'a ApplicabilityRecord>,
}

fn validate_cohort_supersedes<'a>(
    cohort: &[&'a ApplicabilityRecord],
    group: &[&'a ApplicabilityRecord],
) -> Result<CohortValidation<'a>, ResolverError> {
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut superseded_by: HashSet<usize> = HashSet::new();
    for (i, &r) in cohort.iter().enumerate() {
        if let Some(target_idx) = resolve_supersedes_edge(r, cohort, group)? {
            edges.push((i, target_idx));
            superseded_by.insert(target_idx);
        }
    }
    if edges.is_empty() {
        return Ok(CohortValidation {
            declares_ordering: false,
            head: None,
        });
    }
    assert_no_supersedes_cycle(cohort, &edges)?;
    let heads: Vec<usize> = (0..cohort.len())
        .filter(|i| !superseded_by.contains(i))
        .collect();
    if heads.len() != 1 {
        let mut head_ids: Vec<String> = heads.iter().map(|&i| cohort[i].norm_id()).collect();
        head_ids.sort();
        return Err(ResolverError::other(format!(
            "norm \"{}\" has {} applicability records sharing recorded_at {} and their \"supersedes\" relationship leaves {} un-superseded head(s) ({}); a cohort that declares ordering must resolve to exactly one head, so it is fail-closed (rule-resolution.md §4, §9)",
            cohort[0].norm_id(),
            cohort.len(),
            cohort[0].recorded_at(),
            heads.len(),
            head_ids.join(", "),
        )));
    }
    Ok(CohortValidation {
        declares_ordering: true,
        head: Some(cohort[heads[0]]),
    })
}

fn pick_authoritative<'a>(
    records: &[&'a ApplicabilityRecord],
) -> Result<&'a ApplicabilityRecord, ResolverError> {
    let mut cohorts: BTreeMap<String, Vec<&'a ApplicabilityRecord>> = BTreeMap::new();
    for &r in records {
        cohorts
            .entry(r.recorded_at().as_str().to_string())
            .or_default()
            .push(r);
    }

    let mut validated: HashMap<String, CohortValidation<'a>> = HashMap::new();
    for (date, cohort) in &cohorts {
        validated.insert(date.clone(), validate_cohort_supersedes(cohort, records)?);
    }

    let latest_key = cohorts
        .keys()
        .next_back()
        .expect("group is non-empty")
        .clone();
    let latest = &cohorts[&latest_key];
    if latest.len() == 1 {
        return Ok(latest[0]);
    }

    let v = &validated[&latest_key];
    if !v.declares_ordering {
        return Err(ResolverError::other(format!(
            "norm \"{}\" has {} applicability records sharing the latest recorded_at {}; append-only precedence cannot be decided deterministically within the available identity/date, so it is fail-closed — no \"supersedes\" relationship orders them and JSON order is not a tie-breaker (rule-resolution.md §4, §9)",
            latest[0].norm_id(),
            latest.len(),
            latest_key,
        )));
    }
    Ok(v.head.expect("declares_ordering implies a resolved head"))
}

fn resolve_norms(
    sources: &ResolverSources,
    ctx: &Ctx,
    out: &mut ResolverOutput,
) -> Result<(), ResolverError> {
    let mut groups: HashMap<String, Vec<&ApplicabilityRecord>> = HashMap::new();
    for rec in &sources.applicability_records {
        groups.entry(rec.identity_key()).or_default().push(rec);
    }

    // Sort group keys before processing so a run's *error*, when one
    // occurs, is deterministic too — not load-bearing for a successful
    // resolution, whose arrays `finalize` sorts regardless.
    let mut keys: Vec<&String> = groups.keys().collect();
    keys.sort();

    for key in keys {
        let group = &groups[key];
        let tagged: Vec<(&ApplicabilityRecord, RecordClass)> = group
            .iter()
            .map(|&rec| classify_record(rec, ctx).map(|cls| (rec, cls)))
            .collect::<Result<_, _>>()?;

        let authoritative = pick_authoritative(group)?;
        let at_class = tagged
            .iter()
            .find(|(r, _)| std::ptr::eq(*r, authoritative))
            .map(|(_, c)| c)
            .expect("authoritative is a member of its own group");

        if matches!(at_class, RecordClass::NoMatch) {
            continue;
        }

        for (rec, cls) in &tagged {
            if !matches!(cls, RecordClass::NoMatch)
                && !matches!(rec.source(), ApplicabilitySource::Kernel)
            {
                resolve_intake_pointer(rec, sources)?;
            }
        }

        let authoritative_delivery = verify_provenance(authoritative, sources)?;

        match at_class {
            RecordClass::Applies => match authoritative.status() {
                ApplicabilityStatus::Resolved => {
                    out.applicable_norms.push(ApplicableNorm {
                        norm: authoritative.norm_id(),
                        activation_reason: authoritative.activation().kind(),
                        source: authoritative.source().kind(),
                        digest: authoritative.digest().clone(),
                        delivery: authoritative_delivery,
                    });
                }
                ApplicabilityStatus::Unresolved { resume_condition } => {
                    out.unresolved_applicability.push(Unresolved {
                        subject: authoritative.norm_id(),
                        reason: resume_condition.clone(),
                    });
                }
            },
            RecordClass::ActivationUndetermined => {
                out.unresolved_applicability.push(Unresolved {
                    subject: authoritative.norm_id(),
                    reason: "observed activation is \"undetermined\": whether this norm activates for the work item is not deterministically decidable (rule-resolution.md §4)".to_string(),
                });
            }
            RecordClass::NoMatch => unreachable!("handled above"),
        }
    }
    Ok(())
}

fn finalize(mut out: ResolverOutput) -> ResolverOutput {
    out.applicable_norms.sort_by(|a, b| {
        a.norm
            .cmp(&b.norm)
            .then_with(|| a.source.as_str().cmp(b.source.as_str()))
            .then_with(|| a.digest.value().cmp(b.digest.value()))
    });
    out.applicable_protocols.sort_by(|a, b| {
        a.protocol
            .cmp(&b.protocol)
            .then_with(|| a.routed_from.as_str().cmp(b.routed_from.as_str()))
    });
    out.required_verification = unique_sorted(out.required_verification);
    out.conflicts
        .sort_by(|a, b| a.norms.cmp(&b.norms).then_with(|| a.reason.cmp(&b.reason)));
    out.unresolved_applicability.sort_by(|a, b| {
        a.subject
            .cmp(&b.subject)
            .then_with(|| a.reason.cmp(&b.reason))
    });
    out.unresolved_items
        .sort_by(|a, b| a.item.cmp(&b.item).then_with(|| a.reason.cmp(&b.reason)));
    out.pre_decomposition_norms = unique_sorted(out.pre_decomposition_norms);
    out
}
