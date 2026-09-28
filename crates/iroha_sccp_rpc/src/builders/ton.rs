//! TON evidence builders (spec §4.13.3, §7.2).
//!
//! Key-block hops (the forward links of `liteServer.getBlockProof` with `getBlockHeader` and
//! `getConfigParams` proofs of each key block), bootstraps, and transaction proofs (the
//! `getShardBlockProof` walk from a signed masterchain block to the burn's shard block and the
//! `getOneTransaction` proof), all over ADNL liteservers. Liteserver `BoC`s are re-encoded in
//! the canonical proof form the light client accepts. Everything built is untrusted until
//! `iroha_sccp` verifies it.
//!
//! TODO(ws3A): `OldMcBlocksInfo` back-links for burns older than the stored epochs, and
//! captured-mainnet fixtures of every answer the builders use.

use iroha_data_model::sccp::{
    inbound::SccpSourceProofBytesV1,
    light_client::{SccpLcAdvanceBytesV1, SccpLcBootstrapV1},
};
use iroha_sccp::{
    TonBlockIdExtV1, TonBlockSignaturesV1, TonOrdinaryBlockSignaturesV1,
    TonSimplexBlockSignaturesV1, TonValidatorSignatureV1,
    light_client::{
        proof::{SccpLcAdvanceV1, SccpLcBootstrapDataV1, SccpSourceProofV1},
        ton::{
            TonKeyBlockHopV1, TonLcAdvanceV1, TonLcBootstrapV1, TonMasterchainAnchorV1,
            TonShardLinkV1, TonSignedBlockV1, TonSourceProofV1,
        },
    },
    ton_canonical_boc_v1,
};

use super::ethereum::BuildError;
use crate::ton::{
    AccountId, BlockIdExt, LiteClient, LiteClientError,
    schema::{BlockLink, SignatureSet},
};

/// `getBlockHeader` mode: state update, extra and shard hashes.
pub const HEADER_MODE: u32 = 1 | 16 | 32;
/// Config parameters every epoch needs: election timings, catchain and current validators.
pub const EPOCH_CONFIG_PARAMS: [i32; 3] = [15, 28, 34];

fn lite(error: LiteClientError) -> BuildError {
    BuildError::Unavailable(format!("liteserver: {error}"))
}

fn canonical(bytes: &[u8], what: &str) -> Result<Vec<u8>, BuildError> {
    ton_canonical_boc_v1(bytes)
        .ok_or_else(|| BuildError::Inconsistent(format!("the {what} BoC is malformed")))
}

/// The light client's block id of a liteserver block id.
#[must_use]
pub fn block_id(id: &BlockIdExt) -> TonBlockIdExtV1 {
    TonBlockIdExtV1 {
        workchain: id.workchain,
        shard: id.shard,
        seqno: id.seqno,
        root_hash: id.root_hash,
        file_hash: id.file_hash,
    }
}

/// The light client's signatures of a liteserver signature set, in node-id order.
#[must_use]
pub fn signatures(set: &SignatureSet) -> TonBlockSignaturesV1 {
    let mut entries: Vec<TonValidatorSignatureV1> = set
        .signatures()
        .iter()
        .map(|signature| TonValidatorSignatureV1 {
            node_id_short: signature.node_id_short,
            signature: signature.signature.clone(),
        })
        .collect();
    entries.sort_by_key(|entry| entry.node_id_short);
    match set {
        SignatureSet::Ordinary(set) => {
            TonBlockSignaturesV1::Ordinary(TonOrdinaryBlockSignaturesV1 {
                catchain_seqno: set.catchain_seqno,
                validator_list_hash_short: set.validator_set_hash,
                signatures: entries,
            })
        }
        SignatureSet::Simplex(set) => TonBlockSignaturesV1::Simplex(TonSimplexBlockSignaturesV1 {
            catchain_seqno: set.cc_seqno,
            validator_list_hash_short: set.validator_set_hash,
            session_id: set.session_id,
            slot: set.slot,
            candidate_data: set.candidate.clone(),
            signatures: entries,
        }),
    }
}

/// TON evidence builder over a liteserver client.
pub struct TonBuilder {
    lite: LiteClient,
}

impl TonBuilder {
    /// A builder over `lite`.
    #[must_use]
    pub fn new(lite: LiteClient) -> Self {
        Self { lite }
    }

    fn header(&self, id: BlockIdExt) -> Result<Vec<u8>, BuildError> {
        let header = self.lite.get_block_header(id, HEADER_MODE).map_err(lite)?;
        canonical(&header.header_proof, "block header")
    }

    fn config_proof(&self, id: BlockIdExt) -> Result<Vec<u8>, BuildError> {
        let config = self
            .lite
            .get_config_params(0, id, EPOCH_CONFIG_PARAMS.to_vec())
            .map_err(lite)?;
        canonical(&config.config_proof, "config")
    }

    /// The masterchain key block with seqno `seqno` (its full id).
    fn masterchain_block(&self, seqno: u32) -> Result<BlockIdExt, BuildError> {
        Ok(self
            .lite
            .lookup_block(
                crate::ton::BlockId::masterchain(seqno),
                crate::ton::LookupKey::Seqno,
            )
            .map_err(lite)?
            .id)
    }

    /// Seqno of the newest masterchain key block.
    ///
    /// # Errors
    ///
    /// Any liteserver failure or malformed answer.
    pub fn newest_key_block(&self) -> Result<u32, BuildError> {
        let last = self.lite.get_masterchain_info().map_err(lite)?.last;
        // The last block is a key block or names the newest one in `prev_key_block_seqno`.
        newest_key_block_seqno(&self.header(last)?)
    }

    /// Seqno of the key block before key block `seqno` (its `prev_key_block_seqno`).
    ///
    /// # Errors
    ///
    /// Any liteserver failure, a malformed answer, or a block that is not a key block.
    pub fn previous_key_block(&self, seqno: u32) -> Result<u32, BuildError> {
        let header = self.header(self.masterchain_block(seqno)?)?;
        match iroha_sccp::ton_header_key_block_v1(&header) {
            Some((_, true, previous)) => Ok(previous),
            Some(_) => Err(BuildError::Inconsistent(format!(
                "masterchain block {seqno} is not a key block"
            ))),
            None => Err(BuildError::Inconsistent(
                "malformed masterchain header proof".into(),
            )),
        }
    }

    /// Build the `InitializeLightClient` bootstrap of the newest key block.
    ///
    /// # Errors
    ///
    /// Any liteserver failure or malformed answer.
    pub fn bootstrap(&self) -> Result<SccpLcBootstrapV1, BuildError> {
        self.bootstrap_at(self.newest_key_block()?)
    }

    /// Build the `InitializeLightClient` bootstrap of key block `key_seqno`, so every
    /// Parliament member can rebuild the exact bootstrap a proposal names.
    ///
    /// # Errors
    ///
    /// Any liteserver failure or malformed answer.
    pub fn bootstrap_at(&self, key_seqno: u32) -> Result<SccpLcBootstrapV1, BuildError> {
        let key = self.masterchain_block(key_seqno)?;
        SccpLcBootstrapDataV1::Ton(TonLcBootstrapV1 {
            block_id: block_id(&key),
            header_proof: self.header(key)?,
            config_proof: self.config_proof(key)?,
        })
        .to_bootstrap()
        .map_err(|error| BuildError::Inconsistent(format!("bootstrap frame: {error}")))
    }

    /// Build an advance from the newest stored key block `latest_set_id`: the forward key-block
    /// links to the liteserver's last block, at most `max_hops`.
    ///
    /// # Errors
    ///
    /// Any liteserver failure, or no newer key block.
    pub fn advance(
        &self,
        latest_set_id: u64,
        max_hops: usize,
    ) -> Result<SccpLcAdvanceBytesV1, BuildError> {
        let known = self.masterchain_block(
            u32::try_from(latest_set_id)
                .map_err(|_| BuildError::Inconsistent("key block seqno overflows".into()))?,
        )?;
        let mut hops = Vec::new();
        let mut from = known;
        while hops.len() < max_hops {
            let proof = self.lite.get_block_proof(from, None).map_err(lite)?;
            let mut advanced = false;
            for step in &proof.steps {
                let BlockLink::Forward(link) = step else {
                    continue;
                };
                if !link.to_key_block || hops.len() == max_hops {
                    continue;
                }
                hops.push(TonKeyBlockHopV1 {
                    block: TonSignedBlockV1 {
                        block_id: block_id(&link.to),
                        header_proof: self.header(link.to)?,
                        signatures: signatures(&link.signatures),
                    },
                    config_proof: self.config_proof(link.to)?,
                });
                from = link.to;
                advanced = true;
            }
            if proof.complete || !advanced {
                break;
            }
        }
        if hops.is_empty() {
            return Err(BuildError::Unavailable(
                "no key block follows the stored epoch yet".into(),
            ));
        }
        SccpLcAdvanceV1::Ton(TonLcAdvanceV1 { hops })
            .to_bytes()
            .map_err(|error| BuildError::Inconsistent(format!("advance frame: {error}")))
    }

    /// The burned payload of the minter transaction `(lt, hash)` emitting
    /// `sccp_transfer_to_taira` as external message `message_index`.
    ///
    /// # Errors
    ///
    /// Any liteserver failure, or a transaction without that event.
    pub fn transfer_payload(
        &self,
        minter: [u8; 32],
        lt: u64,
        hash: [u8; 32],
        message_index: u16,
    ) -> Result<Vec<u8>, BuildError> {
        let account = AccountId {
            workchain: 0,
            address: minter,
        };
        let list = self
            .lite
            .get_transactions(1, account, lt, hash)
            .map_err(lite)?;
        let block = *list
            .ids
            .first()
            .ok_or_else(|| BuildError::Unavailable("the transaction is not served".into()))?;
        let transaction = self
            .lite
            .get_one_transaction(block, account, lt)
            .map_err(lite)?;
        iroha_sccp::ton_sccp_transfer_payload_v1(
            &transaction.transaction,
            minter,
            lt,
            message_index,
        )
        .ok_or_else(|| {
            BuildError::Inconsistent("the transaction emitted no sccp_transfer_to_taira".into())
        })
    }

    /// Build the inbound or void proof of the minter transaction `(lt, hash)` emitting external
    /// message `message_index`, anchored at a masterchain block signed by the epoch of the
    /// stored key block `known_key_block`.
    ///
    /// # Errors
    ///
    /// Any liteserver failure or malformed answer.
    pub fn source_proof(
        &self,
        minter: [u8; 32],
        lt: u64,
        hash: [u8; 32],
        message_index: u16,
        known_key_block: u32,
    ) -> Result<SccpSourceProofBytesV1, BuildError> {
        let account = AccountId {
            workchain: 0,
            address: minter,
        };
        let list = self
            .lite
            .get_transactions(1, account, lt, hash)
            .map_err(lite)?;
        let event_block = *list
            .ids
            .first()
            .ok_or_else(|| BuildError::Unavailable("the transaction is not served".into()))?;
        let transaction = self
            .lite
            .get_one_transaction(event_block, account, lt)
            .map_err(lite)?;
        let walk = self.lite.get_shard_block_proof(event_block).map_err(lite)?;
        let key = self.masterchain_block(known_key_block)?;
        let signed = self
            .lite
            .get_block_proof(key, Some(walk.masterchain_id))
            .map_err(lite)?
            .steps
            .into_iter()
            .rev()
            .find_map(|step| match step {
                BlockLink::Forward(link) if link.to == walk.masterchain_id => Some(link),
                _ => None,
            })
            .ok_or_else(|| {
                BuildError::Unavailable("no signatures of the anchoring masterchain block".into())
            })?;
        let shard_blocks = walk
            .links
            .iter()
            .filter(|link| link.id.workchain == 0)
            .map(|link| {
                Ok(TonShardLinkV1 {
                    block_id: block_id(&link.id),
                    header_proof: canonical(&link.proof, "shard block")?,
                })
            })
            .collect::<Result<Vec<_>, BuildError>>()?;
        SccpSourceProofV1::Ton(TonSourceProofV1 {
            masterchain: TonMasterchainAnchorV1::Signed(TonSignedBlockV1 {
                block_id: block_id(&walk.masterchain_id),
                header_proof: self.header(walk.masterchain_id)?,
                signatures: signatures(&signed.signatures),
            }),
            shard_blocks,
            event_block_proof: canonical(&transaction.proof, "transaction proof")?,
            transaction: canonical(&transaction.transaction, "transaction")?,
            transaction_lt: lt,
            message_index,
            minter,
        })
        .to_bytes()
        .map_err(|error| BuildError::Inconsistent(format!("proof frame: {error}")))
    }
}

/// The newest key block at a masterchain header proof: the block itself when it is a key
/// block, otherwise its `prev_key_block_seqno`.
///
/// # Errors
///
/// [`BuildError::Inconsistent`] for a malformed proof.
fn newest_key_block_seqno(header_proof: &[u8]) -> Result<u32, BuildError> {
    let (seqno, key_block, prev_key) = iroha_sccp::ton_header_key_block_v1(header_proof)
        .ok_or_else(|| BuildError::Inconsistent("malformed masterchain header proof".into()))?;
    Ok(if key_block { seqno } else { prev_key })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ton::schema::{OrdinarySignatureSet, Signature};

    #[test]
    fn signature_sets_convert_in_node_order() {
        let set = SignatureSet::Ordinary(OrdinarySignatureSet {
            validator_set_hash: 9,
            catchain_seqno: 3,
            signatures: vec![
                Signature {
                    node_id_short: [2; 32],
                    signature: vec![1; 64],
                },
                Signature {
                    node_id_short: [1; 32],
                    signature: vec![2; 64],
                },
            ],
        });
        let TonBlockSignaturesV1::Ordinary(converted) = signatures(&set) else {
            panic!("ordinary");
        };
        assert_eq!(converted.catchain_seqno, 3);
        assert_eq!(converted.validator_list_hash_short, 9);
        assert_eq!(converted.signatures[0].node_id_short, [1; 32]);
        let id = BlockIdExt {
            workchain: -1,
            shard: 0x8000_0000_0000_0000,
            seqno: 5,
            root_hash: [3; 32],
            file_hash: [4; 32],
        };
        assert_eq!(block_id(&id).seqno, 5);
        assert!(canonical(&[1, 2, 3], "junk").is_err());
    }
}
