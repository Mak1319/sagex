use std::path::PathBuf;

use binrw::{BinRead, BinWrite, Endian};
use clap::{Parser, Subcommand};
use rand::RngExt;
use sagex_archive::{
    ArchiveError, build_dek_entry, build_file_record, open_file_record, read_end_header,
    read_wire_value, walk_input_directory, write_magic_bytes, write_restored_file,
    write_wire_value,
};
use sagex_capsule::DekEncapsulation;
use sagex_capsule::format::{
    ArchiveHeader, CentralDir, CentralEntry, DekTable, Eocd, FLAG_HAS_DEK_TABLE, MAGIC, REC_MAGIC,
};
use sagex_capsule::options::DekDecapsulationOptions;
use sagex_crypto::aes::DekSealer;
use sagex_keys::{EncapsulatedKey, PublicKey};
use zeroize::Zeroizing;

/// This is nidhi, the archive maker and opener.
///
/// What it does it locks folders into one capsule file for chosen friends
/// and later opens the capsule back to folders with your private key.
#[derive(Parser)]
#[command(
    name = "nidhi",
    version,
    about = "pack and unpack sagex archives",
    long_about = "Lock folders into one capsule file for chosen friends,\nand later open the capsule back to folders.\n\nExamples:\n  nidhi create -i ./docs -o ./docs.sgx -f ./keys/alice.pub\n  nidhi extract -i ./docs.sgx -k ./keys/alice.prv -o ./restored",
    arg_required_else_help = true
)]
struct NidhiArgs {
    #[command(subcommand)]
    archive_command: ArchiveCommand,
}

#[derive(Subcommand)]
enum ArchiveCommand {
    /// This locks a folder for friends.
    Create {
        /// This is the folder whose files go inside.
        #[arg(short = 'i', long = "in", value_name = "FOLDER")]
        input_folder: PathBuf,
        /// This is the capsule file coming out.
        #[arg(short = 'o', long = "out", value_name = "FILE")]
        output_file: PathBuf,
        /// This is one friend public key file; give many times for many friends.
        #[arg(short = 'f', long = "for", value_name = "PUBKEY")]
        friend_key_files: Vec<PathBuf>,
        /// This is your private key file, used to sign the archive.
        #[arg(short = 's', long = "sign-by", value_name = "PRVKEY")]
        signer_key_file: Option<PathBuf>,
    },
    /// This opens a capsule back to a folder.
    Extract {
        /// This is the capsule file going in.
        #[arg(short = 'i', long = "in", value_name = "FILE")]
        input_file: PathBuf,
        /// This is your private key file.
        #[arg(short = 'k', long = "keys", value_name = "PRVKEY")]
        private_key_file: PathBuf,
        /// This is the folder coming out.
        #[arg(short = 'o', long = "out", value_name = "FOLDER")]
        output_folder: PathBuf,
        /// This is how many bytes may be opened without signer proof.
        #[arg(long = "budget", value_name = "BYTES")]
        memory_budget: Option<u64>,
        /// This is the sender public key file, needed past the budget or with --verify.
        #[arg(long = "trust", value_name = "PUBKEY")]
        trusted_key_file: Option<PathBuf>,
        /// This forces a signature check even inside the budget.
        #[arg(long = "verify", default_value_t = false)]
        force_verify: bool,
        /// This picks what the signature check covers.
        #[arg(
            long = "verify-scope",
            value_name = "central|full",
            default_value = "central"
        )]
        verify_scope: String,
        /// This answers yes to the open-it question without asking.
        #[arg(long = "yes", default_value_t = false)]
        assume_yes: bool,
    },
    /// This prints shell completions to stdout.
    #[command(hide = true)]
    Completions {
        /// This is the shell to complete for.
        #[arg(value_enum)]
        shell_kind: clap_complete::Shell,
    },
}

fn main() -> Result<(), String> {
    let nidhi_arguments = NidhiArgs::parse();
    run_nidhi(nidhi_arguments.archive_command).map_err(|error| error.to_string())
}

fn run_nidhi(archive_command: ArchiveCommand) -> Result<(), ArchiveError> {
    match archive_command {
        ArchiveCommand::Create {
            input_folder,
            output_file,
            friend_key_files,
            signer_key_file,
        } => run_create(
            &input_folder,
            &output_file,
            &friend_key_files,
            signer_key_file.as_ref(),
        ),
        ArchiveCommand::Extract {
            input_file,
            private_key_file,
            output_folder,
            memory_budget,
            trusted_key_file,
            force_verify,
            verify_scope,
            assume_yes,
        } => run_extract(
            &input_file,
            &private_key_file,
            &output_folder,
            memory_budget,
            trusted_key_file.as_ref(),
            force_verify,
            &verify_scope,
            assume_yes,
        ),
        ArchiveCommand::Completions { shell_kind } => {
            use clap::CommandFactory;
            clap_complete::generate(
                shell_kind,
                &mut NidhiArgs::command(),
                "nidhi",
                &mut std::io::stdout(),
            );
            Ok(())
        }
    }
}

fn read_public_key_file(key_path: &PathBuf) -> Result<PublicKey, ArchiveError> {
    let key_bytes =
        std::fs::read(key_path).map_err(|error| ArchiveError::KeyFile(error.to_string()))?;
    serde_json::from_slice(&key_bytes).map_err(|error| ArchiveError::KeyFile(error.to_string()))
}

fn load_signing_seed(signer_key_file: &PathBuf) -> Result<[u8; 32], ArchiveError> {
    let password_prompt = format!("Signer password for {}: ", signer_key_file.display());
    let typed_password: Zeroizing<String> = Zeroizing::new(
        dialoguer::Password::new()
            .with_prompt(&password_prompt)
            .interact()
            .map_err(|error| ArchiveError::InputOutput(error.to_string()))?,
    );
    let private_bytes =
        std::fs::read(signer_key_file).map_err(|error| ArchiveError::KeyFile(error.to_string()))?;
    let wrapped_key: EncapsulatedKey = serde_json::from_slice(&private_bytes)
        .map_err(|error| ArchiveError::KeyFile(error.to_string()))?;
    let (salt_bytes, round_count) = wrapped_key.key.get_signature_deliverable();
    let password_key =
        sagex_crypto::aes::generate_key(typed_password.as_bytes(), salt_bytes, round_count);
    let derived_key = wrapped_key
        .key
        .get_signature_key(password_key)
        .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
    let seed_bytes = derived_key.secret_bytes();
    if seed_bytes.len() != 32 {
        return Err(ArchiveError::Crypto(
            "signer key has a bad size".to_string(),
        ));
    }
    let mut signing_seed = [0u8; 32];
    signing_seed.copy_from_slice(seed_bytes);
    Ok(signing_seed)
}

fn run_create(
    input_folder: &PathBuf,
    output_file: &PathBuf,
    friend_key_files: &[PathBuf],
    signer_key_file: Option<&PathBuf>,
) -> Result<(), ArchiveError> {
    if friend_key_files.is_empty() {
        return Err(ArchiveError::KeyFile(
            "give at least one friend key with -f".to_string(),
        ));
    }
    let plain_files = walk_input_directory(input_folder)?;
    let plain_total_bytes: u64 = plain_files.iter().map(|f| f.file_bytes.len() as u64).sum();
    // Big archives must be signed. Without a key up front, offer to sign now.
    let signer_key_file: Option<PathBuf> = match signer_key_file {
        Some(given_key_file) => Some(given_key_file.clone()),
        None if plain_total_bytes > sagex_safecap::DEFAULT_BUDGET => {
            let sign_now = dialoguer::Confirm::new()
                .with_prompt(format!(
                    "archive holds {} bytes, over 256 MiB: sign it now?",
                    plain_total_bytes
                ))
                .default(true)
                .interact()
                .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
            if !sign_now {
                return Err(ArchiveError::KeyFile(
                    "over-size archive left unsigned: refusing to write".to_string(),
                ));
            }
            let signer_prompt = dialoguer::Input::<String>::new()
                .with_prompt("Signer private key file")
                .interact_text()
                .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
            Some(PathBuf::from(signer_prompt))
        }
        None => None,
    };
    let signing_seed = signer_key_file
        .as_ref()
        .map(load_signing_seed)
        .transpose()?;
    let mut random_source = rand::rand_core::UnwrapErr(rand::rngs::SysRng);
    let archive_key_bytes: [u8; 32] = random_source.random();
    let archive_key = DekSealer::new(archive_key_bytes);
    let mut file_unique_nonce: [u8; 12] = random_source.random();
    // Build every record first so signatures land before serialization.
    let mut built_records = Vec::new();
    for plain_file in &plain_files {
        for nonce_byte in file_unique_nonce.iter_mut() {
            *nonce_byte = nonce_byte.wrapping_add(1);
        }
        let mut file_record = build_file_record(plain_file, &archive_key, file_unique_nonce)?;
        if let Some(signing_seed) = &signing_seed {
            // Flags must be final before the canonical bytes are hashed:
            // the checker feeds the stored flags, so signing pre-flag bytes
            // would never verify. (Signature bytes themselves stay excluded.)
            file_record.record_flags |= sagex_capsule::format::FLAG_HAS_SIG;
            let canonical_bytes = sagex_capsule::verify::file_canonical_bytes(&file_record);
            let signature_bytes = sagex_crypto::aes::dsa44_sign(signing_seed, &canonical_bytes)
                .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
            file_record.signature_length = signature_bytes.len() as u16;
            file_record.signature_bytes = signature_bytes;
        }
        println!(
            "packed {} ({} plain bytes){}",
            plain_file.relative_path,
            plain_file.file_bytes.len(),
            if signing_seed.is_some() {
                " signed"
            } else {
                ""
            }
        );
        built_records.push(file_record);
    }
    let mut archive_bytes = Vec::new();
    write_magic_bytes(&mut archive_bytes, MAGIC);
    write_wire_value(&mut archive_bytes, |c| {
        ArchiveHeader::new(0).write_options(c, Endian::Little, ())
    })?;
    let mut central_entries = Vec::new();
    for file_record in &built_records {
        let record_offset = archive_bytes.len() as u64;
        write_magic_bytes(&mut archive_bytes, REC_MAGIC);
        write_wire_value(&mut archive_bytes, |c| {
            file_record.write_options(c, Endian::Little, ())
        })?;
        let record_length = archive_bytes.len() as u64 - record_offset;
        central_entries.push(CentralEntry {
            record_offset,
            record_length,
        });
    }
    let central_directory = CentralDir {
        entry_count: central_entries.len() as u16,
        has_dek_table: true,
        reserved: 0,
        record_entries: central_entries,
    };
    let central_directory_offset = write_wire_value(&mut archive_bytes, |c| {
        central_directory.write_options(c, Endian::Little, ())
    })?;
    let central_directory_length = archive_bytes.len() as u64 - central_directory_offset;
    let mut dek_lines = Vec::new();
    for friend_key_file in friend_key_files {
        let friend_key = read_public_key_file(friend_key_file)?;
        let friend_name = friend_key.user_name.clone();
        let wrapped_packet = DekEncapsulation::new(archive_key_bytes, friend_key)
            .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
        dek_lines.push(build_dek_entry(
            friend_name,
            *wrapped_packet.nonce(),
            wrapped_packet.dek_ciphertext().to_vec(),
            wrapped_packet.kem_ciphertext().to_vec(),
        ));
    }
    let dek_table = DekTable {
        entry_count: dek_lines.len() as u32,
        recipient_entries: dek_lines,
    };
    let archive_flags = if signing_seed.is_some() {
        FLAG_HAS_DEK_TABLE | sagex_capsule::format::FLAG_ARCHIVE_SIG
    } else {
        FLAG_HAS_DEK_TABLE
    };
    // Archive signature trailer sits immediately after the central map,
    // exactly where the checker reads it back.
    if let Some(signing_seed) = &signing_seed {
        let canonical_bytes =
            sagex_capsule::verify::central_canonical_bytes(&central_directory, &dek_table)
                .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
        let archive_signature = sagex_crypto::aes::dsa44_sign(signing_seed, &canonical_bytes)
            .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
        archive_bytes.extend_from_slice(&archive_signature);
        println!(
            "signed central map ({} signature bytes)",
            archive_signature.len()
        );
    }
    let dek_table_offset = write_wire_value(&mut archive_bytes, |c| {
        dek_table.write_options(c, Endian::Little, ())
    })?;
    let dek_table_length = archive_bytes.len() as u64 - dek_table_offset;
    let mut end_header = Eocd::new(archive_flags, plain_files.len() as u32);
    end_header.central_directory_offset = central_directory_offset;
    end_header.central_directory_length = central_directory_length;
    end_header.dek_table_offset = dek_table_offset;
    end_header.dek_table_len = dek_table_length;
    write_magic_bytes(&mut archive_bytes, MAGIC);
    write_wire_value(&mut archive_bytes, |c| {
        end_header.write_options(c, Endian::Little, ())
    })?;
    std::fs::write(output_file, &archive_bytes)
        .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
    println!(
        "wrote {} ({} bytes) with {} files for {} friends{}",
        output_file.display(),
        archive_bytes.len(),
        plain_files.len(),
        friend_key_files.len(),
        if signing_seed.is_some() {
            ", signed"
        } else {
            ""
        }
    );
    Ok(())
}

fn run_extract(
    input_file: &PathBuf,
    private_key_file: &PathBuf,
    output_folder: &PathBuf,
    memory_budget: Option<u64>,
    trusted_key_file: Option<&PathBuf>,
    force_verify: bool,
    verify_scope: &str,
    assume_yes: bool,
) -> Result<(), ArchiveError> {
    let budget_bytes = memory_budget.unwrap_or(sagex_safecap::DEFAULT_BUDGET);
    let mut archive_reader = std::fs::File::open(input_file)
        .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
    let archive_length = archive_reader
        .metadata()
        .map_err(|error| ArchiveError::InputOutput(error.to_string()))?
        .len();
    // Safety gate first: measure before holding anything big.
    let file_prediction = sagex_safecap::scan(&mut archive_reader, archive_length)
        .map_err(|error| ArchiveError::Encoding(format!("{error:?}")))?;
    let memory_budget = sagex_safecap::Budget {
        maximum_allowed_bytes: budget_bytes,
    };
    let needs_proof = file_prediction.total_claimed_bytes > memory_budget.maximum_allowed_bytes;
    if needs_proof || force_verify {
        let trusted_path = trusted_key_file.ok_or_else(|| {
            ArchiveError::KeyFile("signature proof needed: pass --trust <sender.pub>".to_string())
        })?;
        let scope = match verify_scope {
            "full" => sagex_safecap::VerifyScope::PerFileAndCentral,
            _ => sagex_safecap::VerifyScope::CentralOnly,
        };
        let signer_report = sagex_safecap::Report {
            prediction: file_prediction.clone(),
            scope,
        };
        let trust_bytes = std::fs::read(trusted_path)
            .map_err(|error| ArchiveError::KeyFile(error.to_string()))?;
        let trust_key: PublicKey = serde_json::from_slice(&trust_bytes)
            .map_err(|error| ArchiveError::KeyFile(error.to_string()))?;
        let mut verify_reader = std::fs::File::open(input_file)
            .map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
        let signer_is_good = signer_report
            .verify_signatures(&mut verify_reader, &trust_key.signature_key)
            .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
        if !signer_is_good {
            return Err(ArchiveError::Crypto(
                "signer proof failed: refusing to open".to_string(),
            ));
        }
        let go_on = if assume_yes {
            true
        } else {
            dialoguer::Confirm::new()
                .with_prompt(format!(
                    "archive claims {} bytes (budget {}), signer valid: open it?",
                    signer_report.prediction.total_claimed_bytes, budget_bytes
                ))
                .default(false)
                .interact()
                .map_err(|error| ArchiveError::InputOutput(error.to_string()))?
        };
        if !go_on {
            return Err(ArchiveError::InputOutput("stopped by user".to_string()));
        }
    }
    let archive_bytes =
        std::fs::read(input_file).map_err(|error| ArchiveError::InputOutput(error.to_string()))?;
    let end_header: Eocd = read_end_header(&archive_bytes)?;
    let central_directory: CentralDir =
        read_wire_value(&archive_bytes, end_header.central_directory_offset, |c| {
            CentralDir::read_options(c, Endian::Little, ())
        })?;
    let password_prompt = format!("Password for {}: ", private_key_file.display());
    let typed_password: Zeroizing<String> = Zeroizing::new(
        dialoguer::Password::new()
            .with_prompt(&password_prompt)
            .interact()
            .map_err(|error| ArchiveError::InputOutput(error.to_string()))?,
    );
    let private_bytes = std::fs::read(private_key_file)
        .map_err(|error| ArchiveError::KeyFile(error.to_string()))?;
    let wrapped_key: EncapsulatedKey = serde_json::from_slice(&private_bytes)
        .map_err(|error| ArchiveError::KeyFile(error.to_string()))?;
    let key_owner_name = wrapped_key.key.user_name().to_string();
    let decapsulation_options =
        DekDecapsulationOptions::derive_kem_key(typed_password.as_bytes(), wrapped_key.key)
            .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
    let dek_table: DekTable = read_wire_value(&archive_bytes, end_header.dek_table_offset, |c| {
        DekTable::read_options(c, Endian::Little, ())
    })?;
    let my_line = dek_table
        .recipient_entries
        .iter()
        .find(|one_line| one_line.username == key_owner_name)
        .ok_or_else(|| {
            ArchiveError::KeyFile(format!("no packet for {key_owner_name} in this archive"))
        })?;
    let stored_packet = DekEncapsulation::from_parts(
        my_line.wrapping_nonce,
        my_line.wrapped_dek_bytes.clone(),
        my_line.kem_ciphertext_bytes.clone(),
    );
    let archive_key_bytes = decapsulation_options
        .generate_dek(&stored_packet)
        .map_err(|error| ArchiveError::Crypto(format!("{error:?}")))?;
    let archive_key = DekSealer::new(archive_key_bytes);
    let mut restored_count = 0u32;
    for central_entry in &central_directory.record_entries {
        let file_record: sagex_capsule::format::FileRecord =
            read_wire_value(&archive_bytes, central_entry.record_offset, |c| {
                sagex_capsule::format::FileRecord::read_options(c, Endian::Little, ())
            })?;
        let plain_file = open_file_record(&file_record, &archive_key)?;
        let restored_path = write_restored_file(output_folder, &plain_file)?;
        println!(
            "restored {} ({} bytes)",
            restored_path.display(),
            plain_file.file_bytes.len()
        );
        restored_count += 1;
    }
    println!(
        "restored {restored_count} files into {}",
        output_folder.display()
    );
    Ok(())
}
