//! Bounded restartable replay joins an original carrier to an independently retained parent tip.

use super::*;
use crate::verify::finality::FinalitySource;
use std::num::NonZeroU64;

// A scheduler turn remains short even when an untrusted Applied hint names a far future height.
// The native catch-up verifier independently enforces its canonical byte and proof budgets.
const MAX_REPLAY_SUCCESSORS: u64 = 16;

/// Custody-selected checkpoints only; none is imported from an HTTP response as authority.
#[derive(Clone, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::attachment::ParentReplayV1")]
pub(super) struct ParentReplay {
    carrier_height: u64,
    comparison: Vec<u8>,
    progress: Vec<u8>,
    comparison_verified: bool,
    carrier: Option<ConfirmedReceipt>,
}

impl ParentReplay {
    pub(super) fn new(
        identity: &AttachmentIdentity,
        original: &FinalityVerifier,
        carrier: NonZeroU64,
        comparison: &FinalityVerifier,
    ) -> Result<Self> {
        let result = Self {
            carrier_height: carrier.get(),
            comparison: encode_checkpoint(comparison)?,
            progress: encode_checkpoint(original)?,
            comparison_verified: original.checkpoint().height() == comparison.checkpoint().height(),
            carrier: None,
        };
        result.validate(identity, original)?;
        Ok(result)
    }

    pub(super) fn validate(
        &self,
        identity: &AttachmentIdentity,
        original: &FinalityVerifier,
    ) -> Result<()> {
        let progress = decode_checkpoint(identity, &self.progress)?;
        let comparison = decode_checkpoint(identity, &self.comparison)?;
        let original_height = original.checkpoint().height();
        let height = progress.checkpoint().height();
        let comparison_height = comparison.checkpoint().height();
        if self.carrier_height <= original_height
            || comparison_height < original_height
            || height < original_height
            || height > self.carrier_height.max(comparison_height)
            || (self.comparison_verified != (height >= comparison_height))
            || (height > self.carrier_height && self.carrier.is_none())
        {
            return Err(AttachmentError::Invalid(
                "inconsistent retained parent replay milestones",
            ));
        }
        if height == original_height {
            same_decision(original, &progress)?;
        }
        if comparison_height == original_height {
            same_decision(original, &comparison)?;
        }
        if height == comparison_height {
            same_decision(&comparison, &progress)?;
        }
        if let Some(carrier) = &self.carrier {
            validate_retained_receipt(identity, carrier)?;
            if carrier.proof.parent_height != self.carrier_height || height < self.carrier_height {
                return Err(AttachmentError::Invalid(
                    "retained carrier differs from replay height",
                ));
            }
            if height == self.carrier_height {
                same_decision(
                    &decode_checkpoint(identity, &carrier.checkpoint)?,
                    &progress,
                )?;
            }
        }
        Ok(())
    }

    /// Perform at most one native bounded catch-up page, stopping at either trust milestone.
    pub(super) fn advance_page<S: FinalitySource + ?Sized>(
        &mut self,
        identity: &AttachmentIdentity,
        source: &S,
    ) -> Result<()> {
        let mut progress = decode_checkpoint(identity, &self.progress)?;
        let comparison = decode_checkpoint(identity, &self.comparison)?;
        let height = progress.checkpoint().height();
        // The exact carrier proof must be retained before progressing past its verifier.
        if height == self.carrier_height && self.carrier.is_none() {
            return Ok(());
        }
        let target = self.next_milestone(height, comparison.checkpoint().height());
        if let Some(target) = target {
            progress.catch_up(source, target)?;
            if progress.checkpoint().height() <= height {
                return Err(AttachmentError::Invalid(
                    "parent proof source made no progress",
                ));
            }
        }
        let comparison_verified =
            if progress.checkpoint().height() == comparison.checkpoint().height() {
                same_decision(&comparison, &progress)?;
                true
            } else {
                self.comparison_verified
            };
        // Only mutate after native successor verification and any reached comparison succeed.
        self.progress = encode_checkpoint(&progress)?;
        self.comparison_verified = comparison_verified;
        Ok(())
    }

    fn next_milestone(&self, height: u64, comparison: u64) -> Option<NonZeroU64> {
        [
            (self.carrier.is_none()).then_some(self.carrier_height),
            (!self.comparison_verified).then_some(comparison),
        ]
        .into_iter()
        .flatten()
        .filter(|target| *target > height)
        .min()
        .map(|milestone| milestone.min(height.saturating_add(MAX_REPLAY_SUCCESSORS)))
        .and_then(NonZeroU64::new)
    }

    pub(super) fn needs_carrier_proof(&self) -> Option<NonZeroU64> {
        if self.carrier.is_some() {
            return None;
        }
        let progress = SumeragiFinalityCheckpoint::decode_canonical(&self.progress).ok()?;
        (progress.height() == self.carrier_height)
            .then(|| NonZeroU64::new(self.carrier_height))
            .flatten()
    }

    pub(super) fn retain_carrier(
        &mut self,
        identity: &AttachmentIdentity,
        proof: PrivateDataspaceRecordProof,
    ) -> Result<()> {
        if self.needs_carrier_proof().map(NonZeroU64::get) != Some(proof.parent_height) {
            return Err(AttachmentError::Invalid(
                "record proof is not the retained replay carrier",
            ));
        }
        let receipt = ConfirmedReceipt {
            checkpoint: self.progress.clone(),
            proof,
        };
        validate_retained_receipt(identity, &receipt)?;
        self.carrier = Some(receipt);
        Ok(())
    }

    pub(super) fn completed(
        &self,
        identity: &AttachmentIdentity,
    ) -> Result<Option<(PrivateDataspaceRecordProof, FinalityVerifier)>> {
        if !self.comparison_verified {
            return Ok(None);
        }
        self.carrier
            .as_ref()
            .map(|receipt| {
                Ok((
                    receipt.proof.clone(),
                    decode_checkpoint(identity, &receipt.checkpoint)?,
                ))
            })
            .transpose()
    }
}

fn encode_checkpoint(verifier: &FinalityVerifier) -> Result<Vec<u8>> {
    verifier
        .checkpoint()
        .encode_canonical()
        .map_err(|_| AttachmentError::Invalid("cannot retain verified parent replay checkpoint"))
}

fn decode_checkpoint(identity: &AttachmentIdentity, bytes: &[u8]) -> Result<FinalityVerifier> {
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(bytes)
        .map_err(|_| AttachmentError::Invalid("invalid retained replay checkpoint"))?;
    Ok(FinalityVerifier::from_checkpoint(
        checkpoint,
        identity.parent_network_id,
        &identity.parent_chain_id,
    )?)
}

/// Native same-decision verification tolerates equivalent certificate witnesses, not forks.
fn same_decision(expected: &FinalityVerifier, candidate: &FinalityVerifier) -> Result<()> {
    let selected = expected.checkpoint();
    let verifier =
        iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier::from_trusted_checkpoint(
            selected,
            &selected.network_id(),
            selected.chain_id(),
        )
        .map_err(FinalityError::from)?;
    let _verified = verifier
        .verify_same_decision(selected.tip(), candidate.checkpoint().tip())
        .map_err(FinalityError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests;
