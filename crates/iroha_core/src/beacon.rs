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

/// Build the canonical epoch VRF input bytes.
///
/// Layout: `b"iroha:beacon:v1" || epoch_be || prev_finalized_hash` where
/// - `epoch_be` is the big‑endian encoding of the epoch number;
/// - `prev_finalized_hash` is the 32‑byte block hash anchoring this epoch.
pub fn epoch_input(epoch: u64, prev_finalized_hash: [u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(16 + 8 + 32);
    v.extend_from_slice(b"iroha:beacon:v1");
    v.extend_from_slice(&epoch.to_be_bytes());
    v.extend_from_slice(&prev_finalized_hash);
    v
}

/// Build the canonical leader‑election VRF input (slot‑bound, pk‑bound).
///
/// Layout: `b"iroha:vrf:v1:input|leader|" || chain_id || b"|" || epoch_be || slot_be || prev_finalized_hash || pk_bytes`
pub fn leader_input(
    chain_id: &[u8],
    epoch: u64,
    slot: u64,
    prev_finalized_hash: [u8; 32],
    pk_bytes: &[u8],
) -> Vec<u8> {
    let mut v = Vec::with_capacity(24 + chain_id.len() + 1 + 8 + 8 + 32 + pk_bytes.len());
    v.extend_from_slice(b"iroha:vrf:v1:input|leader|");
    v.extend_from_slice(chain_id);
    v.push(b'|');
    v.extend_from_slice(&epoch.to_be_bytes());
    v.extend_from_slice(&slot.to_be_bytes());
    v.extend_from_slice(&prev_finalized_hash);
    v.extend_from_slice(pk_bytes);
    v
}

/// Aggregate a set of per‑validator VRF outputs deterministically.
///
/// Construction: `Hash(b"iroha:beacon:v1:agg" || sort(outputs))` where sorting
/// is lexicographic on the raw 32‑byte outputs. This prevents order‑based
/// malleability and yields identical results across peers.
pub fn aggregate_outputs(mut outputs: Vec<[u8; 32]>) -> [u8; 32] {
    outputs.sort_unstable();
    outputs.dedup();
    let mut buf = Vec::with_capacity(16 + outputs.len() * 32);
    buf.extend_from_slice(b"iroha:beacon:v1:agg");
    for y in outputs {
        buf.extend_from_slice(&y);
    }
    *Hash::new(&buf).as_ref()
}

/// Aggregate outputs with metadata binding: committee root and a reveal bitmap.
///
/// Layout: `b"iroha:beacon:v1:agg|" || chain_id || b"|" || epoch_be || committee_root || bitmap_len_be || bitmap_bytes || concat(sort_lex(y_i))`
pub fn aggregate_outputs_with_meta(
    chain_id: &[u8],
    epoch: u64,
    committee_root: [u8; 32],
    reveal_bitmap: &[u8],
    mut outputs: Vec<[u8; 32]>,
) -> [u8; 32] {
    outputs.sort_unstable();
    outputs.dedup();
    let mut buf = Vec::with_capacity(
        24 + chain_id.len() + 1 + 8 + 32 + 8 + reveal_bitmap.len() + outputs.len() * 32,
    );
    buf.extend_from_slice(b"iroha:beacon:v1:agg|");
    buf.extend_from_slice(chain_id);
    buf.push(b'|');
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

    #[test]
    fn epoch_input_has_domain_and_sizes() {
        let prev = [7u8; 32];
        let x = epoch_input(42, prev);
        assert!(x.starts_with(b"iroha:beacon:v1"));
        assert_eq!(x.len(), b"iroha:beacon:v1".len() + 8 + 32);
    }

    #[test]
    fn aggregate_is_order_independent() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let c = [3u8; 32];
        let r1 = aggregate_outputs(vec![a, b, c]);
        let r2 = aggregate_outputs(vec![c, a, b]);
        assert_eq!(r1, r2);
    }

    #[test]
    fn aggregate_deduplicates_outputs() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let r1 = aggregate_outputs(vec![a, b]);
        let r2 = aggregate_outputs(vec![a, b, a, b]);
        assert_eq!(r1, r2, "duplicate VRF outputs must not skew the beacon");
    }

    #[test]
    fn leader_input_binds_pk_and_slot() {
        let chain = b"chain-xyz";
        let prev = [7u8; 32];
        let pk = vec![5u8; 48];
        let x = leader_input(chain, 42, 9, prev, &pk);
        assert!(x.starts_with(b"iroha:vrf:v1:input|leader|chain-xyz|"));
        assert!(x.len() >= 16 + chain.len() + 1 + 8 + 8 + 32 + pk.len());
    }
    #[test]
    fn aggregate_with_meta_changes_with_bitmap() {
        let chain = b"chain-xyz";
        let out = [[1u8; 32], [2u8; 32]].to_vec();
        let r1 = aggregate_outputs_with_meta(chain, 1, [9u8; 32], &[0b11], out.clone());
        let r2 = aggregate_outputs_with_meta(chain, 1, [9u8; 32], &[0b01], out);
        assert_ne!(r1, r2);
    }

    #[test]
    fn aggregate_with_meta_deduplicates_outputs() {
        let chain = b"chain-xyz";
        let base = aggregate_outputs_with_meta(chain, 7, [9u8; 32], &[0b11], vec![[1u8; 32]]);
        let duped =
            aggregate_outputs_with_meta(chain, 7, [9u8; 32], &[0b11], vec![[1u8; 32], [1u8; 32]]);
        assert_eq!(base, duped);
    }
}
