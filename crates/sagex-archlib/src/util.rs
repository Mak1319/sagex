//! Shared helpers ported verbatim from the `sagex-archive` binary.

use sagex_capsule::{EndOfCentralDirectory, SIGNATUR_FORMAT, Signature};
use sagex_crypto::aes::{MlDsa65, VerifyingKey};
use signature::Verifier as _;
use std::path::PathBuf;

/// Archives at or above this size must carry a verifiable signature.
pub const LARGE_ARCHIVE: u64 = 256 * 1024 * 1024; // 256 MiB

pub fn permission_u16(path: &PathBuf) -> std::io::Result<u16> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::symlink_metadata(path)?.permissions().mode();
    Ok((mode & 0o7777) as u16)
}

/// SHA-256 over `file[0..end)` with constant 64KB memory.
/// Leaves the cursor wherever the read finished; callers restore it.
pub fn hash_prefix(file: &mut std::fs::File, end: u64) -> std::io::Result<[u8; 32]> {
    use sha2::Digest;
    use std::io::{Read, Seek, SeekFrom};
    let mut hasher = sha2::Sha256::new();
    file.seek(SeekFrom::Start(0))?;
    let mut limited = (&mut *file).take(end);
    let mut buf = [0u8; 65536];
    loop {
        let n = limited.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().into())
}

/// Verify the archive signature against a trust key.
/// Returns the signer identity on success, reason on failure.
pub fn verify_signature(
    file: &mut std::fs::File,
    eocd: &EndOfCentralDirectory,
    trust_path: &PathBuf,
) -> Result<String, String> {
    use sagex_crypto::aes::{DsaKeyInit as _, Signature as MlDsaSignature};
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(eocd.signature_offset))
        .map_err(|e| format!("seek signature: {e}"))?;
    let mut sig_bytes = vec![0u8; eocd.signature_size as usize];
    file.read_exact(&mut sig_bytes)
        .map_err(|e| format!("read signature: {e}"))?;
    let sig: Signature =
        postcard::from_bytes(&sig_bytes).map_err(|_| "cannot parse signature record".to_string())?;
    if sig.magic != SIGNATUR_FORMAT {
        return Err("bad signature magic".to_string());
    }
    let trust_bytes = std::fs::read(trust_path)
        .map_err(|e| format!("cannot read trust key {}: {e}", trust_path.display()))?;
    let trust_ext: sagex_keys::KeyExternal =
        postcard::from_bytes(&trust_bytes).map_err(|_| "not a valid key file".to_string())?;
    let trust_pub = match trust_ext.internals {
        sagex_keys::KeyType::Public(p) => p,
        _ => return Err("trust key must be public".to_string()),
    };
    let vk = VerifyingKey::<MlDsa65>::new_from_slice(&trust_pub.dsa_key)
        .map_err(|_| "invalid DSA trust key".to_string())?;
    let digest = hash_prefix(file, eocd.signature_offset)
        .map_err(|e| format!("cannot hash signed range: {e}"))?;
    let ml_sig = MlDsaSignature::<MlDsa65>::try_from(sig.signature.as_slice())
        .map_err(|_| "malformed ML-DSA signature".to_string())?;
    vk.verify(&digest, &ml_sig)
        .map_err(|_| "cryptographic verification failed".to_string())?;
    if sig.user_name != trust_pub.user_name {
        return Err("signer identity mismatch".to_string());
    }
    Ok(sig.user_name)
}

/// Reject absolute paths and `..` escapes; return path relative-ized for output.
pub fn sanitize_rel(name_bytes: &[u8]) -> Option<PathBuf> {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let p = PathBuf::from(OsStr::from_bytes(name_bytes));
    let mut rel = PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::Normal(c) => rel.push(c),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    Some(rel)
}

#[cfg(unix)]
pub fn set_mode(path: &PathBuf, mode: u16) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode as u32));
}

#[cfg(not(unix))]
pub fn set_mode(_path: &PathBuf, _mode: u16) {}
