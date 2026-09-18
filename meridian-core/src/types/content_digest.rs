//! [`ContentDigest`] — a cryptographic digest of content (algorithm +
//! value, closed form).
//!
//! Closed form (`instance-data-migration.md` §2, `sha256_digest` in
//! `registries/operating-model/instance-data-migration.schema.json`):
//!
//! - `algorithm`: always `sha-256` (the only value the contract admits);
//! - `value`: exactly 64 lowercase hexadecimal characters.

use core::fmt;

use sha2::{Digest, Sha256};

/// The one algorithm a [`ContentDigest`] may name. A closed enum of one
/// variant, not a free string, so a caller cannot construct a digest tagged
/// with an algorithm this contract does not recognise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DigestAlgorithm {
    /// SHA-256, the only algorithm `instance-data-migration.schema.json`
    /// admits.
    Sha256,
}

impl DigestAlgorithm {
    /// The literal string the contract uses for this algorithm.
    pub fn as_str(self) -> &'static str {
        match self {
            DigestAlgorithm::Sha256 => "sha-256",
        }
    }
}

impl fmt::Display for DigestAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A value rejected by [`ContentDigest::from_hex`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentDigestError {
    /// The value was not exactly 64 characters.
    WrongLength {
        /// The number of characters actually supplied.
        length: usize,
    },
    /// The value contained a character outside `[0-9a-f]`, or an uppercase
    /// hex digit — uppercase is never silently lowercased.
    NotLowercaseHex {
        /// The rejected value, carried for a precise diagnostic.
        value: String,
    },
}

impl fmt::Display for ContentDigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentDigestError::WrongLength { length } => {
                write!(
                    f,
                    "digest value must be exactly 64 hex characters, got {length}"
                )
            }
            ContentDigestError::NotLowercaseHex { value } => write!(
                f,
                "\"{value}\" is not a well-formed lowercase SHA-256 hex digest"
            ),
        }
    }
}

impl std::error::Error for ContentDigestError {}

/// A cryptographic digest of content: a fixed algorithm (SHA-256) and a
/// 64-character lowercase hexadecimal value.
///
/// The only public constructors are [`ContentDigest::from_hex`] (wraps an
/// already-computed digest value, validated) and
/// [`ContentDigest::of_bytes`]/[`ContentDigest::of_str`] (compute the digest
/// of given content directly) — there is no constructor that accepts an
/// algorithm other than SHA-256, because the contract admits none.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentDigest(String);

impl ContentDigest {
    /// Validates and wraps an already-computed SHA-256 hex digest: exactly
    /// 64 characters, every one a lowercase hex digit. Neither case-folding
    /// nor truncation/padding is performed.
    pub fn from_hex(value: impl Into<String>) -> Result<Self, ContentDigestError> {
        let value = value.into();
        if value.chars().count() != 64 {
            return Err(ContentDigestError::WrongLength {
                length: value.chars().count(),
            });
        }
        if !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ContentDigestError::NotLowercaseHex { value });
        }
        Ok(Self(value))
    }

    /// Computes the SHA-256 digest of `bytes` directly.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        Self(hex_lowercase(&digest))
    }

    /// Computes the SHA-256 digest of the UTF-8 bytes of `text`.
    pub fn of_str(text: &str) -> Self {
        Self::of_bytes(text.as_bytes())
    }

    /// The digest algorithm — always [`DigestAlgorithm::Sha256`].
    pub fn algorithm(&self) -> DigestAlgorithm {
        DigestAlgorithm::Sha256
    }

    /// Borrows the 64-character lowercase hex value.
    pub fn value(&self) -> &str {
        &self.0
    }
}

fn hex_lowercase(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algorithm(), self.0)
    }
}

impl AsRef<str> for ContentDigest {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_64_HEX: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn accepts_a_valid_64_char_lowercase_hex_value() {
        let d = ContentDigest::from_hex(VALID_64_HEX).unwrap();
        assert_eq!(d.value(), VALID_64_HEX);
        assert_eq!(d.algorithm(), DigestAlgorithm::Sha256);
    }

    #[test]
    fn rejects_short_value() {
        assert!(matches!(
            ContentDigest::from_hex("abc").unwrap_err(),
            ContentDigestError::WrongLength { length: 3 }
        ));
    }

    #[test]
    fn rejects_long_value() {
        let too_long = format!("{VALID_64_HEX}0");
        assert!(matches!(
            ContentDigest::from_hex(too_long).unwrap_err(),
            ContentDigestError::WrongLength { length: 65 }
        ));
    }

    #[test]
    fn rejects_uppercase_hex() {
        let upper = VALID_64_HEX.to_uppercase();
        assert!(matches!(
            ContentDigest::from_hex(upper).unwrap_err(),
            ContentDigestError::NotLowercaseHex { .. }
        ));
    }

    #[test]
    fn rejects_non_hex_characters() {
        let mut bad: String = VALID_64_HEX.chars().skip(1).collect();
        bad.insert(0, 'g');
        assert!(matches!(
            ContentDigest::from_hex(bad).unwrap_err(),
            ContentDigestError::NotLowercaseHex { .. }
        ));
    }

    #[test]
    fn of_str_computes_the_known_sha256_of_the_empty_string() {
        let d = ContentDigest::of_str("");
        assert_eq!(d.value(), VALID_64_HEX);
    }

    #[test]
    fn of_str_is_deterministic() {
        assert_eq!(
            ContentDigest::of_str("hello"),
            ContentDigest::of_str("hello")
        );
        assert_ne!(
            ContentDigest::of_str("hello"),
            ContentDigest::of_str("world")
        );
    }
}
