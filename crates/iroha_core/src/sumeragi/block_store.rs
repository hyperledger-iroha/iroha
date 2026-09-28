//! The driver's [`BlockStore`] over Kura (`specs/sumeragi.md` §12.2; option A of the block
//! format, DECISIONS #1).
//!
//! Kura holds **one canonical `SignedBlockWire` frame per height**: the result-bearing iroha
//! block with its [`CommitCertificate`] — the canonical Norito frames of the core
//! [`BlockHeader`] and `CommitQC` and the preimage of the certified result `R` — written
//! atomically and durably by [`Kura::store_block`]. There is no finality sidecar, so there is no
//! "durable block without its certificate" state to recover from. Genesis (height `g`) keeps its
//! signature and has no certificate; the core never asks the store for it.
//!
//! **Payloads (§3 rule 2).** The core payload of a block is not stored separately: it is the
//! canonical resultless, certificate-free proposal wire of the stored block
//! (`canonical_resultless_proposal().encode_wire()`). Blocks are work-driven and never empty
//! (§6.10), so a stored header with `payload_len == 0` is corrupt.
//! [`KuraBlockStore::append`](BlockStore::append) re-derives it from the frame it is about to
//! write and checks `H(TAG_PAY ‖ payload) == header.payload_hash` first: a mismatch is a local
//! bug, reported as a failed write (retried by the driver), never as an invalid block.
//!
//! **Hand-off.** The executor's `prepare` of a committed block leaves the result-bearing block
//! and its result preimage in the shared [`Staging`] slot; the driver's next step is the append
//! of that block (§12.2 apply sequencing), which picks it up from there.

use std::{io, num::NonZeroUsize, sync::Arc};

use iroha_data_model::block::{CommitCertificate, SignedBlock};
use iroha_sumeragi::{
    message::{Block, BlockHeader, Qc, SyncEntry, VoteKind},
    preimage::payload_hash,
    types::Hash32,
};
use parking_lot::Mutex;
use thiserror::Error;

use super::{
    commitment::result_of_preimage,
    driver::{SharedCrypto, serve, traits::BlockStore},
};
use crate::kura::Kura;

/// A block the executor prepared for commit: what the block store writes for it.
#[derive(Clone, Debug)]
pub struct StagedBlock {
    /// Core block hash of the committed block.
    pub block_hash: Hash32,
    /// The result-bearing iroha block, without a certificate.
    pub executed: Arc<SignedBlock>,
    /// Canonical `ExecutionResultCommitment` bytes, `R = H(tag ‖ result_preimage)`.
    pub result_preimage: Vec<u8>,
}

/// The single-slot hand-off from the executor's `prepare` to the block store's `append` (both
/// run on the executor thread, one after the other).
#[derive(Clone, Debug, Default)]
pub struct Staging {
    slot: Arc<Mutex<Option<StagedBlock>>>,
}

impl Staging {
    /// An empty slot.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stage `block`, replacing whatever was staged.
    pub fn stage(&self, block: StagedBlock) {
        *self.slot.lock() = Some(block);
    }

    /// The staged block of `block_hash`, if that is what is staged.
    #[must_use]
    pub fn get(&self, block_hash: &Hash32) -> Option<StagedBlock> {
        self.slot
            .lock()
            .as_ref()
            .filter(|staged| staged.block_hash == *block_hash)
            .cloned()
    }

    /// Drop the staged block.
    pub fn clear(&self) {
        *self.slot.lock() = None;
    }
}

/// Why a block could not be written or read back. Every variant is a local condition (a bug,
/// corruption or an I/O failure): the driver retries a failed write and treats an unreadable
/// entry as missing.
#[derive(Debug, Error)]
pub enum BlockStoreError {
    /// The `CommitQC` does not certify this block at this height.
    #[error("the commit certificate does not certify block {height}")]
    QcMismatch {
        /// Height of the block.
        height: u64,
    },
    /// Appending would leave a gap.
    #[error("append of {height} after tip {tip}")]
    Gap {
        /// Height of the block.
        height: u64,
        /// Stored tip.
        tip: u64,
    },
    /// Another block is already stored at this height.
    #[error("another block is stored at {height}")]
    Conflict {
        /// Height of the block.
        height: u64,
    },
    /// The executor staged no result-bearing block for this block hash.
    #[error("no prepared block staged for {0}")]
    NotStaged(Hash32),
    /// The staged block does not belong to the core block (height or result).
    #[error("the staged block does not match the committed block: {0}")]
    StagedMismatch(&'static str),
    /// The stored header declares an empty payload; blocks are never empty (§6.10).
    #[error("the stored header declares an empty payload")]
    EmptyPayload,
    /// The re-derived payload does not hash to the header's `payload_hash` (§3 rule 2).
    #[error("the payload re-derived from block {height} does not match its header")]
    PayloadMismatch {
        /// Height of the block.
        height: u64,
    },
    /// A stored block above genesis carries no certificate.
    #[error("the stored block {height} carries no commit certificate")]
    MissingCertificate {
        /// Height of the block.
        height: u64,
    },
    /// A Norito encoding or decoding failure.
    #[error("encoding: {0}")]
    Encoding(String),
    /// Kura refused or failed the write.
    #[error("kura: {0}")]
    Kura(String),
}

impl From<BlockStoreError> for io::Error {
    fn from(error: BlockStoreError) -> Self {
        io::Error::other(error.to_string())
    }
}

/// The core payload of a stored block (§3 rule 2): the canonical resultless, certificate-free
/// proposal wire.
///
/// # Errors
/// [`BlockStoreError::EmptyPayload`] for `payload_len == 0` (blocks are never empty, §6.10), or
/// the proposal wire cannot be encoded.
pub fn derive_payload(block: &SignedBlock, payload_len: u32) -> Result<Vec<u8>, BlockStoreError> {
    if payload_len == 0 {
        return Err(BlockStoreError::EmptyPayload);
    }
    block
        .canonical_resultless_proposal()
        .encode_wire()
        .map_err(|error| BlockStoreError::Encoding(error.to_string()))
}

/// The certificate of a committed block: canonical Norito frames of its core header and
/// `CommitQC`, and its result preimage.
///
/// # Errors
/// A Norito encoding failure.
pub fn commit_certificate(
    header: &BlockHeader,
    commit_qc: &Qc,
    result_preimage: Vec<u8>,
) -> Result<CommitCertificate, BlockStoreError> {
    let encode = |error: norito::Error| BlockStoreError::Encoding(error.to_string());
    Ok(CommitCertificate::new(
        norito::encode_canonical(header).map_err(encode)?,
        norito::encode_canonical(commit_qc).map_err(encode)?,
        result_preimage,
    ))
}

/// The core header and `CommitQC` of a certificate.
///
/// # Errors
/// A part is not one canonical frame.
pub fn decode_certificate(
    certificate: &CommitCertificate,
) -> Result<(BlockHeader, Qc), BlockStoreError> {
    let decode = |error: norito::Error| BlockStoreError::Encoding(error.to_string());
    Ok((
        norito::decode_canonical(&certificate.consensus_header).map_err(decode)?,
        norito::decode_canonical(&certificate.commit_qc).map_err(decode)?,
    ))
}

/// The committed chain in Kura, as the driver sees it.
pub struct KuraBlockStore {
    kura: Arc<Kura>,
    hasher: SharedCrypto,
    genesis_height: u64,
    staging: Staging,
}

impl KuraBlockStore {
    /// A block store over `kura` (holding genesis at `genesis_height`), hashing with the
    /// instance's crypto and taking prepared blocks from `staging`.
    #[must_use]
    pub fn new(
        kura: Arc<Kura>,
        hasher: SharedCrypto,
        genesis_height: u64,
        staging: Staging,
    ) -> Self {
        Self {
            kura,
            hasher,
            genesis_height,
            staging,
        }
    }

    /// The hand-off slot the executor stages prepared blocks in.
    #[must_use]
    pub fn staging(&self) -> &Staging {
        &self.staging
    }

    fn stored(&self, height: u64) -> Option<Arc<SignedBlock>> {
        if height <= self.genesis_height {
            return None;
        }
        let height = NonZeroUsize::new(usize::try_from(height).ok()?)?;
        self.kura.get_block(height)
    }

    /// The core header and `CommitQC` stored at `height` (cheaper than [`BlockStore::entry`]:
    /// no payload re-derivation).
    ///
    /// # Errors
    /// The stored block lacks or carries a malformed certificate, or it certifies another height.
    pub fn certified(&self, height: u64) -> Result<Option<(BlockHeader, Qc)>, BlockStoreError> {
        let Some(block) = self.stored(height) else {
            return Ok(None);
        };
        certified_parts(&block, height).map(Some)
    }

    /// The core header stored at `height`.
    #[must_use]
    pub fn header(&self, height: u64) -> Option<BlockHeader> {
        log_unreadable(height, self.certified(height)).map(|(header, _)| header)
    }

    /// The committed tip above genesis (`None` at genesis).
    #[must_use]
    pub fn tip(&self) -> Option<SyncEntry> {
        self.entry(self.height())
    }

    /// The last `count` committed headers above genesis, oldest first (`Init.recent_headers`
    /// takes `W + 2`).
    ///
    /// # Errors
    /// A height between genesis and the tip is missing or unreadable (local corruption).
    pub fn recent_headers(&self, count: u64) -> io::Result<Vec<BlockHeader>> {
        let tip = self.height();
        let first = tip
            .saturating_sub(count)
            .saturating_add(1)
            .max(self.genesis_height.saturating_add(1));
        (first..=tip)
            .map(|height| match self.certified(height) {
                Ok(Some((header, _))) => Ok(header),
                Ok(None) => Err(io::Error::other(format!(
                    "sumeragi block store: height {height} missing below the tip {tip}"
                ))),
                Err(error) => Err(error.into()),
            })
            .collect()
    }

    /// Consecutive entries from `from_height` for `ServeBlocks` (§3.5): at most `max_count`,
    /// stopping at the first missing height, within `max_bytes` unless a single entry.
    #[must_use]
    pub fn entries(&self, from_height: u64, max_count: u16, max_bytes: u32) -> Vec<SyncEntry> {
        serve::entries(self, from_height, max_count, max_bytes)
    }

    fn write(&self, block: &Block, commit_qc: &Qc) -> Result<(), BlockStoreError> {
        let height = block.header.height;
        if commit_qc.kind != VoteKind::Commit
            || commit_qc.height != height
            || block.hash(&*self.hasher) != commit_qc.block_hash
        {
            return Err(BlockStoreError::QcMismatch { height });
        }
        let tip = self.height();
        if height <= tip {
            // A retry after a write that reached the disk: the same certified block is stored.
            return match self.certified(height)? {
                Some((header, stored))
                    if header == block.header
                        && (stored.block_hash, stored.result)
                            == (commit_qc.block_hash, commit_qc.result) =>
                {
                    Ok(())
                }
                _ => Err(BlockStoreError::Conflict { height }),
            };
        }
        if height != tip.saturating_add(1) {
            return Err(BlockStoreError::Gap { height, tip });
        }
        let staged = self
            .staging
            .get(&commit_qc.block_hash)
            .ok_or(BlockStoreError::NotStaged(commit_qc.block_hash))?;
        if staged.executed.header().height().get() != height {
            return Err(BlockStoreError::StagedMismatch("height"));
        }
        if result_of_preimage(&staged.result_preimage) != commit_qc.result {
            return Err(BlockStoreError::StagedMismatch("result"));
        }
        if !staged.executed.has_results() || staged.executed.commit_certificate().is_some() {
            return Err(BlockStoreError::StagedMismatch(
                "not a result-bearing block",
            ));
        }
        let payload = derive_payload(&staged.executed, block.header.payload_len)?;
        if u32::try_from(payload.len()).ok() != Some(block.header.payload_len)
            || payload_hash(&*self.hasher, &payload) != block.header.payload_hash
        {
            return Err(BlockStoreError::PayloadMismatch { height });
        }
        let certificate =
            commit_certificate(&block.header, commit_qc, staged.result_preimage.clone())?;
        let frame = staged
            .executed
            .as_ref()
            .clone()
            .with_commit_certificate(Some(certificate));
        self.kura
            .store_block(frame)
            .map_err(|error| BlockStoreError::Kura(error.to_string()))
    }
}

/// The certified parts of the block stored at `height`.
fn certified_parts(block: &SignedBlock, height: u64) -> Result<(BlockHeader, Qc), BlockStoreError> {
    let certificate = block
        .commit_certificate()
        .ok_or(BlockStoreError::MissingCertificate { height })?;
    let (header, commit_qc) = decode_certificate(certificate)?;
    if header.height != height || commit_qc.height != height || commit_qc.kind != VoteKind::Commit {
        return Err(BlockStoreError::QcMismatch { height });
    }
    Ok((header, commit_qc))
}

/// The sync entry of a stored block: its core block (payload re-derived and checked against the
/// header, §3 rule 2) and `CommitQC`.
///
/// # Errors
/// A missing or malformed certificate, or a payload that does not match the header.
pub fn stored_entry(
    block: &SignedBlock,
    height: u64,
    hasher: &SharedCrypto,
) -> Result<SyncEntry, BlockStoreError> {
    let (header, commit_qc) = certified_parts(block, height)?;
    let payload = derive_payload(block, header.payload_len)?;
    let block = Block { header, payload };
    if !block.body_ok(&**hasher) || block.hash(&**hasher) != commit_qc.block_hash {
        return Err(BlockStoreError::PayloadMismatch { height });
    }
    Ok(SyncEntry { block, commit_qc })
}

fn log_unreadable<T>(height: u64, read: Result<Option<T>, BlockStoreError>) -> Option<T> {
    read.unwrap_or_else(|error| {
        iroha_logger::error!(height, %error, "sumeragi block store entry unreadable");
        None
    })
}

impl BlockStore for KuraBlockStore {
    fn height(&self) -> u64 {
        u64::try_from(self.kura.blocks_count())
            .unwrap_or(u64::MAX)
            .max(self.genesis_height)
    }

    fn entry(&self, height: u64) -> Option<SyncEntry> {
        let block = self.stored(height)?;
        log_unreadable(height, stored_entry(&block, height, &self.hasher).map(Some))
    }

    fn append(&self, block: &Block, commit_qc: &Qc) -> io::Result<()> {
        self.write(block, commit_qc).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::KeyPair;
    use iroha_data_model::block::decode_framed_signed_block;
    use iroha_sumeragi::{
        testing::FakeCrypto,
        types::{AggregateSignature, Bitmap, SIGNATURE_LEN},
    };

    use super::*;
    use crate::block::ValidBlock;

    const INSTANCE: Hash32 = Hash32([7; 32]);

    struct Chain {
        store: KuraBlockStore,
        hasher: SharedCrypto,
        key: KeyPair,
        /// Iroha hash, core hash and result of the tip.
        tip: (
            iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
            Hash32,
            Hash32,
        ),
    }

    fn executed_block(
        key: &KeyPair,
        height: u64,
        prev: Option<iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>>,
    ) -> SignedBlock {
        ValidBlock::new_dummy_and_modify_header(key.private_key(), |header| {
            header.height = NonZeroU64::new(height).expect("height");
            header.prev_block_hash = prev;
        })
        .into()
    }

    fn chain() -> Chain {
        let kura = Kura::blank_kura_for_testing();
        let key = KeyPair::random();
        let genesis = executed_block(&key, 1, None);
        let genesis_hash = genesis.hash();
        kura.store_block(genesis).expect("genesis");
        let hasher: SharedCrypto = Arc::new(FakeCrypto::new());
        Chain {
            store: KuraBlockStore::new(kura, Arc::clone(&hasher), 1, Staging::new()),
            hasher,
            key,
            tip: (genesis_hash, Hash32([1; 32]), Hash32([2; 32])),
        }
    }

    /// The next core block over the chain's tip, its QC and the staged executed block.
    fn next(chain: &Chain) -> (Block, Qc, StagedBlock) {
        let (iroha_parent, parent_hash, parent_result) = chain.tip;
        let height = chain.store.height() + 1;
        let executed = executed_block(&chain.key, height, Some(iroha_parent));
        let payload = executed
            .canonical_resultless_proposal()
            .encode_wire()
            .expect("wire");
        let header = BlockHeader {
            instance: INSTANCE,
            height,
            origin_view: 0,
            parent_hash,
            parent_result,
            payload_hash: payload_hash(&*chain.hasher, &payload),
            payload_len: u32::try_from(payload.len()).expect("len"),
            proposer: 0,
            skipped_leaders: Vec::new(),
            attest: false,
        };
        let block = Block { header, payload };
        let result_preimage = vec![u8::try_from(height).expect("small"); 40];
        let qc = Qc {
            kind: VoteKind::Commit,
            instance: INSTANCE,
            height,
            view: 0,
            block_hash: block.hash(&*chain.hasher),
            result: result_of_preimage(&result_preimage),
            attest: false,
            signers: Bitmap::from_indices(1, [0]).expect("bitmap"),
            agg_sig: AggregateSignature([3; SIGNATURE_LEN]),
            attestations: Vec::new(),
        };
        let staged = StagedBlock {
            block_hash: qc.block_hash,
            executed: Arc::new(executed),
            result_preimage,
        };
        (block, qc, staged)
    }

    fn commit(chain: &mut Chain) -> (Block, Qc) {
        let (block, qc, staged) = next(chain);
        let iroha_hash = staged.executed.hash();
        chain.store.staging().stage(staged);
        chain.store.append(&block, &qc).expect("append");
        chain.tip = (iroha_hash, qc.block_hash, qc.result);
        (block, qc)
    }

    #[test]
    fn append_writes_one_certified_frame_and_entry_reproduces_it() {
        let mut chain = chain();
        assert_eq!(chain.store.height(), 1);
        assert_eq!(chain.store.entry(1), None, "genesis has no core entry");
        assert_eq!(chain.store.tip(), None);
        let (block, qc) = commit(&mut chain);
        assert_eq!(chain.store.height(), 2);
        let entry = chain.store.entry(2).expect("entry");
        assert_eq!(entry.block, block);
        assert_eq!(entry.commit_qc, qc);
        assert_eq!(chain.store.tip(), Some(entry));
        // The on-disk frame is one canonical SignedBlockWire carrying the certificate.
        let wire = chain
            .store
            .kura
            .canonical_block_wire_bytes_for_testing(NonZeroUsize::new(2).unwrap())
            .expect("frame");
        let decoded = decode_framed_signed_block(&wire).expect("decode frame");
        let certificate = decoded.commit_certificate().expect("certificate");
        assert_eq!(
            decode_certificate(certificate).expect("parts"),
            (block.header.clone(), qc.clone())
        );
        assert_eq!(result_of_preimage(&certificate.result_preimage), qc.result);
        let reread = stored_entry(&decoded, 2, &chain.hasher).expect("entry from frame");
        assert_eq!(reread.block, block);
        // The certificate changes neither the iroha block hash nor the executed wire hash.
        let staged_hash = decoded.clone().with_commit_certificate(None);
        assert_eq!(decoded.hash(), staged_hash.hash());
        assert_eq!(
            decoded.executed_block_wire_hash().unwrap(),
            staged_hash.executed_block_wire_hash().unwrap()
        );
    }

    #[test]
    fn a_stored_empty_payload_is_corrupt() {
        let block = executed_block(&KeyPair::random(), 2, None);
        assert!(matches!(
            derive_payload(&block, 0),
            Err(BlockStoreError::EmptyPayload)
        ));
    }

    #[test]
    fn recent_headers_entries_and_a_fresh_store_over_the_same_kura() {
        let mut chain = chain();
        let blocks: Vec<_> = (0..4).map(|_| commit(&mut chain).0).collect();
        assert_eq!(chain.store.height(), 5);
        let headers: Vec<_> = blocks.iter().map(|b| b.header.clone()).collect();
        assert_eq!(
            chain.store.recent_headers(2).expect("headers"),
            headers[2..]
        );
        assert_eq!(chain.store.recent_headers(100).expect("headers"), headers);
        assert_eq!(chain.store.header(3), Some(headers[1].clone()));
        let all = chain.store.entries(2, 10, u32::MAX);
        assert_eq!(all.len(), 4);
        assert_eq!(chain.store.entries(4, 10, u32::MAX).len(), 2);
        assert_eq!(chain.store.entries(6, 10, u32::MAX), Vec::new());
        assert_eq!(
            chain.store.entries(2, 10, 1).len(),
            1,
            "one oversized entry"
        );
        // A restart: a new store over the same Kura reads the same chain.
        let reopened = KuraBlockStore::new(
            Arc::clone(&chain.store.kura),
            Arc::clone(&chain.hasher),
            1,
            Staging::new(),
        );
        assert_eq!(reopened.height(), 5);
        assert_eq!(reopened.entries(2, 10, u32::MAX), all);
        // The driver assembles Init from it.
        let init = crate::sumeragi::driver::assemble_init(
            &reopened,
            INSTANCE,
            1,
            (Hash32([1; 32]), Hash32([2; 32])),
            1,
            Vec::new(),
            Vec::new(),
            9,
        )
        .expect("init");
        assert_eq!(init.tip.height, 5);
        assert_eq!(init.tip.header.as_ref(), Some(&headers[3]));
        assert_eq!(init.recent_headers, headers[1..]);
    }

    #[test]
    fn append_is_idempotent_and_refuses_conflicts_gaps_and_unstaged_blocks() {
        let mut chain = chain();
        let (block, qc) = commit(&mut chain);
        // An exact retry succeeds without a second write.
        chain.store.append(&block, &qc).expect("retry");
        assert_eq!(chain.store.height(), 2);
        // Another block at a stored height conflicts.
        let (mut other, mut other_qc, _) = next(&chain);
        other.header.height = 2;
        other_qc.height = 2;
        other_qc.block_hash = other.hash(&*chain.hasher);
        assert!(chain.store.append(&other, &other_qc).is_err());
        // Nothing staged for the next block.
        let (block, qc, staged) = next(&chain);
        assert!(chain.store.append(&block, &qc).is_err());
        // A QC for another block.
        chain.store.staging().stage(staged.clone());
        let mut wrong_qc = qc.clone();
        wrong_qc.block_hash = Hash32([9; 32]);
        assert!(chain.store.append(&block, &wrong_qc).is_err());
        // A gap.
        let mut gap = block.clone();
        gap.header.height = 4;
        let mut gap_qc = qc.clone();
        gap_qc.height = 4;
        gap_qc.block_hash = gap.hash(&*chain.hasher);
        chain.store.staging().stage(StagedBlock {
            block_hash: gap_qc.block_hash,
            ..staged.clone()
        });
        assert!(chain.store.append(&gap, &gap_qc).is_err());
        // A staged result preimage that does not hash to the certified result.
        chain.store.staging().stage(StagedBlock {
            result_preimage: vec![0; 3],
            ..staged.clone()
        });
        assert!(chain.store.append(&block, &qc).is_err());
        assert_eq!(chain.store.height(), 2, "nothing was written");
        chain.store.staging().stage(staged);
        chain.store.append(&block, &qc).expect("the real block");
        assert_eq!(chain.store.height(), 3);
    }

    #[test]
    fn payload_mismatch_is_a_failed_write_never_a_stored_block() {
        let mut chain = chain();
        commit(&mut chain);
        let (mut block, _, staged) = next(&chain);
        // A header whose payload is not the stored frame's proposal wire (a local bug).
        block.payload = b"not the proposal".to_vec();
        block.header.payload_hash = payload_hash(&*chain.hasher, &block.payload);
        block.header.payload_len = u32::try_from(block.payload.len()).unwrap();
        let qc = Qc {
            block_hash: block.hash(&*chain.hasher),
            ..next(&chain).1
        };
        chain.store.staging().stage(StagedBlock {
            block_hash: qc.block_hash,
            ..staged
        });
        let error = chain.store.write(&block, &qc).expect_err("mismatch");
        assert!(matches!(
            error,
            BlockStoreError::PayloadMismatch { height: 3 }
        ));
        assert_eq!(io::Error::from(error).kind(), io::ErrorKind::Other);
        assert_eq!(chain.store.height(), 2);
        assert_eq!(chain.store.entry(3), None);
    }

    #[test]
    fn staging_hands_off_only_the_matching_block() {
        let staging = Staging::new();
        let staged = StagedBlock {
            block_hash: Hash32([1; 32]),
            executed: Arc::new(executed_block(&KeyPair::random(), 2, None)),
            result_preimage: vec![1],
        };
        assert!(staging.get(&Hash32([1; 32])).is_none());
        staging.stage(staged);
        assert!(staging.get(&Hash32([2; 32])).is_none());
        assert_eq!(
            staging
                .get(&Hash32([1; 32]))
                .expect("staged")
                .result_preimage,
            vec![1]
        );
        staging.clear();
        assert!(staging.get(&Hash32([1; 32])).is_none());
    }

    #[test]
    fn certificate_parts_round_trip_and_reject_garbage() {
        let chain = chain();
        let (block, qc, _) = next(&chain);
        let certificate = commit_certificate(&block.header, &qc, vec![5]).expect("encode");
        assert_eq!(
            decode_certificate(&certificate).expect("decode"),
            (block.header.clone(), qc)
        );
        let garbage = CommitCertificate::new(vec![1, 2, 3], Vec::new(), Vec::new());
        assert!(decode_certificate(&garbage).is_err());
        // A stored block without a certificate is not an entry.
        let uncertified = executed_block(&KeyPair::random(), 2, None);
        assert!(matches!(
            stored_entry(&uncertified, 2, &chain.hasher),
            Err(BlockStoreError::MissingCertificate { height: 2 })
        ));
        assert!(matches!(
            derive_payload(&uncertified, 0),
            Err(BlockStoreError::EmptyPayload)
        ));
        assert_eq!(
            derive_payload(&uncertified, 1).expect("wire"),
            uncertified
                .canonical_resultless_proposal()
                .encode_wire()
                .unwrap()
        );
    }
}
