use clap::{Parser, Subcommand};
use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "raw-deflate")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Raw DEFLATE compression
    Deflate {
        input: PathBuf,
        output: PathBuf,

        #[arg(short, long, default_value_t = 6)]
        level: u32,
    },

    /// Raw DEFLATE decompression
    Inflate { input: PathBuf, output: PathBuf },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Deflate {
            input,
            output,
            level,
        } => {
            let data = fs::read(input)?;

            let mut compressor = Compress::new(Compression::new(level), false);

            let mut compressed = Vec::new();

            compressor.compress_vec(&data, &mut compressed, FlushCompress::Finish)?;

            fs::write(output, compressed)?;
        }

        Command::Inflate { input, output } => {
            let compressed = fs::read(input)?;

            let mut decompressor = Decompress::new(false);

            let mut decompressed = Vec::new();

            decompressor.decompress_vec(&compressed, &mut decompressed, FlushDecompress::Finish)?;

            fs::write(output, decompressed)?;
        }
    }

    Ok(())
}
