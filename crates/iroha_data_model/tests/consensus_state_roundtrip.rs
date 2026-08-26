//! Encode/decode roundtrip tests for consensus persistence records.
use core::convert::TryFrom;
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{CertPhase, Qc, QcAggregate},
        consensus_v2::PERMISSIONED_TAG,
    },
    peer::PeerId,
};
use norito::codec::{Decode, Encode};
fn sample_hash(seed: u8) -> Hash {
    let mut bytes = [0u8; Hash::LENGTH];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        let idx_u8 = u8::try_from(idx).expect("hash length fits into u8");
        *byte = seed.wrapping_add(idx_u8);
    }
    Hash::prehashed(bytes)
}
fn sample_block_hash(seed: u8) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(sample_hash(seed))
}
fn assert_roundtrip<T>(value: &T)
where
    T: Encode + Decode + PartialEq + core::fmt::Debug,
{
    let bytes = value.encode();
    let mut cursor = bytes.as_slice();
    let decoded = T::decode(&mut cursor).expect("decode succeeds");
    assert!(cursor.is_empty(), "decode must consume all bytes");
    assert_eq!(decoded, *value, "roundtrip must preserve value");
}
fn checked_random_peer_id() -> PeerId {
    PeerId::from(
        KeyPair::try_random()
            .expect("generate checked consensus-state peer keypair")
            .public_key()
            .clone(),
    )
}
#[test]
fn qc_roundtrip() {
    let validator_set = vec![checked_random_peer_id(), checked_random_peer_id()];
    let qc = Qc {
        phase: CertPhase::Commit,
        subject_block_hash: sample_block_hash(0x10),
        parent_state_root: sample_hash(0x11),
        post_state_root: sample_hash(0x20),
        height: 42,
        view: 7,
        epoch: 3,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: 1,
        validator_set,
        aggregate: QcAggregate {
            signers_bitmap: vec![0xAA, 0x55],
            bls_aggregate_signature: vec![0x90, 0x91, 0x92, 0x93],
        },
    };
    assert_roundtrip(&qc);
}
