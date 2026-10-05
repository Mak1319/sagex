use clap::{Args, Parser, Subcommand};
use flate2::{
    Compression,
    write::{DeflateDecoder, DeflateEncoder},
};
use sagex_capsule::{
    CDFH_MAGIC_FORMAT, CHUNK_SIZE, CentralDirectory, CentralDirectoryFileHeader, ChunkTag,
    CipherChunk, LocalFileHeader, NonCipherChunk, ToEncrypted, helper::calculate_ecc,
};
use sagex_crypto::aes::{EncryptionBuffer, NONCE_LEN, key_derivation::KeyDerivation};
use std::{
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
    if is_sign {}



    // Central directory trailer.
    central_directory.entry_count = central_directory.entries.len() as u64;
    let cd_bytes = postcard::to_allocvec(&central_directory).unwrap();
    file.write_all(&cd_bytes).unwrap();

    // End of central directory
}

fn restore(_args: RestoreArgs) {
    todo!()
}
