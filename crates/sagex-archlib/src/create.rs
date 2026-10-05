//! Headless port of the `create` command: same format, same order,
//! same quirks; prompts become option fields, prints become report/errors.

use crate::{
    error::ArchError,
    opts::{CreateOptions, CreateReport},
    util::{hash_prefix, permission_u16},
};
use flate2::{
    Compression,
    write::DeflateEncoder,
};
use sagex_capsule::{
    CDFH_MAGIC_FORMAT, CHUNK_SIZE, CURRENT_VERSION, CentralDirectory, CentralDirectoryFileHeader,
    ChunkTag, CipherChunk, DEK_TABLE_FORMAT, DekEntry, EOCD_MAGIC_FORMAT, EndOfCentralDirectory,
    LicenseEntry, LicenseTable, LocalFileHeader, SIGNATUR_FORMAT, Signature, ToEncrypted,
    helper::calculate_ecc,
};
use sagex_crypto::aes::{
    Encapsulate as _, EncapsulationKey, EncryptionBuffer, MlDsa65, TryKeyInit as _,
    key::FromDerived, key_derivation::KeyDerivation, kem::MlKem768,
};
use signature::Signer as _;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{Seek, Write},
    os::unix::ffi::OsStrExt,
    path::PathBuf,
};

pub fn create(opts: CreateOptions) -> Result<CreateReport, ArchError> {
    let entries: Vec<PathBuf> = walkdir::WalkDir::new(&opts.input)
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .collect();

    let is_enc = !opts.recipient_keys.is_empty();
    let is_sign = opts.sign_key.is_some();

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&opts.out)
        .map_err(|_| ArchError::Invalid(format!("cannot create {}", opts.out.display())))?;

    // Writing all the local entries
    let key = KeyDerivation::get_random_key();
    let key_index = 0;

    let mut central_directory = CentralDirectory::default();

    let mut end_of_central_directory = EndOfCentralDirectory::default();
    let mut warnings: Vec<String> = Vec::new();

    for entry in &entries {
        if entry.is_dir() {
            let permission = permission_u16(entry).unwrap_or(0o755);
            let mut central_directory_header = CentralDirectoryFileHeader {
                magic_number: CDFH_MAGIC_FORMAT,
                permission,
                is_directory: true,
                compression: sagex_capsule::CompressionMethod::None,
                encryption: is_enc,
                ecc: sagex_capsule::ErrorCorrectionMethod::None,
                crc32_compressed: 0,
                crc32_uncompressed: 0,
                file_name: entry.as_os_str().as_bytes().to_vec(),
                file_nonce: None,
                compressed_size: 0,
                uncompressed_size: 0,
                local_file_header_offset: 0,
                encrypted_sized: 0,
                local_file_header_size: 0,
                comment: "".into(),
            };

            // Encrypt the file
            if is_enc {
                match central_directory_header.encrypt(key) {
                    Ok(()) => (),
                    Err(sagex_capsule::error::CapsuleError::EncryptionError(_)) => {
                        return Err(ArchError::Crypto("cannot encrypt header".to_string()));
                    }
                    _ => {}
                }
            }

            // Local file header
            let local_file_header: LocalFileHeader = central_directory_header.clone().into();
            let buffer = postcard::to_allocvec(&local_file_header).unwrap();
            file.write_all(&buffer).unwrap();

            central_directory_header.local_file_header_offset = file.stream_position().unwrap();
            central_directory_header.local_file_header_size = buffer.len() as u64;

            central_directory.entries.push(central_directory_header);
        }
        // If the file
        // Required
        // 1. compress
        // 2. encrypt
        // 3. ecc
        else if entry.is_file() {
            let permission = permission_u16(entry).unwrap_or(0o755);
            let mut central_directory_header = CentralDirectoryFileHeader {
                magic_number: CDFH_MAGIC_FORMAT,
                permission,
                is_directory: false,
                compression: if opts.compress {
                    sagex_capsule::CompressionMethod::Deflate
                } else {
                    sagex_capsule::CompressionMethod::None
                },
                encryption: is_enc,
                ecc: if opts.ecc {
                    sagex_capsule::ErrorCorrectionMethod::ReedSolomon
                } else {
                    sagex_capsule::ErrorCorrectionMethod::None
                },
                crc32_compressed: 0,
                crc32_uncompressed: 0,
                file_name: entry.as_os_str().as_bytes().to_vec(),
                file_nonce: None,
                compressed_size: 0,
                uncompressed_size: 0,
                local_file_header_offset: 0,
                encrypted_sized: 0,
                local_file_header_size: 0,
                comment: "".into(),
            };

            // Encrypt the file
            if is_enc {
                match central_directory_header.encrypt(key) {
                    Ok(()) => (),
                    Err(sagex_capsule::error::CapsuleError::EncryptionError(_)) => {
                        return Err(ArchError::Crypto("cannot encrypt header".to_string()));
                    }
                    _ => {}
                }
            }

            // Actual file data
            let mut file_data: Vec<u8> = match std::fs::read(entry) {
                Ok(data) => data,
                Err(e) => {
                    warnings.push(format!("skipping {}: {e}", entry.display()));
                    continue;
                }
            };

            // update header
            let uncompressed_crc32 = crc32fast::hash(&file_data);
            let uncompressed_size = file_data.len();

            // Compress the file_data
            if opts.compress {
                let mut data = Vec::new();
                let mut encoder = DeflateEncoder::new(&mut data, Compression::default());
                encoder.write_all(&file_data).unwrap();
                if encoder.finish().is_err() {
                    warnings.push(format!("cannot compress {}", entry.display()));
                }

                file_data = data;
            }

            // update header
            let compressed_crc32 = crc32fast::hash(&file_data);
            let compressed_size = file_data.len();

            let mut final_buffer: Vec<u8> = Vec::new();

            let mut current_file_offset = file.stream_position().unwrap();

            // Encrypted: ciphertext (+ ECC parity) per chunk.
            if is_enc {
                for chunk in file_data.chunks(CHUNK_SIZE as usize) {
                    let nonce_bytes = KeyDerivation::get_nonce();
                    let Ok(mut payload) = EncryptionBuffer::encrypt(chunk, key, nonce_bytes) else {
                        return Err(ArchError::Crypto("cannot encrypt buffer".to_string()));
                    };

                    // ECC: append parity shards so a damaged chunk can be rebuilt.
                    if opts.ecc {
                        match calculate_ecc(&payload) {
                            Ok(ecc) => payload.extend(ecc),
                            Err(_) => {
                                return Err(ArchError::Crypto("cannot calculate ecc".to_string()));
                            }
                        }
                    }

                    let ciphered_chunk = CipherChunk {
                        key_index,
                        buffer: payload,
                        nonce: nonce_bytes,
                    };
                    let writable_buffer = postcard::to_allocvec(&ciphered_chunk).unwrap();

                    let chunk_tag = ChunkTag {
                        offset: current_file_offset,
                        size: writable_buffer.len() as u64,
                    };

                    let tag_bytes = postcard::to_allocvec(&chunk_tag).unwrap();

                    current_file_offset += (tag_bytes.len() + writable_buffer.len()) as u64;
                    final_buffer.extend(tag_bytes);
                    final_buffer.extend(writable_buffer);
                }
            }
            // For no encrypted text
            else {
                // Plain: raw chunk (+ ECC parity) per chunk.
                for chunk in file_data.chunks(CHUNK_SIZE as usize) {
                    let mut payload = chunk.to_vec();

                    // ECC: append parity shards so a damaged chunk can be rebuilt.
                    if opts.ecc {
                        match calculate_ecc(chunk) {
                            Ok(ecc) => payload.extend(ecc),
                            Err(_) => {
                                return Err(ArchError::Crypto("cannot calculate ecc".to_string()));
                            }
                        }
                    }

                    let plain_chunk = sagex_capsule::NonCipherChunk { buffer: payload };
                    let writable_buffer = postcard::to_allocvec(&plain_chunk).unwrap();

                    let chunk_tag = ChunkTag {
                        offset: current_file_offset,
                        size: writable_buffer.len() as u64,
                    };

                    let tag_bytes = postcard::to_allocvec(&chunk_tag).unwrap();

                    current_file_offset += (tag_bytes.len() + writable_buffer.len()) as u64;
                    final_buffer.extend(tag_bytes);
                    final_buffer.extend(writable_buffer);
                }
            }

            // Update header with sizes and checksums.
            central_directory_header.crc32_compressed = compressed_crc32;
            central_directory_header.crc32_uncompressed = uncompressed_crc32;
            central_directory_header.compressed_size = compressed_size as u64;
            central_directory_header.uncompressed_size = uncompressed_size as u64;
            central_directory_header.encrypted_sized = final_buffer.len() as u64;

            // Local file header followed by the chunk data.
            let local_file_header: LocalFileHeader = central_directory_header.clone().into();
            let header_bytes = postcard::to_allocvec(&local_file_header).unwrap();
            file.write_all(&final_buffer).unwrap();
            file.write_all(&header_bytes).unwrap();
            let header_offset = file.stream_position().unwrap();

            central_directory_header.local_file_header_offset = header_offset;
            central_directory_header.local_file_header_size = header_bytes.len() as u64;
            central_directory.entries.push(central_directory_header);
        } else {
            warnings.push(format!(
                "unknown entry, cannot write things: {}",
                entry.display()
            ));
        }
    }

    // Signature
    if is_sign {
        let sign_path = opts.sign_key.as_ref().ok_or(ArchError::MissingPassword("sign"))?;
        let password = opts
            .sign_password
            .as_ref()
            .ok_or(ArchError::MissingPassword("sign"))?;
        let password_bytes = password.as_bytes();

        let key_bytes = std::fs::read(sign_path).map_err(|e| {
            ArchError::KeyFile(format!("cannot read signing key {}: {e}", sign_path.display()))
        })?;

        let key_ext: sagex_keys::KeyExternal = postcard::from_bytes(&key_bytes)
            .map_err(|_| ArchError::KeyFile(format!("not a valid key file: {}", sign_path.display())))?;

        let internal = match key_ext.internals {
            sagex_keys::KeyType::Private(value) => value,
            sagex_keys::KeyType::Public(_) => {
                return Err(ArchError::KeyFile("cannot sign with a public key".to_string()));
            }
        };

        let dsa_key = internal.dsa_key();

        let key_derived = dsa_key
            .decrypt_vault(password_bytes)
            .map_err(|_| ArchError::Crypto("cannot open private signature key".to_string()))?;

        let signing_key = MlDsa65::private_from_derived(key_derived)
            .map_err(|_| ArchError::Crypto("cannot build private signing key".to_string()))?;

        use std::io::{Seek, SeekFrom};

        let end = file
            .stream_position()
            .map_err(|e| ArchError::Invalid(format!("cannot tell archive position: {e}")))?;
        let digest = hash_prefix(&mut file, end)
            .map_err(|e| ArchError::Invalid(format!("cannot hash archive: {e}")))?;
        file.seek(SeekFrom::Start(end))?;

        let signature = signing_key.sign(&digest);
        let signature_bytes = signature.encode().to_vec();

        let signature = Signature {
            magic: SIGNATUR_FORMAT,
            signature: signature_bytes,
            user_name: internal.user_name().into(),
        };

        let signature_bytes = postcard::to_allocvec(&signature).unwrap();

        let sig_offset = file.stream_position().unwrap();

        end_of_central_directory.signature = true;
        end_of_central_directory.signature_offset = sig_offset;
        end_of_central_directory.signature_size = signature_bytes.len() as u64;

        file.write_all(&signature_bytes).unwrap();
    }

    if is_enc {
        let mut dek_entries: HashMap<String, DekEntry> = HashMap::new();
        for (idx, arg) in opts.recipient_keys.iter().enumerate() {
            let pub_bytes = match std::fs::read(arg) {
                Ok(b) => b,
                Err(e) => {
                    warnings.push(format!("cannot read recipient key {}: {e}", arg.display()));
                    continue;
                }
            };
            let key_ext: sagex_keys::KeyExternal = match postcard::from_bytes(&pub_bytes) {
                Ok(k) => k,
                Err(_) => {
                    warnings.push(format!("not a valid key file: {}", arg.display()));
                    continue;
                }
            };
            let (user_name, kem_bytes) = match key_ext.internals {
                sagex_keys::KeyType::Public(p) => (p.user_name, p.kem_key),
                _ => {
                    warnings.push(format!("recipient key must be public: {}", arg.display()));
                    continue;
                }
            };
            let ek = match EncapsulationKey::<MlKem768>::new_from_slice(&kem_bytes) {
                Ok(ek) => ek,
                Err(_) => {
                    warnings.push(format!("invalid KEM public key: {}", arg.display()));
                    continue;
                }
            };
            let (ct, ss) = ek.encapsulate();
            let ss_bytes: [u8; 32] = match ss.as_slice().try_into() {
                Ok(b) => b,
                Err(_) => {
                    warnings.push(format!("bad shared secret length: {}", arg.display()));
                    continue;
                }
            };
            let kem_cipher: Vec<u8> = ct.to_vec();
            let nonce = KeyDerivation::get_nonce();
            let dek_cipher = match EncryptionBuffer::encrypt(&key, ss_bytes, nonce) {
                Ok(c) => c,
                Err(_) => {
                    warnings.push(format!("cannot wrap DEK for {}", arg.display()));
                    continue;
                }
            };
            dek_entries.insert(
                user_name,
                DekEntry {
                    key_id: idx as u32,
                    kem_cipher,
                    dek_cipher,
                    dek_nonce: nonce,
                },
            );
        }

        if dek_entries.is_empty() {
            return Err(ArchError::NoRecipients);
        }

        let mut table_entries: HashMap<u32, LicenseEntry> = HashMap::new();
        table_entries.insert(
            0,
            LicenseEntry {
                key_id: 0,
                user_name: String::new(),
                dek_entries,
            },
        );
        let table = LicenseTable {
            magic_format: DEK_TABLE_FORMAT,
            entries: table_entries,
        };
        let table_bytes = postcard::to_allocvec(&table).unwrap();

        let license_mode = opts.license || opts.license_out.is_some();
        if license_mode {
            use sha2::Digest;
            let mut table_hasher = sha2::Sha256::new();
            table_hasher.update(&table_bytes);
            let table_digest: [u8; 32] = table_hasher.finalize().into();

            let lic_eocd = EndOfCentralDirectory {
                magic_number: EOCD_MAGIC_FORMAT,
                version: CURRENT_VERSION,
                table: true,
                table_size: table_bytes.len() as u64,
                table_offset: 0,
                signature: false,
                signature_offset: 0,
                signature_size: 0,
                central_directory_offset: 0,
                central_directory_size: 0,
                checksum: table_digest,
            };
            let lic_eocd_bytes = postcard::to_allocvec(&lic_eocd).unwrap();

            let license_path = opts
                .license_out
                .clone()
                .unwrap_or_else(|| PathBuf::from("license.sgx"));
            let mut sidecar = table_bytes;
            sidecar.extend_from_slice(&lic_eocd_bytes);
            std::fs::write(&license_path, &sidecar).map_err(|e| {
                ArchError::Invalid(format!(
                    "cannot write license file {}: {e}",
                    license_path.display()
                ))
            })?;
        } else {
            let table_offset = file.stream_position().unwrap();
            file.write_all(&table_bytes).unwrap();

            end_of_central_directory.table = true;
            end_of_central_directory.table_offset = table_offset;
            end_of_central_directory.table_size = table_bytes.len() as u64;
        }
    }

    // Central directory trailer.
    central_directory.entry_count = central_directory.entries.len() as u64;
    let entries = central_directory.entries.len();
    let cd_bytes = postcard::to_allocvec(&central_directory).unwrap();
    let cd_offset = file.stream_position().unwrap();
    file.write_all(&cd_bytes).unwrap();
    let cd_size = cd_bytes.len();

    end_of_central_directory.central_directory_offset = cd_offset;
    end_of_central_directory.central_directory_size = cd_size as u64;

    // Checksum over everything preceding the EOCD.
    let eocd_start = file.stream_position().unwrap();
    match hash_prefix(&mut file, eocd_start) {
        Ok(d) => end_of_central_directory.checksum = d,
        Err(e) => {
            return Err(ArchError::Invalid(format!("cannot checksum archive: {e}")));
        }
    }

    let eocd_bytes = postcard::to_allocvec(&end_of_central_directory).unwrap();
    file.write_all(&eocd_bytes).unwrap();
    // End of central directory

    let license = if opts.license || opts.license_out.is_some() {
        Some(
            opts.license_out
                .clone()
                .unwrap_or_else(|| PathBuf::from("license.sgx")),
        )
    } else {
        None
    };
    Ok(CreateReport {
        out: opts.out.clone(),
        license,
        entries,
        warnings,
    })
}
