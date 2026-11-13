use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use super::{Category, TextTool, Tool};

pub fn jwt() -> Box<dyn Tool> {
    Box::new(TextTool::new("JWT Debugger", "JW", Category::Security, vec![("Inspect", jwt_decode)]))
}

fn jwt_decode(input: &str) -> Result<String, String> {
    let parts: Vec<_> = input.trim().split('.').collect();
    if parts.len() < 2 { return Err("Expected a JWT with header and payload".into()); }
    let decode = |s: &str| -> Result<String, String> {
        let bytes = URL_SAFE_NO_PAD.decode(s).map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
    };
    Ok(format!("Header:\n{}\n\nPayload:\n{}\n\nSignature is not verified.", decode(parts[0])?, decode(parts[1])?))
}

pub fn hash() -> Box<dyn Tool> {
    Box::new(TextTool::new("Hash Generator", "#", Category::Security, vec![("Generate", hashes)])
        .describe("Hash the exact input text (UTF-8 bytes, whitespace included)."))
}

fn hashes(input: &str) -> Result<String, String> {
    use sha2::Digest;
    fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
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
