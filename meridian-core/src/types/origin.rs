//! [`Origin`] — where a record came from
//! (`scoped-record.schema.json` `definitions.origin`).

use core::fmt;

use super::nonempty::{NonEmptyString, NonEmptyStringError};

/// The plain discriminant of an [`Origin`] value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OriginKind {
    BuiltIn,
    Declared,
    Migrated,
    Imported,
    Derived,
}

impl OriginKind {
    /// The literal string the contract uses for this origin kind.
    pub fn as_str(self) -> &'static str {
        match self {
            OriginKind::BuiltIn => "built-in",
            OriginKind::Declared => "declared",
            OriginKind::Migrated => "migrated",
            OriginKind::Imported => "imported",
            OriginKind::Derived => "derived",
        }
    }
}

impl fmt::Display for OriginKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A value rejected while building an [`Origin`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginError(NonEmptyStringError);

impl fmt::Display for OriginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid origin source_ref: {}", self.0)
    }
}

impl std::error::Error for OriginError {}

/// Where a record came from.
///
/// `kind: built-in` never carries a `source_ref` (there is no field for
/// one on that variant); every other kind requires a non-empty
/// `source_ref` — the schema's `if kind=built-in then not source_ref, else
/// required source_ref` is expressed directly by which variant has the
/// field, not by a runtime check.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Origin {
    BuiltIn,
    Declared { source_ref: NonEmptyString },
    Migrated { source_ref: NonEmptyString },
    Imported { source_ref: NonEmptyString },
    Derived { source_ref: NonEmptyString },
}

impl Origin {
    /// The built-in origin — never carries a `source_ref`.
    pub fn built_in() -> Self {
        Origin::BuiltIn
    }

    /// A `declared` origin: an explicit act of stating the record, with a
    /// required, non-empty `source_ref`.
    pub fn declared(source_ref: impl Into<String>) -> Result<Self, OriginError> {
        Ok(Origin::Declared {
            source_ref: NonEmptyString::new(source_ref).map_err(OriginError)?,
        })
    }

    /// A `migrated` origin, with a required, non-empty `source_ref`.
    pub fn migrated(source_ref: impl Into<String>) -> Result<Self, OriginError> {
        Ok(Origin::Migrated {
            source_ref: NonEmptyString::new(source_ref).map_err(OriginError)?,
        })
    }

    /// An `imported` origin, with a required, non-empty `source_ref`.
    pub fn imported(source_ref: impl Into<String>) -> Result<Self, OriginError> {
        Ok(Origin::Imported {
            source_ref: NonEmptyString::new(source_ref).map_err(OriginError)?,
        })
    }

    /// A `derived` origin, with a required, non-empty `source_ref`.
    pub fn derived(source_ref: impl Into<String>) -> Result<Self, OriginError> {
        Ok(Origin::Derived {
            source_ref: NonEmptyString::new(source_ref).map_err(OriginError)?,
        })
    }

    /// The plain discriminant of this origin.
    pub fn kind(&self) -> OriginKind {
        match self {
            Origin::BuiltIn => OriginKind::BuiltIn,
            Origin::Declared { .. } => OriginKind::Declared,
            Origin::Migrated { .. } => OriginKind::Migrated,
            Origin::Imported { .. } => OriginKind::Imported,
            Origin::Derived { .. } => OriginKind::Derived,
        }
    }

    /// The source reference, when this origin's kind carries one (every
    /// kind but `built-in`).
    pub fn source_ref(&self) -> Option<&str> {
        match self {
            Origin::BuiltIn => None,
            Origin::Declared { source_ref }
            | Origin::Migrated { source_ref }
            | Origin::Imported { source_ref }
            | Origin::Derived { source_ref } => Some(source_ref.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_never_carries_a_source_ref() {
        let o = Origin::built_in();
        assert_eq!(o.kind(), OriginKind::BuiltIn);
        assert_eq!(o.source_ref(), None);
    }

    #[test]
    fn declared_requires_a_non_empty_source_ref() {
        assert!(Origin::declared("owner-decision:branch-naming").is_ok());
        assert!(Origin::declared("").is_err());
        assert!(Origin::declared("   ").is_err());
    }

    #[test]
    fn every_non_built_in_kind_carries_its_source_ref() {
        let o = Origin::migrated("record-unit:legacy-1").unwrap();
        assert_eq!(o.kind(), OriginKind::Migrated);
        assert_eq!(o.source_ref(), Some("record-unit:legacy-1"));
    }
}
