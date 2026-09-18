//! Strict, canonical standard-alphabet Base64 — decode and encode, used
//! only to prove a [`super::ContentEnvelope`]'s declared `content` is
//! canonical Base64 of the bytes its `digest` actually claims
//! (`checkContentEnvelope`, `instance-data-migration.md` §4.1:
//! `isCanonicalBase64` — content must round-trip byte-for-byte through
//! decode-then-re-encode, since a plain alphabet/padding check alone
//! accepts non-canonical encodings a permissive decoder would silently
//! normalise).

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn decode_symbol(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn decode(s: &str) -> Option<Vec<u8>> {
    let bytes = s.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let quad_count = bytes.len() / 4;
    for (quad_index, chunk) in bytes.chunks(4).enumerate() {
        let is_last = quad_index == quad_count - 1;
        // Padding ('=') may appear only as the last one or two characters
        // of the FINAL quad.
        if chunk[0] == b'=' || chunk[1] == b'=' {
            return None;
        }
        if !is_last && (chunk[2] == b'=' || chunk[3] == b'=') {
            return None;
        }
        let pad = match (chunk[2] == b'=', chunk[3] == b'=') {
            (false, false) => 0,
            (false, true) => 1,
            (true, true) => 2,
            (true, false) => return None, // "=X" pattern is never valid
        };
        let v0 = decode_symbol(chunk[0])?;
        let v1 = decode_symbol(chunk[1])?;
        let v2 = if pad >= 2 {
            0
        } else {
            decode_symbol(chunk[2])?
        };
        let v3 = if pad >= 1 {
            0
        } else {
            decode_symbol(chunk[3])?
        };
        let n = ((v0 as u32) << 18) | ((v1 as u32) << 12) | ((v2 as u32) << 6) | (v3 as u32);
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// Decodes `content` as Base64 and returns the bytes only if `content` is
/// already the CANONICAL Base64 encoding of those exact bytes — standard
/// alphabet, canonical padding, and no non-canonical "don't-care" bits set
/// in a padded final group (caught by the decode-then-re-encode round
/// trip, `Buffer.from(str,'base64').toString('base64') === str`'s Rust
/// equivalent). Returns `None` for anything else: wrong length, invalid
/// characters, misplaced padding, or a non-canonical spelling of an
/// otherwise-valid byte sequence.
pub(crate) fn decode_canonical(content: &str) -> Option<Vec<u8>> {
    let bytes = decode(content)?;
    if encode(&bytes) == content {
        Some(bytes)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_canonical_value() {
        // "hello" in canonical base64.
        assert_eq!(decode_canonical("aGVsbG8=").unwrap(), b"hello");
    }

    #[test]
    fn round_trips_arbitrary_bytes() {
        for bytes in [&b""[..], b"a", b"ab", b"abc", b"abcd", b"\x00\x01\x02\xff"] {
            let encoded = encode(bytes);
            if bytes.is_empty() {
                continue; // empty content is rejected by ContentEnvelope separately
            }
            assert_eq!(decode_canonical(&encoded).unwrap(), bytes);
        }
    }

    #[test]
    fn rejects_wrong_length() {
        assert_eq!(decode_canonical("abc"), None);
    }

    #[test]
    fn rejects_invalid_characters() {
        assert_eq!(decode_canonical("aGVsbG8!"), None);
    }

    #[test]
    fn rejects_misplaced_padding() {
        assert_eq!(decode_canonical("=GVsbG8="), None);
        assert_eq!(decode_canonical("aGV=bG8="), None);
    }

    #[test]
    fn rejects_non_canonical_dont_care_bits_in_a_padded_group() {
        // "/w==" is the canonical single-padding encoding of the byte
        // 0xff: its data character 'w' has zero "don't-care" low bits.
        assert_eq!(decode_canonical("/w=="), Some(vec![0xff]));
        // "/x==" decodes to the SAME byte 0xff (only the top 2 bits of the
        // second character matter for a 1-byte output) but is a
        // non-canonical spelling: 'x' has non-zero don't-care low bits, so
        // re-encoding 0xff reproduces "/w==", not "/x==" — the round trip
        // fails and the input is rejected.
        assert_eq!(decode_canonical("/x=="), None);
    }
}
