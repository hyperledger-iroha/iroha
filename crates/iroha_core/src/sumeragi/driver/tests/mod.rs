//! Tests of the Sumeragi driver: the execution scheduler (O3, O4), the kernel (O1, O2, O5, O7),
//! the §13.5 conformance runs of the production kernel inside the `iroha_sumeragi` simulator
//! (F9, F13, F14, F27, F29, F32, F37 under O-AGR, O-SIGN, O-PBS, O-LIVE, O-MEM, plus the O2
//! kill at every write completion and an O4 answer oracle), and threaded runs of the full
//! driver over the fake backends (O9 with two instances in one process).

pub(super) mod fakes;

mod conformance;
mod kernel;
mod sched;
mod sim_host;
mod threaded;

use iroha_sumeragi::{
    message::{Block, BlockHeader, Qc, VoteKind},
    preimage::payload_hash,
    testing::FakeCrypto,
    types::{AggregateSignature, Bitmap, Hash32, SIGNATURE_LEN},
};

/// A block of `height` on `parent` (with result `parent_result`) carrying `payload`.
pub(super) fn block(height: u64, parent: Hash32, parent_result: Hash32, payload: Vec<u8>) -> Block {
    let crypto = FakeCrypto::new();
    Block {
        header: BlockHeader {
            instance: Hash32([5; 32]),
            height,
            origin_view: 0,
            parent_hash: parent,
            parent_result,
            payload_hash: payload_hash(&crypto, &payload),
            payload_len: u32::try_from(payload.len()).unwrap_or(u32::MAX),
            proposer: 0,
            skipped_leaders: Vec::new(),
            attest: false,
        },
        payload,
    }
}

/// The block hash under the fake crypto.
pub(super) fn hash(block: &Block) -> Hash32 {
    block.hash(&FakeCrypto::new())
}

/// A `CommitQC` of `block` certifying `result` (no signers: only the driver reads it here).
pub(super) fn commit_qc(block: &Block, result: Hash32) -> Qc {
    Qc {
        kind: VoteKind::Commit,
        instance: block.header.instance,
        height: block.header.height,
        view: 0,
        block_hash: hash(block),
        result,
        attest: false,
        signers: Bitmap::new(4),
        agg_sig: AggregateSignature([0; SIGNATURE_LEN]),
        attestations: Vec::new(),
    }
}
