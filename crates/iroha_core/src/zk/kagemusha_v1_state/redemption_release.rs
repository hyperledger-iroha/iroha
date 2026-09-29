//! Compact public projection of a finalized KAGEMUSHA redemption settlement.
//!
//! A Torii operation status is much larger than the native sender command budget because it
//! carries complete consensus finality and an ordinary-write membership witness. The compact
//! projection below is hardware selector material only; it never authorizes release of a durable
//! redemption outbox entry, and Core exposes no redemption release path in this release.

use iroha_data_model::NetworkId;
use norito::codec::{Decode, Encode};

use super::{DigestV1, KAGEMUSHA_STATE_VERSION_V1, KagemushaStateErrorV1, canonical_sha256_digest};

/// Domain of the compact, Core-authenticated redemption terminal receipt.
pub const KAGEMUSHA_REDEMPTION_TERMINAL_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:device:v1:redemption-terminal-receipt";

/// Compact public projection of one fully authenticated redemption settlement.
///
/// This value is safe to pass through the bounded native command ABI, but decoding or hashing it
/// does not grant release authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::redemption_release::KagemushaRedemptionTerminalReceiptV1",
    frame = "iroha.kagemusha.device.v1.redemption-terminal-receipt"
)]
pub struct KagemushaRedemptionTerminalReceiptV1 {
    /// Sole first-release layout version.
    pub version: u16,
    /// Caller-pinned network that finalized the settlement.
    pub network_id: NetworkId,
    /// Exact idempotent Torii operation and native operation-index key.
    pub operation_id: DigestV1,
    /// Exact identity of the installed redemption voucher.
    pub redemption_id: DigestV1,
    /// Proof-derived one-use terminal nullifier consumed by consensus.
    pub terminal_nullifier: DigestV1,
    /// Digest of the byte-identical voucher retained in the native outbox.
    pub envelope_digest: DigestV1,
    /// Canonical digest of the reserve receipt proven under the finalized block.
    pub reserve_receipt_digest: DigestV1,
    /// Digest of the complete status after full caller-pinned finality verification.
    pub authenticated_status_digest: DigestV1,
    /// Finalized block height pinned by the caller.
    pub finalized_block_height: u64,
    /// Original finalized Iroha block hash; selector bytes alone are not authority.
    pub finalized_block_hash: DigestV1,
    /// Original native consensus header hash certified at this height.
    pub finalized_core_hash: DigestV1,
    /// Original native execution result commitment certified at this height.
    pub finalized_result: DigestV1,
}

impl KagemushaRedemptionTerminalReceiptV1 {
    /// Validate the compact selector and finality-anchor shape.
    ///
    /// This is intentionally structural and grants no release authority.
    ///
    /// # Errors
    ///
    /// Returns an error for a wrong version, zero identity, or malformed finality anchor.
    pub fn validate_shape(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.version != KAGEMUSHA_STATE_VERSION_V1
            || self.network_id.as_bytes() == &[0; 32]
            || self.finalized_block_height <= 1
            || [
                self.operation_id,
                self.redemption_id,
                self.terminal_nullifier,
                self.envelope_digest,
                self.reserve_receipt_digest,
                self.authenticated_status_digest,
                self.finalized_block_hash,
                self.finalized_core_hash,
                self.finalized_result,
            ]
            .contains(&[0; 32])
        {
            return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
        }
        Ok(())
    }

    /// Compute the canonical digest consumed by the operation-index tombstone and hardware op 12.
    ///
    /// # Errors
    ///
    /// Returns an error if the projection is malformed or canonical Norito encoding fails.
    pub fn canonical_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.validate_shape()?;
        canonical_sha256_digest(KAGEMUSHA_REDEMPTION_TERMINAL_RECEIPT_DOMAIN_V1, self)
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};

    use super::*;

    fn receipt() -> KagemushaRedemptionTerminalReceiptV1 {
        KagemushaRedemptionTerminalReceiptV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"kagemusha-redemption-terminal-receipt-network",
            ))),
            operation_id: [0x11; 32],
            redemption_id: [0x12; 32],
            terminal_nullifier: [0x13; 32],
            envelope_digest: [0x14; 32],
            reserve_receipt_digest: [0x15; 32],
            authenticated_status_digest: [0x16; 32],
            finalized_block_height: 7,
            finalized_block_hash: [0x17; 32],
            finalized_core_hash: [0x18; 32],
            finalized_result: [0x19; 32],
        }
    }

    #[test]
    fn captured_redemption_receipt_frame_identity() {
        let value = receipt();
        value.validate_shape().unwrap();
        let digest = value.canonical_digest().unwrap();
        println!(
            "NATIVE_KAGEMUSHA_REDEMPTION_SELECTOR_HEX={}",
            hex::encode(norito::encode_canonical(&value).unwrap())
        );
        super::super::outgoing_operation_index::frame_identity_tests::check(
            "KagemushaRedemptionTerminalReceiptV1",
            &value,
        );
        let _flags = norito::core::DecodeFlagsGuard::enter(
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
        );
        assert_eq!(value.canonical_digest().unwrap(), digest);
    }

    #[test]
    fn selector_requires_all_native_facts_and_refuses_unsigned_genesis_output() {
        let valid = receipt();
        for height in [0, 1] {
            let mut invalid = valid;
            invalid.finalized_block_height = height;
            assert!(invalid.validate_shape().is_err());
            assert!(invalid.canonical_digest().is_err());
        }
        let mutations: [fn(&mut KagemushaRedemptionTerminalReceiptV1); 3] = [
            |value| value.finalized_block_hash = [0; 32],
            |value| value.finalized_core_hash = [0; 32],
            |value| value.finalized_result = [0; 32],
        ];
        for mutate in mutations {
            let mut invalid = valid;
            mutate(&mut invalid);
            assert!(invalid.validate_shape().is_err());
        }
        let mut maximum_height = valid;
        maximum_height.finalized_block_height = u64::MAX;
        maximum_height.validate_shape().unwrap();
    }

    #[test]
    fn exact_terminal_receipt_retry_has_one_digest() {
        let receipt = receipt();
        assert_eq!(receipt.canonical_digest(), receipt.canonical_digest());
    }

    #[test]
    fn conflicting_terminal_receipt_changes_digest() {
        let receipt = receipt();
        let mut conflict = receipt;
        conflict.reserve_receipt_digest[0] ^= 1;
        assert_ne!(
            receipt.canonical_digest().expect("valid receipt"),
            conflict.canonical_digest().expect("valid conflict shape")
        );
        let mutations: [fn(&mut KagemushaRedemptionTerminalReceiptV1); 3] = [
            |value| value.finalized_block_hash[0] ^= 1,
            |value| value.finalized_core_hash[0] ^= 1,
            |value| value.finalized_result[0] ^= 1,
        ];
        for mutate in mutations {
            let mut conflict = receipt;
            mutate(&mut conflict);
            assert_ne!(
                receipt.canonical_digest().unwrap(),
                conflict.canonical_digest().unwrap()
            );
        }
    }
}
