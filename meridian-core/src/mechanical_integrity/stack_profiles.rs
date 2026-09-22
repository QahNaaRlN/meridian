//! Pure half of the `stack-profiles` check (package 7, subpackage 7a) — the
//! pool agreement only: `stack-profiles/stack-profiles.{yaml,md}` names and
//! signatures must agree, and `"universal"` (the declared absence of a
//! profile) must never appear in the pool itself. The per-repository
//! declaration half stays the still-blocked, unchanged
//! `BLOCKED_CHECKS` advisory this package does not wire an `--instance` flag
//! for.

use std::borrow::Borrow;
use std::collections::BTreeSet;
use std::fmt;

use crate::mechanical_integrity::{fail, ordered_pool_agreement};
use crate::types::Diagnostic;

/// A value rejected by [`StackProfileName::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackProfileNameError {
    Empty,
    /// `"universal"` is the declared absence of a profile, not a profile —
    /// the type itself makes this state unrepresentable in a loaded pool,
    /// rather than relying on every reader of a raw pool value to remember
    /// to re-check it.
    Universal,
}

impl fmt::Display for StackProfileNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StackProfileNameError::Empty => write!(f, "stack profile name must not be empty"),
            StackProfileNameError::Universal => {
                write!(f, "\"universal\" is not a stack profile")
            }
        }
    }
}

impl std::error::Error for StackProfileNameError {}

/// A validated stack-profile name: non-empty, and never `"universal"`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StackProfileName(String);

impl StackProfileName {
    pub fn new(value: impl Into<String>) -> Result<Self, StackProfileNameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(StackProfileNameError::Empty);
        }
        if value == "universal" {
            return Err(StackProfileNameError::Universal);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StackProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Borrow<str> for StackProfileName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// The agreed, `"universal"`-free stack-profile pool.
pub type StackProfilePool = BTreeSet<StackProfileName>;

/// Neither `stack-profiles.yaml` nor `stack-profiles.md` could be read.
pub fn missing_pool_files(missing: &[&str]) -> Diagnostic {
    fail(format!(
        "stack-profiles: the profile pool is missing from the Kernel ({}); no declaration could be checked against it, and an unreadable pool is a defect of the Kernel, not a reason to skip the check",
        missing.join(", ")
    ))
}

/// `"profiles"` was present but not a JSON array.
pub fn profiles_not_a_list(found: &str) -> Diagnostic {
    fail(format!(
        "stack-profiles: \"profiles\" must be a list, found {found}"
    ))
}

/// The marked `stack-profile-pool` region of `stack-profiles.md` could not
/// be read.
pub fn region_unreadable(error: &str) -> Diagnostic {
    fail(format!(
        "stack-profiles: the pool region of stack-profiles.md is not readable — {error}; the signatures the gate compares against are the ones inside the markers, and nothing else"
    ))
}

/// `"universal"` (the declared absence of a stack profile) was itself listed
/// in the pool.
pub fn universal_in_pool() -> Diagnostic {
    fail("stack-profiles: \"universal\" is not a stack profile and must not be listed in the pool; it is the declared absence of one")
}

/// Neither half's failure — every name here already passed both checks
/// above (agreed with a non-empty documented signature, and is not
/// `"universal"`), so `StackProfileName::new` cannot fail for it in
/// practice: an empty documented id is structurally impossible (the row
/// pattern that extracts it requires at least one character) and would
/// otherwise already have produced the disagreement diagnostic above. Kept
/// as an explicit, typed outcome rather than a silently-dropping
/// `filter_map` — a construction failure this function did not expect is
/// reported, not discarded.
pub fn internal_pool_construction_failure(
    check_name: &str,
    value: &str,
    error: &str,
) -> Diagnostic {
    fail(format!(
        "{check_name}: internal error building the agreed pool from \"{value}\": {error}; every declared name should already have been validated by the agreement and \"universal\" checks above"
    ))
}

/// Compares the declared and documented halves of the pool. `Ok` carries the
/// agreed, typed [`StackProfilePool`] — a value that, by construction, can
/// never contain `"universal"` or an empty name. `Err` carries the one
/// disagreement or `"universal"` diagnostic, or (only were the structural
/// guarantee above ever violated) [`internal_pool_construction_failure`].
pub fn check_pool_agreement(
    declared_ordered: &[String],
    documented_ordered: &[String],
) -> Result<StackProfilePool, Diagnostic> {
    if let Some(diagnostic) =
        ordered_pool_agreement("stack-profiles", declared_ordered, documented_ordered)
    {
        return Err(diagnostic);
    }
    if declared_ordered.iter().any(|n| n == "universal") {
        return Err(universal_in_pool());
    }
    let mut pool = StackProfilePool::new();
    for name in declared_ordered {
        let id = StackProfileName::new(name.as_str()).map_err(|error| {
            internal_pool_construction_failure("stack-profiles", name, &error.to_string())
        })?;
        pool.insert(id);
    }
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agreeing_halves_with_no_universal_are_clean() {
        let declared = vec!["vue-spa".to_string()];
        let documented = vec!["vue-spa".to_string()];
        let pool = check_pool_agreement(&declared, &documented).unwrap();
        assert!(pool.contains("vue-spa"));
    }

    #[test]
    fn universal_in_the_pool_is_rejected() {
        let declared = vec!["universal".to_string()];
        let documented = vec!["universal".to_string()];
        let error = check_pool_agreement(&declared, &documented).unwrap_err();
        assert!(error.message().contains("is not a stack profile"));
    }

    #[test]
    fn disagreement_is_reported_before_the_universal_check() {
        let declared = vec!["ghost".to_string()];
        let documented: Vec<String> = Vec::new();
        let error = check_pool_agreement(&declared, &documented).unwrap_err();
        assert!(error.message().contains("disagree"));
    }

    #[test]
    fn stack_profile_name_rejects_universal_and_empty_directly() {
        assert_eq!(
            StackProfileName::new("universal").unwrap_err(),
            StackProfileNameError::Universal
        );
        assert_eq!(
            StackProfileName::new("").unwrap_err(),
            StackProfileNameError::Empty
        );
        assert!(StackProfileName::new("vue-spa").is_ok());
    }
}
