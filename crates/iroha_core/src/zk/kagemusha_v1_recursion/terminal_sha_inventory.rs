//! Non-authorizing Terminal SHA inventory and ordered-claim bridge prerequisites.
//!
//! This module is test-only until the production consumer verifies the actual hybrid claim,
//! carrier binding and complete history. An inventory describes the supplied queue; it does not
//! establish that the caller supplied the entire Terminal queue or any valid proof.

use crate::zk::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::PastaSha256JobsV1,
    pasta_sha256_table8::{BLOCK_BYTE_SIZE, canonical_padding_suffix},
};
use halo2_base::utils::fe_to_biguint;

/// Native metadata copied from one exact ordinary job, without serializing its private bytes.
#[derive(Clone, PartialEq, Eq)]
struct TerminalShaJobInventoryV1 {
    message_bytes: usize,
    compression_blocks: u32,
    output_words: [u32; 8],
}

/// Ephemeral preparation for the existing ordered SHA plan; never an authorization token.
pub(super) struct TerminalShaInventoryV1<'a, F: KagemushaPoseidonFieldV1> {
    queue: &'a PastaSha256JobsV1<F>,
    messages: Vec<Vec<u8>>,
    jobs: Vec<TerminalShaJobInventoryV1>,
    compression_blocks: u64,
}

impl<'a, F: KagemushaPoseidonFieldV1> TerminalShaInventoryV1<'a, F> {
    /// Exact per-message block counts retained from the original queue.
    pub(super) fn job_block_counts(&self) -> Vec<u32> {
        self.jobs.iter().map(|job| job.compression_blocks).collect()
    }

    /// Consume the temporary inventory without exposing or serializing private messages.
    pub(super) fn into_messages(self) -> Vec<Vec<u8>> {
        self.messages
    }

    /// Borrow the original assigned queue and retain its exact ordered planning messages.
    ///
    /// The paired semantic planner supplies the shared production queue, including its final
    /// candidate projection. Other callers describe only their supplied queue.
    /// TODO: Verify the paired hybrid claim and its carrier/current/history bindings before
    /// using this inventory to replace Table8 synthesis.
    pub(super) fn from_queue(queue: &'a PastaSha256JobsV1<F>) -> Result<Self, String> {
        // canonical_messages also exposes bounded-job capacity bytes. Reject that different
        // relation before extracting anything into an ordinary claim plan.
        let claims = queue.claim_jobs()?;
        if claims.is_empty() {
            return Err("terminal SHA inventory cannot contain an empty queue".to_owned());
        }
        let messages = queue.canonical_messages()?;
        if messages.len() != claims.len() {
            return Err("terminal SHA inventory queue shape changed".to_owned());
        }
        let mut jobs = Vec::with_capacity(claims.len());
        let mut compression_blocks = 0_u64;
        for (claim, message) in claims.iter().zip(&messages) {
            let suffix = canonical_padding_suffix(message.len())
                .ok_or_else(|| "terminal SHA inventory message is not encodable".to_owned())?;
            let padded = message
                .len()
                .checked_add(suffix.len())
                .ok_or_else(|| "terminal SHA inventory padding overflowed".to_owned())?;
            let blocks = u32::try_from(padded / BLOCK_BYTE_SIZE)
                .map_err(|_| "terminal SHA inventory job exceeds u32 blocks".to_owned())?;
            let mut output_words = [0_u32; 8];
            for (target, source) in output_words.iter_mut().zip(claim.output_words) {
                if source.cell.is_none() {
                    return Err("terminal SHA inventory output is not an assigned cell".to_owned());
                }
                *target = u32::try_from(fe_to_biguint(source.value()))
                    .map_err(|_| "terminal SHA inventory output word exceeds u32".to_owned())?;
            }
            compression_blocks = compression_blocks
                .checked_add(u64::from(blocks))
                .ok_or_else(|| "terminal SHA inventory block count overflowed".to_owned())?;
            jobs.push(TerminalShaJobInventoryV1 {
                message_bytes: message.len(),
                compression_blocks: blocks,
                output_words,
            });
        }
        if u64::try_from(queue.compression_blocks()?).ok() != Some(compression_blocks) {
            return Err("terminal SHA inventory differs from the original queue blocks".to_owned());
        }
        Ok(Self {
            queue,
            messages,
            jobs,
            compression_blocks,
        })
    }
}

#[path = "terminal_sha_inventory_tests.rs"]
mod tests;
