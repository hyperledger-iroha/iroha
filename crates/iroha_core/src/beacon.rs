//! Epoch randomness beacon scaffolding built on top of BLS‑based VRF.
//!
//! This module provides minimal helpers to:
//! - derive a canonical VRF input for an epoch from finalized chain data; and
//! - aggregate a set of VRF outputs into a single 32‑byte beacon value.
//!
//! Notes and roadmap
//! - This is a deterministic, on‑chain verifiable construction. It does not by
//!   itself implement commit‑reveal, DKG or slashing; consensus must enforce
//!   participation and penalties.
//! - Input derivation deliberately excludes current block contents to remove
//!   grinding opportunities for the current proposer.

use iroha_crypto::Hash;
use iroha_data_model::NetworkId;

/// Build the canonical epoch VRF input bytes.
///
/// Layout: `b"iroha:beacon:v1" || network_id[32] || epoch_be || prev_finalized_hash` where
/// - `network_id` is the exact genesis-derived deployment identity;
/// - `epoch_be` is the big‑endian encoding of the epoch number;
/// - `prev_finalized_hash` is the 32‑byte block hash anchoring this epoch.
pub fn epoch_input(network_id: &NetworkId, epoch: u64, prev_finalized_hash: [u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(16 + 32 + 8 + 32);
    v.extend_from_slice(b"iroha:beacon:v1");
    v.extend_from_slice(network_id.as_bytes());
    v.extend_from_slice(&epoch.to_be_bytes());
    v.extend_from_slice(&prev_finalized_hash);
    v
}

/// Build the canonical leader‑election VRF input (slot‑bound, pk‑bound).
///
/// Layout: `b"iroha:vrf:v1:input|leader|" || network_id[32] || epoch_be || slot_be || prev_finalized_hash || pk_bytes`
pub fn leader_input(
    network_id: &NetworkId,
    epoch: u64,
    slot: u64,
    prev_finalized_hash: [u8; 32],
    pk_bytes: &[u8],
) -> Vec<u8> {
    let mut v = Vec::with_capacity(24 + 32 + 8 + 8 + 32 + pk_bytes.len());
    v.extend_from_slice(b"iroha:vrf:v1:input|leader|");
    v.extend_from_slice(network_id.as_bytes());
    v.extend_from_slice(&epoch.to_be_bytes());
    v.extend_from_slice(&slot.to_be_bytes());
    v.extend_from_slice(&prev_finalized_hash);
    v.extend_from_slice(pk_bytes);
    v
}

/// Aggregate a set of per‑validator VRF outputs deterministically.
///
/// Construction: `Hash(b"iroha:beacon:v1:agg" || network_id[32] || sort(outputs))` where sorting
/// is lexicographic on the raw 32‑byte outputs. This prevents order‑based
/// malleability and yields identical results across peers.
pub fn aggregate_outputs(network_id: &NetworkId, mut outputs: Vec<[u8; 32]>) -> [u8; 32] {
    outputs.sort_unstable();
    outputs.dedup();
    let mut buf = Vec::with_capacity(16 + 32 + outputs.len() * 32);
    buf.extend_from_slice(b"iroha:beacon:v1:agg");
    buf.extend_from_slice(network_id.as_bytes());
    for y in outputs {
        buf.extend_from_slice(&y);
    }
    *Hash::new(&buf).as_ref()
}

/// Aggregate outputs with metadata binding: committee root and a reveal bitmap.
///
/// Layout: `b"iroha:beacon:v1:agg|" || network_id[32] || epoch_be || committee_root || bitmap_len_be || bitmap_bytes || concat(sort_lex(y_i))`
pub fn aggregate_outputs_with_meta(
    network_id: &NetworkId,
    epoch: u64,
    committee_root: [u8; 32],
    reveal_bitmap: &[u8],
    mut outputs: Vec<[u8; 32]>,
) -> [u8; 32] {
    outputs.sort_unstable();
    outputs.dedup();
    let mut buf =
        Vec::with_capacity(24 + 32 + 8 + 32 + 8 + reveal_bitmap.len() + outputs.len() * 32);
    buf.extend_from_slice(b"iroha:beacon:v1:agg|");
    buf.extend_from_slice(network_id.as_bytes());
    buf.extend_from_slice(&epoch.to_be_bytes());
    buf.extend_from_slice(&committee_root);
    buf.extend_from_slice(&(reveal_bitmap.len() as u64).to_be_bytes());
    buf.extend_from_slice(reveal_bitmap);
    for y in outputs {
        buf.extend_from_slice(&y);
    }
    *Hash::new(&buf).as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::HashOf;
    use iroha_data_model::block::BlockHeader;

    fn network_id(marker: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([marker; Hash::LENGTH]),
        ))
    }

    #[test]
    fn epoch_input_has_domain_and_sizes() {
        let prev = [7u8; 32];
        let network_id = network_id(0x81);
        let x = epoch_input(&network_id, 42, prev);
        assert!(x.starts_with(b"iroha:beacon:v1"));
        assert_eq!(x.len(), b"iroha:beacon:v1".len() + 32 + 8 + 32);
    }

    #[test]
    fn aggregate_is_order_independent() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let c = [3u8; 32];
        let network_id = network_id(0x81);
        let r1 = aggregate_outputs(&network_id, vec![a, b, c]);
        let r2 = aggregate_outputs(&network_id, vec![c, a, b]);
        assert_eq!(r1, r2);
    }

    #[test]
    fn aggregate_deduplicates_outputs() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let network_id = network_id(0x81);
        let r1 = aggregate_outputs(&network_id, vec![a, b]);
        let r2 = aggregate_outputs(&network_id, vec![a, b, a, b]);
        assert_eq!(r1, r2, "duplicate VRF outputs must not skew the beacon");
    }

    #[test]
    fn leader_input_binds_pk_and_slot() {
        let network_id = network_id(0x81);
        let prev = [7u8; 32];
        let pk = vec![5u8; 48];
        let x = leader_input(&network_id, 42, 9, prev, &pk);
        assert!(x.starts_with(b"iroha:vrf:v1:input|leader|"));
        assert_eq!(
            x.len(),
            b"iroha:vrf:v1:input|leader|".len() + 32 + 8 + 8 + 32 + pk.len()
        );
    }
    #[test]
    fn aggregate_with_meta_changes_with_bitmap() {
        let network_id = network_id(0x81);
        let out = [[1u8; 32], [2u8; 32]].to_vec();
        let r1 = aggregate_outputs_with_meta(&network_id, 1, [9u8; 32], &[0b11], out.clone());
        let r2 = aggregate_outputs_with_meta(&network_id, 1, [9u8; 32], &[0b01], out);
        assert_ne!(r1, r2);
    }

    #[test]
    fn aggregate_with_meta_deduplicates_outputs() {
        let network_id = network_id(0x81);
        let base = aggregate_outputs_with_meta(&network_id, 7, [9u8; 32], &[0b11], vec![[1u8; 32]]);
        let duped = aggregate_outputs_with_meta(
            &network_id,
            7,
            [9u8; 32],
            &[0b11],
            vec![[1u8; 32], [1u8; 32]],
        );
        assert_eq!(base, duped);
    }

    #[test]
    fn every_beacon_domain_rejects_same_label_different_genesis_by_construction() {
        let first = network_id(0x81);
        let second = network_id(0x82);
        let prev = [7_u8; 32];
        let output = vec![[1_u8; 32]];
        assert_ne!(
            epoch_input(&first, 42, prev),
            epoch_input(&second, 42, prev)
        );
        assert_ne!(
            leader_input(&first, 42, 9, prev, &[5_u8; 48]),
            leader_input(&second, 42, 9, prev, &[5_u8; 48])
        );
        assert_ne!(
            aggregate_outputs(&first, output.clone()),
            aggregate_outputs(&second, output)
        );
    }
}
