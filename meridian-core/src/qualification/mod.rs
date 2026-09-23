//! The two closing qualification contracts —
//! `workspace-compatibility-qualification` ([`workspace`]) and
//! `upgrade-integration-qualification` ([`upgrade`]) — and the pinned
//! references both use ([`pin`]).
//!
//! A qualification composes, never duplicates: every composed record is
//! resolved and pinned by its recomputed content digest, then checked by
//! the REAL check of its own contract (the plan and export checks of
//! [`crate::migration`] directly; the app-owned connection, handoff, field
//! report, task specification and execution run routes through their
//! typed composition points). The qualification reads its decision inputs
//! from those checks' ACCEPTED typed records only — a composed record that
//! is not accepted contributes the fail-closed signal of its slot, never a
//! value re-read from its JSON — and recomputes its closed verdict with a
//! decision matrix instead of trusting the declared one.

pub mod pin;
pub mod upgrade;
pub mod workspace;

use crate::types::{Diagnostic, DiagnosticLevel};

/// A qualification's closed verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QualificationState {
    Qualified,
    Blocked,
    Unverified,
}

impl QualificationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            QualificationState::Qualified => "QUALIFIED",
            QualificationState::Blocked => "BLOCKED",
            QualificationState::Unverified => "UNVERIFIED",
        }
    }

    pub fn parse(value: &str) -> Option<QualificationState> {
        match value {
            "QUALIFIED" => Some(QualificationState::Qualified),
            "BLOCKED" => Some(QualificationState::Blocked),
            "UNVERIFIED" => Some(QualificationState::Unverified),
            _ => None,
        }
    }
}

impl core::fmt::Display for QualificationState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A composed record after its own contract's check: every problem that
/// check reported (prefix-free, in its order) and its accepted typed form
/// when there were none.
#[derive(Debug, Clone, PartialEq)]
pub struct Composed<T> {
    pub diagnostics: Vec<Diagnostic>,
    pub accepted: Option<T>,
}

impl<T> Composed<T> {
    pub fn accepted(accepted: T) -> Self {
        Self {
            diagnostics: Vec::new(),
            accepted: Some(accepted),
        }
    }

    pub fn rejected(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            diagnostics,
            accepted: None,
        }
    }
}

/// Static invariant: every message of this module family is a `format!`
/// whose template carries fixed non-whitespace text.
pub(crate) fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message)
        .expect("every qualification message carries fixed non-whitespace template text")
}

/// `problems.push(...composed.map((m) => `${prefix}${m}`))`.
pub(crate) fn prefixed(prefix: &str, composed: &[Diagnostic]) -> Vec<Diagnostic> {
    composed
        .iter()
        .map(|d| fail(format!("{prefix}{}", d.message())))
        .collect()
}

#[cfg(test)]
mod tests;
