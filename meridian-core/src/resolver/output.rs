//! [`ResolverOutput`] — the resolver's return value
//! (`registries/rule-resolution/resolver-output.schema.json`).

use core::fmt;

use crate::types::ContentDigest;

use super::applicability::{ActivationKind, ApplicabilitySourceKind, Delivery};
use super::work_item::{ChangeClass, WorkKind};

/// One applicable norm (`resolver-output.schema.json`
/// `definitions.applicable_norm`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApplicableNorm {
    pub norm: String,
    pub activation_reason: ActivationKind,
    pub source: ApplicabilitySourceKind,
    pub digest: ContentDigest,
    pub delivery: Delivery,
}

/// The `work_kind`/`change_class` value a protocol route was keyed on
/// (`resolver-output.schema.json` `applicable_protocol.routed_from`: one
/// pool because a route may be keyed on either).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteKey {
    WorkKind(WorkKind),
    ChangeClass(ChangeClass),
}

impl RouteKey {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteKey::WorkKind(w) => w.as_str(),
            RouteKey::ChangeClass(c) => c.as_str(),
        }
    }
}

impl fmt::Display for RouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a protocol route came from (`rule-resolution.md` §7) — a
/// deliberately different three-value pool than
/// [`ApplicabilitySourceKind`], because a norm and a route answer different
/// questions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteSource {
    Kernel,
    Instance,
    Repository,
}

impl RouteSource {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteSource::Kernel => "kernel",
            RouteSource::Instance => "instance",
            RouteSource::Repository => "repository",
        }
    }

    /// `sourceRank` (`rule-resolver.mjs` `routeProtocols`): kernel < instance
    /// < repository, used to pick the more specific route when several
    /// reach the same protocol key.
    pub(super) fn rank(self) -> u8 {
        match self {
            RouteSource::Kernel => 0,
            RouteSource::Instance => 1,
            RouteSource::Repository => 2,
        }
    }
}

impl fmt::Display for RouteSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The scope of a protocol route (`resolver-output.schema.json`
/// `applicable_protocol.scope`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteScope {
    Universal,
    ProductDomain,
    Repository,
}

impl RouteScope {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteScope::Universal => "universal",
            RouteScope::ProductDomain => "product-domain",
            RouteScope::Repository => "repository",
        }
    }
}

impl fmt::Display for RouteScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A route record carries either a digest or a revision, whichever its own
/// store identifies it by (`resolver-output.schema.json`
/// `applicable_protocol`: `anyOf [digest, revision]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RouteProvenance {
    Digest(ContentDigest),
    Revision(String),
}

/// One applicable protocol route (`resolver-output.schema.json`
/// `definitions.applicable_protocol`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApplicableProtocol {
    pub protocol: String,
    pub routed_from: RouteKey,
    pub source: RouteSource,
    pub scope: RouteScope,
    pub provenance: RouteProvenance,
}

/// Two or more norms/routes in conflict (`resolver-output.schema.json`
/// `definitions.conflict`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Conflict {
    pub norms: Vec<String>,
    pub reason: String,
}

/// A norm whose applicability could not be decided deterministically
/// (`resolver-output.schema.json` `definitions.unresolved`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Unresolved {
    pub subject: String,
    pub reason: String,
}

/// One missing input blocking an initiative's decomposition
/// (`resolver-output.schema.json` `definitions.decomposition_blocker`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DecompositionBlocker {
    pub item: String,
    pub reason: String,
}

/// The resolver's full return value (`resolver-output.schema.json`).
///
/// Every field is always present — the schema keeps the same shape whether
/// or not each array happens to be empty — and array order is
/// deterministic ([`crate::resolver::resolve_rules`] sorts every array
/// before returning).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolverOutput {
    pub applicable_norms: Vec<ApplicableNorm>,
    pub applicable_protocols: Vec<ApplicableProtocol>,
    pub required_verification: Vec<String>,
    pub conflicts: Vec<Conflict>,
    pub unresolved_applicability: Vec<Unresolved>,
    pub unresolved_items: Vec<DecompositionBlocker>,
    pub requires_reresolution: bool,
    pub decomposition_required: bool,
    pub applicable_initiative_protocol: Option<String>,
    pub pre_decomposition_norms: Vec<String>,
}
