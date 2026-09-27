//! SCCP v1 EIP-712 digests (spec §3.6).
//!
//! Everything a bridge key signs is EIP-712 typed data under one domain
//! `EIP712Domain(string name,string version,bytes32 salt)` with `name = "SCCP"`,
//! `version = "1"` and `salt` = the Taira `NetworkId`. The domain omits `chainId` and
//! `verifyingContract`: one attestation is valid on every destination, and destination binding
//! lives in the Merkle leaves and payload domains. A bridge key signs exactly two structs: the
//! block attestation ([`AttestationFieldsV1`]) and its own key proof of possession
//! ([`BridgeKeyPopFieldsV1`]).

use super::{
    constants::{
        ATTESTATION_TYPEHASH, BRIDGE_KEY_TYPEHASH, DOMAIN_TYPEHASH, MAX_BLOCK_LEAVES, NAME_HASH,
        VERSION_HASH,
    },
    hashes::{keccak256, word_address, word_u64},
};

unit_error! {
    /// Violations of the §3.6.1 attestation invariants.
    pub enum AttestationInvariantError {
        /// `messageCount = 0` but `sccpRoot ≠ 0`, or `messageCount > 0` but `sccpRoot = 0`.
        RootCountMismatch => "messageCount = 0 must coincide with sccpRoot = 0",
        /// `historySize = 0` but `historyRoot ≠ 0`, or the converse.
        HistoryMismatch => "historySize = 0 must coincide with historyRoot = 0",
        /// `messageCount > 512`.
        TooManyMessages => "messageCount exceeds 512",
    }
}

/// The ten fields of the `SccpAttestation` struct (§3.6.1), in type-string order.
///
/// TODO(ws15): add conversions to and from
/// `iroha_data_model::sccp::attestation::SccpAttestationStatementV1` once that type is final;
/// the core and attestor waves then sign and verify the data-model statement through this type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AttestationFieldsV1 {
    /// Taira block height `h`.
    pub height: u64,
    /// `NPoS` epoch of `h`.
    pub epoch: u64,
    /// `creation_time_ms` of block `h`.
    pub timestamp_ms: u64,
    /// `HashOf<BlockHeader>` of block `h`.
    pub block_hash: [u8; 32],
    /// Root of `h`'s commitment tree, or zero if `message_count = 0`.
    pub sccp_root: [u8; 32],
    /// Number of SCCP leaves committed by `h` (0..=512).
    pub message_count: u32,
    /// `history_root(history_size)`.
    pub history_root: [u8; 32],
    /// Number of SCCP-bearing blocks with height ≤ `h`.
    pub history_size: u64,
    /// Digest of the generation that signs `h`.
    pub roster_digest: [u8; 32],
    /// Digest of the successor generation at a rotation boundary, else zero.
    pub next_roster_digest: [u8; 32],
}

impl AttestationFieldsV1 {
    /// Check the §3.6.1 invariants every verifier enforces.
    ///
    /// # Errors
    ///
    /// Returns the first violated [`AttestationInvariantError`].
    pub fn check_invariants(&self) -> Result<(), AttestationInvariantError> {
        if (self.message_count == 0) != (self.sccp_root == [0; 32]) {
            return Err(AttestationInvariantError::RootCountMismatch);
        }
        if (self.history_size == 0) != (self.history_root == [0; 32]) {
            return Err(AttestationInvariantError::HistoryMismatch);
        }
        if self.message_count > MAX_BLOCK_LEAVES {
            return Err(AttestationInvariantError::TooManyMessages);
        }
        Ok(())
    }

    /// Whether this attestation hands off to a new generation (`nextRosterDigest ≠ 0`).
    #[must_use]
    pub fn is_rotation(&self) -> bool {
        self.next_roster_digest != [0; 32]
    }

    /// `hashStruct` = `keccak256(TYPEHASH ‖ word(field_1) ‖ … ‖ word(field_10))`.
    #[must_use]
    pub fn struct_hash(&self) -> [u8; 32] {
        keccak256(&[
            &ATTESTATION_TYPEHASH,
            &word_u64(self.height),
            &word_u64(self.epoch),
            &word_u64(self.timestamp_ms),
            &self.block_hash,
            &self.sccp_root,
            &word_u64(u64::from(self.message_count)),
            &self.history_root,
            &word_u64(self.history_size),
            &self.roster_digest,
            &self.next_roster_digest,
        ])
    }

    /// The EIP-712 digest a bridge key signs for this attestation.
    #[must_use]
    pub fn digest(&self, taira_network_id: &[u8; 32]) -> [u8; 32] {
        typed_data_digest(&domain_separator(taira_network_id), &self.struct_hash())
    }
}

/// The three fields of the `SccpBridgeKey` proof of possession (§3.6.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct BridgeKeyPopFieldsV1 {
    /// `keccak256` of the raw consensus public key bytes (48-byte compressed BLS-normal G1).
    pub peer_key_hash: [u8; 32],
    /// 20-byte address of the bridge key (§3.8).
    pub bridge_address: [u8; 20],
    /// Activation epoch as registered.
    pub activation_epoch: u64,
}

impl BridgeKeyPopFieldsV1 {
    /// `hashStruct` of the proof of possession.
    #[must_use]
    pub fn struct_hash(&self) -> [u8; 32] {
        keccak256(&[
            &BRIDGE_KEY_TYPEHASH,
            &self.peer_key_hash,
            &word_address(&self.bridge_address),
            &word_u64(self.activation_epoch),
        ])
    }

    /// The EIP-712 digest the bridge key signs as its proof of possession.
    #[must_use]
    pub fn digest(&self, taira_network_id: &[u8; 32]) -> [u8; 32] {
        typed_data_digest(&domain_separator(taira_network_id), &self.struct_hash())
    }
}

/// `peerKeyHash = keccak256(raw consensus public key bytes)` (§3.6.2).
#[must_use]
pub fn peer_key_hash(consensus_public_key: &[u8]) -> [u8; 32] {
    keccak256(&[consensus_public_key])
}

/// `DOMAIN_SEPARATOR = keccak256(DOMAIN_TYPEHASH ‖ NAME_HASH ‖ VERSION_HASH ‖ salt)`.
#[must_use]
pub fn domain_separator(taira_network_id: &[u8; 32]) -> [u8; 32] {
    keccak256(&[
        &DOMAIN_TYPEHASH,
        &NAME_HASH,
        &VERSION_HASH,
        taira_network_id,
    ])
}

/// `digest = keccak256(0x19 ‖ 0x01 ‖ DOMAIN_SEPARATOR ‖ hashStruct)`.
#[must_use]
pub fn typed_data_digest(domain_separator: &[u8; 32], struct_hash: &[u8; 32]) -> [u8; 32] {
    keccak256(&[&[0x19, 0x01], domain_separator, struct_hash])
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAIRA: [u8; 32] = [0x11; 32];

    fn attestation() -> AttestationFieldsV1 {
        AttestationFieldsV1 {
            height: 100,
            epoch: 2,
            timestamp_ms: 1_800_000_000_000,
            block_hash: [0xb1; 32],
            sccp_root: [0xc2; 32],
            message_count: 3,
            history_root: [0xd3; 32],
            history_size: 9,
            roster_digest: [0xe4; 32],
            next_roster_digest: [0; 32],
        }
    }

    #[test]
    fn domain_separator_formula() {
        let mut preimage = Vec::new();
        preimage.extend_from_slice(&DOMAIN_TYPEHASH);
        preimage.extend_from_slice(&NAME_HASH);
        preimage.extend_from_slice(&VERSION_HASH);
        preimage.extend_from_slice(&TAIRA);
        assert_eq!(preimage.len(), 128);
        assert_eq!(domain_separator(&TAIRA), keccak256(&[&preimage]));
        assert_ne!(domain_separator(&TAIRA), domain_separator(&[0x12; 32]));
    }

    #[test]
    fn attestation_struct_hash_is_eleven_words() {
        let fields = attestation();
        let mut preimage = Vec::new();
        preimage.extend_from_slice(&ATTESTATION_TYPEHASH);
        for word in [
            word_u64(100),
            word_u64(2),
            word_u64(1_800_000_000_000),
            [0xb1; 32],
            [0xc2; 32],
            word_u64(3),
            [0xd3; 32],
            word_u64(9),
            [0xe4; 32],
            [0; 32],
        ] {
            preimage.extend_from_slice(&word);
        }
        assert_eq!(preimage.len(), 11 * 32);
        assert_eq!(fields.struct_hash(), keccak256(&[&preimage]));
        let digest = fields.digest(&TAIRA);
        assert_eq!(
            digest,
            keccak256(&[
                &[0x19, 0x01],
                &domain_separator(&TAIRA),
                &fields.struct_hash()
            ])
        );
        let mut other = fields;
        other.epoch = 3;
        assert_ne!(other.digest(&TAIRA), digest);
        assert!(!fields.is_rotation());
        other.next_roster_digest = [1; 32];
        assert!(other.is_rotation());
    }

    #[test]
    fn bridge_key_digest_formula() {
        let pop = BridgeKeyPopFieldsV1 {
            peer_key_hash: peer_key_hash(&[0xaa; 48]),
            bridge_address: [0x5a; 20],
            activation_epoch: 4,
        };
        assert_eq!(pop.peer_key_hash, keccak256(&[&[0xaa; 48]]));
        let expected_struct = keccak256(&[
            &BRIDGE_KEY_TYPEHASH,
            &pop.peer_key_hash,
            &word_address(&[0x5a; 20]),
            &word_u64(4),
        ]);
        assert_eq!(pop.struct_hash(), expected_struct);
        assert_eq!(
            pop.digest(&TAIRA),
            typed_data_digest(&domain_separator(&TAIRA), &expected_struct)
        );
        // Attestation and PoP digests never coincide for equal words (different typehash).
        assert_ne!(pop.struct_hash(), attestation().struct_hash());
    }

    #[test]
    fn invariants() {
        assert_eq!(attestation().check_invariants(), Ok(()));
        let empty = AttestationFieldsV1 {
            sccp_root: [0; 32],
            message_count: 0,
            history_root: [0; 32],
            history_size: 0,
            ..attestation()
        };
        assert_eq!(empty.check_invariants(), Ok(()));
        let mut bad = attestation();
        bad.message_count = 0;
        assert_eq!(
            bad.check_invariants(),
            Err(AttestationInvariantError::RootCountMismatch)
        );
        let mut bad = attestation();
        bad.sccp_root = [0; 32];
        assert_eq!(
            bad.check_invariants(),
            Err(AttestationInvariantError::RootCountMismatch)
        );
        let mut bad = attestation();
        bad.history_root = [0; 32];
        assert_eq!(
            bad.check_invariants(),
            Err(AttestationInvariantError::HistoryMismatch)
        );
        let mut bad = attestation();
        bad.message_count = 513;
        assert_eq!(
            bad.check_invariants(),
            Err(AttestationInvariantError::TooManyMessages)
        );
        bad.message_count = 512;
        assert_eq!(bad.check_invariants(), Ok(()));
    }
}
