use std::fmt::Write as _;

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Lowercase hex encoding without per-byte allocations.
pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize]);
        out.push(HEX[(byte & 0x0f) as usize]);
    }
    // The table only emits ASCII, so the buffer is always valid UTF-8.
    String::from_utf8(out).expect("hex output is ASCII")
}

/// Like [`to_hex`] with a caller-supplied prefix, e.g. "0x".
pub fn to_hex_prefixed(prefix: &str, bytes: &[u8]) -> String {
    let mut out = String::with_capacity(prefix.len() + bytes.len() * 2);
    out.push_str(prefix);
    for &byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_lowercase_and_pads() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
        assert_eq!(to_hex(&[]), "");
        assert_eq!(to_hex_prefixed("0x", &[0x01, 0x0a]), "0x010a");
    }
}
