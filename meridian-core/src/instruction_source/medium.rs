//! [`Medium`] — the closed `payload.medium` pool
//! (`instruction-source-registry.md`): the declared bearer of a registered
//! instruction source, which its [`super::Location`] is confined to.

use core::fmt;

/// The declared bearer of a registered instruction source. A `File` source
/// is located by a relative path AND an opaque container identity; an
/// `ExternalService` source by opaque service and resource identifiers with
/// no filesystem path — [`super::Location`] makes the two forms mutually
/// exclusive by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Medium {
    File,
    ExternalService,
}

impl Medium {
    pub fn as_str(self) -> &'static str {
        match self {
            Medium::File => "file",
            Medium::ExternalService => "external-service",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "file" => Some(Medium::File),
            "external-service" => Some(Medium::ExternalService),
            _ => None,
        }
    }
}

impl fmt::Display for Medium {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_both_closed_values() {
        assert_eq!(Medium::parse("file"), Some(Medium::File));
        assert_eq!(
            Medium::parse("external-service"),
            Some(Medium::ExternalService)
        );
        assert_eq!(Medium::File.as_str(), "file");
    }

    #[test]
    fn parse_rejects_values_outside_the_closed_pool() {
        assert_eq!(Medium::parse("invented"), None);
    }
}
