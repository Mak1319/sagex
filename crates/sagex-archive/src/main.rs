use clap::{Args, Parser, Subcommand};
use flate2::{Compression, write::DeflateEncoder};
use promptuity::{Promptuity, Term, prompts::Password, themes::FancyTheme};
use sagex_capsule::{
    CDFH_MAGIC_FORMAT, CHUNK_SIZE, CURRENT_VERSION, CentralDirectory, CentralDirectoryFileHeader,
    ChunkTag, CipherChunk, DEK_TABLE_FORMAT, DekEntry, EOCD_MAGIC_FORMAT, EndOfCentralDirectory,
    LicenseEntry, LicenseTable, LocalFileHeader, NonCipherChunk, SIGNATUR_FORMAT, Signature,
    ToEncrypted, helper::calculate_ecc,
};
use sagex_crypto::aes::{
    Encapsulate as _, EncapsulationKey, EncryptionBuffer, MlDsa65, TryKeyInit as _, kem::MlKem768,
    key::FromDerived, key_derivation::KeyDerivation,
};
use signature::Signer as _;
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
    trust: PathBuf,
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

fn restore(_args: RestoreArgs) {
    todo!()
}
