use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct JwtClaims {
    pub iss: String,
    #[allow(dead_code)]
    pub sub: String,
    pub username: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Verify compact ML-DSA JWT: `b64u(header).b64u(payload).b64u(sig)`.
/// Returns parsed claims on success.
pub fn verify_jwt(
    token: &str,
    chatsrv_vk: &[u8],
    expect_iss: &str,
    expect_aud: &str,
) -> anyhow::Result<JwtClaims> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        anyhow::bail!("jwt must have 3 parts");
    }
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    let sig = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|e| anyhow::anyhow!("jwt sig b64: {e}"))?;
    sagex_crypto::pqc::dsa::verify_signature_bytes(chatsrv_vk, signing_input.as_bytes(), &sig)
        .map_err(|e| anyhow::anyhow!("jwt ML-DSA verify failed: {e}"))?;
    let payload = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| anyhow::anyhow!("jwt payload b64: {e}"))?;
    let claims: JwtClaims =
        serde_json::from_slice(&payload).map_err(|e| anyhow::anyhow!("jwt claims json: {e}"))?;
    if claims.iss != expect_iss {
        anyhow::bail!("jwt iss mismatch");
    }
    if claims.aud != expect_aud {
        anyhow::bail!("jwt aud mismatch");
    }
    let now = now_unix();
    if !(claims.iat - 60 <= now && now <= claims.exp + 60) {
        anyhow::bail!("jwt expired or not yet valid");
    }
    Ok(claims)
}

/// Proof-of-possession: user ML-DSA-signs canonical CSR bytes.
pub fn verify_pop(
    csr: &sagex_format::format::PublicFileFormatExternal,
    pop_sig: &[u8],
) -> anyhow::Result<()> {
    use sagex_format::format::{MAGIC_NUMBER, VERSION};
    if csr.internal.magic_number != MAGIC_NUMBER {
        anyhow::bail!("bad magic");
    }
    if csr.internal.version != VERSION {
        anyhow::bail!("unsupported version");
    }
    let canonical =
        postcard::to_allocvec(&csr.internal).map_err(|e| anyhow::anyhow!("csr encode: {e}"))?;
    sagex_crypto::pqc::dsa::verify_signature_bytes(&csr.internal.key_dsa, &canonical, pop_sig)
        .map_err(|e| anyhow::anyhow!("PoP verify failed: {e}"))?;
    Ok(())
}

pub fn decode_sig(s: &str) -> anyhow::Result<Vec<u8>> {
    use base64::Engine as _;
    let t = s.trim();
    // try hex first, then b64url, then std b64
    if t.len() % 2 == 0 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(v) = crate::config::hex_decode(t) {
            return Ok(v);
        }
    }
    if let Ok(v) = URL_SAFE_NO_PAD.decode(t) {
        return Ok(v);
    }
    base64::engine::general_purpose::STANDARD
        .decode(t)
        .map_err(|e| anyhow::anyhow!("sig decode: {e}"))
}
