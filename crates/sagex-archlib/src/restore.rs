//! Headless port of the `restore` command: same policy matrix,
//! same greedy extraction; prompts become option fields.

use crate::{
    error::ArchError,
    opts::{RestoreOptions, RestoreReport, SigStatus, SkippedEntry},
    util::{LARGE_ARCHIVE, hash_prefix, sanitize_rel, set_mode, verify_signature},
};
use flate2::write::DeflateDecoder;
use sagex_capsule::{
    CentralDirectory, ChunkTag, CipherChunk, EOCD_MAGIC_FORMAT, EndOfCentralDirectory,
    LicenseTable, ToEncrypted, CURRENT_VERSION,
};
use sagex_crypto::aes::{
    Ciphertext, Decapsulate as _, EncryptionBuffer, key::FromDerived, kem::MlKem768,
};

pub fn restore(opts: RestoreOptions) -> Result<RestoreReport, ArchError> {
    use std::io::{Read, Seek, SeekFrom};

    // Phase 0: open archive, tail-scan the EOCD.
    let mut file = std::fs::File::open(&opts.input)
        .map_err(|e| ArchError::Invalid(format!("cannot open archive {}: {e}", opts.input.display())))?;
    let file_len = file
        .seek(SeekFrom::End(0))
        .map_err(|e| ArchError::Invalid(format!("cannot size archive: {e}")))?;
    let tail_len = file_len.min(8192);
    file.seek(SeekFrom::Start(file_len - tail_len))?;
    let mut tail = vec![0u8; tail_len as usize];
    file.read_exact(&mut tail)
        .map_err(|e| ArchError::Invalid(format!("cannot read archive tail: {e}")))?;
    let eocd_magic = [0x5Au8, 0x6E, 0x10, 0x01];
    let eocd_rel = tail
        .windows(4)
        .rposition(|w| w == eocd_magic)
        .ok_or(ArchError::NotAnArchive)?;
    let eocd: EndOfCentralDirectory = postcard::from_bytes(&tail[eocd_rel..])
        .map_err(|_| ArchError::Invalid("cannot parse end-of-central-directory".to_string()))?;
    if eocd.magic_number != EOCD_MAGIC_FORMAT || eocd.version != CURRENT_VERSION {
        return Err(ArchError::Invalid("bad EOCD magic or version".to_string()));
    }
    let eocd_start = file_len - (tail_len - eocd_rel as u64);

    // Phase 1: checksum tripwire (warn + salvage, never fatal by itself).
    let mut checksum_ok = true;
    let intact = match hash_prefix(&mut file, eocd_start) {
        Ok(d) if d == eocd.checksum => true,
        Ok(_) => {
            checksum_ok = false;
            false
        }
        Err(e) => {
            return Err(ArchError::Invalid(format!("cannot checksum archive: {e}")));
        }
    };
    let large = file_len >= LARGE_ARCHIVE;

    // Phase 2: signature policy over signed/intact/large/trust.
    let mut signature = SigStatus::Absent;
    if !eocd.signature {
        if !intact {
            return Err(ArchError::SignatureRequired(
                "checksum mismatch and no signature present".to_string(),
            ));
        }
        if large {
            return Err(ArchError::SignatureRequired(
                "archives >= 256 MiB require a signature".to_string(),
            ));
        }
    } else {
        let required = large || !intact;
        match opts.trust.as_ref() {
            Some(tp) => match verify_signature(&mut file, &eocd, tp) {
                Ok(signer) => signature = SigStatus::Verified(signer),
                Err(e) => {
                    return Err(ArchError::VerificationFailed(e));
                }
            },
            None if required => {
                return Err(ArchError::SignatureRequired(
                    "signature verification required (large archive or checksum mismatch) but no trust key provided".to_string(),
                ));
            }
            None => signature = SigStatus::Skipped,
        }
    }

    // Phase 3: central directory + DEK table source.
    file.seek(SeekFrom::Start(eocd.central_directory_offset))
        .map_err(|e| ArchError::Invalid(format!("cannot seek central directory: {e}")))?;
    let mut cd_buf = vec![0u8; eocd.central_directory_size as usize];
    file.read_exact(&mut cd_buf)
        .map_err(|e| ArchError::Invalid(format!("cannot read central directory: {e}")))?;
    let central_directory: CentralDirectory = postcard::from_bytes(&cd_buf)
        .map_err(|_| ArchError::Invalid("cannot parse central directory".to_string()))?;

    let table: Option<LicenseTable> = if eocd.table {
        // Self-licensed: table embedded in the archive.
        file.seek(SeekFrom::Start(eocd.table_offset))
            .map_err(|e| ArchError::Invalid(format!("cannot seek DEK table: {e}")))?;
        let mut buf = vec![0u8; eocd.table_size as usize];
        file.read_exact(&mut buf)
            .map_err(|e| ArchError::Invalid(format!("cannot read DEK table: {e}")))?;
        Some(
            postcard::from_bytes(&buf)
                .map_err(|_| ArchError::Invalid("cannot parse DEK table".to_string()))?,
        )
    } else {
        // External license sidecar required.
        let lic_path = opts.license.as_ref().ok_or(ArchError::LicenseRequired)?;
        let lic_bytes = std::fs::read(lic_path).map_err(|e| {
            ArchError::Invalid(format!("cannot read license file {}: {e}", lic_path.display()))
        })?;
        Some(
            postcard::from_bytes(&lic_bytes).map_err(|_| {
                ArchError::Invalid(format!("cannot parse license file: {}", lic_path.display()))
            })?,
        )
    };

    // Phase 4: recover the DEK (skipped for unencrypted archives).
    let need_dek = central_directory.entries.iter().any(|e| e.encryption);
    let mut dek: [u8; 32] = [0u8; 32];
    if need_dek {
        let table = table
            .as_ref()
            .ok_or(ArchError::Invalid(
                "encrypted archive but no DEK table available".to_string(),
            ))?;
        let password = opts
            .key_password
            .as_ref()
            .ok_or(ArchError::MissingPassword("key"))?;
        let prv_bytes = std::fs::read(&opts.key).map_err(|e| {
            ArchError::KeyFile(format!("cannot read key {}: {e}", opts.key.display()))
        })?;
        let prv_ext: sagex_keys::KeyExternal = postcard::from_bytes(&prv_bytes)
            .map_err(|_| ArchError::KeyFile(format!("not a valid key file: {}", opts.key.display())))?;
        let dek_raw: Vec<u8> = match prv_ext.internals {
            sagex_keys::KeyType::Private(k) => {
                let name = k.user_name().to_string();
                let derived = k
                    .kem_key()
                    .decrypt_vault(password.as_bytes())
                    .map_err(|_| ArchError::Crypto("cannot unlock archive key".to_string()))?;
                let dk = MlKem768::private_from_derived(derived)
                    .map_err(|_| ArchError::Crypto("cannot rebuild KEM key".to_string()))?;
                let dek_entry = table
                    .entries
                    .values()
                    .flat_map(|le| le.dek_entries.get(&name))
                    .next();
                let dek_entry = match dek_entry {
                    Some(d) => d,
                    None => return Err(ArchError::NoLicenseFor(name)),
                };
                let ct: Ciphertext<MlKem768> = dek_entry
                    .kem_cipher
                    .as_slice()
                    .try_into()
                    .map_err(|_| ArchError::Crypto("malformed KEM ciphertext".to_string()))?;
                let ss = dk.decapsulate(&ct);
                let ss_bytes: [u8; 32] = ss
                    .as_slice()
                    .try_into()
                    .map_err(|_| ArchError::Crypto("bad shared secret length".to_string()))?;
                EncryptionBuffer::decrypt(&dek_entry.dek_cipher, ss_bytes, dek_entry.dek_nonce)
                    .map_err(|_| ArchError::Crypto("DEK unwrap failed".to_string()))?
            }
            _ => {
                return Err(ArchError::KeyFile(format!(
                    "archive key must be private: {}",
                    opts.key.display()
                )));
            }
        };
        if dek_raw.len() != 32 {
            return Err(ArchError::Crypto("unwrapped DEK has bad length".to_string()));
        }
        dek.copy_from_slice(&dek_raw);
    }

    // Phase 5: extract entries (greedy).
    let mut ok: Vec<std::path::PathBuf> = Vec::new();
    let mut skipped: Vec<SkippedEntry> = Vec::new();
    let mut skip = |entry: String, reason: &str| {
        skipped.push(SkippedEntry {
            entry,
            reason: reason.to_string(),
        });
    };
    for (idx, mut header) in central_directory.entries.into_iter().enumerate() {
        let tag = format!("entry #{idx}");
        if header.encryption && header.decrypt(dek).is_err() {
            skip(tag, "cannot decrypt header");
            continue;
        }
        let name_bytes = header.file_name.clone();
        let rel = match sanitize_rel(&name_bytes) {
            Some(r) => r,
            None => {
                skip(tag, "unsafe entry path");
                continue;
            }
        };
        let dest = opts.out_dir.join(rel);
        if header.is_directory {
            if let Err(e) = std::fs::create_dir_all(&dest) {
                skip(tag, &format!("cannot create dir {}: {e}", dest.display()));
                continue;
            }
            set_mode(&dest, header.permission);
            ok.push(dest);
            continue;
        }
        // Chunk-blob range: create writes final_buffer THEN header_bytes,
        // and records the offset AFTER both.
        let blob_end = match header
            .local_file_header_offset
            .checked_sub(header.local_file_header_size)
        {
            Some(v) => v,
            None => {
                skip(tag, "bad entry offsets");
                continue;
            }
        };
        let blob_start = match blob_end.checked_sub(header.encrypted_sized) {
            Some(v) => v,
            None => {
                skip(tag, "bad entry offsets");
                continue;
            }
        };
        if let Err(e) = file.seek(SeekFrom::Start(blob_start)) {
            skip(tag, &format!("cannot seek entry data: {e}"));
            continue;
        }
        let mut blob = vec![0u8; header.encrypted_sized as usize];
        if let Err(e) = file.read_exact(&mut blob) {
            skip(tag, &format!("cannot read entry data: {e}"));
            continue;
        }
        // Sequential tag+chunk parse.
        let mut wire: Vec<u8> = Vec::new();
        let mut pos = 0usize;
        let mut chunk_ok = true;
        let mut chunk_reason = String::new();
        while pos < blob.len() {
            let (chunk_tag, rest): (ChunkTag, &[u8]) = match postcard::take_from_bytes(&blob[pos..]) {
                Ok(v) => v,
                Err(_) => {
                    chunk_reason = "bad chunk tag".to_string();
                    chunk_ok = false;
                    break;
                }
            };
            let used = blob.len() - pos - rest.len();
            pos += used;
            if chunk_tag.size as usize > rest.len() {
                chunk_reason = "chunk overruns entry".to_string();
                chunk_ok = false;
                break;
            }
            let chunk_raw = &rest[..chunk_tag.size as usize];
            pos += chunk_tag.size as usize;
            let payload: Vec<u8> = if header.encryption {
                let chunk: CipherChunk = match postcard::from_bytes(chunk_raw) {
                    Ok(c) => c,
                    Err(_) => {
                        chunk_reason = "bad cipher chunk".to_string();
                        chunk_ok = false;
                        break;
                    }
                };
                let mut data = chunk.buffer;
                if !matches!(
                    header.ecc,
                    sagex_capsule::ErrorCorrectionMethod::None
                ) {
                    data = match sagex_capsule::helper::repair_ecc(&data) {
                        Ok(d) => d,
                        Err(e) => {
                            chunk_reason = format!("ECC repair failed ({e})");
                            chunk_ok = false;
                            break;
                        }
                    };
                }
                match EncryptionBuffer::decrypt(&data, dek, chunk.nonce) {
                    Ok(d) => d,
                    Err(_) => {
                        chunk_reason = "chunk decrypt failed".to_string();
                        chunk_ok = false;
                        break;
                    }
                }
            } else {
                let chunk: sagex_capsule::NonCipherChunk = match postcard::from_bytes(chunk_raw) {
                    Ok(c) => c,
                    Err(_) => {
                        chunk_reason = "bad plain chunk".to_string();
                        chunk_ok = false;
                        break;
                    }
                };
                if !matches!(
                    header.ecc,
                    sagex_capsule::ErrorCorrectionMethod::None
                ) {
                    match sagex_capsule::helper::repair_ecc(&chunk.buffer) {
                        Ok(d) => d,
                        Err(e) => {
                            chunk_reason = format!("ECC repair failed ({e})");
                            chunk_ok = false;
                            break;
                        }
                    }
                } else {
                    chunk.buffer
                }
            };
            wire.extend_from_slice(&payload);
        }
        if !chunk_ok {
            skip(tag, &chunk_reason);
            continue;
        }
        if crc32fast::hash(&wire) != header.crc32_compressed {
            skip(tag, "CRC mismatch on stored form");
            continue;
        }
        let raw: Vec<u8> = match header.compression {
            sagex_capsule::CompressionMethod::None => wire,
            sagex_capsule::CompressionMethod::Deflate => {
                let mut dec = DeflateDecoder::new(Vec::new());
                use std::io::Write as _;
                if let Err(e) = dec.write_all(&wire) {
                    skip(tag, &format!("decompress failed ({e})"));
                    continue;
                }
                match dec.finish() {
                    Ok(d) => d,
                    Err(e) => {
                        skip(tag, &format!("decompress finish failed ({e})"));
                        continue;
                    }
                }
            }
        };
        if crc32fast::hash(&raw) != header.crc32_uncompressed {
            skip(tag, "CRC mismatch on content");
            continue;
        }
        if let Some(parent) = dest.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                skip(tag, &format!("cannot create parent {}: {e}", parent.display()));
                continue;
            }
        }
        if let Err(e) = std::fs::write(&dest, &raw) {
            skip(tag, &format!("cannot write {}: {e}", dest.display()));
            continue;
        }
        set_mode(&dest, header.permission);
        ok.push(dest);
    }

    Ok(RestoreReport {
        ok,
        skipped,
        checksum_ok,
        signature,
    })
}
