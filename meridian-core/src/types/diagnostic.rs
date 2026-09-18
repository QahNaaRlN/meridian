//! [`Diagnostic`] — one check message: a level, non-empty text, and an
//! optional checkable reference — plus deterministic sorting and worst-level
//! aggregation over a collection of them.

use core::cmp::Ordering;
use core::fmt;

use super::nonempty::{NonEmptyString, NonEmptyStringError};

/// The closed severity of a [`Diagnostic`].
///
/// Ordered worst-to-best for aggregation: `Fail` outranks `Warn` outranks
/// `Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticLevel {
    Fail,
    Warn,
    Info,
}

impl DiagnosticLevel {
    /// The literal string used for this level.
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticLevel::Fail => "FAIL",
            DiagnosticLevel::Warn => "WARN",
            DiagnosticLevel::Info => "INFO",
        }
    }

    /// A rank where `Fail` is worst (lowest number), for building a
    /// worst-first deterministic order without relying on declaration
    /// order.
    fn severity_rank(self) -> u8 {
        match self {
            DiagnosticLevel::Fail => 0,
            DiagnosticLevel::Warn => 1,
            DiagnosticLevel::Info => 2,
        }
    }
}

impl fmt::Display for DiagnosticLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialOrd for DiagnosticLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DiagnosticLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        self.severity_rank().cmp(&other.severity_rank())
    }
}

/// A checkable reference a [`Diagnostic`] may point at — carried as data
/// only; `meridian-core` performs no file I/O to confirm the path or record
/// actually exists.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticReference {
    /// A path, relative and portable — not resolved or read.
    Path(NonEmptyString),
    /// A record identifier — not resolved or read.
    Record(NonEmptyString),
}

/// A value rejected while building a [`Diagnostic`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticError {
    /// The message was empty or whitespace-only.
    InvalidMessage(NonEmptyStringError),
    /// The reference value was empty or whitespace-only.
    InvalidReference(NonEmptyStringError),
}

impl fmt::Display for DiagnosticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticError::InvalidMessage(e) => write!(f, "invalid diagnostic message: {e}"),
            DiagnosticError::InvalidReference(e) => write!(f, "invalid diagnostic reference: {e}"),
        }
    }
}

impl std::error::Error for DiagnosticError {}

/// One check message: a closed level, non-empty text, and an optional
/// checkable reference to a path or a record.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    level: DiagnosticLevel,
    message: NonEmptyString,
    reference: Option<DiagnosticReference>,
}

impl Diagnostic {
    /// Builds a diagnostic with no reference.
    pub fn new(
        level: DiagnosticLevel,
        message: impl Into<String>,
    ) -> Result<Self, DiagnosticError> {
        let message = NonEmptyString::new(message).map_err(DiagnosticError::InvalidMessage)?;
        Ok(Self {
            level,
            message,
            reference: None,
        })
    }

    /// Builds a diagnostic that references a path.
    pub fn with_path_reference(
        level: DiagnosticLevel,
        message: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<Self, DiagnosticError> {
        let mut d = Self::new(level, message)?;
        let path = NonEmptyString::new(path).map_err(DiagnosticError::InvalidReference)?;
        d.reference = Some(DiagnosticReference::Path(path));
        Ok(d)
    }

    /// Builds a diagnostic that references a record.
    pub fn with_record_reference(
        level: DiagnosticLevel,
        message: impl Into<String>,
        record: impl Into<String>,
    ) -> Result<Self, DiagnosticError> {
        let mut d = Self::new(level, message)?;
        let record = NonEmptyString::new(record).map_err(DiagnosticError::InvalidReference)?;
        d.reference = Some(DiagnosticReference::Record(record));
        Ok(d)
    }

    pub fn level(&self) -> DiagnosticLevel {
        self.level
    }

    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    pub fn reference(&self) -> Option<&DiagnosticReference> {
        self.reference.as_ref()
    }

    /// The worst (lowest-ranked) level among `diagnostics`, or `None` for an
    /// empty slice. Used to decide an aggregate pass/fail without asserting
    /// a status the diagnostics themselves do not support
    /// (`evidence-before-status`, `operating-principles.md`).
    pub fn worst_level(diagnostics: &[Diagnostic]) -> Option<DiagnosticLevel> {
        diagnostics.iter().map(Diagnostic::level).min()
    }

    /// Sorts `diagnostics` into a stable, deterministic order — by level
    /// (worst first), then message, then reference — independent of the
    /// order they were pushed in.
    pub fn sort(diagnostics: &mut [Diagnostic]) {
        diagnostics.sort_by(|a, b| {
            a.level
                .cmp(&b.level)
                .then_with(|| a.message.as_str().cmp(b.message.as_str()))
                .then_with(|| a.reference.cmp(&b.reference))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_message_only_diagnostic() {
        let d = Diagnostic::new(DiagnosticLevel::Warn, "something looks off").unwrap();
        assert_eq!(d.level(), DiagnosticLevel::Warn);
        assert_eq!(d.message(), "something looks off");
        assert_eq!(d.reference(), None);
    }

    #[test]
    fn rejects_empty_message() {
        assert!(Diagnostic::new(DiagnosticLevel::Info, "").is_err());
    }

    #[test]
    fn rejects_blank_message() {
        assert!(Diagnostic::new(DiagnosticLevel::Info, "   ").is_err());
    }

    #[test]
    fn carries_a_path_reference_without_reading_it() {
        let d = Diagnostic::with_path_reference(
            DiagnosticLevel::Fail,
            "missing schema",
            "registries/x.json",
        )
        .unwrap();
        assert_eq!(
            d.reference(),
            Some(&DiagnosticReference::Path(
                NonEmptyString::new("registries/x.json").unwrap()
            ))
        );
    }

    #[test]
    fn worst_level_prefers_fail_over_warn_and_info() {
        let diags = vec![
            Diagnostic::new(DiagnosticLevel::Info, "a").unwrap(),
            Diagnostic::new(DiagnosticLevel::Fail, "b").unwrap(),
            Diagnostic::new(DiagnosticLevel::Warn, "c").unwrap(),
        ];
        assert_eq!(Diagnostic::worst_level(&diags), Some(DiagnosticLevel::Fail));
    }

    #[test]
    fn worst_level_of_empty_slice_is_none() {
        assert_eq!(Diagnostic::worst_level(&[]), None);
    }

    #[test]
    fn sort_is_deterministic_regardless_of_input_order() {
        let mut a = vec![
            Diagnostic::new(DiagnosticLevel::Warn, "b-message").unwrap(),
            Diagnostic::new(DiagnosticLevel::Fail, "z-message").unwrap(),
            Diagnostic::new(DiagnosticLevel::Info, "a-message").unwrap(),
        ];
        let mut b = vec![a[2].clone(), a[0].clone(), a[1].clone()];
        Diagnostic::sort(&mut a);
        Diagnostic::sort(&mut b);
        assert_eq!(a, b);
        assert_eq!(a[0].level(), DiagnosticLevel::Fail);
    }
}
