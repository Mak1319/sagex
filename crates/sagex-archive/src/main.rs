use clap::{Args, Parser, Subcommand};
use flate2::{Compression, write::{DeflateDecoder, DeflateEncoder}};
use promptuity::{Promptuity, Term, prompts::Password, themes::FancyTheme};
use sagex_capsule::{
    CDFH_MAGIC_FORMAT, CHUNK_SIZE, CURRENT_VERSION, CentralDirectory, CentralDirectoryFileHeader,
    ChunkTag, CipherChunk, DEK_TABLE_FORMAT, DekEntry, EOCD_MAGIC_FORMAT, EndOfCentralDirectory,
    LicenseEntry, LicenseTable, LocalFileHeader, NonCipherChunk, SIGNATUR_FORMAT, Signature,
    ToEncrypted, helper::{calculate_ecc, repair_ecc},
};
use sagex_crypto::aes::{
    Ciphertext, Decapsulate as _, DsaKeyInit as _, Encapsulate as _, EncapsulationKey,
    EncryptionBuffer, MlDsa65, Signature as MlDsaSignature, TryKeyInit as _, VerifyingKey,
    key::FromDerived, key_derivation::KeyDerivation, kem::MlKem768,
};
use signature::{Signer as _, Verifier as _};
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{Seek, Write},
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::PathBuf,
};

#[derive(Parser, Debug)]
#[command(name = "nidhi", version, about = "sagex archive tool")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Create(CreateArgs),
    Restore(RestoreArgs),
}

#[derive(Args, Debug)]
struct CreateArgs {
    #[arg(short = 'i', long = "input")]
    input: PathBuf,

    #[arg(short = 'o', long = "out", value_name = "FILE")]
    out: PathBuf,

    #[arg(short = 'k', long = "key")]
    key: Vec<PathBuf>,

    #[arg(short = 's', long = "sign")]
    sign: Option<PathBuf>,

    #[arg(short = 'c', long = "compress")]
    compress: bool,

    #[arg(short = 'e', long = "ecc")]
    ecc: bool,

    #[arg(short = 'l', long = "license")]
    license: bool,

    #[arg(long = "license-out", value_name = "FILE")]
    license_out: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct RestoreArgs {
    #[arg(short = 'i', long = "input")]
    input: PathBuf,

    #[arg(long = "od", alias = "out-dir", value_name = "DIR")]
    out_dir: PathBuf,

    #[arg(long = "key")]
    key: PathBuf,

    #[arg(long = "trust", value_name = "PATH")]
    trust: Option<PathBuf>,

    #[arg(long = "license", value_name = "FILE")]
    license: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Create(args) => create(args),
        Cmd::Restore(args) => restore(args),
    }
}

fn permission_u16(path: &PathBuf) -> std::io::Result<u16> {
    let mode = std::fs::symlink_metadata(path)?.permissions().mode();
    Ok((mode & 0o7777) as u16)
}

/// SHA-256 over `file[0..end)` with constant 64KB memory.
/// Leaves the cursor wherever the read finished; callers restore it.
fn hash_prefix(file: &mut std::fs::File, end: u64) -> std::io::Result<[u8; 32]> {
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

fn create(args: CreateArgs) {
    let entries: Vec<PathBuf> = walkdir::WalkDir::new(&args.input)
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .collect();

    let is_enc = !args.key.is_empty();
    let is_sign = args.sign.is_some();
    let is_ecc = &args.ecc;
    let is_compress = &args.compress;

    let Ok(mut file) = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(args.out)
    else {
        println!("Can not create new file");
        return;
    };

    // Writing all the local entries
    let key = KeyDerivation::get_random_key();
    let key_index = 0;

    let mut central_directory = CentralDirectory::default();

    let mut end_of_central_directory = EndOfCentralDirectory::default();

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
                        println!("Can not encrypt header files");
                        return;
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
                compression: if *is_compress {
                    sagex_capsule::CompressionMethod::Deflate
                } else {
                    sagex_capsule::CompressionMethod::None
                },
                encryption: is_enc,
                ecc: if *is_ecc {
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
                        println!("Can not encrypt header files");
                        return;
                    }
                    _ => {}
                }
            }

            // Actual file data
            let mut file_data: Vec<u8> = match std::fs::read(entry) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("skipping {}: {e}", entry.display());
                    continue;
                }
            };


            // update header
            let uncompressed_crc32 = crc32fast::hash(&file_data);
            let uncompressed_size = file_data.len();


            // Compress the file_data
            if *is_compress {
                let mut data = Vec::new();
                let mut encoder = DeflateEncoder::new(&mut data, Compression::default());
                encoder.write_all(&file_data).unwrap();
                match encoder.finish() {
                    Err(_) => println!(
                        "Can not compress the file {:?}",
                        entry.clone().into_string()
                    ),
                    _ => {}
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
                        println!("Can not encrypt buffer ");
                        return;
                    };

                    // ECC: append parity shards so a damaged chunk can be rebuilt.
                    if *is_ecc {
                        match calculate_ecc(&payload) {
                            Ok(ecc) => payload.extend(ecc),
                            Err(_) => {
                                println!("Can not calculate ecc");
                                return;
                            }
                        }
                    }

                    let ciphered_chunk = CipherChunk {
                        key_index: key_index,
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
                    if *is_ecc {
                        match calculate_ecc(chunk) {
                            Ok(ecc) => payload.extend(ecc),
                            Err(_) => {
                                println!("Can not calculate ecc");
                                return;
                            }
                        }
                    }

                    let plain_chunk = NonCipherChunk { buffer: payload };
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
            println!("Un known entry found can not write things")
        }
    }

    // Signature
    if is_sign {
        let Some(sign_path) = args.sign.as_ref() else {
            println!("No signing key provided");
            return;
        };

        let mut term = Term::default();
        let mut theme = FancyTheme::default();
        let mut p = Promptuity::new(&mut term, &mut theme);
        if let Err(e) = p.with_intro("Unlock signing key").begin() {
            println!("Cannot start prompt: {e}");
            return;
        }

        let mut pw = Password::new(format!("Password for {}", sign_path.display()));
        let password: String = match p.prompt(&mut pw) {
            Ok(v) => v,
            Err(e) => {
                println!("Prompt aborted: {e}");
                return;
            }
        };
        if let Err(e) = p.with_outro("Signing key unlocked").finish() {
            println!("Cannot finish prompt: {e}");
            return;
        }

        let password_bytes = password.as_bytes();

        let key_bytes = match std::fs::read(sign_path) {
            Ok(b) => b,
            Err(e) => {
                println!("Cannot read signing key {}: {e}", sign_path.display());
                return;
            }
        };

        let key_ext: sagex_keys::KeyExternal = match postcard::from_bytes(&key_bytes) {
            Ok(k) => k,
            Err(_) => {
                println!("Not a valid key file: {}", sign_path.display());
                return;
            }
        };

        let internal = match key_ext.internals {
            sagex_keys::KeyType::Private(value) => value,
            sagex_keys::KeyType::Public(_) => {
                println!("Can not sign a key with public keys");
                return;
            }
        };

        let dsa_key = internal.dsa_key();

        let key_derived = match dsa_key.decrypt_vault(password_bytes) {
            Ok(result) => result,
            Err(_err) => {
                println!("Can not open private signature key");
                return;
            }
        };

        let signing_key = match MlDsa65::private_from_derived(key_derived) {
            Ok(value) => value,
            Err(_) => {
                println!("Can not generate private signing key");
                return;
            }
        };




        use std::io::{Seek, SeekFrom};

        let end = match file.stream_position() {
            Ok(p) => p,
            Err(e) => {
                println!("Cannot tell archive position: {e}");
                return;
            }
        };
        let digest = match hash_prefix(&mut file, end) {
            Ok(d) => d,
            Err(e) => {
                println!("Cannot hash archive: {e}");
                return;
            }
        };
        if let Err(e) = file.seek(SeekFrom::Start(end)) {
            println!("Cannot resume archive: {e}");
            return;
        }

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
        for (idx, arg) in args.key.iter().enumerate() {
            let pub_bytes = match std::fs::read(arg) {
                Ok(b) => b,
                Err(e) => {
                    println!("Cannot read recipient key {}: {e}", arg.display());
                    continue;
                }
            };
            let key_ext: sagex_keys::KeyExternal = match postcard::from_bytes(&pub_bytes) {
                Ok(k) => k,
                Err(_) => {
                    println!("Not a valid key file: {}", arg.display());
                    continue;
                }
            };
            let (user_name, kem_bytes) = match key_ext.internals {
                sagex_keys::KeyType::Public(p) => (p.user_name, p.kem_key),
                _ => {
                    println!("Recipient key must be public: {}", arg.display());
                    continue;
                }
            };
            let ek = match EncapsulationKey::<MlKem768>::new_from_slice(&kem_bytes) {
                Ok(ek) => ek,
                Err(_) => {
                    println!("Invalid KEM public key: {}", arg.display());
                    continue;
                }
            };
            let (ct, ss) = ek.encapsulate();
            let ss_bytes: [u8; 32] = match ss.as_slice().try_into() {
                Ok(b) => b,
                Err(_) => {
                    println!("Bad shared secret length: {}", arg.display());
                    continue;
                }
            };
            let kem_cipher: Vec<u8> = ct.to_vec();
            let nonce = KeyDerivation::get_nonce();
            let dek_cipher = match EncryptionBuffer::encrypt(&key, ss_bytes, nonce) {
                Ok(c) => c,
                Err(_) => {
                    println!("Cannot wrap DEK for {}", arg.display());
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
            println!("No recipient could be wrapped; archive would be unreadable by anybody");
            return;
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

        let license_mode = args.license || args.license_out.is_some();
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

            let license_path = args
                .license_out
                .clone()
                .unwrap_or_else(|| PathBuf::from("license.sgx"));
            let mut sidecar = table_bytes;
            sidecar.extend_from_slice(&lic_eocd_bytes);
            match std::fs::write(&license_path, &sidecar) {
                Ok(()) => println!("wrote {}", license_path.display()),
                Err(e) => {
                    println!("Cannot write license file {}: {e}", license_path.display());
                    return;
                }
            }
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
            println!("Cannot checksum archive: {e}");
            return;
        }
    }

    let eocd_bytes = postcard::to_allocvec(&end_of_central_directory).unwrap();
    file.write_all(&eocd_bytes).unwrap();
    // End of central directory
}

/// Archives at or above this size must carry a verifiable signature.
const LARGE_ARCHIVE: u64 = 256 * 1024 * 1024; // 256 MiB

/// Verify the archive signature against a trust key.
/// Returns the signer identity on success, reason on failure.
fn verify_signature(
    file: &mut std::fs::File,
    eocd: &EndOfCentralDirectory,
    trust_path: &PathBuf,
) -> Result<String, String> {
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

fn restore(args: RestoreArgs) {
    use std::io::{Read, Seek, SeekFrom};

    // Phase 0: open archive, tail-scan the EOCD.
    let mut file = match std::fs::File::open(&args.input) {
        Ok(f) => f,
        Err(e) => {
            println!("Cannot open archive {}: {e}", args.input.display());
            return;
        }
    };
    let file_len = match file.seek(SeekFrom::End(0)) {
        Ok(n) => n,
        Err(e) => {
            println!("Cannot size archive: {e}");
            return;
        }
    };
    let tail_len = file_len.min(8192);
    if let Err(e) = file.seek(SeekFrom::Start(file_len - tail_len)) {
        println!("Cannot seek archive: {e}");
        return;
    }
    let mut tail = vec![0u8; tail_len as usize];
    if let Err(e) = file.read_exact(&mut tail) {
        println!("Cannot read archive tail: {e}");
        return;
    }
    let eocd_magic = [0x5Au8, 0x6E, 0x10, 0x01];
    let eocd_rel = match tail.windows(4).rposition(|w| w == eocd_magic) {
        Some(p) => p,
        None => {
            println!("No end-of-central-directory found: not an archive");
            return;
        }
    };
    let eocd: EndOfCentralDirectory = match postcard::from_bytes(&tail[eocd_rel..]) {
        Ok(e) => e,
        Err(_) => {
            println!("Cannot parse end-of-central-directory");
            return;
        }
    };
    if eocd.magic_number != EOCD_MAGIC_FORMAT || eocd.version != CURRENT_VERSION {
        println!("Bad EOCD magic or version");
        return;
    }
    let eocd_start = file_len - (tail_len - eocd_rel as u64);

    // Phase 1: checksum tripwire (warn + salvage, never fatal by itself).
    let intact = match hash_prefix(&mut file, eocd_start) {
        Ok(d) if d == eocd.checksum => {
            println!("checksum OK");
            true
        }
        Ok(_) => {
            println!(
                "WARNING: archive checksum mismatch — file modified; attempting ECC-assisted salvage"
            );
            false
        }
        Err(e) => {
            println!("Cannot checksum archive: {e}");
            return;
        }
    };
    let large = file_len >= LARGE_ARCHIVE;

    // Phase 2: signature policy over signed/intact/large/trust.
    if !eocd.signature {
        if !intact {
            println!("WARNING: checksum mismatch and no signature present; aborting");
            return;
        }
        if large {
            println!("WARNING: archives >= 256 MiB require a signature; aborting");
            return;
        }
    } else {
        let required = large || !intact;
        match args.trust.as_ref() {
            Some(tp) => match verify_signature(&mut file, &eocd, tp) {
                Ok(signer) => println!("signature OK (signer: {signer})"),
                Err(e) => {
                    println!("Signature verification FAILED ({e}); aborting");
                    return;
                }
            },
            None if required => {
                println!(
                    "WARNING: signature verification required (large archive or checksum mismatch) but --trust not provided; aborting"
                );
                return;
            }
            None => println!("note: signed archive, skipping verification (--trust not given)"),
        }
    }

    // Phase 3: central directory + DEK table source.
    if let Err(e) = file.seek(SeekFrom::Start(eocd.central_directory_offset)) {
        println!("Cannot seek central directory: {e}");
        return;
    }
    let mut cd_buf = vec![0u8; eocd.central_directory_size as usize];
    if let Err(e) = file.read_exact(&mut cd_buf) {
        println!("Cannot read central directory: {e}");
        return;
    }
    let central_directory: CentralDirectory = match postcard::from_bytes(&cd_buf) {
        Ok(c) => c,
        Err(_) => {
            println!("Cannot parse central directory");
            return;
        }
    };

    let table: Option<LicenseTable> = if eocd.table {
        // Self-licensed: table embedded in the archive.
        if let Err(e) = file.seek(SeekFrom::Start(eocd.table_offset)) {
            println!("Cannot seek DEK table: {e}");
            return;
        }
        let mut buf = vec![0u8; eocd.table_size as usize];
        if let Err(e) = file.read_exact(&mut buf) {
            println!("Cannot read DEK table: {e}");
            return;
        }
        match postcard::from_bytes(&buf) {
            Ok(t) => Some(t),
            Err(_) => {
                println!("Cannot parse DEK table");
                return;
            }
        }
    } else {
        // External license sidecar required.
        let lic_path = match args.license.as_ref() {
            Some(p) => p.clone(),
            None => {
                println!("Archive needs an external license file; pass --license <FILE>");
                return;
            }
        };
        let lic_bytes = match std::fs::read(&lic_path) {
            Ok(b) => b,
            Err(e) => {
                println!("Cannot read license file {}: {e}", lic_path.display());
                return;
            }
        };
        let lic_table: LicenseTable = match postcard::from_bytes(&lic_bytes) {
            Ok(t) => t,
            Err(_) => {
                println!("Cannot parse license file: {}", lic_path.display());
                return;
            }
        };
        Some(lic_table)
    };

    // Phase 4: recover the DEK (skipped for unencrypted archives).
    let need_dek = central_directory.entries.iter().any(|e| e.encryption);
    let mut dek: [u8; 32] = [0u8; 32];
    if need_dek {
        let table = match table.as_ref() {
            Some(t) => t,
            None => {
                println!("Encrypted archive but no DEK table available");
                return;
            }
        };
        let mut term = Term::default();
        let mut theme = FancyTheme::default();
        let mut p = Promptuity::new(&mut term, &mut theme);
        if let Err(e) = p.with_intro("Unlock archive key").begin() {
            println!("Cannot start prompt: {e}");
            return;
        }
        let mut pw = Password::new(format!("Password for {}", args.key.display()));
        let password: String = match p.prompt(&mut pw) {
            Ok(v) => v,
            Err(e) => {
                println!("Prompt aborted: {e}");
                return;
            }
        };
        if let Err(e) = p.with_outro("Key unlocked").finish() {
            println!("Cannot finish prompt: {e}");
            return;
        }
        let prv_bytes = match std::fs::read(&args.key) {
            Ok(b) => b,
            Err(e) => {
                println!("Cannot read key {}: {e}", args.key.display());
                return;
            }
        };
        let prv_ext: sagex_keys::KeyExternal = match postcard::from_bytes(&prv_bytes) {
            Ok(k) => k,
            Err(_) => {
                println!("Not a valid key file: {}", args.key.display());
                return;
            }
        };
        let dek_raw: Vec<u8> = match prv_ext.internals {
            sagex_keys::KeyType::Private(k) => {
                let name = k.user_name().to_string();
                let derived = match k.kem_key().decrypt_vault(password.as_bytes()) {
                    Ok(d) => d,
                    Err(_) => {
                        println!("Cannot unlock archive key (wrong password?)");
                        return;
                    }
                };
                let dk = match MlKem768::private_from_derived(derived) {
                    Ok(d) => d,
                    Err(_) => {
                        println!("Cannot rebuild KEM key");
                        return;
                    }
                };
                let dek_entry = table
                    .entries
                    .values()
                    .flat_map(|le| le.dek_entries.get(&name))
                    .next();
                let dek_entry = match dek_entry {
                    Some(d) => d,
                    None => {
                        println!("No license for identity '{name}'");
                        return;
                    }
                };
                let ct: Ciphertext<MlKem768> = match dek_entry.kem_cipher.as_slice().try_into() {
                    Ok(c) => c,
                    Err(_) => {
                        println!("Malformed KEM ciphertext");
                        return;
                    }
                };
                let ss = dk.decapsulate(&ct);
                let ss_bytes: [u8; 32] = match ss.as_slice().try_into() {
                    Ok(b) => b,
                    Err(_) => {
                        println!("Bad shared secret length");
                        return;
                    }
                };
                match EncryptionBuffer::decrypt(
                    &dek_entry.dek_cipher,
                    ss_bytes,
                    dek_entry.dek_nonce,
                ) {
                    Ok(d) => d,
                    Err(_) => {
                        println!("DEK unwrap failed");
                        return;
                    }
                }
            }
            _ => {
                println!("Archive key must be private: {}", args.key.display());
                return;
            }
        };
        if dek_raw.len() != 32 {
            println!("Unwrapped DEK has bad length");
            return;
        }
        dek.copy_from_slice(&dek_raw);
    }

    // Phase 5: extract entries (greedy).
    let mut ok_count = 0u32;
    let mut skip_count = 0u32;
    for mut header in central_directory.entries {
        if header.encryption {
            if header.decrypt(dek).is_err() {
                println!("Cannot decrypt header; skipping entry");
                skip_count += 1;
                continue;
            }
        }
        let name_bytes = header.file_name.clone();
        let rel = match sanitize_rel(&name_bytes) {
            Some(r) => r,
            None => {
                println!("Unsafe entry path; skipping");
                skip_count += 1;
                continue;
            }
        };
        let dest = args.out_dir.join(rel);
        if header.is_directory {
            if let Err(e) = std::fs::create_dir_all(&dest) {
                println!("Cannot create dir {}: {e}", dest.display());
                skip_count += 1;
                continue;
            }
            set_mode(&dest, header.permission);
            ok_count += 1;
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
                println!("Bad entry offsets; skipping");
                skip_count += 1;
                continue;
            }
        };
        let blob_start = match blob_end.checked_sub(header.encrypted_sized) {
            Some(v) => v,
            None => {
                println!("Bad entry offsets; skipping");
                skip_count += 1;
                continue;
            }
        };
        if let Err(e) = file.seek(SeekFrom::Start(blob_start)) {
            println!("Cannot seek entry data: {e}");
            skip_count += 1;
            continue;
        }
        let mut blob = vec![0u8; header.encrypted_sized as usize];
        if let Err(e) = file.read_exact(&mut blob) {
            println!("Cannot read entry data: {e}");
            skip_count += 1;
            continue;
        }
        // Sequential tag+chunk parse.
        let mut wire: Vec<u8> = Vec::new();
        let mut pos = 0usize;
        let mut chunk_ok = true;
        while pos < blob.len() {
            let (tag, rest): (ChunkTag, &[u8]) = match postcard::take_from_bytes(&blob[pos..]) {
                Ok(v) => v,
                Err(_) => {
                    println!("Bad chunk tag; skipping entry");
                    chunk_ok = false;
                    break;
                }
            };
            let used = blob.len() - pos - rest.len();
            pos += used;
            if tag.size as usize > rest.len() {
                println!("Chunk overruns entry; skipping entry");
                chunk_ok = false;
                break;
            }
            let chunk_raw = &rest[..tag.size as usize];
            pos += tag.size as usize;
            let payload: Vec<u8> = if header.encryption {
                let chunk: CipherChunk = match postcard::from_bytes(chunk_raw) {
                    Ok(c) => c,
                    Err(_) => {
                        println!("Bad cipher chunk; skipping entry");
                        chunk_ok = false;
                        break;
                    }
                };
                let mut data = chunk.buffer;
                if !matches!(
                    header.ecc,
                    sagex_capsule::ErrorCorrectionMethod::None
                ) {
                    data = match repair_ecc(&data) {
                        Ok(d) => d,
                        Err(e) => {
                            println!("ECC repair failed ({e}); skipping entry");
                            chunk_ok = false;
                            break;
                        }
                    };
                }
                match EncryptionBuffer::decrypt(&data, dek, chunk.nonce) {
                    Ok(d) => d,
                    Err(_) => {
                        println!("Chunk decrypt failed; skipping entry");
                        chunk_ok = false;
                        break;
                    }
                }
            } else {
                let chunk: NonCipherChunk = match postcard::from_bytes(chunk_raw) {
                    Ok(c) => c,
                    Err(_) => {
                        println!("Bad plain chunk; skipping entry");
                        chunk_ok = false;
                        break;
                    }
                };
                if !matches!(
                    header.ecc,
                    sagex_capsule::ErrorCorrectionMethod::None
                ) {
                    match repair_ecc(&chunk.buffer) {
                        Ok(d) => d,
                        Err(e) => {
                            println!("ECC repair failed ({e}); skipping entry");
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
            skip_count += 1;
            continue;
        }
        if crc32fast::hash(&wire) != header.crc32_compressed {
            println!("CRC mismatch on stored form; skipping entry");
            skip_count += 1;
            continue;
        }
        let raw: Vec<u8> = match header.compression {
            sagex_capsule::CompressionMethod::None => wire,
            sagex_capsule::CompressionMethod::Deflate => {
                let mut dec = DeflateDecoder::new(Vec::new());
                use std::io::Write as _;
                if let Err(e) = dec.write_all(&wire) {
                    println!("Decompress failed ({e}); skipping entry");
                    skip_count += 1;
                    continue;
                }
                match dec.finish() {
                    Ok(d) => d,
                    Err(e) => {
                        println!("Decompress finish failed ({e}); skipping entry");
                        skip_count += 1;
                        continue;
                    }
                }
            }
        };
        if crc32fast::hash(&raw) != header.crc32_uncompressed {
            println!("CRC mismatch on content; skipping entry");
            skip_count += 1;
            continue;
        }
        if let Some(parent) = dest.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                println!("Cannot create parent {}: {e}", parent.display());
                skip_count += 1;
                continue;
            }
        }
        if let Err(e) = std::fs::write(&dest, &raw) {
            println!("Cannot write {}: {e}", dest.display());
            skip_count += 1;
            continue;
        }
        set_mode(&dest, header.permission);
        ok_count += 1;
    }

    println!("restore done: {ok_count} ok, {skip_count} skipped");
}

/// Reject absolute paths and `..` escapes; return path relative-ized for `--od`.
fn sanitize_rel(name_bytes: &[u8]) -> Option<PathBuf> {
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
fn set_mode(path: &PathBuf, mode: u16) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode as u32));
}

#[cfg(not(unix))]
fn set_mode(_path: &PathBuf, _mode: u16) {}
