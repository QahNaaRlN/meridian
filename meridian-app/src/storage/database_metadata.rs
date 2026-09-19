//! [`DatabaseMetadata`] — the pair of facts every storage adapter's backing
//! database carries about itself (`meridian-rust-migration-program-plan.md`
//! §5.4, items 1–2).
//!
//! Lives in `meridian-app::storage`, next to [`super::DatabaseRole`], for
//! the same crate-boundary reason: `meridian-core` carries no notion of a
//! database at all.
//!
//! A `tool` database's [`Revision`] IS the one Kernel edition it holds; a
//! `workspace` database's [`Revision`] names the Kernel edition it declares
//! itself compatible with — the same field, two roles, two meanings, the
//! same way one `Revision` already serves both "source revision" and
//! "compatible edition" elsewhere. Neither an empty nor an unrecognised
//! value can reach this type: [`super::DatabaseRole`] is a closed enum with
//! no "unknown" variant, and [`Revision`] rejects the empty string at its
//! own constructor.

use meridian_core::types::Revision;

use super::database_role::DatabaseRole;

/// The role and the Kernel edition a database declares about itself, read
/// from inside the database — never from its file path or file name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseMetadata {
    role: DatabaseRole,
    kernel_edition: Revision,
}

impl DatabaseMetadata {
    /// Builds a metadata value from an already-known role and edition. Both
    /// arguments are already-strict types, so no further validation happens
    /// here — the strictness lives in [`DatabaseRole`] and [`Revision`]
    /// themselves, not duplicated at this call site.
    pub fn new(role: DatabaseRole, kernel_edition: Revision) -> Self {
        Self {
            role,
            kernel_edition,
        }
    }

    pub fn role(&self) -> DatabaseRole {
        self.role
    }

    pub fn kernel_edition(&self) -> &Revision {
        &self.kernel_edition
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_role_and_edition_back_unchanged() {
        let metadata = DatabaseMetadata::new(DatabaseRole::Tool, Revision::new("0.6.0").unwrap());
        assert_eq!(metadata.role(), DatabaseRole::Tool);
        assert_eq!(metadata.kernel_edition().as_str(), "0.6.0");
    }
}
