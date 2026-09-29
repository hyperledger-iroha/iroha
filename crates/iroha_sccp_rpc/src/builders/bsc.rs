//! BSC evidence builders (spec §4.13.3, §7.2).
//!
//! Skipping advances (every set-transition checkpoint after the newest stored set, then the
//! latest finalized block), bootstraps and receipt proofs, built from standard BSC JSON-RPC:
//! `eth_getBlockByNumber` (BSC resolves the `finalized` tag by fast finality),
//! `eth_getTransactionReceipt` and `eth_getBlockReceipts`. Vote attestations are read from the
//! `extraData` of descendant headers. Everything built is untrusted until `iroha_sccp` verifies
//! it; the builders check header hashes and receipt roots locally so a lying endpoint is caught
//! before submission.

use std::ops::RangeInclusive;

use iroha_data_model::sccp::{
    inbound::SccpSourceProofBytesV1,
    light_client::{SccpLcAdvanceBytesV1, SccpLcBootstrapV1, SccpLcConsensusSetV1},
};
use iroha_sccp::{
    ethereum_source::{mpt_proof, mpt_root, rlp_encode_u64},
    light_client::{
        bsc::{
            BscAdvanceStepV1, BscFinalityV1, BscLcAdvanceV1, BscLcBootstrapV1, BscProofAnchorV1,
            BscSourceProofV1, BscValidatorV1, BscVoteV1, activation_offset, header_announced_set,
            header_attestation,
        },
        ethereum::{EthereumEventSelectorV1, EthereumLogRangeV1, EthereumLogRefV1},
        profile::{BSC_MAX_SEGMENT_HEADERS, BscChainProfileV1, SccpChainProfilesV1},
        proof::{SccpLcAdvanceV1, SccpLcBootstrapDataV1, SccpSourceProofV1},
    },
    v1::hashes::keccak256,
};

use super::ethereum::{BuildError, EthereumEventV1, header_rlp, receipt_rlp};
use crate::{
    EvmClient,
    evm::{BlockId, BlockTag, EvmBlock},
    http::MAX_JSON_RPC_BATCH,
};

/// Headers scanned after a block for a vote attestation finalizing it.
pub const MAX_ATTESTATION_SCAN: u64 = 32;

/// BSC evidence builder over one JSON-RPC endpoint.
pub struct BscBuilder {
    rpc: EvmClient,
    profile: BscChainProfileV1,
}

/// The stored set covering vote target `target` and its successor, from the light client's
/// stored sets (any order): the set with the highest first covered height at or below `target`.
///
/// # Errors
///
/// [`BuildError::Unavailable`] when no stored set covers `target`.
pub fn covering_set(
    sets: &[SccpLcConsensusSetV1],
    target: u64,
) -> Result<(u64, Option<u64>), BuildError> {
    let mut ordered: Vec<(u64, u64)> = sets
        .iter()
        .map(|set| (set.valid_from_source_height, set.set_id))
        .collect();
    ordered.sort_unstable();
    let position = ordered
        .iter()
        .rposition(|(valid_from, _)| *valid_from <= target)
        .ok_or_else(|| {
            BuildError::Unavailable(format!("no stored BSC set covers vote target {target}"))
        })?;
    Ok((
        ordered[position].1,
        ordered.get(position + 1).map(|(_, set_id)| *set_id),
    ))
}

impl BscBuilder {
    /// A builder over `rpc` under the compiled BSC profile.
    #[must_use]
    pub fn new(rpc: EvmClient) -> Self {
        Self {
            rpc,
            profile: SccpChainProfilesV1::compiled().bsc,
        }
    }

    fn checked_rlp(block: &EvmBlock) -> Result<Vec<u8>, BuildError> {
        let rlp = header_rlp(&block.header);
        if keccak256(&[&rlp]) != block.header.hash {
            return Err(BuildError::Inconsistent(format!(
                "the RLP of BSC block {} does not hash to its hash",
                block.header.number
            )));
        }
        Ok(rlp)
    }

    /// Header RLPs of `range`, checked against their hashes and parent links.
    fn headers(&self, range: RangeInclusive<u64>) -> Result<Vec<Vec<u8>>, BuildError> {
        let numbers: Vec<u64> = range.collect();
        self.headers_at(&numbers)
    }

    fn headers_at(&self, numbers: &[u64]) -> Result<Vec<Vec<u8>>, BuildError> {
        let mut out = Vec::with_capacity(numbers.len());
        for chunk in numbers.chunks(MAX_JSON_RPC_BATCH) {
            for (number, block) in chunk.iter().zip(self.rpc.blocks_by_number(chunk)?) {
                let block = block.ok_or_else(|| {
                    BuildError::Unavailable(format!("BSC block {number} is not served"))
                })?;
                if block.header.number != *number {
                    return Err(BuildError::Inconsistent(format!(
                        "asked for BSC block {number}, got {}",
                        block.header.number
                    )));
                }
                out.push(Self::checked_rlp(&block)?);
            }
        }
        Ok(out)
    }

    fn header(&self, number: u64) -> Result<Vec<u8>, BuildError> {
        self.headers_at(&[number])?
            .pop()
            .ok_or_else(|| BuildError::Unavailable(format!("BSC block {number} is not served")))
    }

    /// Height of the latest block BSC fast finality has finalized.
    ///
    /// # Errors
    ///
    /// Any endpoint failure.
    pub fn finalized_number(&self) -> Result<u64, BuildError> {
        Ok(self
            .rpc
            .block_by_number(BlockTag::Finalized)?
            .ok_or_else(|| BuildError::Unavailable("no finalized BSC block is served".into()))?
            .header
            .number)
    }

    /// The first attestation, carried by a header after `from`, that finalizes a block in
    /// `from..=until`.
    fn finalizing_vote(&self, from: u64, until: u64) -> Result<(Vec<u8>, BscVoteV1), BuildError> {
        let latest = self.rpc.block_number()?;
        let end = from.saturating_add(MAX_ATTESTATION_SCAN).min(latest);
        if end <= from {
            return Err(BuildError::Unavailable(format!(
                "BSC block {from} has no descendants yet"
            )));
        }
        for header in self.headers(from + 1..=end)? {
            let vote = header_attestation(&self.profile, &header)
                .map_err(|error| BuildError::Inconsistent(format!("BSC header: {error}")))?;
            if let Some((attestation, vote)) = vote
                && vote.finalizes_source()
                && (from..=until).contains(&vote.source_number)
            {
                return Ok((attestation, vote));
            }
        }
        Err(BuildError::Unavailable(format!(
            "no vote attestation finalizes a BSC block in {from}..={until} yet"
        )))
    }

    fn announced(header: &[u8]) -> Result<(Vec<BscValidatorV1>, u8), BuildError> {
        header_announced_set(header)
            .map_err(|error| BuildError::Inconsistent(format!("BSC epoch checkpoint: {error}")))
    }

    fn activation(
        checkpoint: u64,
        previous: &(Vec<BscValidatorV1>, u8),
    ) -> Result<u64, BuildError> {
        activation_offset(previous.0.len(), previous.1)
            .ok()
            .and_then(|offset| checkpoint.checked_add(offset)?.checked_add(1))
            .ok_or_else(|| BuildError::Inconsistent("BSC activation height overflows".into()))
    }

    /// Build the `InitializeLightClient` bootstrap of the latest finalized epoch checkpoint.
    ///
    /// # Errors
    ///
    /// Any endpoint failure or inconsistent response.
    pub fn bootstrap(&self) -> Result<SccpLcBootstrapV1, BuildError> {
        let epoch = self.profile.epoch_length;
        let checkpoint = self.finalized_number()? / epoch * epoch;
        if checkpoint < epoch {
            return Err(BuildError::Unavailable(
                "no finalized BSC epoch checkpoint".into(),
            ));
        }
        let headers = self.headers_at(&[checkpoint - epoch, checkpoint])?;
        SccpLcBootstrapDataV1::Bsc(BscLcBootstrapV1 {
            previous_checkpoint_header: headers[0].clone(),
            checkpoint_header: headers[1].clone(),
        })
        .to_bootstrap()
        .map_err(|error| BuildError::Inconsistent(format!("bootstrap frame: {error}")))
    }

    /// Build an advance from the newest stored set `latest_set_id` (its checkpoint height): a
    /// transition step for every later finalized checkpoint that announces another set, then a
    /// step for the latest finalized block, at most `max_steps` in all.
    ///
    /// # Errors
    ///
    /// Any endpoint failure, an inconsistent response, or nothing to advance.
    pub fn advance(
        &self,
        latest_set_id: u64,
        max_steps: usize,
    ) -> Result<SccpLcAdvanceBytesV1, BuildError> {
        let epoch = self.profile.epoch_length;
        let finalized = self.finalized_number()?;
        let stored = self.headers_at(&[latest_set_id.saturating_sub(epoch), latest_set_id])?;
        let mut newest = latest_set_id;
        let mut current = Self::announced(&stored[1])?;
        let mut valid_from = Self::activation(latest_set_id, &Self::announced(&stored[0])?)?;
        let checkpoints: Vec<u64> = (1..)
            .map_while(|index: u64| latest_set_id.checked_add(index.checked_mul(epoch)?))
            .take_while(|height| *height <= finalized)
            .collect();
        let mut steps = Vec::new();
        for (height, header) in checkpoints.iter().zip(self.headers_at(&checkpoints)?) {
            if steps.len() == max_steps {
                break;
            }
            let announced = Self::announced(&header)?;
            if announced == current {
                continue;
            }
            let next_valid_from = Self::activation(*height, &current)?;
            let until = height
                .saturating_add(BSC_MAX_SEGMENT_HEADERS as u64 - 1)
                .min(next_valid_from.saturating_sub(2))
                .min(finalized);
            let (attestation, vote) = self.finalizing_vote(*height, until)?;
            steps.push(BscAdvanceStepV1 {
                headers: self.headers(*height..=vote.source_number)?,
                finality: BscFinalityV1 {
                    set_id: newest,
                    successor_set_id: None,
                    attestation,
                },
            });
            newest = *height;
            current = announced;
            valid_from = next_valid_from;
        }
        if steps.len() < max_steps && finalized.saturating_add(1) >= valid_from {
            let (attestation, _) = self.finalizing_vote(finalized, finalized)?;
            steps.push(BscAdvanceStepV1 {
                headers: vec![self.header(finalized)?],
                finality: BscFinalityV1 {
                    set_id: newest,
                    successor_set_id: None,
                    attestation,
                },
            });
        }
        if steps.is_empty() {
            return Err(BuildError::Unavailable(
                "the newest BSC set does not cover the finalized block yet".into(),
            ));
        }
        SccpLcAdvanceV1::Bsc(BscLcAdvanceV1 { steps })
            .to_bytes()
            .map_err(|error| BuildError::Inconsistent(format!("advance frame: {error}")))
    }

    /// Build the inbound or void proof of `event` in transaction `tx_hash`: the event block and
    /// its descendants up to the first finalized block, under the stored set (from `sets`, the
    /// light client's stored sets) covering that block's child.
    ///
    /// # Errors
    ///
    /// Any endpoint failure, an inconsistent response, a transaction that is not finalized yet,
    /// or no stored set covering it.
    pub fn source_proof(
        &self,
        tx_hash: &[u8; 32],
        event: EthereumEventV1,
        sets: &[SccpLcConsensusSetV1],
    ) -> Result<SccpSourceProofBytesV1, BuildError> {
        let receipt = self
            .rpc
            .transaction_receipt(tx_hash)?
            .ok_or_else(|| BuildError::Unavailable("the transaction is not mined".into()))?;
        let event_number = receipt.block_number;
        let receipts = self
            .rpc
            .block_receipts(BlockId::Hash(receipt.block_hash))?
            .ok_or_else(|| BuildError::Unavailable("the block receipts are not served".into()))?;
        let entries: Vec<(Vec<u8>, Vec<u8>)> = receipts
            .iter()
            .map(|entry| (rlp_encode_u64(entry.transaction_index), receipt_rlp(entry)))
            .collect();
        let receipt_proof = mpt_proof(&entries, &rlp_encode_u64(receipt.transaction_index))
            .ok_or_else(|| BuildError::Inconsistent("the receipt is not in its block".into()))?;
        let until = event_number.saturating_add(BSC_MAX_SEGMENT_HEADERS as u64 - 1);
        let (attestation, vote) = self.finalizing_vote(event_number, until)?;
        let headers = self.headers(event_number..=vote.source_number)?;
        let event_block = self
            .rpc
            .block_by_number(BlockTag::Number(event_number))?
            .ok_or_else(|| {
                BuildError::Unavailable(format!("BSC block {event_number} is not served"))
            })?;
        if keccak256(&[&headers[0]]) != receipt.block_hash
            || mpt_root(&entries) != Some(event_block.header.receipts_root)
        {
            return Err(BuildError::Inconsistent(
                "the served receipts do not rebuild the event block".into(),
            ));
        }
        let (set_id, successor_set_id) = covering_set(sets, vote.target_number)?;
        let transaction_index = u32::try_from(receipt.transaction_index)
            .map_err(|_| BuildError::Inconsistent("transaction index overflows".into()))?;
        let event = match event {
            EthereumEventV1::TransferToTaira { log_index } => {
                EthereumEventSelectorV1::TransferToTaira(EthereumLogRefV1 { log_index })
            }
            EthereumEventV1::Void {
                first_log_index,
                log_count,
            } => EthereumEventSelectorV1::Void(EthereumLogRangeV1 {
                first_log_index,
                log_count,
            }),
        };
        SccpSourceProofV1::Bsc(BscSourceProofV1 {
            anchor: BscProofAnchorV1::Finality(BscFinalityV1 {
                set_id,
                successor_set_id,
                attestation,
            }),
            headers,
            transaction_index,
            receipt_proof,
            event,
        })
        .to_bytes()
        .map_err(|error| BuildError::Inconsistent(format!("proof frame: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(set_id: u64, valid_from_source_height: u64) -> SccpLcConsensusSetV1 {
        SccpLcConsensusSetV1 {
            set_id,
            valid_from_source_height,
            superseded_at_source_ms: None,
            set_bytes: Vec::new(),
        }
    }

    #[test]
    fn covering_sets_name_their_successor() {
        let sets = vec![set(7_000, 7_176), set(5_000, 5_176), set(9_000, 9_200)];
        assert_eq!(
            covering_set(&sets, 7_100).expect("covered"),
            (5_000, Some(7_000))
        );
        assert_eq!(
            covering_set(&sets, 7_176).expect("covered"),
            (7_000, Some(9_000))
        );
        assert_eq!(covering_set(&sets, 9_500).expect("covered"), (9_000, None));
        assert!(matches!(
            covering_set(&sets, 5_000),
            Err(BuildError::Unavailable(_))
        ));
        assert!(covering_set(&[], 1).is_err());
    }
}
