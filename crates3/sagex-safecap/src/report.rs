use std::io::{Read, Seek};

use sagex_capsule::verify;

use crate::scan::{Budget, Prediction};

/// This is the choice of what the signer must prove.
///
/// What it does it lets the caller pick: check every file plus the central
/// map, or trust the files and check only the central map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyScope {
    PerFileAndCentral,
    CentralOnly,
}

/// This is the over-budget evidence handed to the caller.
///
/// What it does it holds the size claims and file offsets, never file
/// bytes. The caller shows it to the user, asks whether to go on, and then
/// calls [`Report::verify_signatures`] to check the signer.
#[derive(Debug, Clone)]
pub struct Report {
    pub prediction: Prediction,
    pub scope: VerifyScope,
}

/// This is the gate decision.
///
/// What it does it tells the caller what to do next: `Proceed` means the
/// file fits the budget and can be opened straight away, `OverBudget`
/// means stop and ask the user first. The lib itself never prompts.
#[derive(Debug, Clone)]
pub enum Gate {
    Proceed(Prediction),
    OverBudget(Report),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    ReaderFailed,
    CapsuleCheckFailed,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VerifyError {}

/// This is the budget gate.
///
/// What it does it compares the predicted memory against the budget: inside
/// the budget it hands back the prediction to proceed with, above it builds
/// a [`Report`] for the user to decide on.
pub fn judge(
    prediction: Prediction,
    memory_budget: &Budget,
    verification_scope: VerifyScope,
) -> Gate {
    if prediction.total_claimed_bytes <= memory_budget.maximum_allowed_bytes {
        Gate::Proceed(prediction)
    } else {
        Gate::OverBudget(Report {
            prediction,
            scope: verification_scope,
        })
    }
}

impl Report {
    /// This asks sagex-capsule to verify signatures for the report scope.
    ///
    /// What it does it checks the central signature first, then every file
    /// when the scope says so. It gives back true only when everything
    /// required proves out; false means missing or bad signature and the
    /// caller must deny. Only broken structure or unreadable bytes are
    /// errors.
    pub fn verify_signatures<Reader: Read + Seek>(
        &self,
        file_reader: &mut Reader,
        sender_public_key_bytes: &[u8],
    ) -> Result<bool, VerifyError> {
        let end_header = &self.prediction.end_header;
        let central_map_is_good = verify::verify_central(
            file_reader,
            end_header.central_directory_offset,
            end_header.central_directory_length,
            end_header.dek_table_offset,
            end_header.dek_table_length,
            end_header.archive_is_signed,
            sender_public_key_bytes,
        )
        .map_err(|_| VerifyError::CapsuleCheckFailed)?;
        if !central_map_is_good {
            return Ok(false);
        }
        if self.scope == VerifyScope::CentralOnly {
            return Ok(true);
        }
        for record_claim in &self.prediction.record_claims {
            let stored_file_is_good = verify::verify_file(
                file_reader,
                record_claim.record_offset,
                sender_public_key_bytes,
            )
            .map_err(|_| VerifyError::CapsuleCheckFailed)?;
            if !stored_file_is_good {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
