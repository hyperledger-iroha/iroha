//! Verify transferred checkpoint ancestry without granting native full-body custody.
//!
//! Each original row proof authenticates its executed parent header. The caller pins the
//! independent network and complete hash journal; the final successor authenticates P_H.
//! One frame is read at a time. Observers consume only sealed historical result owners.

use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId,
    block::{BlockHeader, SignedBlock},
    sumeragi_finality::{
        FinalityReadError, FinalityValidator, PortableCertifiedBlockV1, PortablePrefixFrameV1,
        PortablePrefixVerifierV1, SumeragiFinalityVerifier, VerifiedPortableCheckpointV1,
        authenticated_genesis,
    },
};
use iroha_model_base::chain::ChainId;

use crate::execution_attempt::{
    ExecutionAttemptError, canonical_decode_attempt_error, genesis_read_attempt_error,
    kura_read_attempt_error,
};

mod export;
pub use export::CheckpointPrefixExport;

/// A completed checkpoint-history rejection or original storage error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The source, identity, or cryptographic relation does not hold.
    #[error("checkpoint prefix: {0}")]
    Invalid(String),
    /// The original durable source could not be read.
    #[error("checkpoint prefix storage: {0}")]
    Storage(#[source] Box<crate::kura::Error>),
}

/// Original local refusal remains distinct from a completed history verdict.
pub type Attempt<T> = Result<T, ExecutionAttemptError<Error>>;

/// Classify a storage failure before formatting can erase its original resource owner.
pub(crate) fn storage_error(error: crate::kura::Error) -> ExecutionAttemptError<Error> {
    kura_read_attempt_error(error, |error| Error::Storage(Box::new(error)))
}

/// Classify the portable verifier's original decoder and allocation failure exactly once.
pub(crate) fn finality_error(error: FinalityReadError) -> ExecutionAttemptError<Error> {
    match error {
        FinalityReadError::Invalid(error) => invalid(error.to_string()),
        FinalityReadError::Genesis(error) => {
            genesis_read_attempt_error(error, |error| Error::Invalid(error.to_string()))
        }
        FinalityReadError::DecodeResource(original) => {
            canonical_decode_attempt_error(original, |error| Error::Invalid(error.to_string()))
        }
        FinalityReadError::ScratchLimit(original) => {
            ExecutionAttemptError::Deferred(original.into())
        }
        FinalityReadError::Allocator(original) => ExecutionAttemptError::Deferred(original.into()),
    }
}

fn invalid(message: impl Into<String>) -> ExecutionAttemptError<Error> {
    ExecutionAttemptError::Rejected(Error::Invalid(message.into()))
}

/// Start the same portable authority from the independently pinned signed genesis.
/// Its result remains a claim until the exact height-two certificate authenticates it.
pub(crate) fn verifier(
    genesis: &SignedBlock,
    chain_id: &ChainId,
    network: NetworkId,
) -> Attempt<PortablePrefixVerifierV1> {
    if NetworkId::from_genesis_hash(genesis.hash()) != network {
        return Err(invalid("signed genesis differs from the pinned network"));
    }
    let (epoch, _) = authenticated_genesis(genesis)
        .map_err(|error| {
            genesis_read_attempt_error(error, |error| Error::Invalid(error.to_string()))
        })?
        .into_parts();
    let validators = epoch
        .committee
        .into_iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession,
        })
        .collect();
    let selected = SumeragiFinalityVerifier::new(genesis, chain_id.as_ref(), validators)
        .map_err(finality_error)?;
    let certificate = genesis
        .commit_certificate()
        .ok_or_else(|| invalid("signed genesis has no original result preimage"))?;
    if !certificate.consensus_header().is_empty()
        || !certificate.commit_qc().is_empty()
        || !certificate.availability().is_empty()
    {
        return Err(invalid("genesis carries a non-genesis certificate"));
    }
    PortablePrefixVerifierV1::new(&selected, certificate.result_preimage()).map_err(finality_error)
}

/// Verify one original proof for every height 2..=H+1 against the exact H-hash journal.
///
/// `genesis` and `network` must come from the independently selected trust root. The reader
/// must bind its original files or remote transfer through the complete call; this function
/// performs no file replacement, journal extension, snapshot publication or history deletion.
/// A source failure leaves the caller's snapshot unauthenticated. Observation does not grant
/// full-block or transaction authority, and callers publish progress only after success.
///
/// # Errors
/// Missing, foreign, reordered or substituted history, an invalid certificate or row, an
/// observer refusal, or the original local resource failure. H1-only results are refused.
pub fn verify(
    genesis: &SignedBlock,
    chain_id: &ChainId,
    network: NetworkId,
    hashes: &[HashOf<BlockHeader>],
    read: &mut dyn FnMut(u64) -> Attempt<PortablePrefixFrameV1>,
    observe: &mut dyn FnMut(&PortableCertifiedBlockV1) -> Attempt<()>,
) -> Attempt<VerifiedPortableCheckpointV1> {
    if hashes.len() < 2 || hashes.first() != Some(&genesis.hash()) {
        return Err(invalid(
            "checkpoint journal lacks its exact genesis and positive successor",
        ));
    }
    let height = u64::try_from(hashes.len())
        .map_err(|_| invalid("checkpoint journal height exceeds u64"))?;
    let successor = height
        .checked_add(1)
        .ok_or_else(|| invalid("checkpoint successor height overflows"))?;
    let mut verifier = verifier(genesis, chain_id, network)?;
    let mut terminal = None;
    for next in 2..=successor {
        let frame = read(next)?;
        let verified = verifier.verify(&frame).map_err(finality_error)?;
        let parent = verified.checkpoint();
        let index = usize::try_from(next - 2)
            .map_err(|_| invalid("checkpoint parent index exceeds host capacity"))?;
        if parent.header().height().get() != next - 1
            || hashes.get(index) != Some(&parent.header().hash())
            || verified.network_id() != network
            || verified.chain_id() != chain_id.to_string()
        {
            return Err(invalid(
                "certified parent differs from the exact checkpoint journal",
            ));
        }
        observe(parent)?;
        terminal = Some(verified);
    }
    terminal.ok_or_else(|| invalid("checkpoint prefix ended before its certified successor"))
}

#[cfg(test)]
pub(crate) mod tests;
