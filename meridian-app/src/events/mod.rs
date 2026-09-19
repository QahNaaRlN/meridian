//! Observed events — a versioned, closed-vocabulary envelope for what a
//! command did, and a port to record it
//! (`meridian-rust-migration-program-plan.md` §5.4, item 5;
//! `governance/research/hypothesis-registry.yaml`,
//! `meridian-hypothesis-model-independent-decision-trace`).
//!
//! An [`ObservedEvent`] can describe input context, a source access, a
//! found reference, a tool invocation, a change, a check, an error, a
//! correction, or an outcome — the closed set [`EventKind`] names. None of
//! the nine [`EventKind`] variants names a model's internal chain of
//! reasoning, and [`ObservedEvent`] carries no structured, arbitrary detail
//! field a caller could use to attach one (an earlier revision did; it was
//! removed — see [`ObservedEvent`]'s own documentation). What this module
//! does **not** guarantee: `summary` is a plain, unbounded string, and
//! nothing in this type system stops a caller from writing an extended
//! reasoning trace into that string itself — that is a matter of what
//! callers choose to pass, not a type-level closure this package provides.
//! This module does not decide when an event is emitted or by which
//! command — that belongs to the CLI packages that use it
//! (`meridian-cli-foundation`, `meridian-cli-migration`), not this
//! corrective package.

mod envelope;
mod sink;

pub use envelope::{EventEnvelopeVersion, EventKind, ObservedEvent};
pub use sink::{EventSink, NoOpEventSink};
