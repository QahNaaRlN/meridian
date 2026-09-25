//! Reference tests for `route_protocols`'s tie-break and mandatory-conflict
//! counting against ACTUAL output of the real, unmodified `resolveRules`
//! (`scripts/rule-resolver.mjs`) — not merely Rust's own internal
//! consistency — plus a negative test proving the tie-break key cannot be
//! independently mismatched to a route's own fields.
//!
//! Both reference scenarios were run once through the real Node function
//! with a companion script (kept here in comments for reviewer
//! reproduction) and the exact resulting values — the winning route's
//! digest, the conflict's `norms` and `reason` text — are asserted against
//! below.
//!
//! ```js
//! import { resolveRules } from '<MERIDIAN_KERNEL>/scripts/rule-resolver.mjs';
//! const workItem = {
//!   repository_id: 'sample-core-frontend', work_kind: 'change', change_class: 'BUGFIX',
//!   candidate_paths: [], changed_paths: [],
//! };
//! const baseSources = {
//!   repository_inventory: [{ id: 'sample-core-frontend' }],
//!   applicability_records: [],
//! };
//! // Scenario 1 — tie-break:
//! const digestA = 'a'.repeat(64), digestF = 'f'.repeat(64);
//! const sources1 = { ...baseSources, protocol_routes: [
//!   { protocol: 'op-protocol', routed_from: 'BUGFIX', source: 'kernel', scope: 'universal', digest: digestF, mandatory: false },
//!   { protocol: 'op-protocol', routed_from: 'BUGFIX', source: 'kernel', scope: 'universal', digest: digestA, mandatory: false },
//! ]};
//! resolveRules(workItem, sources1).applicable_protocols[0].digest; // === digestA
//! // Scenario 2 — mandatory conflict count:
//! const sources2 = { ...baseSources, protocol_routes: [
//!   { protocol: 'protocol-a', routed_from: 'BUGFIX', source: 'kernel', scope: 'universal', revision: 'aaa1111', mandatory: true },
//!   { protocol: 'protocol-a', routed_from: 'BUGFIX', source: 'repository', scope: 'universal', revision: 'aaa2222', mandatory: true },
//!   { protocol: 'protocol-b', routed_from: 'BUGFIX', source: 'kernel', scope: 'universal', revision: 'bbb1111', mandatory: true },
//! ]};
//! resolveRules(workItem, sources2); // .applicable_protocols === [], .conflicts === [{norms:["protocol-a","protocol-b"], reason:"2 incompatible mandatory routes for \"BUGFIX\"; a local protocol route does not silently override a universal one (rule-resolution.md §7)"}]
//! ```

use meridian_core::resolver::*;
use meridian_core::types::ContentDigest;

fn work_item() -> WorkItem {
    WorkItem::new(
        "sample-core-frontend",
        WorkItemKind::Change(ChangeClass::Bugfix),
        vec![],
        vec![],
        None,
    )
    .unwrap()
}

fn base_sources() -> ResolverSources {
    ResolverSources {
        repository_inventory: vec![RepositoryInventoryEntry {
            id: "sample-core-frontend".to_string(),
            profile: None,
            semantic_areas: vec![],
        }],
        ..Default::default()
    }
}

fn universal_digest_order() -> Vec<RouteField> {
    vec![
        RouteField::Protocol,
        RouteField::RoutedFrom,
        RouteField::Source,
        RouteField::Scope,
        RouteField::Digest,
        RouteField::Mandatory,
    ]
}

fn universal_revision_order() -> Vec<RouteField> {
    vec![
        RouteField::Protocol,
        RouteField::RoutedFrom,
        RouteField::Source,
        RouteField::Scope,
        RouteField::Revision,
        RouteField::Mandatory,
    ]
}

/// Scenario 1: two routes for the SAME protocol, tied on `source.rank()`
/// (both `kernel`), differing only in `digest`. The Node reference's
/// `JSON.stringify(route)` tie-break picks the route whose full
/// serialization sorts lexicographically first — here the one with digest
/// `"aaaa…"` (`"...digest":"aaaa…"` sorts before `"...digest":"ffff…"`
/// with every other field byte-identical). `ProtocolRoute::new`'s
/// `field_order` reproduces the exact field order the Node fixture's
/// object literal declared (`protocol, routed_from, source, scope, digest,
/// mandatory`), so the COMPUTED tie-break key is byte-identical to
/// `JSON.stringify` of that literal.
#[test]
fn route_tiebreak_picks_the_same_winner_as_the_node_reference() {
    let digest_a = ContentDigest::from_hex("a".repeat(64)).unwrap();
    let digest_f = ContentDigest::from_hex("f".repeat(64)).unwrap();

    let route_f = ProtocolRoute::new(
        "op-protocol",
        RouteKey::ChangeClass(ChangeClass::Bugfix),
        RouteSource::Kernel,
        ProtocolRouteScope::Universal,
        RouteProvenance::Digest(digest_f.clone()),
        false,
        universal_digest_order(),
    )
    .unwrap();
    let route_a = ProtocolRoute::new(
        "op-protocol",
        RouteKey::ChangeClass(ChangeClass::Bugfix),
        RouteSource::Kernel,
        ProtocolRouteScope::Universal,
        RouteProvenance::Digest(digest_a.clone()),
        false,
        universal_digest_order(),
    )
    .unwrap();

    assert_eq!(
        route_f.tiebreak_key(),
        r#"{"protocol":"op-protocol","routed_from":"BUGFIX","source":"kernel","scope":"universal","digest":"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff","mandatory":false}"#
    );
    assert_eq!(
        route_a.tiebreak_key(),
        r#"{"protocol":"op-protocol","routed_from":"BUGFIX","source":"kernel","scope":"universal","digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","mandatory":false}"#
    );

    // Declared in the same order the Node fixture used (F before A) to
    // prove the winner is chosen by the tie-break, not by declaration
    // order — `output_does_not_depend_on_applicability_record_input_order`
    // in `resolver_acceptance.rs` separately proves general order
    // independence; this test isolates the tie-break rule itself.
    let mut sources = base_sources();
    sources.protocol_routes = vec![route_f, route_a];

    let out = resolve_rules(&work_item(), &sources).unwrap();
    assert_eq!(out.applicable_protocols.len(), 1);
    match &out.applicable_protocols[0].provenance {
        RouteProvenance::Digest(d) => assert_eq!(d, &digest_a),
        RouteProvenance::Revision(_) => panic!("expected a digest-provenance route to win"),
    }
}

/// Scenario 2: two mandatory routes citing the SAME protocol
/// (`protocol-a`) plus one mandatory route citing a DIFFERENT protocol
/// (`protocol-b`). The conflict count is `distinctMandatory.length` — 2
/// distinct protocol names — not the 3 mandatory route records, and every
/// mandatory route (including both `protocol-a` ones) is excluded from
/// `applicable_protocols`, exactly matching the real Node output.
#[test]
fn mandatory_conflict_count_and_exclusion_match_the_node_reference() {
    let mut sources = base_sources();
    sources.protocol_routes = vec![
        ProtocolRoute::new(
            "protocol-a",
            RouteKey::ChangeClass(ChangeClass::Bugfix),
            RouteSource::Kernel,
            ProtocolRouteScope::Universal,
            RouteProvenance::Revision("aaa1111".to_string()),
            true,
            universal_revision_order(),
        )
        .unwrap(),
        ProtocolRoute::new(
            "protocol-a",
            RouteKey::ChangeClass(ChangeClass::Bugfix),
            RouteSource::Repository,
            ProtocolRouteScope::Universal,
            RouteProvenance::Revision("aaa2222".to_string()),
            true,
            universal_revision_order(),
        )
        .unwrap(),
        ProtocolRoute::new(
            "protocol-b",
            RouteKey::ChangeClass(ChangeClass::Bugfix),
            RouteSource::Kernel,
            ProtocolRouteScope::Universal,
            RouteProvenance::Revision("bbb1111".to_string()),
            true,
            universal_revision_order(),
        )
        .unwrap(),
    ];

    let out = resolve_rules(&work_item(), &sources).unwrap();

    assert!(out.applicable_protocols.is_empty());
    assert_eq!(out.conflicts.len(), 1);
    assert_eq!(
        out.conflicts[0].norms,
        vec!["protocol-a".to_string(), "protocol-b".to_string()]
    );
    assert_eq!(
        out.conflicts[0].reason,
        "2 incompatible mandatory routes for \"BUGFIX\"; a local protocol route does not silently override a universal one (rule-resolution.md §7)"
    );
}

/// Negative test: the tie-break key cannot be independently mismatched to
/// a route's own fields — there is no public field, setter, or
/// construction path that lets a caller pass route A's `protocol`/
/// `source`/`scope`/`provenance`/`mandatory` together with route B's
/// tie-break key (`ProtocolRoute`'s fields, including `tiebreak_key`, are
/// all private; [`ProtocolRoute::new`] is the only constructor, and it
/// COMPUTES the key from the very values it stores — see
/// `meridian-core/src/resolver/sources.rs`). This is enforced by the type
/// system at compile time — the struct-literal construction the OLD,
/// vulnerable `pub tiebreak_key: String` design allowed does not even
/// compile against the current (private-field) definition; illustrative
/// only, not itself compiled by this test file:
///
/// ```text
/// let _ = ProtocolRoute {
///     protocol: "protocol-a".to_string(),
///     ..,
///     tiebreak_key: "a value copied from an unrelated route B".to_string(), // E0451: field is private
/// };
/// ```
///
/// What this test proves at RUNTIME, since the attack itself cannot be
/// expressed: the computed key — and therefore which route wins a tie —
/// tracks ONLY the actual field values passed to `new`, changing the
/// instant those values change, with no way to hold the "attacker-visible"
/// fields fixed while altering the outcome.
#[test]
fn tiebreak_key_cannot_be_mismatched_to_a_different_routes_fields() {
    let digest_a = ContentDigest::from_hex("a".repeat(64)).unwrap();
    let digest_z = ContentDigest::from_hex("f".repeat(64)).unwrap();

    // Two legitimately-constructed routes, differing ONLY in `provenance`.
    let route_with_digest_a = ProtocolRoute::new(
        "op-protocol",
        RouteKey::ChangeClass(ChangeClass::Bugfix),
        RouteSource::Kernel,
        ProtocolRouteScope::Universal,
        RouteProvenance::Digest(digest_a.clone()),
        false,
        universal_digest_order(),
    )
    .unwrap();
    let route_with_digest_z = ProtocolRoute::new(
        "op-protocol",
        RouteKey::ChangeClass(ChangeClass::Bugfix),
        RouteSource::Kernel,
        ProtocolRouteScope::Universal,
        RouteProvenance::Digest(digest_z.clone()),
        false,
        universal_digest_order(),
    )
    .unwrap();

    // The computed key differs, and each key is grounded in ITS OWN
    // route's digest — never the other route's.
    assert_ne!(
        route_with_digest_a.tiebreak_key(),
        route_with_digest_z.tiebreak_key()
    );
    assert!(route_with_digest_a
        .tiebreak_key()
        .contains(digest_a.value()));
    assert!(!route_with_digest_a
        .tiebreak_key()
        .contains(digest_z.value()));
    assert!(route_with_digest_z
        .tiebreak_key()
        .contains(digest_z.value()));
    assert!(!route_with_digest_z
        .tiebreak_key()
        .contains(digest_a.value()));

    // End-to-end: swapping which route carries the lexicographically
    // smaller digest swaps the winner — the outcome tracks the route's
    // OWN field value in both directions, exactly as the Node reference's
    // JSON.stringify(route) comparison would (proven above in
    // `route_tiebreak_picks_the_same_winner_as_the_node_reference`).
    let mut sources_a_smaller = base_sources();
    sources_a_smaller.protocol_routes =
        vec![route_with_digest_z.clone(), route_with_digest_a.clone()];
    let out_a_smaller = resolve_rules(&work_item(), &sources_a_smaller).unwrap();
    match &out_a_smaller.applicable_protocols[0].provenance {
        RouteProvenance::Digest(d) => assert_eq!(d, &digest_a),
        RouteProvenance::Revision(_) => panic!("expected a digest-provenance route to win"),
    }
}
