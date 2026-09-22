//! Pure half of the `instruction-topics` check (package 7, subpackage 7a):
//! `standards/workspace/instruction-topics.{yaml,md}` must declare the same
//! topic pool from both a machine-readable list and a human-signed table.
//! Transport parsing (YAML, the marked-region table-row extraction, and the
//! JS-truthy `"topics"` type handling) stays in
//! `meridian_app::validation::mechanical_integrity::instruction_topics`; this
//! module owns the pool-agreement predicate (shared with `stack-profiles`
//! via [`crate::mechanical_integrity::ordered_pool_agreement`]) and the typed
//! pool it produces on agreement.

use std::borrow::Borrow;
use std::collections::BTreeSet;
use std::fmt;

use crate::mechanical_integrity::{fail, ordered_pool_agreement};
use crate::types::Diagnostic;

/// A value rejected by [`TopicId::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopicIdError {
    Empty,
}

impl fmt::Display for TopicIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TopicIdError::Empty => write!(f, "topic id must not be empty"),
        }
    }
}

impl std::error::Error for TopicIdError {}

/// A validated, non-empty topic identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TopicId(String);

impl TopicId {
    pub fn new(value: impl Into<String>) -> Result<Self, TopicIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(TopicIdError::Empty);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TopicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Borrow<str> for TopicId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// The declared topic pool — present only once the two halves parsed and
/// agreed. Consumed by `agent-instruction-identity`, which must never reject
/// a topic against a pool this check itself already rejected.
pub type TopicPool = BTreeSet<TopicId>;

/// Neither `instruction-topics.yaml` nor `instruction-topics.md` could be
/// read.
pub fn missing_pool_files(missing: &[&str]) -> Diagnostic {
    fail(format!(
        "instruction-topics: the topic pool is missing from the Kernel ({}); no topic in any register could be checked against it, and an unreadable pool is a defect of the Kernel, not a reason to skip the check",
        missing.join(", ")
    ))
}

/// `"topics"` was present but not a JSON array (and not one of the several
/// JS-falsy scalars the transport layer already treats as an empty, legal
/// pool).
pub fn topics_not_a_list(found: &str) -> Diagnostic {
    fail(format!(
        "instruction-topics: \"topics\" must be a list, found {found}"
    ))
}

/// The marked `topic-pool` region of `instruction-topics.md` could not be
/// read.
pub fn region_unreadable(error: &str) -> Diagnostic {
    fail(format!(
        "instruction-topics: the pool region of instruction-topics.md is not readable — {error}; the signatures the gate compares against are the ones inside the markers, and nothing else"
    ))
}

/// Neither half's failure — every name here already agreed with a
/// documented signature (whose row pattern requires at least one
/// character), so an empty declared topic id is structurally impossible to
/// reach this point without [`ordered_pool_agreement`] already having
/// produced the disagreement diagnostic instead. Kept as an explicit, typed
/// outcome rather than a silently-dropping `filter_map` — a construction
/// failure this function did not expect is reported, not discarded.
pub fn internal_pool_construction_failure(
    check_name: &str,
    value: &str,
    error: &str,
) -> Diagnostic {
    fail(format!(
        "{check_name}: internal error building the agreed pool from \"{value}\": {error}; every declared name should already have been validated by the agreement check above"
    ))
}

/// Compares the declared (data) and documented (table) halves of the pool,
/// both already in first-seen order. `Ok` carries the agreed, typed
/// [`TopicPool`] — a value that, by construction, can never contain an empty
/// topic id. `Err` carries the one disagreement diagnostic, or (only were
/// the structural guarantee above ever violated)
/// [`internal_pool_construction_failure`].
pub fn check_pool_agreement(
    declared_ordered: &[String],
    documented_ordered: &[String],
) -> Result<TopicPool, Diagnostic> {
    if let Some(diagnostic) =
        ordered_pool_agreement("instruction-topics", declared_ordered, documented_ordered)
    {
        return Err(diagnostic);
    }
    let mut pool = TopicPool::new();
    for name in declared_ordered {
        let id = TopicId::new(name.as_str()).map_err(|error| {
            internal_pool_construction_failure("instruction-topics", name, &error.to_string())
        })?;
        pool.insert(id);
    }
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agreeing_halves_produce_the_pool() {
        let declared = vec!["a".to_string(), "b".to_string()];
        let documented = vec!["b".to_string(), "a".to_string()];
        let pool = check_pool_agreement(&declared, &documented).unwrap();
        assert_eq!(pool.len(), 2);
        assert!(pool.contains("a"));
    }

    #[test]
    fn disagreeing_halves_produce_one_diagnostic() {
        let declared = vec!["a".to_string()];
        let documented: Vec<String> = Vec::new();
        let error = check_pool_agreement(&declared, &documented).unwrap_err();
        assert!(error.message().contains("disagree"));
    }

    #[test]
    fn topic_id_rejects_empty() {
        assert_eq!(TopicId::new("").unwrap_err(), TopicIdError::Empty);
        assert!(TopicId::new("agent-conduct").is_ok());
    }
}
