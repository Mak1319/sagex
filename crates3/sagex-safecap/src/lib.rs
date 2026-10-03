pub mod report;
pub mod scan;

pub use report::{Gate, Report, VerifyError, VerifyScope, judge};
pub use scan::{Budget, EocdInfo, Prediction, RecordClaim, ScanError, scan};

/// This is the memory budget used when the caller sets none.
///
/// What it does it draws the line: predictions above this many bytes need
/// signer proof before the file is opened.
pub const DEFAULT_BUDGET: u64 = 256 << 20;
