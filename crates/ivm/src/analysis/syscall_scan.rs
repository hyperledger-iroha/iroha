//! Borrowed syscall classification without a materialized histogram.

use super::{ProgramAnalysisError, aggregate::syscall_number};
use crate::{ProgramMetadata, ivm_cache::ValidatedInstructions, prepared::PreparedContract};

pub(super) fn parsed_stream(
    bytes: &[u8],
) -> Result<(ProgramMetadata, ValidatedInstructions<'_>), ProgramAnalysisError> {
    let parsed = ProgramMetadata::parse(bytes).map_err(ProgramAnalysisError::Metadata)?;
    let stream = ValidatedInstructions::new(&bytes[parsed.code_offset..])
        .map_err(ProgramAnalysisError::Decode)?;
    Ok((parsed.metadata, stream))
}

/// Scan syscall numbers in executable order without allocating a decoded array or histogram.
/// Repeated calls appear repeatedly; this scans every instruction, not only reachable code.
/// Metadata decoding retains its existing canonical checks and resource ownership.
///
/// # Errors
/// Returns the same original metadata or instruction error as aggregate program analysis.
pub fn program_syscall_numbers(
    bytes: &[u8],
) -> Result<impl Iterator<Item = u32> + '_, ProgramAnalysisError> {
    let (_, stream) = parsed_stream(bytes)?;
    Ok(stream.iter().filter_map(syscall_number))
}

/// Scan the complete prepared instruction stream without allocating or cloning metadata.
/// Results retain executable order and repeated occurrences.
pub fn prepared_syscall_numbers(contract: &PreparedContract) -> impl Iterator<Item = u32> + '_ {
    contract
        .decoded()
        .iter()
        .copied()
        .filter_map(syscall_number)
}

#[cfg(test)]
mod tests;
