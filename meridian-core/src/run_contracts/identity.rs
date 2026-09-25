//! Run identity and the validated scalar shapes the three run-contract
//! schemas declare. Each type admits exactly the values its JSON Schema
//! definition admits — no more, no less:
//!
//! - [`RecordText`] is the schemas' `non_empty`/`portable_ref`
//!   (`minLength: 1`). A whitespace-only string IS schema-valid, so it is
//!   representable here; blankness is a DOMAIN rejection the checks report
//!   in their own words. [`crate::types::NonEmptyString`] rejects
//!   whitespace-only input and therefore is NOT this contract's type.
//! - [`ActorRef`]/[`PortableRef`] are the same shape, kept distinct so an
//!   actor and a record reference are never interchangeable at a call site.
//! - [`ScopeRevision`] and [`TransitionSequence`] are `integer, minimum 1`.
//! - [`RunId`] is a record's `semantic_id`, whose pattern is exactly
//!   [`crate::types::SemanticId`]'s; [`RunStateScope`] is the only scope a
//!   run record may occupy (`run-state`, with a mandatory workspace).

use core::fmt;

use crate::types::{SemanticId, SemanticIdError, WorkspaceId};

/// A value rejected by one of this module's constructors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunValueError {
    /// A `minLength: 1` string was empty.
    EmptyText,
    /// An `integer, minimum: 1` value was zero.
    NotPositive,
    /// A run id did not match the semantic-id pattern.
    RunId(SemanticIdError),
}

impl fmt::Display for RunValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunValueError::EmptyText => {
                write!(f, "text is empty (the schema requires minLength 1)")
            }
            RunValueError::NotPositive => write!(f, "value is not a positive integer"),
            RunValueError::RunId(e) => write!(f, "invalid run id: {e}"),
        }
    }
}

impl std::error::Error for RunValueError {}

/// A schema `non_empty` string: at least one character, stored verbatim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordText(String);

impl RecordText {
    pub fn new(value: impl Into<String>) -> Result<Self, RunValueError> {
        let value = value.into();
        if value.is_empty() {
            return Err(RunValueError::EmptyText);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whitespace-only — schema-valid, but carries no statement.
    pub fn is_blank(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl fmt::Display for RecordText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

macro_rules! text_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(RecordText);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, RunValueError> {
                RecordText::new(value).map(Self)
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            pub fn is_blank(&self) -> bool {
                self.0.is_blank()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

text_newtype! {
    /// An opaque, portable reference to a participant. It grants no role
    /// or authority by itself.
    ActorRef
}

text_newtype! {
    /// A portable reference to another record or source (never an embedded
    /// body).
    PortableRef
}

macro_rules! positive_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn new(value: u64) -> Result<Self, RunValueError> {
                if value == 0 {
                    return Err(RunValueError::NotPositive);
                }
                Ok(Self(value))
            }

            pub fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

positive_newtype! {
    /// The positive revision of a run's resolved scope. It never decreases
    /// along the transition history.
    ScopeRevision
}

positive_newtype! {
    /// The explicit ordinal of one history record; the ordering of a
    /// history, never object or file order.
    TransitionSequence
}

/// The stable identity of one execution run.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunId(SemanticId);

impl RunId {
    pub fn new(value: impl Into<String>) -> Result<Self, RunValueError> {
        SemanticId::new(value)
            .map(Self)
            .map_err(RunValueError::RunId)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The `run-state` scope: the only area a run's state, control record or
/// context manifest may occupy (`workspace-scope-model.md` §1). Every
/// other scope type is unrepresentable here.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RunStateScope {
    run_id: RunId,
    workspace_id: WorkspaceId,
}

impl RunStateScope {
    pub fn new(run_id: RunId, workspace_id: WorkspaceId) -> Self {
        Self {
            run_id,
            workspace_id,
        }
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_text_admits_whitespace_but_not_empty() {
        assert_eq!(RecordText::new(""), Err(RunValueError::EmptyText));
        let blank = RecordText::new("   ").unwrap();
        assert!(blank.is_blank());
        assert_eq!(blank.as_str(), "   ");
        assert!(!ActorRef::new("owner-a").unwrap().is_blank());
    }

    #[test]
    fn positive_integers_reject_zero() {
        assert_eq!(ScopeRevision::new(0), Err(RunValueError::NotPositive));
        assert_eq!(TransitionSequence::new(3).unwrap().get(), 3);
    }

    #[test]
    fn run_id_follows_the_semantic_id_pattern() {
        assert!(RunId::new("example-run-001").is_ok());
        assert!(RunId::new("Example").is_err());
        assert!(RunId::new("").is_err());
    }
}
