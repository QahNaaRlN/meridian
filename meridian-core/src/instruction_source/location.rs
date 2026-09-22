//! [`Location`] — where a registered instruction source is, confined to its
//! declared [`super::Medium`] (`instruction-source-registry.md`).
//!
//! [`RelativePath`] and [`OpaqueRef`] make the confinement rule a property
//! of the TYPE, not a check layered on top of a plain `String` — a value
//! that is an absolute path, uses a backslash separator, or escapes through
//! `".."` is rejected at construction and can never reach
//! [`super::InstructionSource`] (`rust-architecture-conformance-2`, §1: the
//! Node reference's `checkLocationPath`/`opaqueRefProblem` string checks,
//! made a validating constructor instead of a checker that runs against an
//! already-representable-but-invalid value).

use core::fmt;

use super::Medium;

/// A value rejected by [`RelativePath::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelativePathError {
    Empty,
    BackslashSeparator { value: String },
    Absolute { value: String },
    NotNormalised { value: String },
}

impl fmt::Display for RelativePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelativePathError::Empty => write!(f, "the location path is empty"),
            RelativePathError::BackslashSeparator { value } => write!(
                f,
                "the location path \"{value}\" uses a backslash as a separator; a source path is POSIX-relative"
            ),
            RelativePathError::Absolute { value } => write!(
                f,
                "the location path \"{value}\" is absolute; a registered source is located by a relative path inside its declared medium"
            ),
            RelativePathError::NotNormalised { value } => write!(
                f,
                "the location path \"{value}\" carries an empty, \".\" or \"..\" segment; it must already be normalised and must not escape the declared medium"
            ),
        }
    }
}

impl std::error::Error for RelativePathError {}

/// A source's location path, confined to its declared medium: relative,
/// already normalised, no absolute form and no escape through `".."`. The
/// source file itself is never required to exist — this type checks only
/// the SHAPE of the path, not whether anything is actually there.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelativePath(String);

impl RelativePath {
    pub fn new(value: impl Into<String>) -> Result<Self, RelativePathError> {
        let value = value.into();
        if value.is_empty() {
            return Err(RelativePathError::Empty);
        }
        if value.contains('\\') {
            return Err(RelativePathError::BackslashSeparator { value });
        }
        if is_windows_drive(&value) || value.starts_with('/') {
            return Err(RelativePathError::Absolute { value });
        }
        if value
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err(RelativePathError::NotNormalised { value });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelativePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A value rejected by [`OpaqueRef::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpaqueRefError {
    Empty,
    LooksLikeAbsolutePath { value: String },
    ParentSegment { value: String },
}

impl fmt::Display for OpaqueRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpaqueRefError::Empty => write!(f, "the reference is empty"),
            OpaqueRefError::LooksLikeAbsolutePath { value } => write!(
                f,
                "\"{value}\" looks like an absolute machine path; it must be an opaque identifier, not a filesystem location"
            ),
            OpaqueRefError::ParentSegment { value } => write!(
                f,
                "\"{value}\" carries a \"..\" segment; it must be an opaque identifier"
            ),
        }
    }
}

impl std::error::Error for OpaqueRefError {}

/// An opaque identifier (`container_ref`, `service_ref`, `resource_ref`)
/// that must not be a disguised filesystem location.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpaqueRef(String);

impl OpaqueRef {
    pub fn new(value: impl Into<String>) -> Result<Self, OpaqueRefError> {
        let value = value.into();
        if value.is_empty() {
            return Err(OpaqueRefError::Empty);
        }
        if value.contains('\\') || is_windows_drive(&value) || value.starts_with('/') {
            return Err(OpaqueRefError::LooksLikeAbsolutePath { value });
        }
        if value.split('/').any(|s| s == "..") {
            return Err(OpaqueRefError::ParentSegment { value });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OpaqueRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn is_windows_drive(p: &str) -> bool {
    let bytes = p.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Where a registered instruction source is, confined to its declared
/// medium. The two forms are mutually exclusive by construction: `File`
/// carries a path and a container reference, never a service/resource
/// reference; `ExternalService` carries a service and resource reference,
/// never a path — an invalid combination (a `file` medium with a
/// `service_ref`, for instance) is unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Location {
    File {
        path: RelativePath,
        container_ref: OpaqueRef,
    },
    ExternalService {
        service_ref: OpaqueRef,
        resource_ref: OpaqueRef,
    },
}

impl Location {
    pub fn file(path: RelativePath, container_ref: OpaqueRef) -> Self {
        Location::File {
            path,
            container_ref,
        }
    }

    pub fn external_service(service_ref: OpaqueRef, resource_ref: OpaqueRef) -> Self {
        Location::ExternalService {
            service_ref,
            resource_ref,
        }
    }

    pub fn medium(&self) -> Medium {
        match self {
            Location::File { .. } => Medium::File,
            Location::ExternalService { .. } => Medium::ExternalService,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_rejects_backslash() {
        assert!(matches!(
            RelativePath::new("a\\b").unwrap_err(),
            RelativePathError::BackslashSeparator { .. }
        ));
    }

    #[test]
    fn relative_path_rejects_dot_dot() {
        assert!(matches!(
            RelativePath::new("a/../b").unwrap_err(),
            RelativePathError::NotNormalised { .. }
        ));
    }

    #[test]
    fn relative_path_rejects_an_absolute_path() {
        assert!(matches!(
            RelativePath::new("/etc/passwd").unwrap_err(),
            RelativePathError::Absolute { .. }
        ));
        assert!(matches!(
            RelativePath::new("C:\\x").unwrap_err(),
            RelativePathError::BackslashSeparator { .. } | RelativePathError::Absolute { .. }
        ));
    }

    #[test]
    fn relative_path_accepts_a_normalised_relative_path() {
        assert!(RelativePath::new("a/b/c.md").is_ok());
    }

    #[test]
    fn opaque_ref_flags_a_disguised_absolute_path() {
        let err = OpaqueRef::new("/etc/passwd").unwrap_err();
        assert!(matches!(err, OpaqueRefError::LooksLikeAbsolutePath { .. }));
    }

    #[test]
    fn opaque_ref_is_silent_on_a_plain_opaque_id() {
        assert!(OpaqueRef::new("container-abc123").is_ok());
    }

    #[test]
    fn opaque_ref_rejects_empty() {
        assert!(matches!(
            OpaqueRef::new("").unwrap_err(),
            OpaqueRefError::Empty
        ));
    }

    #[test]
    fn location_medium_matches_its_own_variant() {
        let file = Location::file(
            RelativePath::new("a.md").unwrap(),
            OpaqueRef::new("container-1").unwrap(),
        );
        assert_eq!(file.medium(), Medium::File);
        let service = Location::external_service(
            OpaqueRef::new("service-1").unwrap(),
            OpaqueRef::new("resource-1").unwrap(),
        );
        assert_eq!(service.medium(), Medium::ExternalService);
    }
}
