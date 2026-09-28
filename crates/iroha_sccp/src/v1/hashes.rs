//! SCCP v1 payload hash, message id and leaves (spec §3.3, §3.4, §3.5).
//!
//! `payload_hash`, `message_id`, `transfer_leaf`, `control_leaf`, the internal `node` hash and
//! `history_leaf`, plus the EIP-712 `word(x)` helpers of §0. Every hash is Ethereum Keccak-256
//! over an explicit big-endian preimage.

use iroha_data_model::bridge::SccpNetworkV1;
use tiny_keccak::{Hasher as _, Keccak};

use super::{
    constants::{CONTROL_TAG, HISTORY_TAG, LEAF_TAG, MESSAGE_TAG, NODE_TAG, PAYLOAD_TAG},
    network::lane_bytes,
};

/// Length of the control-leaf preimage (15 + 66 + 32 + 4 + 8 + 1 bytes).
pub const CONTROL_LEAF_PREIMAGE_BYTES: usize = 126;

/// Ethereum Keccak-256 over the concatenation of `parts`.
#[must_use]
pub fn keccak256(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    for part in parts {
        hasher.update(part);
    }
    let mut out = [0_u8; 32];
    hasher.finalize(&mut out);
    out
}

/// `word(x)` of an unsigned integer: left-zero-padded big-endian 32 bytes.
#[must_use]
pub fn word_u128(value: u128) -> [u8; 32] {
    let mut word = [0_u8; 32];
    word[16..].copy_from_slice(&value.to_be_bytes());
    word
}

/// `word(x)` of a `u64`.
#[must_use]
pub fn word_u64(value: u64) -> [u8; 32] {
    word_u128(u128::from(value))
}

/// `word(x)` of an `address`: twelve zero bytes then the 20 address bytes.
#[must_use]
pub fn word_address(address: &[u8; 20]) -> [u8; 32] {
    let mut word = [0_u8; 32];
    word[12..].copy_from_slice(address);
    word
}

/// `word(x)` of a `bool`: `uint256` 0 or 1.
#[must_use]
pub fn word_bool(value: bool) -> [u8; 32] {
    word_u64(u64::from(value))
}

unit_error! {
    /// Errors of the leaf constructors.
    pub enum LeafError {
        /// The control target is not an external profile.
        TargetNotExternal => "control leaf target must be an external network",
        /// `route_revision` is zero.
        ZeroRevision => "route revision must be nonzero",
        /// `control_nonce` is zero.
        ZeroControlNonce => "control nonce must be at least 1",
    }
}

/// `payload_hash = keccak256("SCCP/PAYLOAD/V1" ‖ payload)` (§3.3).
#[must_use]
pub fn payload_hash(payload: &[u8]) -> [u8; 32] {
    keccak256(&[PAYLOAD_TAG, payload])
}

/// `message_id = keccak256("SCCP/MESSAGE/V1" ‖ lane_bytes ‖ payload_hash)` (§3.3).
#[must_use]
pub fn message_id(lane: &[u8; 66], payload_hash: &[u8; 32]) -> [u8; 32] {
    keccak256(&[MESSAGE_TAG, lane, payload_hash])
}

/// `transfer_leaf = keccak256("SCCP/LEAF/V1" ‖ message_id ‖ destination_word)` (§3.4).
#[must_use]
pub fn transfer_leaf(message_id: &[u8; 32], destination_word: &[u8; 32]) -> [u8; 32] {
    keccak256(&[LEAF_TAG, message_id, destination_word])
}

/// The 126-byte control-leaf preimage of §3.4.
///
/// # Errors
///
/// Returns [`LeafError`] when `target` is Taira, `route_revision` is zero or `control_nonce`
/// is zero.
pub fn control_leaf_preimage(
    taira_network_id: &[u8; 32],
    target: SccpNetworkV1,
    destination_word: &[u8; 32],
    route_revision: u32,
    control_nonce: u64,
    paused: bool,
) -> Result<[u8; CONTROL_LEAF_PREIMAGE_BYTES], LeafError> {
    if !target.is_external() {
        return Err(LeafError::TargetNotExternal);
    }
    if route_revision == 0 {
        return Err(LeafError::ZeroRevision);
    }
    if control_nonce == 0 {
        return Err(LeafError::ZeroControlNonce);
    }
    let lane = lane_bytes(SccpNetworkV1::SoraTaira, target, taira_network_id)
        .ok_or(LeafError::TargetNotExternal)?;
    let mut out = [0_u8; CONTROL_LEAF_PREIMAGE_BYTES];
    out[..15].copy_from_slice(CONTROL_TAG);
    out[15..81].copy_from_slice(&lane);
    out[81..113].copy_from_slice(destination_word);
    out[113..117].copy_from_slice(&route_revision.to_be_bytes());
    out[117..125].copy_from_slice(&control_nonce.to_be_bytes());
    out[125] = u8::from(paused);
    Ok(out)
}

/// `control_leaf` of §3.4 for a Parliament-enacted destination pause state.
///
/// # Errors
///
/// See [`control_leaf_preimage`].
pub fn control_leaf(
    taira_network_id: &[u8; 32],
    target: SccpNetworkV1,
    destination_word: &[u8; 32],
    route_revision: u32,
    control_nonce: u64,
    paused: bool,
) -> Result<[u8; 32], LeafError> {
    let preimage = control_leaf_preimage(
        taira_network_id,
        target,
        destination_word,
        route_revision,
        control_nonce,
        paused,
    )?;
    Ok(keccak256(&[&preimage]))
}

/// Lowercase hex of `bytes` without a prefix (key file names, logs and diagnostics).
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(2 * bytes.len());
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

/// `node(l, r) = keccak256("SCCP/NODE/V1" ‖ l ‖ r)` (§3.4).
#[must_use]
pub fn node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    keccak256(&[NODE_TAG, left, right])
}

/// `history_leaf = keccak256("SCCP/HISTORY/V1" ‖ u64 height ‖ sccp_root ‖ u32 message_count)`.
#[must_use]
pub fn history_leaf(height: u64, sccp_root: &[u8; 32], message_count: u32) -> [u8; 32] {
    keccak256(&[
        HISTORY_TAG,
        &height.to_be_bytes(),
        sccp_root,
        &message_count.to_be_bytes(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        to_hex(bytes)
    }

    #[test]
    fn to_hex_is_lowercase_and_unprefixed() {
        assert_eq!(to_hex(&[]), "");
        assert_eq!(to_hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    }

    fn example_destination() -> [u8; 32] {
        word_address(&[0x22; 20])
    }

    #[test]
    fn keccak256_known_vectors() {
        assert_eq!(
            hex(&keccak256(&[])),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
        // Concatenation is part-boundary independent.
        assert_eq!(keccak256(&[b"ab", b"c"]), keccak256(&[b"abc"]));
    }

    #[test]
    fn word_helpers_pad_big_endian() {
        assert_eq!(word_u64(1)[31], 1);
        assert_eq!(word_u64(1)[..31], [0; 31]);
        assert_eq!(word_u128(u128::MAX)[..16], [0; 16]);
        assert_eq!(word_u128(u128::MAX)[16..], [0xff; 16]);
        assert_eq!(word_bool(true), word_u64(1));
        assert_eq!(word_bool(false), [0; 32]);
        let word = word_address(&[0x22; 20]);
        assert_eq!(word[..12], [0; 12]);
        assert_eq!(word[12..], [0x22; 20]);
    }

    #[test]
    fn control_leaf_spec_examples() {
        let network = [0x11; 32];
        let destination = example_destination();
        let pause = control_leaf(
            &network,
            SccpNetworkV1::EthereumMainnet,
            &destination,
            1,
            1,
            true,
        )
        .unwrap();
        assert_eq!(
            hex(&pause),
            "93d641053e51b4d28f40930e098212e8b6ab340ceaaf8ad38a458203ce98c662"
        );
        let resume = control_leaf(
            &network,
            SccpNetworkV1::EthereumMainnet,
            &destination,
            1,
            2,
            false,
        )
        .unwrap();
        assert_eq!(
            hex(&resume),
            "85fcfc8718c845c212df8ae486161d03bde8116eaa7dd74bbb2312667bce5cdf"
        );
    }

    #[test]
    fn control_leaf_preimage_layout() {
        let preimage = control_leaf_preimage(
            &[0x11; 32],
            SccpNetworkV1::TonMainnet,
            &[0x33; 32],
            0x0102_0304,
            0x0506_0708_090a_0b0c,
            true,
        )
        .unwrap();
        assert_eq!(&preimage[..15], b"SCCP/CONTROL/V1");
        assert_eq!(preimage[15], 0x40);
        assert_eq!(preimage[16..48], [0x11; 32]);
        assert_eq!(preimage[48], 0x44);
        assert_eq!(preimage[49..80], [0xff; 31]);
        assert_eq!(preimage[80], 0x11);
        assert_eq!(preimage[81..113], [0x33; 32]);
        assert_eq!(preimage[113..117], [1, 2, 3, 4]);
        assert_eq!(preimage[117..125], [5, 6, 7, 8, 9, 10, 11, 12]);
        assert_eq!(preimage[125], 1);
    }

    #[test]
    fn control_leaf_rejects_bad_fields() {
        let destination = example_destination();
        assert_eq!(
            control_leaf(
                &[0x11; 32],
                SccpNetworkV1::SoraTaira,
                &destination,
                1,
                1,
                true
            ),
            Err(LeafError::TargetNotExternal)
        );
        assert_eq!(
            control_leaf(
                &[0x11; 32],
                SccpNetworkV1::BscMainnet,
                &destination,
                0,
                1,
                true
            ),
            Err(LeafError::ZeroRevision)
        );
        assert_eq!(
            control_leaf(
                &[0x11; 32],
                SccpNetworkV1::BscMainnet,
                &destination,
                1,
                0,
                true
            ),
            Err(LeafError::ZeroControlNonce)
        );
    }

    #[test]
    fn leaf_kinds_are_domain_separated() {
        let a = [0xaa; 32];
        let b = [0xbb; 32];
        // Transfer leaves and internal nodes share a 76-byte length but differ in prefix.
        assert_ne!(transfer_leaf(&a, &b), node(&a, &b));
        assert_eq!(transfer_leaf(&a, &b), keccak256(&[b"SCCP/LEAF/V1", &a, &b]));
        assert_eq!(node(&a, &b), keccak256(&[b"SCCP/NODE/V1", &a, &b]));
        assert_ne!(node(&a, &b), node(&b, &a));
    }

    #[test]
    fn payload_hash_and_message_id_follow_the_formula() {
        let payload = [1_u8, 2, 3];
        let hash = payload_hash(&payload);
        assert_eq!(hash, keccak256(&[b"SCCP/PAYLOAD/V1", &payload]));
        let lane = [0x5a; 66];
        assert_eq!(
            message_id(&lane, &hash),
            keccak256(&[b"SCCP/MESSAGE/V1", &lane, &hash])
        );
    }

    #[test]
    fn history_leaf_layout() {
        let root = [0x77; 32];
        let expected = keccak256(&[
            b"SCCP/HISTORY/V1",
            &[0, 0, 0, 0, 0, 0, 0x01, 0x02],
            &root,
            &[0, 0, 0, 3],
        ]);
        assert_eq!(history_leaf(0x0102, &root, 3), expected);
        assert_ne!(history_leaf(0x0102, &root, 4), expected);
    }
}
