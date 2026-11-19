//! Hash Generator: MD5, SHA-1, SHA-256 and SHA-512 digests of the input text.
use std::fmt::Write;
use super::{Category, TextTool, Tool};
use crate::ui::icon;

/// The Hash Generator tool.
pub fn hash() -> Box<dyn Tool> {
    Box::new(TextTool::new("Hash Generator", icon::HASH, Category::Security, vec![("Generate", hashes)])
        .describe("Hash the exact input text (UTF-8 bytes, whitespace included).").sample("The quick brown fox jumps over the lazy dog").key_value_output())
}

/// MD5, SHA-1, SHA-256 and SHA-512 of the input's UTF-8 bytes, as lowercase hex.
fn hashes(input: &str) -> Result<String, String> {
    use sha2::Digest;
    /// Lowercase hex, two digits per byte.
    fn hex(bytes: &[u8]) -> String { bytes.iter().fold(String::new(), |mut out, b| { let _ = write!(out, "{b:02x}"); out }) }
    let data = input.as_bytes();
    Ok(format!("MD5: {}\nSHA-1: {}\nSHA-256: {}\nSHA-512: {}",
        hex(&md5::Md5::digest(data)), hex(&sha1::Sha1::digest(data)), hex(&sha2::Sha256::digest(data)), hex(&sha2::Sha512::digest(data))))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hashes_known_vectors() {
        let out = hashes("abc").unwrap();
        assert!(out.contains("MD5: 900150983cd24fb0d6963f7d28e17f72"));
        assert!(out.contains("SHA-1: a9993e364706816aba3e25717850c26c9cd0d89d"));
        assert!(out.contains("SHA-256: ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
        assert!(out.contains("SHA-512: ddaf35a193617aba"));
    }
}
