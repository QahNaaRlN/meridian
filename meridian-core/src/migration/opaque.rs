//! `checkOpaqueRef(ref, label)`: the migration contracts' wording of the
//! one portable-ref shape rule (`crate::types::opaque_ref_fault`) —
//! properties 1 and 8, a storage-neutral identity never shaped like a
//! filesystem location.

use crate::types::{opaque_ref_fault, EvidenceRefError};

/// The problem `checkOpaqueRef` reports for `value` under `label`, if any.
pub(crate) fn opaque_ref_problem(value: &str, label: &str) -> Option<String> {
    Some(match opaque_ref_fault(value)? {
        EvidenceRefError::Empty | EvidenceRefError::Blank => format!("{label} is empty"),
        EvidenceRefError::FileUrl { .. } => format!(
            "{label} \"{value}\" is a file:// URL; a portable ref must be an opaque identifier, not a filesystem location"
        ),
        EvidenceRefError::AbsoluteMachinePath { .. } => format!(
            "{label} \"{value}\" looks like an absolute machine path; it must be an opaque identifier, not a filesystem location"
        ),
        EvidenceRefError::ParentEscape { .. } => format!(
            "{label} \"{value}\" carries a \"..\" segment; it must be an opaque identifier"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_fault_is_worded_as_the_reference_words_it() {
        assert_eq!(opaque_ref_problem("instance:sample", "x"), None);
        assert_eq!(
            opaque_ref_problem("file:///tmp/x", "x").unwrap(),
            "x \"file:///tmp/x\" is a file:// URL; a portable ref must be an opaque identifier, not a filesystem location"
        );
        assert!(opaque_ref_problem("/srv/x", "x")
            .unwrap()
            .contains("looks like an absolute machine path"));
        assert!(opaque_ref_problem("a/../b", "x")
            .unwrap()
            .contains("carries a \"..\" segment"));
        assert_eq!(opaque_ref_problem("   ", "x"), None);
    }
}
