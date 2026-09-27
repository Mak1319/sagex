use crate::{ecc::error::EccError, format::PrivateFileFormatInternal};
use binrw::BinWrite;
use reed_solomon::Encoder;

pub const ECC_LEN: usize = 32;
pub const ECC_CHUNK_SIZE: usize = 256 - ECC_LEN;

pub type EccChunk = [u8; ECC_LEN];

pub mod error {
    use std::fmt;

    #[derive(Debug)]
    pub enum EccError {
        Serialize,
        ChunkLengthMismatch {
            chunk_index: usize,
            expected: usize,
            actual: usize,
        },
    }

    impl fmt::Display for EccError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                EccError::Serialize => write!(f, "failed to serialize internal"),
                EccError::ChunkLengthMismatch {
                    chunk_index,
                    expected,
                    actual,
                } => {
                    write!(
                        f,
                        "chunk {chunk_index}: expected {expected} ecc bytes, got {actual}"
                    )
                }
            }
        }
    }

    impl std::error::Error for EccError {}

    impl From<EccError> for binrw::Error {
        fn from(e: EccError) -> Self {
            binrw::Error::Custom {
                pos: 0,
                err: Box::new(e),
            }
        }
    }
}

pub fn calculate_ecc(ke: &PrivateFileFormatInternal) -> Result<Vec<EccChunk>, EccError> {
    let mut buf = Vec::new();
    ke.write(&mut std::io::Cursor::new(&mut buf))
        .map_err(|_e| EccError::Serialize)?;

    let encoder = Encoder::new(ECC_LEN);

    buf.chunks(ECC_CHUNK_SIZE)
        .enumerate()
        .map(|(i, chunk)| {
            let encoded_bytes = encoder.encode(chunk);
            let ecc_slice = encoded_bytes.ecc();

            ecc_slice
                .try_into()
                .map_err(|_| EccError::ChunkLengthMismatch {
                    chunk_index: i,
                    expected: ECC_LEN,
                    actual: ecc_slice.len(),
                })
        })
        .collect::<Result<Vec<EccChunk>, EccError>>()
}
