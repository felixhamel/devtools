//! Certificate Decoder (X.509): PEM or DER → subject, validity, SANs, key and fingerprints.
use super::{encoding::decode_base64, Category, TextTool, Tool};

pub fn certificate() -> Box<dyn Tool> {
    Box::new(TextTool::new("Certificate Decoder (X.509)", "CRT", Category::Security, vec![("Decode", decode_certificates)])
        .describe("Paste one or more PEM certificates (-----BEGIN CERTIFICATE-----) or Base64 DER."))
}

/// Accepts one or more PEM blocks, or a single Base64-encoded DER certificate.
fn decode_certificates(src: &str) -> Result<String, String> {
    let ders: Vec<Vec<u8>> = if src.contains("-----BEGIN") {
        x509_parser::pem::Pem::iter_from_buffer(src.as_bytes())
            .map(|pem| pem.map(|p| p.contents).map_err(|e| format!("Invalid PEM: {e}")))
            .collect::<Result<_, _>>()?
    } else {
        if src.trim().is_empty() { return Err("Paste a certificate".into()); }
        vec![decode_base64(src).map_err(|e| format!("Not PEM, and not DER either: {e}"))?]
    };
    if ders.is_empty() { return Err("No certificate found".into()); }
    let blocks: Vec<String> = ders.iter().enumerate()
        .map(|(i, der)| describe(der).map(|text| if ders.len() > 1 { format!("Certificate {}\n{text}", i + 1) } else { text }))
        .collect::<Result<_, _>>()?;
    Ok(blocks.join("\n\n"))
}

/// Short name of an OID (e.g. `sha256WithRSAEncryption`), or its dotted form if unknown.
fn oid_name(oid: &x509_parser::der_parser::oid::Oid) -> String {
    x509_parser::objects::oid2sn(oid, x509_parser::objects::oid_registry()).map_or_else(|_| oid.to_id_string(), str::to_string)
}

fn fingerprint(digest: &[u8]) -> String { digest.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":") }

/// The human-readable summary of one DER certificate.
fn describe(der: &[u8]) -> Result<String, String> {
    use sha2::Digest;
    use x509_parser::{extensions::GeneralName, public_key::PublicKey};
    let (_, cert) = x509_parser::parse_x509_certificate(der).map_err(|e| format!("Invalid certificate: {e}"))?;
    let validity = cert.validity();
    let status = match validity.time_to_expiration() {
        Some(left) => format!("valid, expires in {} days", left.whole_days()),
        None if validity.not_before.timestamp() > chrono::Utc::now().timestamp() => "not valid yet".into(),
        None => "EXPIRED".into(),
    };
    let key = cert.public_key();
    let key_desc = match key.parsed() {
        Ok(PublicKey::RSA(rsa)) => format!("RSA {} bits", rsa.key_size()),
        Ok(PublicKey::EC(point)) => format!("EC {} bits ({})", point.key_size(), key.algorithm.parameters.as_ref().and_then(|p| p.as_oid().ok()).map_or("unknown curve".into(), |o| oid_name(&o))),
        _ => oid_name(&key.algorithm.algorithm),
    };
    let sans: Vec<String> = match cert.subject_alternative_name() {
        Ok(Some(ext)) => ext.value.general_names.iter().map(|name| match name {
            GeneralName::DNSName(d) => format!("DNS:{d}"),
            GeneralName::RFC822Name(e) => format!("Email:{e}"),
            GeneralName::URI(u) => format!("URI:{u}"),
            GeneralName::IPAddress(ip) => match ip.len() {
                4 => format!("IP:{}", std::net::Ipv4Addr::from(<[u8; 4]>::try_from(*ip).unwrap())),
                16 => format!("IP:{}", std::net::Ipv6Addr::from(<[u8; 16]>::try_from(*ip).unwrap())),
                _ => format!("IP:{ip:02X?}"),
            },
            other => format!("{other}"),
        }).collect(),
        _ => Vec::new(),
    };
    Ok([
        format!("Subject: {}", cert.subject()),
        format!("Issuer: {}", cert.issuer()),
        format!("Serial number: {}", cert.raw_serial_as_string().to_uppercase()),
        format!("Version: {}", cert.version().0 + 1),
        format!("Valid from: {}", validity.not_before),
        format!("Valid until: {}", validity.not_after),
        format!("Status: {status}"),
        format!("Subject alternative names: {}", if sans.is_empty() { "(none)".into() } else { sans.join(", ") }),
        format!("Public key: {key_desc}"),
        format!("Signature algorithm: {}", oid_name(&cert.signature_algorithm.algorithm)),
        format!("Certificate authority: {}", if cert.is_ca() { "yes" } else { "no" }),
        format!("SHA-256 fingerprint: {}", fingerprint(&sha2::Sha256::digest(der))),
        format!("SHA-1 fingerprint: {}", fingerprint(&sha1::Sha1::digest(der))),
    ].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    // Self-signed test certificate generated with openssl for these tests.
    const PEM: &str = "-----BEGIN CERTIFICATE-----
MIIDTzCCAjegAwIBAgIULfQKeSZfsVttDxjdv2q9FtlzVk4wDQYJKoZIhvcNAQEL
BQAwLzEVMBMGA1UEAwwMZXhhbXBsZS50ZXN0MRYwFAYDVQQKDA1EZXZVdGlscyBU
ZXN0MB4XDTI2MTExMzIwNTgxMloXDTM2MTExMDIwNTgxMlowLzEVMBMGA1UEAwwM
ZXhhbXBsZS50ZXN0MRYwFAYDVQQKDA1EZXZVdGlscyBUZXN0MIIBIjANBgkqhkiG
9w0BAQEFAAOCAQ8AMIIBCgKCAQEAu/YDG+DRuiMVFehhZaByP8XEzX6OC+xFdDha
Q9C6mYMEUHBJ1tSVeTBEMyPqzP2EbahoyMZguslvtLjWWklpFmbCuvncUJXDbDcz
BYNfXwmnHK2eoFlk1xSB9jeiKqPpd3X+c2OrWjUQsVbU/fXcy4/boFDyxaDNVhsM
Ui06ZiLj8988YiR+gucbVZfvQuHYZjlUREIbTgfqrZpjig/W5kHw2UbM5aSt2ACe
tm9q/GMjJJICVMs6AcTgBUD4iwgdDVHs5Sb5J5CrcZH5hKtb8ehfj9aSbbNlzS+B
zWYinZtEJoElY/uoRjh5IBJA29yQTbGzNOvEN9jlOlTfcD0RIQIDAQABo2MwYTAd
BgNVHQ4EFgQUW7et5oktj1vi+7v8+6O+yrWdTHQwDwYDVR0TAQH/BAUwAwEB/zAv
BgNVHREEKDAmggxleGFtcGxlLnRlc3SCEHd3dy5leGFtcGxlLnRlc3SHBH8AAAEw
DQYJKoZIhvcNAQELBQADggEBAJLk1+ww4+50vCppPBDpmTAk+v08ALvWmN7xdZMj
qLW54JeG7qHFFdRhFRWTugHkq28wLQePquNxHsLOlIEXvSkxzZFRK5sJxzS1TsQB
56E4wqZFz4yPoPe/dFeooE3UMa+VVAX/OSfjOUlKWjD5rpZ0EmdLHVqZqFJeJSWW
W194Z5d+B1nOyFEVbQrccrAKSsFqEGTVC5Yb19afcMIB3epygzTu/Pvk3TnxSKSD
hAz7MV4K6FT23r2G9isgcoJ1KpW0OcUrJxElDSkAqa0WRrWy60IYg13wOZETMKac
z9j1itOwfIKkpDVQMQI6vgfgw+/uOwQlETpzJvMRaawtnK0=
-----END CERTIFICATE-----";

    #[test]
    fn decodes_pem_matching_openssl() {
        let out = decode_certificates(PEM).unwrap();
        // Values below are what `openssl x509 -noout -fingerprint -sha256 -serial` reports.
        assert!(out.contains("SHA-256 fingerprint: 28:17:E8:CD:9A:F9:0B:BF:F3:74:11:3A:52:83:33:12:14:CE:A6:09:E1:27:9F:88:A4:8A:7C:E0:CA:51:48:2F"), "{out}");
        assert!(out.contains("Serial number: 2D:F4:0A:79:26:5F:B1:5B:6D:0F:18:DD:BF:6A:BD:16:D9:73:56:4E"), "{out}");
        assert!(out.contains("Subject: CN=example.test, O=DevUtils Test"), "{out}");
        assert!(out.contains("Subject alternative names: DNS:example.test, DNS:www.example.test, IP:127.0.0.1"), "{out}");
        assert!(out.contains("Public key: RSA 2048 bits"), "{out}");
        assert!(out.contains("Certificate authority: yes"), "{out}");
        assert!(out.contains("Valid until: Nov 10 20:58:12 2036"), "{out}");
    }

    #[test]
    fn accepts_der_base64_and_chains() {
        let body: String = PEM.lines().filter(|l| !l.starts_with("-----")).collect();
        assert!(decode_certificates(&body).unwrap().contains("Subject: CN=example.test"));
        let chain = decode_certificates(&format!("{PEM}\n{PEM}")).unwrap();
        assert!(chain.starts_with("Certificate 1\n") && chain.contains("\n\nCertificate 2\n"), "{chain}");
        assert!(decode_certificates("hello").is_err());
    }
}
