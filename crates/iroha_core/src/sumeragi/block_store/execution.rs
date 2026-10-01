//! Bind the exact executed block, including all outputs, to its certified result preimage.
use iroha_data_model::{
    block::SignedBlock,
    sumeragi_finality::{CommitmentError, ExecutionResultCommitment},
};
use std::io;

pub(super) fn validate(block: &SignedBlock) -> io::Result<()> {
    let invalid = |message: &'static str| io::Error::new(io::ErrorKind::InvalidData, message);
    let certificate = block
        .commit_certificate()
        .ok_or_else(|| invalid("executed block has no result certificate"))?;
    if !block.has_results() {
        return Err(invalid("certified block has no execution results"));
    }
    block
        .validate_proposal_commitments()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    block
        .validate_output_merkle_cache()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    // The owning decoder enforces the finite result-preimage ceiling and input-derived
    // cumulative decode budgets. No schedule decoded here selects availability authority.
    let result =
        ExecutionResultCommitment::decode(certificate.result_preimage()).map_err(|error| {
            // A local decoder refusal says nothing about the original certified bytes. Preserve
            // its typed cause and the caller's retained read instead of declaring disk corruption.
            let kind = if matches!(&error, CommitmentError::Resource(_))
                && !cfg!(all(test, sumeragi_core_mutation = "HC20"))
            {
                io::ErrorKind::WouldBlock
            } else {
                io::ErrorKind::InvalidData
            };
            io::Error::new(kind, error)
        })?;
    let (len, hash) = block
        .executed_block_wire_identity()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if result.height != block.header().height().get()
        || result.execution.executed_block_wire_len != len
        || result.execution.executed_block_wire_hash != hash
        || result.execution.transaction_input_commitment != block.network_input_merkle_commitment()
        || result.execution.transaction_output_commitment != block.output_merkle_commitment()
    {
        return Err(invalid(
            "executed block differs from its certified result commitment",
        ));
    }
    Ok(())
}
