//! Ethereum evidence builders (spec §4.13.3, §7.2).
//!
//! Sync-committee advances, bootstraps, execution ancestry (`SameBlock`, `HeaderChain`) and
//! receipt proofs, built from the standard beacon light-client API (JSON) and execution
//! JSON-RPC. Everything built is untrusted until `iroha_sccp` verifies it; the builders check
//! header hashes and receipt roots locally so a lying endpoint is caught before submission.
//!
//! TODO(ws37): `HistoryContract` ancestry (EIP-2935 `eth_getProof` at the anchor) and
//! `Backfill` segments from a permanent checkpoint for events older than 256 blocks before the
//! finalized anchor.

use iroha_data_model::sccp::{
    inbound::SccpSourceProofBytesV1,
    light_client::{SccpLcAdvanceBytesV1, SccpLcBootstrapV1},
};
use iroha_sccp::{
    BeaconBlockHeader, BlsPublicKey, BlsSignature, CapellaExecutionPayloadHeader,
    CurrentSyncCommitteeBranch, DenebExecutionPayloadHeader, EthereumFork, ExtraData,
    FinalityBranch, ForkSchedule, LightClientBootstrap, LightClientHeader, LightClientUpdate,
    NextSyncCommitteeBranch, NextSyncCommitteeProof, Root, SyncAggregate, SyncCommittee,
    ethereum_source::{
        EthereumLogV1, EthereumNativeLightClientBootstrapV1, EthereumNativeLightClientUpdateV1,
        EthereumReceiptV1, encode_receipt, mpt_proof, mpt_root, rlp_encode_bytes, rlp_encode_list,
        rlp_encode_u64, rlp_encode_uint_bytes,
    },
    light_client::{
        ethereum::{
            EthereumAncestryV1, EthereumEventSelectorV1, EthereumHeaderSegmentV1,
            EthereumLcAdvanceV1, EthereumLogRangeV1, EthereumLogRefV1, EthereumProofAnchorV1,
            EthereumSourceProofV1,
        },
        profile::SccpChainProfilesV1,
        proof::{SccpLcAdvanceV1, SccpLcBootstrapDataV1, SccpSourceProofV1},
    },
    v1::hashes::keccak256,
};
use norito::json::Value;

use crate::{
    BeaconClient, EvmClient, RpcError,
    evm::{BlockTag, EvmBlock, EvmHeader, EvmReceipt, format_data},
};

/// Most headers a `HeaderChain` ancestry carries (§4.13.3).
pub const MAX_HEADER_CHAIN: u64 = 256;
/// Sync-committee period length in slots.
const SLOTS_PER_PERIOD: u64 = 8_192;

/// Why a build failed.
#[derive(Debug)]
pub enum BuildError {
    /// An endpoint failed.
    Rpc(RpcError),
    /// A response is not the expected JSON.
    Json(String),
    /// A response contradicts itself (hash, root or linkage mismatch).
    Inconsistent(String),
    /// The requested evidence cannot be built from the available data.
    Unavailable(String),
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Rpc(error) => write!(formatter, "RPC: {error}"),
            Self::Json(detail) => write!(formatter, "malformed JSON: {detail}"),
            Self::Inconsistent(detail) => write!(formatter, "inconsistent response: {detail}"),
            Self::Unavailable(detail) => write!(formatter, "unavailable: {detail}"),
        }
    }
}

impl std::error::Error for BuildError {}

impl From<RpcError> for BuildError {
    fn from(error: RpcError) -> Self {
        Self::Rpc(error)
    }
}

fn json(detail: impl Into<String>) -> BuildError {
    BuildError::Json(detail.into())
}

// ---------------------------------------------------------------------------------------------
// Beacon JSON
// ---------------------------------------------------------------------------------------------

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, BuildError> {
    value
        .get(key)
        .ok_or_else(|| json(format!("missing field `{key}`")))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, BuildError> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| json(format!("field `{key}` is not a string")))
}

fn decimal(value: &Value, key: &str) -> Result<u64, BuildError> {
    text(value, key)?
        .parse()
        .map_err(|_| json(format!("field `{key}` is not a decimal integer")))
}

fn hex_bytes(text: &str) -> Result<Vec<u8>, BuildError> {
    hex::decode(text.strip_prefix("0x").unwrap_or(text))
        .map_err(|_| json(format!("`{text}` is not hex")))
}

fn hex_fixed<const N: usize>(value: &Value, key: &str) -> Result<[u8; N], BuildError> {
    hex_bytes(text(value, key)?)?
        .try_into()
        .map_err(|_| json(format!("field `{key}` is not {N} bytes")))
}

fn hex_list(value: &Value, key: &str) -> Result<Vec<Vec<u8>>, BuildError> {
    field(value, key)?
        .as_array()
        .ok_or_else(|| json(format!("field `{key}` is not an array")))?
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or_else(|| json(format!("`{key}` holds a non-string")))
                .and_then(hex_bytes)
        })
        .collect()
}

fn roots<const N: usize>(value: &Value, key: &str) -> Result<[Root; N], BuildError> {
    hex_list(value, key)?
        .into_iter()
        .map(|root| {
            Root::try_from(root.as_slice()).map_err(|_| json("a branch root is not 32 bytes"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| json(format!("branch `{key}` has the wrong length")))
}

fn le_u256_from_decimal(text: &str) -> Result<[u8; 32], BuildError> {
    let value: u128 = text
        .parse()
        .map_err(|_| json("base fee is not a decimal below 2^128"))?;
    let mut out = [0_u8; 32];
    out[..16].copy_from_slice(&value.to_le_bytes());
    Ok(out)
}

fn fork_from_version(response: &Value) -> Result<EthereumFork, BuildError> {
    match text(response, "version")? {
        "capella" => Ok(EthereumFork::Capella),
        "deneb" => Ok(EthereumFork::Deneb),
        "electra" => Ok(EthereumFork::Electra),
        "fulu" => Ok(EthereumFork::Fulu),
        other => Err(json(format!("unsupported consensus version `{other}`"))),
    }
}

fn beacon_header(value: &Value) -> Result<BeaconBlockHeader, BuildError> {
    Ok(BeaconBlockHeader {
        slot: decimal(value, "slot")?,
        proposer_index: decimal(value, "proposer_index")?,
        parent_root: hex_fixed(value, "parent_root")?,
        state_root: hex_fixed(value, "state_root")?,
        body_root: hex_fixed(value, "body_root")?,
    })
}

fn light_client_header(value: &Value, fork: EthereumFork) -> Result<LightClientHeader, BuildError> {
    let beacon = beacon_header(field(value, "beacon")?)?;
    let execution = field(value, "execution")?;
    let capella = CapellaExecutionPayloadHeader {
        parent_hash: hex_fixed(execution, "parent_hash")?,
        fee_recipient: hex_fixed(execution, "fee_recipient")?,
        state_root: hex_fixed(execution, "state_root")?,
        receipts_root: hex_fixed(execution, "receipts_root")?,
        logs_bloom: hex_fixed(execution, "logs_bloom")?,
        prev_randao: hex_fixed(execution, "prev_randao")?,
        block_number: decimal(execution, "block_number")?,
        gas_limit: decimal(execution, "gas_limit")?,
        gas_used: decimal(execution, "gas_used")?,
        timestamp: decimal(execution, "timestamp")?,
        extra_data: ExtraData::new(hex_bytes(text(execution, "extra_data")?)?)
            .map_err(|_| json("extra data exceeds 32 bytes"))?,
        base_fee_per_gas: le_u256_from_decimal(text(execution, "base_fee_per_gas")?)?,
        block_hash: hex_fixed(execution, "block_hash")?,
        transactions_root: hex_fixed(execution, "transactions_root")?,
        withdrawals_root: hex_fixed(execution, "withdrawals_root")?,
    };
    let execution_branch: [Root; 4] = roots(value, "execution_branch")?;
    let deneb = || -> Result<Box<DenebExecutionPayloadHeader>, BuildError> {
        Ok(Box::new(DenebExecutionPayloadHeader {
            capella: capella.clone(),
            blob_gas_used: decimal(execution, "blob_gas_used")?,
            excess_blob_gas: decimal(execution, "excess_blob_gas")?,
        }))
    };
    Ok(match fork {
        EthereumFork::Capella => LightClientHeader::Capella {
            beacon,
            execution: Box::new(capella.clone()),
            execution_branch,
        },
        EthereumFork::Deneb => LightClientHeader::Deneb {
            beacon,
            execution: deneb()?,
            execution_branch,
        },
        EthereumFork::Electra => LightClientHeader::Electra {
            beacon,
            execution: deneb()?,
            execution_branch,
        },
        EthereumFork::Fulu => LightClientHeader::Fulu {
            beacon,
            execution: deneb()?,
            execution_branch,
        },
        EthereumFork::Altair | EthereumFork::Bellatrix => {
            return Err(json("pre-Capella light-client headers are not supported"));
        }
    })
}

fn committee(value: &Value) -> Result<SyncCommittee, BuildError> {
    let keys = hex_list(value, "pubkeys")?
        .into_iter()
        .map(|key| {
            <[u8; 48]>::try_from(key)
                .map(BlsPublicKey::new)
                .map_err(|_| json("a committee key is not 48 bytes"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SyncCommittee::new(
        Box::new(
            keys.try_into()
                .map_err(|_| json("a sync committee has 512 keys"))?,
        ),
        BlsPublicKey::new(hex_fixed(value, "aggregate_pubkey")?),
    ))
}

/// Parse a beacon `/light_client/bootstrap` JSON response (`{version, data}`).
///
/// # Errors
///
/// Returns [`BuildError::Json`] for any shape or width mismatch.
pub fn bootstrap_from_beacon_json(
    response: &Value,
) -> Result<EthereumNativeLightClientBootstrapV1, BuildError> {
    let fork = fork_from_version(response)?;
    let data = field(response, "data")?;
    let electra = matches!(fork, EthereumFork::Electra | EthereumFork::Fulu);
    let branch = if electra {
        CurrentSyncCommitteeBranch::Electra(roots(data, "current_sync_committee_branch")?)
    } else {
        CurrentSyncCommitteeBranch::PreElectra(roots(data, "current_sync_committee_branch")?)
    };
    Ok(EthereumNativeLightClientBootstrapV1::from_native(
        &LightClientBootstrap {
            header: light_client_header(field(data, "header")?, fork)?,
            current_sync_committee: committee(field(data, "current_sync_committee")?)?,
            current_sync_committee_branch: branch,
        },
    ))
}

/// Parse one beacon light-client update or finality update JSON object (`{version, data}`).
///
/// The finalized header is decoded with the fork its slot has in `schedule`, which may be
/// earlier than the attested header's `version`.
///
/// # Errors
///
/// Returns [`BuildError::Json`] for any shape or width mismatch.
pub fn update_from_beacon_json(
    response: &Value,
    schedule: &ForkSchedule,
) -> Result<EthereumNativeLightClientUpdateV1, BuildError> {
    let fork = fork_from_version(response)?;
    let data = field(response, "data")?;
    let electra = matches!(fork, EthereumFork::Electra | EthereumFork::Fulu);
    let finalized = field(data, "finalized_header")?;
    let finalized_slot = decimal(field(finalized, "beacon")?, "slot")?;
    let finalized_fork = schedule
        .fork_at_slot(finalized_slot)
        .map_err(|_| json("the finalized slot precedes Altair"))?
        .0;
    let next_sync_committee = match data.get("next_sync_committee") {
        Some(next) => Some(NextSyncCommitteeProof {
            committee: committee(next)?,
            branch: if electra {
                NextSyncCommitteeBranch::Electra(roots(data, "next_sync_committee_branch")?)
            } else {
                NextSyncCommitteeBranch::PreElectra(roots(data, "next_sync_committee_branch")?)
            },
        }),
        None => None,
    };
    let aggregate = field(data, "sync_aggregate")?;
    Ok(EthereumNativeLightClientUpdateV1::from_native(
        &LightClientUpdate {
            attested_header: light_client_header(field(data, "attested_header")?, fork)?,
            next_sync_committee,
            finalized_header: light_client_header(finalized, finalized_fork)?,
            finality_branch: if electra {
                FinalityBranch::Electra(roots(data, "finality_branch")?)
            } else {
                FinalityBranch::PreElectra(roots(data, "finality_branch")?)
            },
            sync_aggregate: SyncAggregate::new(
                hex_fixed(aggregate, "sync_committee_bits")?,
                BlsSignature::new(hex_fixed(aggregate, "sync_committee_signature")?),
            ),
            signature_slot: decimal(data, "signature_slot")?,
        },
    ))
}

// ---------------------------------------------------------------------------------------------
// Execution RLP
// ---------------------------------------------------------------------------------------------

/// Re-encode `header` as its consensus RLP (London through Prague fields in order; optional
/// trailing fields are included when present).
#[must_use]
pub fn header_rlp(header: &EvmHeader) -> Vec<u8> {
    let mut fields = vec![
        rlp_encode_bytes(&header.parent_hash),
        rlp_encode_bytes(&header.ommers_hash),
        rlp_encode_bytes(&header.beneficiary),
        rlp_encode_bytes(&header.state_root),
        rlp_encode_bytes(&header.transactions_root),
        rlp_encode_bytes(&header.receipts_root),
        rlp_encode_bytes(&header.logs_bloom),
        rlp_encode_uint_bytes(header.difficulty.minimal_be_bytes()),
        rlp_encode_u64(header.number),
        rlp_encode_u64(header.gas_limit),
        rlp_encode_u64(header.gas_used),
        rlp_encode_u64(header.timestamp),
        rlp_encode_bytes(&header.extra_data),
        rlp_encode_bytes(&header.mix_hash),
        rlp_encode_bytes(&header.nonce),
    ];
    if let Some(base_fee) = header.base_fee_per_gas {
        fields.push(rlp_encode_uint_bytes(base_fee.minimal_be_bytes()));
    }
    if let Some(root) = header.withdrawals_root {
        fields.push(rlp_encode_bytes(&root));
    }
    if let Some(value) = header.blob_gas_used {
        fields.push(rlp_encode_u64(value));
    }
    if let Some(value) = header.excess_blob_gas {
        fields.push(rlp_encode_u64(value));
    }
    if let Some(root) = header.parent_beacon_block_root {
        fields.push(rlp_encode_bytes(&root));
    }
    if let Some(hash) = header.requests_hash {
        fields.push(rlp_encode_bytes(&hash));
    }
    rlp_encode_list(&fields)
}

/// Re-encode `receipt` in its receipts-trie form.
#[must_use]
pub fn receipt_rlp(receipt: &EvmReceipt) -> Vec<u8> {
    let parsed = EthereumReceiptV1 {
        tx_type: receipt.tx_type,
        success: receipt.status == Some(1),
        cumulative_gas_used: receipt.cumulative_gas_used,
        logs: receipt
            .logs
            .iter()
            .map(|log| EthereumLogV1 {
                address: log.address,
                topics: log.topics.clone(),
                data: log.data.clone(),
            })
            .collect(),
    };
    encode_receipt(&parsed, &receipt.logs_bloom)
}

// ---------------------------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------------------------

/// Ethereum evidence builder over one beacon endpoint and one execution endpoint.
pub struct EthereumBuilder {
    beacon: BeaconClient,
    execution: EvmClient,
}

/// Which source event a proof selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EthereumEventV1 {
    /// One `SccpTransferToTaira` log at this index within the receipt.
    TransferToTaira {
        /// Log index within the receipt.
        log_index: u32,
    },
    /// A run of void logs within the receipt.
    Void {
        /// First log index within the receipt.
        first_log_index: u32,
        /// Number of logs.
        log_count: u32,
    },
}

impl EthereumBuilder {
    /// A builder over `beacon` and `execution`.
    #[must_use]
    pub fn new(beacon: BeaconClient, execution: EvmClient) -> Self {
        Self { beacon, execution }
    }

    fn schedule() -> Result<ForkSchedule, BuildError> {
        SccpChainProfilesV1::compiled()
            .ethereum
            .schedule()
            .map_err(|error| BuildError::Inconsistent(format!("fork schedule: {error:?}")))
    }

    /// The latest finality update.
    ///
    /// # Errors
    ///
    /// Any endpoint or JSON failure.
    pub fn finality_update(&self) -> Result<EthereumNativeLightClientUpdateV1, BuildError> {
        let response = self
            .beacon
            .transport()
            .get_json("/eth/v1/beacon/light_client/finality_update")?;
        update_from_beacon_json(&response, &Self::schedule()?)
    }

    /// Build the `InitializeLightClient` bootstrap of the finalized beacon block `block_root`.
    ///
    /// # Errors
    ///
    /// Any endpoint or JSON failure.
    pub fn bootstrap(&self, block_root: &[u8; 32]) -> Result<SccpLcBootstrapV1, BuildError> {
        let response = self.beacon.transport().get_json(&format!(
            "/eth/v1/beacon/light_client/bootstrap/{}",
            format_data(block_root)
        ))?;
        SccpLcBootstrapDataV1::Ethereum(bootstrap_from_beacon_json(&response)?)
            .to_bootstrap()
            .map_err(|error| BuildError::Inconsistent(format!("bootstrap frame: {error}")))
    }

    /// Build the bootstrap of the current finalized beacon block.
    ///
    /// # Errors
    ///
    /// Any endpoint or JSON failure.
    pub fn finalized_bootstrap(&self) -> Result<SccpLcBootstrapV1, BuildError> {
        let root = self.beacon.finalized_header()?.root;
        self.bootstrap(&root)
    }

    /// Build an advance from the stored committee period `stored_period` to the latest
    /// finality: the committee updates of every later period, then the finality update, at
    /// most `max_updates` in all.
    ///
    /// # Errors
    ///
    /// Any endpoint or JSON failure, or a head more than `max_updates − 1` periods ahead.
    pub fn advance(
        &self,
        stored_period: u64,
        max_updates: usize,
    ) -> Result<SccpLcAdvanceBytesV1, BuildError> {
        let schedule = Self::schedule()?;
        let finality = self.finality_update()?;
        let finality_period = finality.signature_slot / SLOTS_PER_PERIOD;
        let mut updates = Vec::new();
        if finality_period > stored_period {
            let count = finality_period - stored_period;
            if count >= u64::try_from(max_updates).unwrap_or(u64::MAX) {
                return Err(BuildError::Unavailable(format!(
                    "the light client is {count} periods behind; advance it in steps"
                )));
            }
            let response = self.beacon.transport().get_json(&format!(
                "/eth/v1/beacon/light_client/updates?start_period={stored_period}&count={count}"
            ))?;
            for item in response
                .as_array()
                .ok_or_else(|| json("light-client updates are not an array"))?
            {
                updates.push(update_from_beacon_json(item, &schedule)?);
            }
        }
        updates.push(finality);
        SccpLcAdvanceV1::Ethereum(EthereumLcAdvanceV1 { updates })
            .to_bytes()
            .map_err(|error| BuildError::Inconsistent(format!("advance frame: {error}")))
    }

    fn block(&self, number: u64) -> Result<EvmBlock, BuildError> {
        self.execution
            .block_by_number(BlockTag::Number(number), false)?
            .ok_or_else(|| BuildError::Unavailable(format!("block {number} is not served")))
    }

    fn checked_header_rlp(block: &EvmBlock) -> Result<Vec<u8>, BuildError> {
        let rlp = header_rlp(&block.header);
        if keccak256(&[&rlp]) != block.header.hash {
            return Err(BuildError::Inconsistent(format!(
                "the RLP of block {} does not hash to its hash",
                block.header.number
            )));
        }
        Ok(rlp)
    }

    /// Build the inbound or void proof of `event` in transaction `tx_hash`, anchored at the
    /// latest finality update.
    ///
    /// # Errors
    ///
    /// Any endpoint failure, an inconsistent response, a transaction that is not yet finalized
    /// or older than 256 blocks before the finalized anchor (TODO(ws37)).
    pub fn source_proof(
        &self,
        tx_hash: &[u8; 32],
        event: EthereumEventV1,
    ) -> Result<SccpSourceProofBytesV1, BuildError> {
        let receipt = self
            .execution
            .transaction_receipt(tx_hash)?
            .ok_or_else(|| BuildError::Unavailable("the transaction is not mined".into()))?;
        let event_block = self.block(receipt.block_number)?;
        let event_header = Self::checked_header_rlp(&event_block)?;
        let receipts = self
            .execution
            .block_receipts(crate::evm::BlockId::Hash(receipt.block_hash))?
            .ok_or_else(|| BuildError::Unavailable("the block receipts are not served".into()))?;
        let entries: Vec<(Vec<u8>, Vec<u8>)> = receipts
            .iter()
            .map(|entry| (rlp_encode_u64(entry.transaction_index), receipt_rlp(entry)))
            .collect();
        if mpt_root(&entries) != Some(event_block.header.receipts_root) {
            return Err(BuildError::Inconsistent(
                "the served receipts do not rebuild the receipts root".into(),
            ));
        }
        let receipt_proof = mpt_proof(&entries, &rlp_encode_u64(receipt.transaction_index))
            .ok_or_else(|| BuildError::Inconsistent("the receipt is not in its block".into()))?;
        let finality = self.finality_update()?;
        let anchor_number = finality
            .finalized_header
            .to_native()
            .ok()
            .and_then(|header| header.authenticated_execution_block())
            .map(|block| block.block_number)
            .ok_or_else(|| {
                BuildError::Inconsistent("the finality update has no execution block".into())
            })?;
        let event_number = receipt.block_number;
        if anchor_number < event_number {
            return Err(BuildError::Unavailable(format!(
                "block {event_number} is not finalized yet (finalized: {anchor_number})"
            )));
        }
        let ancestry = if anchor_number == event_number {
            EthereumAncestryV1::SameBlock
        } else if anchor_number - event_number <= MAX_HEADER_CHAIN {
            let headers = (event_number + 1..=anchor_number)
                .map(|number| {
                    self.block(number)
                        .and_then(|block| Self::checked_header_rlp(&block))
                })
                .collect::<Result<Vec<_>, _>>()?;
            EthereumAncestryV1::HeaderChain(EthereumHeaderSegmentV1 { headers })
        } else {
            return Err(BuildError::Unavailable(
                "the event is more than 256 blocks older than the finalized anchor \
                 (HistoryContract and Backfill: TODO(ws37))"
                    .into(),
            ));
        };
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
        SccpSourceProofV1::Ethereum(EthereumSourceProofV1 {
            anchor: EthereumProofAnchorV1::FinalityUpdate(Box::new(finality)),
            ancestry,
            event_header,
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

    #[test]
    fn json_helpers_reject_malformed_fields() {
        let value = norito::json!({"a": "0x0102", "b": "7", "c": 5, "d": ["0x01"]});
        assert_eq!(
            hex_bytes(text(&value, "a").expect("a")).expect("hex"),
            vec![1, 2]
        );
        assert_eq!(decimal(&value, "b").expect("decimal"), 7);
        assert!(decimal(&value, "c").is_err());
        assert!(text(&value, "missing").is_err());
        assert!(hex_fixed::<2>(&value, "a").is_ok());
        assert!(hex_fixed::<3>(&value, "a").is_err());
        assert_eq!(hex_list(&value, "d").expect("list"), vec![vec![1]]);
        assert!(fork_from_version(&norito::json!({"version": "altair"})).is_err());
        assert_eq!(
            fork_from_version(&norito::json!({"version": "electra"})).expect("fork"),
            EthereumFork::Electra
        );
    }

    #[test]
    fn base_fees_are_little_endian_words() {
        let word = le_u256_from_decimal("258").expect("fee");
        assert_eq!(&word[..2], &[2, 1]);
        assert!(le_u256_from_decimal("-1").is_err());
    }
}
