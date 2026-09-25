//! M-07 — external dependencies of a workspace: which ones are vendored or
//! SHA-pinned, and which ones cannot be verified on another machine.

use crate::types::{Diagnostic, DiagnosticLevel, NonEmptyString};

use super::diagnostic;

/// How a dependency is held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyState {
    /// Copied into the Kernel; its provenance is the Kernel's own pin.
    Vendored,
    /// Any other declared state (`external`, `mirrored`, …), kept verbatim.
    Other(NonEmptyString),
}

impl DependencyState {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = NonEmptyString::new(value).map_err(|e| format!("state: {e}"))?;
        Ok(if value.as_str() == "vendored" {
            DependencyState::Vendored
        } else {
            DependencyState::Other(value)
        })
    }

    pub fn as_str(&self) -> &str {
        match self {
            DependencyState::Vendored => "vendored",
            DependencyState::Other(value) => value.as_str(),
        }
    }
}

/// One declared external dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDependency {
    id: NonEmptyString,
    state: DependencyState,
    sha_pinned: bool,
}

impl ExternalDependency {
    /// `sha_pinned` is whether the record names an expected artifact or
    /// entry digest (`expected_sha256`/`entry_sha256`).
    pub fn new(
        id: impl Into<String>,
        state: DependencyState,
        sha_pinned: bool,
    ) -> Result<Self, String> {
        let id = NonEmptyString::new(id).map_err(|e| format!("id: {e}"))?;
        Ok(Self {
            id,
            state,
            sha_pinned,
        })
    }
}

/// The reference's per-dependency verdicts: a vendored or SHA-pinned
/// dependency is informational, any other one is a warning.
pub fn check_dependencies(dependencies: &[ExternalDependency]) -> Vec<Diagnostic> {
    dependencies
        .iter()
        .map(|d| match &d.state {
            DependencyState::Vendored => diagnostic(
                DiagnosticLevel::Info,
                format!("ext-dependency: {} vendored", d.id.as_str()),
            ),
            state if d.sha_pinned => diagnostic(
                DiagnosticLevel::Info,
                format!(
                    "ext-dependency: {} — {}, SHA pinned but file not vendored into Kernel",
                    d.id.as_str(),
                    state.as_str()
                ),
            ),
            state => diagnostic(
                DiagnosticLevel::Warn,
                format!(
                    "ext-dependency: {} is \"{}\" with no pinned SHA — unverifiable on another machine",
                    d.id.as_str(),
                    state.as_str()
                ),
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dep(id: &str, state: &str, pinned: bool) -> ExternalDependency {
        ExternalDependency::new(id, DependencyState::parse(state).unwrap(), pinned).unwrap()
    }

    #[test]
    fn workspace_state_dependencies_warn_only_for_an_unpinned_external_one() {
        let out = check_dependencies(&[
            dep("a", "vendored", false),
            dep("b", "external", true),
            dep("c", "external", false),
        ]);
        let levels: Vec<DiagnosticLevel> = out.iter().map(Diagnostic::level).collect();
        assert_eq!(
            levels,
            [
                DiagnosticLevel::Info,
                DiagnosticLevel::Info,
                DiagnosticLevel::Warn
            ]
        );
        assert_eq!(
            out[2].message(),
            "ext-dependency: c is \"external\" with no pinned SHA — unverifiable on another machine"
        );
    }

    #[test]
    fn workspace_state_dependencies_reject_an_empty_id_or_state() {
        assert!(DependencyState::parse(" ").is_err());
        assert!(ExternalDependency::new("", DependencyState::Vendored, false).is_err());
    }
}
