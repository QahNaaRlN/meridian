//! The environmental data `resolve_rules` takes by dependency injection
//! (`rule-resolver.mjs`'s `sources` parameter) — everything the resolver
//! needs beyond the work item itself, supplied by the caller rather than
//! read from a file, an environment variable or a global.

use std::collections::{HashMap, HashSet};

use crate::json::Json;

use super::applicability::{ApplicabilityRecord, Delivery, IntakeVerdict};
use super::date::IsoDate;
use super::output::{RouteKey, RouteProvenance, RouteScope, RouteSource};

/// One repository inventory entry
/// (`rule-resolver.mjs`: `repository_inventory`). Read structurally, not
/// schema-validated by the pure resolver core — `rule-resolver.mjs` itself
/// does not run `validate()` against this array either, only against
/// `applicability_records`, so no field-format check is added here that
/// the Node reference does not also perform.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepositoryInventoryEntry {
    pub id: String,
    pub profile: Option<String>,
    pub semantic_areas: Vec<String>,
}

/// A value rejected while building a [`ProtocolRoute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolRouteError {
    /// A `repository`/`product_domain` scope selector was the empty string.
    EmptySelector { field: &'static str },
    /// `field_order` named a field more than once.
    DuplicateField { field: RouteField },
    /// `field_order` did not name EXACTLY the set of fields this route's
    /// own `scope`/`provenance` require — missing one, or naming one this
    /// route does not carry.
    FieldOrderMismatch {
        /// The exact set `field_order` should have named, for diagnosis.
        expected: Vec<RouteField>,
    },
}

impl std::fmt::Display for ProtocolRouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolRouteError::EmptySelector { field } => {
                write!(f, "{field} selector must not be empty")
            }
            ProtocolRouteError::DuplicateField { field } => {
                write!(f, "field_order names {field:?} more than once")
            }
            ProtocolRouteError::FieldOrderMismatch { expected } => {
                write!(
                    f,
                    "field_order does not name exactly {expected:?}, each once"
                )
            }
        }
    }
}

impl std::error::Error for ProtocolRouteError {}

/// The name of one field a source protocol-route document declared, used
/// ONLY to reproduce the exact field ORDER that document used —
/// `route_protocols`'s tie-break among routes sharing a protocol and
/// `source.rank()` (`rule-resolver.mjs`'s `byProtocol` map) compares
/// `JSON.stringify(route)`, and that string depends on the source
/// document's own key order, which `meridian-core` does not otherwise
/// need. See [`ProtocolRoute::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteField {
    Protocol,
    RoutedFrom,
    Source,
    Scope,
    Repository,
    ProductDomain,
    Digest,
    Revision,
    Mandatory,
}

/// The scope of an injected protocol route, together with the exact scope
/// selector `rule-resolver.mjs`'s `routeInScope` requires for that scope —
/// a repository route with no selector cannot be constructed, so the
/// "route carries no exact selector" fail-closed case Node checks at
/// resolution time is instead impossible to represent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProtocolRouteScope {
    Universal,
    Repository { repository: String },
    ProductDomain { product_domain: String },
}

impl ProtocolRouteScope {
    pub fn repository(repository: impl Into<String>) -> Result<Self, ProtocolRouteError> {
        let repository = repository.into();
        if repository.is_empty() {
            return Err(ProtocolRouteError::EmptySelector {
                field: "repository",
            });
        }
        Ok(ProtocolRouteScope::Repository { repository })
    }

    pub fn product_domain(product_domain: impl Into<String>) -> Result<Self, ProtocolRouteError> {
        let product_domain = product_domain.into();
        if product_domain.is_empty() {
            return Err(ProtocolRouteError::EmptySelector {
                field: "product_domain",
            });
        }
        Ok(ProtocolRouteScope::ProductDomain { product_domain })
    }

    pub fn route_scope(&self) -> RouteScope {
        match self {
            ProtocolRouteScope::Universal => RouteScope::Universal,
            ProtocolRouteScope::Repository { .. } => RouteScope::Repository,
            ProtocolRouteScope::ProductDomain { .. } => RouteScope::ProductDomain,
        }
    }
}

/// An injected protocol route record (`rule-resolver.mjs`:
/// `protocol_routes`) — deliberately temporary, per the Node reference's own
/// comment: no canonical registry or schema for protocol routes exists yet.
///
/// [`ProtocolRoute::new`] is the ONLY constructor. It COMPUTES the
/// `JSON.stringify(route)`-equivalent tie-break key
/// ([`ProtocolRoute::tiebreak_key`]) FROM this route's own field values and
/// a caller-supplied field ORDER — never from an independently-suppliable
/// opaque string. Every field is private, so a caller cannot construct a
/// value with one route's fields and a mismatched tie-break key: the key is
/// always a pure function of the SAME field values `protocol`/
/// `routed_from`/`source`/`scope`/`provenance`/`mandatory` return.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolRoute {
    protocol: String,
    routed_from: RouteKey,
    source: RouteSource,
    scope: ProtocolRouteScope,
    provenance: RouteProvenance,
    mandatory: bool,
    tiebreak_key: String,
}

impl ProtocolRoute {
    /// `field_order` must name EXACTLY the fields this route's own `scope`
    /// and `provenance` require — `Protocol`/`RoutedFrom`/`Source`/`Scope`/
    /// `Mandatory` always; `Repository`/`ProductDomain` iff `scope` is that
    /// variant; `Digest`/`Revision` iff `provenance` is that variant — each
    /// exactly once, in the order the source protocol-route document
    /// declared them (`rule-resolver.mjs`'s injected route shape: `{
    /// protocol, routed_from, source, scope, repository?, product_domain?,
    /// digest?|revision?, mandatory }`). A caller cannot lie about which
    /// fields this route carries — only about the ORDER of the fields it
    /// is otherwise required to name — so the computed tie-break key is
    /// always grounded in this route's actual field set and values.
    pub fn new(
        protocol: impl Into<String>,
        routed_from: RouteKey,
        source: RouteSource,
        scope: ProtocolRouteScope,
        provenance: RouteProvenance,
        mandatory: bool,
        field_order: Vec<RouteField>,
    ) -> Result<Self, ProtocolRouteError> {
        let protocol = protocol.into();

        let mut expected = vec![
            RouteField::Protocol,
            RouteField::RoutedFrom,
            RouteField::Source,
            RouteField::Scope,
        ];
        match &scope {
            ProtocolRouteScope::Universal => {}
            ProtocolRouteScope::Repository { .. } => expected.push(RouteField::Repository),
            ProtocolRouteScope::ProductDomain { .. } => expected.push(RouteField::ProductDomain),
        }
        match &provenance {
            RouteProvenance::Digest(_) => expected.push(RouteField::Digest),
            RouteProvenance::Revision(_) => expected.push(RouteField::Revision),
        }
        expected.push(RouteField::Mandatory);

        let mut seen: HashSet<RouteField> = HashSet::with_capacity(field_order.len());
        for &field in &field_order {
            if !seen.insert(field) {
                return Err(ProtocolRouteError::DuplicateField { field });
            }
        }
        let expected_set: HashSet<RouteField> = expected.iter().copied().collect();
        if seen != expected_set {
            return Err(ProtocolRouteError::FieldOrderMismatch { expected });
        }

        let tiebreak_key = compute_tiebreak_key(
            &protocol,
            routed_from,
            source,
            &scope,
            &provenance,
            mandatory,
            &field_order,
        );

        Ok(Self {
            protocol,
            routed_from,
            source,
            scope,
            provenance,
            mandatory,
            tiebreak_key,
        })
    }

    pub fn protocol(&self) -> &str {
        &self.protocol
    }
    pub fn routed_from(&self) -> RouteKey {
        self.routed_from
    }
    pub fn source(&self) -> RouteSource {
        self.source
    }
    pub fn scope(&self) -> &ProtocolRouteScope {
        &self.scope
    }
    pub fn provenance(&self) -> &RouteProvenance {
        &self.provenance
    }
    pub fn mandatory(&self) -> bool {
        self.mandatory
    }
    /// The `JSON.stringify(route)`-equivalent tie-break key, COMPUTED at
    /// construction from this route's own field values — never an
    /// independently-suppliable opaque string. See [`ProtocolRoute::new`].
    pub fn tiebreak_key(&self) -> &str {
        &self.tiebreak_key
    }
}

/// Builds the tie-break key as a JSON object literal (declared field
/// order, never sorted — matching `JSON.stringify` on the source route
/// object) from `field_order` and this route's own field values.
#[allow(clippy::too_many_arguments)]
fn compute_tiebreak_key(
    protocol: &str,
    routed_from: RouteKey,
    source: RouteSource,
    scope: &ProtocolRouteScope,
    provenance: &RouteProvenance,
    mandatory: bool,
    field_order: &[RouteField],
) -> String {
    let entries: Vec<(&'static str, Json)> = field_order
        .iter()
        .map(|field| match field {
            RouteField::Protocol => ("protocol", Json::str(protocol)),
            RouteField::RoutedFrom => ("routed_from", Json::str(routed_from.as_str())),
            RouteField::Source => ("source", Json::str(source.as_str())),
            RouteField::Scope => ("scope", Json::str(scope.route_scope().as_str())),
            RouteField::Repository => {
                let repository = match scope {
                    ProtocolRouteScope::Repository { repository } => repository.as_str(),
                    _ => unreachable!("ProtocolRoute::new validated field_order against this route's own scope"),
                };
                ("repository", Json::str(repository))
            }
            RouteField::ProductDomain => {
                let product_domain = match scope {
                    ProtocolRouteScope::ProductDomain { product_domain } => product_domain.as_str(),
                    _ => unreachable!("ProtocolRoute::new validated field_order against this route's own scope"),
                };
                ("product_domain", Json::str(product_domain))
            }
            RouteField::Digest => {
                let digest = match provenance {
                    RouteProvenance::Digest(d) => d.value(),
                    _ => unreachable!("ProtocolRoute::new validated field_order against this route's own provenance"),
                };
                ("digest", Json::str(digest))
            }
            RouteField::Revision => {
                let revision = match provenance {
                    RouteProvenance::Revision(r) => r.as_str(),
                    _ => unreachable!("ProtocolRoute::new validated field_order against this route's own provenance"),
                };
                ("revision", Json::str(revision))
            }
            RouteField::Mandatory => ("mandatory", Json::Bool(mandatory)),
        })
        .collect();
    Json::Literal(entries).to_canonical_string()
}

/// The target a verification route applies to (`rule-resolver.mjs`:
/// `verification_routes`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VerificationTarget {
    /// `applies_to: "*"` — every applicable norm.
    All,
    /// `applies_to: <norm id>` — one specific norm.
    Norm(String),
}

/// One injected verification route.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VerificationRoute {
    pub applies_to: VerificationTarget,
    pub path: String,
}

/// One intake register's records, as needed to resolve an
/// [`super::applicability::IntakePointer`]
/// (`rule-resolver.mjs`: `intake_registers`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntakeRecordEntry {
    pub artifact: String,
    pub region: Option<String>,
    pub recorded_at: IsoDate,
    pub verdict: IntakeVerdict,
    pub delivery: Delivery,
}

/// One intake register (`rule-resolver.mjs`: `intake_registers`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntakeRegister {
    pub register: String,
    pub records: Vec<IntakeRecordEntry>,
}

/// A prior resolution's `candidate_paths`, for `requires_reresolution`
/// (`rule-resolution.md` §9 / `rule-resolver.mjs`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreviousResolution {
    pub candidate_paths: Vec<String>,
}

/// The explicit decomposition of an initiative into child work items
/// (`rule-resolution.md` §6). Only presence/non-emptiness of
/// `child_work_items` affects resolution (`rule-resolver.mjs`:
/// `decomp.child_work_items.length > 0`); the identifiers themselves are
/// carried for completeness.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Decomposition {
    pub child_work_items: Vec<String>,
}

/// One behavior-change finding surfaced during a `REFACTOR` work item
/// (`rule-resolution.md` §3.2). Only the presence of at least one finding
/// affects resolution; `finding_type` is carried for completeness.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefactorFinding {
    pub finding_type: String,
}

/// Previous-resolution / decomposition / initiative data
/// (`rule-resolver.mjs`: `prior_state`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PriorState {
    pub previous_resolution: Option<PreviousResolution>,
    pub decomposition: Option<Decomposition>,
    pub initiative_protocol: Option<String>,
    pub pre_decomposition_norms: Vec<String>,
    pub source_repository_ids: Option<Vec<String>>,
    pub refactor_findings: Vec<RefactorFinding>,
}

/// All environmental data `resolve_rules` needs, supplied explicitly by the
/// caller (`rule-resolver.mjs`'s `sources` parameter — dependency
/// injection, no hidden default).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolverSources {
    pub repository_inventory: Vec<RepositoryInventoryEntry>,
    pub applicability_records: Vec<ApplicabilityRecord>,
    pub intake_registers: Vec<IntakeRegister>,
    pub protocol_routes: Vec<ProtocolRoute>,
    pub verification_routes: Vec<VerificationRoute>,
    /// Current norm text, keyed by `ApplicabilityRecord::norm_text_key`, for
    /// digest recomputation (`rule-resolution.md` §9).
    pub norm_texts: HashMap<String, String>,
    pub prior_state: PriorState,
}
