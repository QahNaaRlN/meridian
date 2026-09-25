//! The future Knowledge Resolver — a reserved, separate architectural
//! boundary from [`crate::rule_resolution`]
//! (`meridian-rust-migration-program-plan.md` §5.4, item 6;
//! `governance/meridian-owner-intent-contract.md` §17;
//! `governance/research/hypothesis-registry.yaml`,
//! `meridian-hypothesis-deterministic-navigation`).
//!
//! [`crate::rule_resolution`] computes mandatory norms, protocols and gates
//! from task context — a Rule Resolver result. A future Knowledge Resolver
//! would compute *relevant knowledge* and its provenance instead: a
//! different result type, different inputs, and a different authority. The
//! owner intent contract requires the two never be merged into one
//! resolver, and a knowledge object found by one is never treated as a
//! norm the other would have produced.
//!
//! This module intentionally contains no resolver, no index, no search, no
//! ranking, no vector store, and no storage adapter of any kind. Reserving
//! this name and this boundary — so a later package has a place to land
//! without redesigning `meridian-app`'s module layout — is everything
//! package `knowledge-agent-foundation` does here. Building any of the
//! above is explicitly out of this package's scope and gated by the
//! research hypothesis above, not by this module's existence.
