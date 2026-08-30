//! Move-only split-decryption to corrected-source writer corridor.
//!
//! Admission requires a split-decryption result whose verified profile digest
//! is exactly the corrected 40-limb V2 candidate.  The current runtime
//! decryption verifier still reports the legacy 38-limb digest, so it is
//! rejected rather than relabelled.  Once a corrected verifier exists, this
//! state machine writes one recovered polynomial directly into 512 canonical
//! source chunks, accepts the three 128-block encryption witnesses and nonce
//! in the fixed order, and seals only after all 43 records.
//!
//! Receipts bind the local source-domain checks but remain non-authorizing.
//! The exact ordered 43-receipt set can additionally be bound to one complete
//! RNS-native public transcript as a move-only future-verifier input. That
//! input still does not prove the RLWE ciphertext/source equations;
//! persistent equality and release availability remain false.

use super::{
    ZkAmsMkheRnsNativePublicCiphertextIdentityManifestV1,
    decryption::{ZkAmsMkheDecryptedPlaintextV1, ZkAmsMkheStreamingFullRosterDecryptionResultV1},
    manifest::ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1, ZK_AMS_MKHE_RNS_NATIVE_RESIDUAL_BITS_V1,
        zk_ams_mkhe_rns_native_profile_manifest_v1,
    },
    rns_native_profile_authority_v2::{
        ZkAmsMkheRnsNativeProfileAuthorityV2, ZkAmsMkheRnsNativeProfileGenerationV2,
        resolve_zk_ams_mkhe_rns_native_profile_authority_v2,
    },
    rns_native_source::{
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1,
        ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1, ZkAmsMkheRnsNativeSecretChunkV1,
        ZkAmsMkheRnsNativeSourceArenaV1, ZkAmsMkheRnsNativeSourceErrorV1,
        ZkAmsMkheRnsNativeSourceLayoutV1, ZkAmsMkheRnsNativeSourceReceiptV1,
        ZkAmsMkheRnsNativeSourceSnapshotV1, ZkAmsMkheRnsNativeSourceWriterV1,
    },
    rns_native_transcript::ZkAmsMkheRnsNativeChallengeSeedsV1,
};
use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, sponge::Keccak256};

const CORRIDOR_VERSION_V2: u8 = 2;
const CANONICAL_COEFFICIENT_BYTES_V2: usize = 32;
const CANONICAL_COEFFICIENTS_PER_BLOCK_V2: usize =
    ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize / CANONICAL_COEFFICIENT_BYTES_V2;
const CANONICAL_BLOCKS_PER_RECORD_V2: u16 = 512;
const WITNESS_BLOCKS_PER_POLYNOMIAL_V2: u16 = 128;
const WITNESS_POLYNOMIAL_COUNT_V2: u16 = 3;
const WITNESS_BLOCKS_PER_RECORD_V2: u16 =
    WITNESS_BLOCKS_PER_POLYNOMIAL_V2 * WITNESS_POLYNOMIAL_COUNT_V2;
const RECORD_RECEIPT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-split-decryption-source-record";
const ORDERED_RECORD_ROOT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-split-decryption-source-record-root";
const SEAL_RECEIPT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-split-decryption-source-seal";
const CIPHERTEXT_CORRESPONDENCE_ROOT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-split-decryption-ciphertext-correspondence-root";
const CIPHERTEXT_CORRESPONDENCE_RECORD_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-split-decryption-ciphertext-correspondence-record";
const CIPHERTEXT_EQUALITY_INPUT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-split-decryption-ciphertext-equality-input";
/// Exact number of ordered split-decryption receipts consumed by the V2 equality input.
pub const ZK_AMS_MKHE_RNS_NATIVE_SPLIT_DECRYPTION_RECORD_COUNT_V2: usize =
    ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 as usize;

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 == 43);
    assert!(CANONICAL_COEFFICIENTS_PER_BLOCK_V2 == 256);
    assert!(
        CANONICAL_BLOCKS_PER_RECORD_V2 as usize * CANONICAL_COEFFICIENTS_PER_BLOCK_V2
            == ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
    );
    assert!(WITNESS_BLOCKS_PER_RECORD_V2 == 384);
    assert!(
        (CANONICAL_BLOCKS_PER_RECORD_V2 + WITNESS_BLOCKS_PER_RECORD_V2) as u64
            == ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1
    );
};

/// Failure in the versioned split-decryption/source writer corridor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2 {
    /// A legacy or otherwise foreign decryption profile was supplied.
    ProfileGenerationMismatch,
    /// Record order or verified public binding is invalid.
    InvalidRecord,
    /// The recovered plaintext is not one exact T256 ring polynomial.
    InvalidPlaintext,
    /// A witness block was supplied out of the fixed `r,e0,e1` order.
    UnexpectedWitness,
    /// A witness coefficient or nonce violates the native source relation.
    InvalidWitness,
    /// The exact 43-record chronology is incomplete.
    Incomplete,
    /// The confidential source backend rejected an operation.
    Source(ZkAmsMkheRnsNativeSourceErrorV1),
}

impl core::fmt::Display for ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::ProfileGenerationMismatch => "split decryption profile generation mismatch",
            Self::InvalidRecord => "invalid split-decryption source record",
            Self::InvalidPlaintext => "invalid split-decryption plaintext owner",
            Self::UnexpectedWitness => "unexpected split-decryption source witness block",
            Self::InvalidWitness => "invalid split-decryption source witness",
            Self::Incomplete => "incomplete 43-record split-decryption source",
            Self::Source(_) => "confidential split-decryption source failure",
        })
    }
}

impl std::error::Error for ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2 {}

impl From<ZkAmsMkheRnsNativeSourceErrorV1> for ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2 {
    fn from(error: ZkAmsMkheRnsNativeSourceErrorV1) -> Self {
        Self::Source(error)
    }
}

/// Fixed encryption-witness order following each recovered plaintext.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheRnsNativeEncryptionWitnessKindV2 {
    /// Ternary encryption ephemeral `r`.
    Ephemeral = 1,
    /// First bounded encryption error `e0`.
    ErrorZero = 2,
    /// Second bounded encryption error `e1`.
    ErrorOne = 3,
}

impl ZkAmsMkheRnsNativeEncryptionWitnessKindV2 {
    const fn from_absolute_block_v2(block: u16) -> Option<(Self, u16)> {
        let kind = match block / WITNESS_BLOCKS_PER_POLYNOMIAL_V2 {
            0 => Self::Ephemeral,
            1 => Self::ErrorZero,
            2 => Self::ErrorOne,
            _ => return None,
        };
        Some((kind, block % WITNESS_BLOCKS_PER_POLYNOMIAL_V2))
    }
}

/// Move-only corrected-profile decryption owner admitted for source writing.
#[must_use = "dropping the verified record zeroizes its recovered plaintext"]
pub struct ZkAmsMkheRnsNativeVerifiedSplitDecryptionRecordV2 {
    result: ZkAmsMkheStreamingFullRosterDecryptionResultV1,
}

impl ZkAmsMkheRnsNativeVerifiedSplitDecryptionRecordV2 {
    /// Consume a real split-decryption result after exact V2 profile admission.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch`]
    /// unless every result and provider axis belongs to the corrected profile.
    pub fn admit_v2(
        result: ZkAmsMkheStreamingFullRosterDecryptionResultV1,
        authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
    ) -> Result<Self, ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
        authority.validate().map_err(|_| {
            ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch
        })?;
        let snapshot = result.snapshot();
        if authority.release_available()
            || result.profile_digest() != authority.profile_digest()
            || result.ciphertext_digest() == [0; 32]
            || result.ciphertext_record_index()
                >= u32::from(ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1)
            || result.result().ordered_share_set_digest == [0; 32]
            || result.result().maximum_residual_bits > ZK_AMS_MKHE_RNS_NATIVE_RESIDUAL_BITS_V1
            || snapshot.provider_identity() == [0; 32]
            || snapshot.snapshot_identity() == [0; 32]
            || snapshot.provider_identity() == snapshot.snapshot_identity()
        {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch);
        }
        Ok(Self { result })
    }

    /// Exact ciphertext/source record ordinal.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.result.ciphertext_record_index()
    }
}

/// Source-domain receipt for one completely written record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2 {
    version: u8,
    profile_authority_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    record_index: u8,
    ciphertext_digest: [u8; 32],
    decryption_provider_identity: [u8; 32],
    decryption_snapshot_identity: [u8; 32],
    ordered_share_set_digest: [u8; 32],
    maximum_residual_bits: u16,
    canonical_block_count: u16,
    witness_block_count: u16,
    canonical_plaintext_verified: bool,
    witness_ranges_verified: bool,
    ephemeral_nonzero: bool,
    nonce_written: bool,
    nonce_nonzero: bool,
    persistent_equality_verified: bool,
    release_available: bool,
    receipt_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2 {
    /// Revalidate the exact record shape against its live source layout.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2`] when any
    /// profile, source, record, semantic, equality, release, or digest field
    /// differs from the exact non-authorizing receipt language.
    pub fn validate(
        self,
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
    ) -> Result<(), ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
        layout.validate()?;
        authority.validate().map_err(|_| {
            ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch
        })?;
        if self.version != CORRIDOR_VERSION_V2
            || self.profile_authority_digest != authority.authority_digest()
            || self.source_binding_digest != layout.source_binding_digest()
            || u32::from(self.record_index) >= u32::from(ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1)
            || self.ciphertext_digest == [0; 32]
            || self.decryption_provider_identity == [0; 32]
            || self.decryption_snapshot_identity == [0; 32]
            || self.decryption_provider_identity == self.decryption_snapshot_identity
            || self.ordered_share_set_digest == [0; 32]
            || self.maximum_residual_bits > ZK_AMS_MKHE_RNS_NATIVE_RESIDUAL_BITS_V1
            || self.canonical_block_count != CANONICAL_BLOCKS_PER_RECORD_V2
            || self.witness_block_count != WITNESS_BLOCKS_PER_RECORD_V2
            || !self.canonical_plaintext_verified
            || !self.witness_ranges_verified
            || !self.ephemeral_nonzero
            || !self.nonce_written
            || !self.nonce_nonzero
            || self.persistent_equality_verified
            || self.release_available
            || self.receipt_digest == [0; 32]
            || self.receipt_digest != record_receipt_digest_v2(self)
        {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord);
        }
        Ok(())
    }

    /// Zero-based record ordinal.
    #[must_use]
    pub const fn record_index(self) -> u8 {
        self.record_index
    }

    /// Corrected-profile authority identity admitted for this record.
    #[must_use]
    pub const fn profile_authority_digest(self) -> [u8; 32] {
        self.profile_authority_digest
    }

    /// Exact confidential-source identity receiving this record.
    #[must_use]
    pub const fn source_binding_digest(self) -> [u8; 32] {
        self.source_binding_digest
    }

    /// Authenticated ciphertext identity verified by split decryption.
    #[must_use]
    pub const fn ciphertext_digest(self) -> [u8; 32] {
        self.ciphertext_digest
    }

    /// Authenticated decryption provider identity.
    #[must_use]
    pub const fn decryption_provider_identity(self) -> [u8; 32] {
        self.decryption_provider_identity
    }

    /// Immutable decryption snapshot identity.
    #[must_use]
    pub const fn decryption_snapshot_identity(self) -> [u8; 32] {
        self.decryption_snapshot_identity
    }

    /// Ordered all-eight decryption-share set identity.
    #[must_use]
    pub const fn ordered_share_set_digest(self) -> [u8; 32] {
        self.ordered_share_set_digest
    }

    /// Verified maximum centered residual width.
    #[must_use]
    pub const fn maximum_residual_bits(self) -> u16 {
        self.maximum_residual_bits
    }

    /// Digest of this non-authorizing source-domain receipt.
    #[must_use]
    pub const fn receipt_digest(self) -> [u8; 32] {
        self.receipt_digest
    }

    /// Whether every local source-domain check for this record passed.
    #[must_use]
    pub const fn source_domain_checks_verified(self) -> bool {
        self.canonical_plaintext_verified
            && self.witness_ranges_verified
            && self.ephemeral_nonzero
            && self.nonce_written
            && self.nonce_nonzero
    }

    /// Whether ciphertext/source equality was proven for this record.
    #[must_use]
    pub const fn persistent_equality_verified(self) -> bool {
        self.persistent_equality_verified
    }

    /// Whether this record can authorize release.
    #[must_use]
    pub const fn release_available(self) -> bool {
        self.release_available
    }
}

struct PendingDecryptionRecordV2 {
    ciphertext_digest: [u8; 32],
    decryption_provider_identity: [u8; 32],
    decryption_snapshot_identity: [u8; 32],
    ordered_share_set_digest: [u8; 32],
    maximum_residual_bits: u16,
}

fn plaintext_coefficients_are_canonical_v2(coefficients: &[[u8; 32]]) -> bool {
    coefficients.len() == ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
        && coefficients
            .iter()
            .all(|coefficient| coefficient < &VEGA_T256_SCALAR_MODULUS_BE_V1)
}

fn validate_witness_chunk_v2(
    kind: ZkAmsMkheRnsNativeEncryptionWitnessKindV2,
    bytes: &[u8],
) -> Result<bool, ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
    if bytes.len() != ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize {
        return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness);
    }
    let mut any_nonzero = false;
    for encoded in bytes.chunks_exact(core::mem::size_of::<i64>()) {
        let coefficient = i64::from_be_bytes(
            encoded
                .try_into()
                .map_err(|_| ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness)?,
        );
        let valid = match kind {
            ZkAmsMkheRnsNativeEncryptionWitnessKindV2::Ephemeral => (-1..=1).contains(&coefficient),
            ZkAmsMkheRnsNativeEncryptionWitnessKindV2::ErrorZero
            | ZkAmsMkheRnsNativeEncryptionWitnessKindV2::ErrorOne => {
                (-2..=2).contains(&coefficient)
            }
        };
        if !valid {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness);
        }
        any_nonzero |= coefficient != 0;
    }
    Ok(any_nonzero)
}

fn validate_nonce_chunk_v2(
    bytes: &[u8],
) -> Result<(), ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
    if bytes.len() != ZkAmsMkheRnsNativeSourceArenaV1::Nonce.plaintext_bytes() as usize
        || bytes.iter().all(|byte| *byte == 0)
    {
        return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness);
    }
    Ok(())
}

/// Move-only writer which admits exact corrected-profile decryption records.
#[must_use = "dropping this writer seals no source snapshot"]
pub struct ZkAmsMkheRnsNativeSplitDecryptionSourceWriterV2<W: ZkAmsMkheRnsNativeSourceWriterV1> {
    writer: W,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
    next_record: u8,
    ordered_record_hash: Keccak256,
}

impl<W: ZkAmsMkheRnsNativeSourceWriterV1> ZkAmsMkheRnsNativeSplitDecryptionSourceWriterV2<W> {
    /// Bind a live confidential writer to the corrected V2 candidate.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2`] if the
    /// layout, profile authority, or injected writer identity differs.
    pub fn begin_v2(
        writer: W,
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
    ) -> Result<Self, ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
        layout.validate()?;
        authority.validate().map_err(|_| {
            ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch
        })?;
        if authority.release_available()
            || layout.profile_digest() != authority.profile_digest()
            || writer.layout() != layout
        {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch);
        }
        let mut ordered_record_hash = Keccak256::new();
        ordered_record_hash.update(ORDERED_RECORD_ROOT_DOMAIN_V2);
        ordered_record_hash.update(&[CORRIDOR_VERSION_V2]);
        ordered_record_hash.update(&authority.authority_digest());
        ordered_record_hash.update(&layout.source_binding_digest());
        Ok(Self {
            writer,
            layout,
            authority,
            next_record: 0,
            ordered_record_hash,
        })
    }

    /// Write one verified plaintext directly into its 512 canonical chunks.
    ///
    /// No plaintext vector is returned, cloned, hashed, or detached.  The
    /// verified owner is consumed and zeroized after the last chunk is written.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2`] for a wrong
    /// record/profile, non-canonical plaintext, or rejected confidential write.
    pub fn write_verified_plaintext_v2(
        mut self,
        verified: ZkAmsMkheRnsNativeVerifiedSplitDecryptionRecordV2,
    ) -> Result<
        ZkAmsMkheRnsNativeEncryptionWitnessWriterV2<W>,
        ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2,
    > {
        if verified.result.profile_digest() != self.authority.profile_digest()
            || verified.result.ciphertext_record_index() != u32::from(self.next_record)
        {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord);
        }
        let snapshot = verified.result.snapshot();
        let pending = PendingDecryptionRecordV2 {
            ciphertext_digest: verified.result.ciphertext_digest(),
            decryption_provider_identity: snapshot.provider_identity(),
            decryption_snapshot_identity: snapshot.snapshot_identity(),
            ordered_share_set_digest: verified.result.result().ordered_share_set_digest,
            maximum_residual_bits: verified.result.result().maximum_residual_bits,
        };
        let result = verified.result.into_result();
        let coefficients = match &result.plaintext {
            ZkAmsMkheDecryptedPlaintextV1::T256(coefficients)
                if plaintext_coefficients_are_canonical_v2(coefficients) =>
            {
                coefficients
            }
            #[cfg(test)]
            ZkAmsMkheDecryptedPlaintextV1::Tiny(_) => {
                return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidPlaintext);
            }
            _ => {
                return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidPlaintext);
            }
        };
        for (block, block_coefficients) in coefficients
            .chunks_exact(CANONICAL_COEFFICIENTS_PER_BLOCK_V2)
            .enumerate()
        {
            let mut chunk = self
                .writer
                .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Main)?;
            if chunk.arena() != ZkAmsMkheRnsNativeSourceArenaV1::Main
                || chunk.as_mut_slice().len()
                    != ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize
            {
                return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidPlaintext);
            }
            for (destination, coefficient) in chunk
                .as_mut_slice()
                .chunks_exact_mut(CANONICAL_COEFFICIENT_BYTES_V2)
                .zip(block_coefficients)
            {
                destination.copy_from_slice(coefficient);
            }
            let absolute_slot = u64::from(self.next_record)
                * ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1
                + u64::try_from(block).map_err(|_| {
                    ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidPlaintext
                })?;
            self.writer
                .write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, absolute_slot, chunk)?;
        }
        drop(result);
        Ok(ZkAmsMkheRnsNativeEncryptionWitnessWriterV2 {
            live: Some(LiveWitnessWriterV2 {
                writer: self.writer,
                layout: self.layout,
                authority: self.authority,
                record_index: self.next_record,
                next_witness_block: 0,
                ephemeral_nonzero: false,
                ordered_record_hash: self.ordered_record_hash,
                pending,
            }),
        })
    }

    /// Seal only after the exact 43-record chronology is complete.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2`] when fewer
    /// than 43 records were written or the confidential backend cannot seal.
    pub fn seal_exact_v2(
        self,
    ) -> Result<
        ZkAmsMkheRnsNativeSplitDecryptionSourceSnapshotV2<W::Snapshot>,
        ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2,
    > {
        if self.next_record != ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::Incomplete);
        }
        let ordered_record_root = self.ordered_record_hash.finalize();
        let snapshot = self.writer.seal()?;
        let source_receipt = snapshot.structural_receipt()?;
        let receipt = ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2::new_v2(
            self.layout,
            self.authority,
            ordered_record_root,
            source_receipt,
        )?;
        Ok(ZkAmsMkheRnsNativeSplitDecryptionSourceSnapshotV2 { snapshot, receipt })
    }
}

struct LiveWitnessWriterV2<W: ZkAmsMkheRnsNativeSourceWriterV1> {
    writer: W,
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
    record_index: u8,
    next_witness_block: u16,
    ephemeral_nonzero: bool,
    ordered_record_hash: Keccak256,
    pending: PendingDecryptionRecordV2,
}

/// Poison-on-failure writer for the three signed witness polynomials and nonce.
#[must_use = "dropping this stage abandons the confidential source"]
pub struct ZkAmsMkheRnsNativeEncryptionWitnessWriterV2<W: ZkAmsMkheRnsNativeSourceWriterV1> {
    live: Option<LiveWitnessWriterV2<W>>,
}

impl<W: ZkAmsMkheRnsNativeSourceWriterV1> ZkAmsMkheRnsNativeEncryptionWitnessWriterV2<W> {
    /// Allocate one exact zeroed 8 KiB witness chunk.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2`] if this
    /// poison-on-failure stage is closed or the backend rejects allocation.
    pub fn allocate_witness_chunk_v2(
        &mut self,
    ) -> Result<W::Chunk, ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::Incomplete)?;
        match live
            .writer
            .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Main)
        {
            Ok(chunk) => {
                self.live = Some(live);
                Ok(chunk)
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Consume the exact next witness block in `r,e0,e1` order.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2`] for a closed
    /// stage, wrong coordinate/shape, out-of-range witness, or failed write.
    pub fn write_next_witness_block_v2(
        &mut self,
        kind: ZkAmsMkheRnsNativeEncryptionWitnessKindV2,
        block: u16,
        chunk: W::Chunk,
    ) -> Result<(), ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::Incomplete)?;
        let expected = ZkAmsMkheRnsNativeEncryptionWitnessKindV2::from_absolute_block_v2(
            live.next_witness_block,
        )
        .ok_or(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::UnexpectedWitness)?;
        if (kind, block) != expected
            || chunk.arena() != ZkAmsMkheRnsNativeSourceArenaV1::Main
            || chunk.as_slice().len()
                != ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize
        {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::UnexpectedWitness);
        }
        let chunk_nonzero = validate_witness_chunk_v2(kind, chunk.as_slice())?;
        if kind == ZkAmsMkheRnsNativeEncryptionWitnessKindV2::Ephemeral {
            live.ephemeral_nonzero |= chunk_nonzero;
        }
        let absolute_slot = u64::from(live.record_index)
            * ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1
            + u64::from(CANONICAL_BLOCKS_PER_RECORD_V2)
            + u64::from(live.next_witness_block);
        live.writer
            .write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, absolute_slot, chunk)?;
        live.next_witness_block = live
            .next_witness_block
            .checked_add(1)
            .ok_or(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::UnexpectedWitness)?;
        self.live = Some(live);
        Ok(())
    }

    /// Persist the nonce, mint the record receipt, and advance exactly once.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2`] unless every
    /// witness block is complete, `r` and the nonce are nonzero, and the final
    /// nonce write and receipt validation succeed.
    pub fn finish_record_v2(
        mut self,
        nonce: W::Chunk,
    ) -> Result<
        (
            ZkAmsMkheRnsNativeSplitDecryptionSourceWriterV2<W>,
            ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2,
        ),
        ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2,
    > {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::Incomplete)?;
        if live.next_witness_block != WITNESS_BLOCKS_PER_RECORD_V2
            || nonce.arena() != ZkAmsMkheRnsNativeSourceArenaV1::Nonce
            || nonce.as_slice().len()
                != ZkAmsMkheRnsNativeSourceArenaV1::Nonce.plaintext_bytes() as usize
        {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::Incomplete);
        }
        if !live.ephemeral_nonzero {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness);
        }
        validate_nonce_chunk_v2(nonce.as_slice())?;
        live.writer.write_slot(
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
            u64::from(live.record_index),
            nonce,
        )?;
        let mut receipt = ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2 {
            version: CORRIDOR_VERSION_V2,
            profile_authority_digest: live.authority.authority_digest(),
            source_binding_digest: live.layout.source_binding_digest(),
            record_index: live.record_index,
            ciphertext_digest: live.pending.ciphertext_digest,
            decryption_provider_identity: live.pending.decryption_provider_identity,
            decryption_snapshot_identity: live.pending.decryption_snapshot_identity,
            ordered_share_set_digest: live.pending.ordered_share_set_digest,
            maximum_residual_bits: live.pending.maximum_residual_bits,
            canonical_block_count: CANONICAL_BLOCKS_PER_RECORD_V2,
            witness_block_count: WITNESS_BLOCKS_PER_RECORD_V2,
            canonical_plaintext_verified: true,
            witness_ranges_verified: true,
            ephemeral_nonzero: true,
            nonce_written: true,
            nonce_nonzero: true,
            persistent_equality_verified: false,
            release_available: false,
            receipt_digest: [0; 32],
        };
        receipt.receipt_digest = record_receipt_digest_v2(receipt);
        receipt.validate(live.layout, live.authority)?;
        live.ordered_record_hash.update(&[live.record_index]);
        live.ordered_record_hash.update(&receipt.receipt_digest);
        let next_record = live
            .record_index
            .checked_add(1)
            .ok_or(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord)?;
        Ok((
            ZkAmsMkheRnsNativeSplitDecryptionSourceWriterV2 {
                writer: live.writer,
                layout: live.layout,
                authority: live.authority,
                next_record,
                ordered_record_hash: live.ordered_record_hash,
            },
            receipt,
        ))
    }
}

fn record_receipt_digest_v2(receipt: ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(RECORD_RECEIPT_DOMAIN_V2);
    hash.update(&[receipt.version]);
    hash.update(&receipt.profile_authority_digest);
    hash.update(&receipt.source_binding_digest);
    hash.update(&[receipt.record_index]);
    hash.update(&receipt.ciphertext_digest);
    hash.update(&receipt.decryption_provider_identity);
    hash.update(&receipt.decryption_snapshot_identity);
    hash.update(&receipt.ordered_share_set_digest);
    hash.update(&receipt.maximum_residual_bits.to_be_bytes());
    hash.update(&receipt.canonical_block_count.to_be_bytes());
    hash.update(&receipt.witness_block_count.to_be_bytes());
    hash.update(&[
        receipt.canonical_plaintext_verified.into(),
        receipt.witness_ranges_verified.into(),
        receipt.ephemeral_nonzero.into(),
        receipt.nonce_written.into(),
        receipt.nonce_nonzero.into(),
        receipt.persistent_equality_verified.into(),
        receipt.release_available.into(),
    ]);
    hash.finalize()
}

/// Final structural receipt for the exact 43-record confidential source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2 {
    version: u8,
    profile_authority_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    ordered_record_root: [u8; 32],
    source_receipt_digest: [u8; 32],
    record_count: u8,
    split_decryption_records_complete: bool,
    record_source_domain_checks_verified: bool,
    source_writer_complete: bool,
    persistent_equality_verified: bool,
    release_available: bool,
    receipt_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2 {
    fn new_v2(
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
        ordered_record_root: [u8; 32],
        source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
        let mut receipt = Self {
            version: CORRIDOR_VERSION_V2,
            profile_authority_digest: authority.authority_digest(),
            source_binding_digest: layout.source_binding_digest(),
            ordered_record_root,
            source_receipt_digest: source_receipt.receipt_digest,
            record_count: ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1,
            split_decryption_records_complete: true,
            record_source_domain_checks_verified: true,
            source_writer_complete: true,
            persistent_equality_verified: false,
            release_available: false,
            receipt_digest: [0; 32],
        };
        receipt.receipt_digest = seal_receipt_digest_v2(receipt);
        receipt.validate(layout, authority, source_receipt)?;
        Ok(receipt)
    }

    /// Validate the exact count, live source receipt, and closed stronger gates.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2`] if any exact
    /// 43-record/source identity differs or an equality/release bit is set.
    pub fn validate(
        self,
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
        source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    ) -> Result<(), ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
        layout.validate()?;
        authority.validate().map_err(|_| {
            ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch
        })?;
        source_receipt.validate(layout)?;
        if self.version != CORRIDOR_VERSION_V2
            || self.profile_authority_digest != authority.authority_digest()
            || self.source_binding_digest != layout.source_binding_digest()
            || self.ordered_record_root == [0; 32]
            || self.source_receipt_digest != source_receipt.receipt_digest
            || self.record_count != ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1
            || !self.split_decryption_records_complete
            || !self.record_source_domain_checks_verified
            || !self.source_writer_complete
            || self.persistent_equality_verified
            || self.release_available
            || self.receipt_digest == [0; 32]
            || self.receipt_digest != seal_receipt_digest_v2(self)
        {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord);
        }
        Ok(())
    }

    /// Non-authorizing receipt digest.
    #[must_use]
    pub const fn receipt_digest(self) -> [u8; 32] {
        self.receipt_digest
    }

    /// Exact number of completely written split-decryption records.
    #[must_use]
    pub const fn record_count(self) -> u8 {
        self.record_count
    }

    /// Whether every record passed the local plaintext/witness/nonce checks.
    #[must_use]
    pub const fn record_source_domain_checks_verified(self) -> bool {
        self.record_source_domain_checks_verified
    }

    /// Whether persistent ciphertext/source equality was verified.
    #[must_use]
    pub const fn persistent_equality_verified(self) -> bool {
        self.persistent_equality_verified
    }

    /// Whether this receipt can authorize release.
    #[must_use]
    pub const fn release_available(self) -> bool {
        self.release_available
    }
}

fn seal_receipt_digest_v2(
    receipt: ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SEAL_RECEIPT_DOMAIN_V2);
    hash.update(&[receipt.version]);
    hash.update(&receipt.profile_authority_digest);
    hash.update(&receipt.source_binding_digest);
    hash.update(&receipt.ordered_record_root);
    hash.update(&receipt.source_receipt_digest);
    hash.update(&[
        receipt.record_count,
        receipt.split_decryption_records_complete.into(),
        receipt.record_source_domain_checks_verified.into(),
        receipt.source_writer_complete.into(),
        receipt.persistent_equality_verified.into(),
        receipt.release_available.into(),
    ]);
    hash.finalize()
}

struct CiphertextEqualityInputSealV2;

/// Move-only, non-authorizing input for a future ciphertext/source equality verifier.
///
/// Construction consumes the exact ordered 43 record receipts, the exact
/// validated public-record identity manifest, and the final source seal. It
/// replays the receipt root, rejects ciphertext reuse, and binds each receipt's
/// native ciphertext digest to the same-ordinal public RLWE record digest. It
/// proves no RLWE equation: all equality and release flags remain false.
#[must_use = "dropping this value discards the bound ciphertext-equality input"]
pub struct ZkAmsMkheRnsNativeCiphertextEqualityInputV2 {
    _seal: CiphertextEqualityInputSealV2,
    version: u8,
    profile_authority_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    source_seal_receipt_digest: [u8; 32],
    transcript_digest: [u8; 32],
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    governed_roster_digest: [u8; 32],
    public_ciphertext_digest: [u8; 32],
    public_identity_manifest_digest: [u8; 32],
    ordered_record_root: [u8; 32],
    ordered_ciphertext_correspondence_root: [u8; 32],
    public_identity_manifest: ZkAmsMkheRnsNativePublicCiphertextIdentityManifestV1,
    records: Box<
        [ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2;
            ZK_AMS_MKHE_RNS_NATIVE_SPLIT_DECRYPTION_RECORD_COUNT_V2],
    >,
    record_count: u8,
    ciphertext_equations_verified: bool,
    persistent_equality_verified: bool,
    release_available: bool,
    input_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheRnsNativeCiphertextEqualityInputV2 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheRnsNativeCiphertextEqualityInputV2")
            .field(
                "ordered_ciphertext_correspondence_root",
                &hex::encode(self.ordered_ciphertext_correspondence_root),
            )
            .field("input_digest", &hex::encode(self.input_digest))
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheRnsNativeCiphertextEqualityInputV2 {
    /// Revalidate every retained context axis and both exact ordered roots.
    ///
    /// # Errors
    ///
    /// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord`]
    /// if any receipt, transcript, root, or closed equality flag differs from
    /// the exact input bound at construction.
    pub fn validate(
        &self,
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
        source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<(), ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
        validate_ciphertext_equality_input_context_v2(
            layout,
            authority,
            source_receipt,
            transcript,
        )?;
        self.public_identity_manifest
            .validate_v1(transcript, layout)
            .map_err(|_| ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord)?;
        validate_ciphertext_equality_record_receipts_v2(self.records.as_ref(), layout, authority)?;
        let expected_record_root = ordered_record_root_from_receipts_v2(
            authority.authority_digest(),
            layout.source_binding_digest(),
            self.records.as_ref(),
        );
        let expected_correspondence_root = ordered_ciphertext_correspondence_root_v2(
            authority.authority_digest(),
            layout.source_binding_digest(),
            self.source_seal_receipt_digest,
            transcript,
            &self.public_identity_manifest,
            self.records.as_ref(),
        )?;
        if self.version != CORRIDOR_VERSION_V2
            || self.profile_authority_digest != authority.authority_digest()
            || self.source_binding_digest != layout.source_binding_digest()
            || self.source_receipt_digest != source_receipt.receipt_digest
            || self.source_seal_receipt_digest == [0; 32]
            || self.transcript_digest != transcript.transcript_digest()
            || self.statement_digest != transcript.statement_digest()
            || self.operational_context_digest != transcript.operational_context_digest()
            || self.governed_roster_digest != transcript.governed_roster_digest()
            || self.public_ciphertext_digest != transcript.public_ciphertext_digest()
            || self.public_identity_manifest.public_bundle_digest()
                != transcript.public_ciphertext_digest()
            || self.public_identity_manifest_digest
                != self.public_identity_manifest.manifest_digest()
            || self.ordered_record_root != expected_record_root
            || self.ordered_ciphertext_correspondence_root != expected_correspondence_root
            || self.ordered_record_root == self.ordered_ciphertext_correspondence_root
            || self.record_count != ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1
            || self.ciphertext_equations_verified
            || self.persistent_equality_verified
            || self.release_available
            || self.input_digest == [0; 32]
            || self.input_digest != ciphertext_equality_input_digest_v2(self)
        {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord);
        }
        Ok(())
    }

    /// Exact one-to-one binding of the 43 public-record identities to the 43
    /// verified split-decryption ciphertext identities.
    #[must_use]
    pub const fn ordered_ciphertext_correspondence_root(&self) -> [u8; 32] {
        self.ordered_ciphertext_correspondence_root
    }

    /// Public-ciphertext bundle bound by the full RNS-native transcript.
    #[must_use]
    pub const fn public_ciphertext_digest(&self) -> [u8; 32] {
        self.public_ciphertext_digest
    }

    /// Digest of this non-authorizing equality-verifier input.
    #[must_use]
    pub const fn input_digest(&self) -> [u8; 32] {
        self.input_digest
    }

    /// Whether the RLWE ciphertext equations were proven.
    #[must_use]
    pub const fn ciphertext_equations_verified(&self) -> bool {
        self.ciphertext_equations_verified
    }

    /// Whether the persistent secret/share equality proof was verified.
    #[must_use]
    pub const fn persistent_equality_verified(&self) -> bool {
        self.persistent_equality_verified
    }

    /// Whether this input can authorize release.
    #[must_use]
    pub const fn release_available(&self) -> bool {
        self.release_available
    }
}

/// Bind the exact corrected-source record set to one RNS public transcript.
///
/// This closes only the missing typed input handoff. A later verifier must
/// still prove every ciphertext/source equation and consume this move-only
/// value before either equality audit bit can change.
///
/// # Errors
///
/// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2`] if the corrected
/// profile/source/transcript axes differ, a record is missing, reordered,
/// duplicated, or invalid, or the recomputed exact-43 root differs from the
/// final source seal.
pub fn bind_zk_ams_mkhe_rns_native_ciphertext_equality_input_v2(
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
    source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    source_seal_receipt: ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2,
    record_receipts: Box<
        [ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2;
            ZK_AMS_MKHE_RNS_NATIVE_SPLIT_DECRYPTION_RECORD_COUNT_V2],
    >,
    public_identity_manifest: ZkAmsMkheRnsNativePublicCiphertextIdentityManifestV1,
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> Result<
    ZkAmsMkheRnsNativeCiphertextEqualityInputV2,
    ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2,
> {
    validate_ciphertext_equality_input_context_v2(layout, authority, source_receipt, transcript)?;
    source_seal_receipt.validate(layout, authority, source_receipt)?;
    public_identity_manifest
        .validate_v1(transcript, layout)
        .map_err(|_| ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord)?;
    validate_ciphertext_equality_record_receipts_v2(record_receipts.as_ref(), layout, authority)?;
    let ordered_record_root = ordered_record_root_from_receipts_v2(
        authority.authority_digest(),
        layout.source_binding_digest(),
        record_receipts.as_ref(),
    );
    if ordered_record_root != source_seal_receipt.ordered_record_root {
        return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord);
    }
    let public_identity_manifest_digest = public_identity_manifest.manifest_digest();
    let ordered_ciphertext_correspondence_root = ordered_ciphertext_correspondence_root_v2(
        authority.authority_digest(),
        layout.source_binding_digest(),
        source_seal_receipt.receipt_digest,
        transcript,
        &public_identity_manifest,
        record_receipts.as_ref(),
    )?;
    let mut input = ZkAmsMkheRnsNativeCiphertextEqualityInputV2 {
        _seal: CiphertextEqualityInputSealV2,
        version: CORRIDOR_VERSION_V2,
        profile_authority_digest: authority.authority_digest(),
        source_binding_digest: layout.source_binding_digest(),
        source_receipt_digest: source_receipt.receipt_digest,
        source_seal_receipt_digest: source_seal_receipt.receipt_digest,
        transcript_digest: transcript.transcript_digest(),
        statement_digest: transcript.statement_digest(),
        operational_context_digest: transcript.operational_context_digest(),
        governed_roster_digest: transcript.governed_roster_digest(),
        public_ciphertext_digest: transcript.public_ciphertext_digest(),
        public_identity_manifest_digest,
        ordered_record_root,
        ordered_ciphertext_correspondence_root,
        public_identity_manifest,
        records: record_receipts,
        record_count: ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1,
        ciphertext_equations_verified: false,
        persistent_equality_verified: false,
        release_available: false,
        input_digest: [0; 32],
    };
    input.input_digest = ciphertext_equality_input_digest_v2(&input);
    input.validate(layout, authority, source_receipt, transcript)?;
    Ok(input)
}

fn validate_ciphertext_equality_input_context_v2(
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
    source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> Result<(), ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
    layout.validate()?;
    authority
        .validate()
        .map_err(|_| ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch)?;
    source_receipt.validate(layout)?;
    let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()
        .map_err(|_| ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch)?;
    if authority.release_available()
        || layout.profile_digest() != authority.profile_digest()
        || layout.topology_digest() != authority.topology_digest()
        || transcript.profile_manifest_digest() != manifest.manifest_digest
        || transcript.profile_digest() != authority.profile_digest()
        || transcript.topology_digest() != authority.topology_digest()
        || transcript.release_candidate_digest() != layout.release_candidate_digest()
        || transcript.statement_digest() != layout.statement_digest()
        || transcript.operational_context_digest() != layout.operational_context_digest()
        || transcript.source_binding_digest() != layout.source_binding_digest()
        || transcript.main_snapshot_digest() != source_receipt.main_snapshot_digest
        || transcript.nonce_snapshot_digest() != source_receipt.nonce_snapshot_digest
        || transcript.source_receipt_digest() != source_receipt.receipt_digest
        || transcript.governed_roster_digest() == [0; 32]
        || transcript.public_ciphertext_digest() == [0; 32]
        || transcript.transcript_digest() == [0; 32]
    {
        return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord);
    }
    Ok(())
}

fn validate_ciphertext_equality_record_receipts_v2(
    records: &[ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2;
         ZK_AMS_MKHE_RNS_NATIVE_SPLIT_DECRYPTION_RECORD_COUNT_V2],
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
) -> Result<(), ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
    for (record_index, record) in records.iter().enumerate() {
        record.validate(layout, authority)?;
        if usize::from(record.record_index) != record_index
            || records[..record_index].iter().any(|prior| {
                prior.receipt_digest == record.receipt_digest
                    || prior.ciphertext_digest == record.ciphertext_digest
            })
        {
            return Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord);
        }
    }
    Ok(())
}

fn ordered_record_root_from_receipts_v2(
    profile_authority_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    records: &[ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2;
         ZK_AMS_MKHE_RNS_NATIVE_SPLIT_DECRYPTION_RECORD_COUNT_V2],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(ORDERED_RECORD_ROOT_DOMAIN_V2);
    hash.update(&[CORRIDOR_VERSION_V2]);
    hash.update(&profile_authority_digest);
    hash.update(&source_binding_digest);
    for record in records {
        hash.update(&[record.record_index]);
        hash.update(&record.receipt_digest);
    }
    hash.finalize()
}

fn ordered_ciphertext_correspondence_root_v2(
    profile_authority_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    source_seal_receipt_digest: [u8; 32],
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    public_identity_manifest: &ZkAmsMkheRnsNativePublicCiphertextIdentityManifestV1,
    records: &[ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2;
         ZK_AMS_MKHE_RNS_NATIVE_SPLIT_DECRYPTION_RECORD_COUNT_V2],
) -> Result<[u8; 32], ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
    let mut hash = Keccak256::new();
    hash.update(CIPHERTEXT_CORRESPONDENCE_ROOT_DOMAIN_V2);
    hash.update(&[CORRIDOR_VERSION_V2]);
    hash.update(&profile_authority_digest);
    hash.update(&source_binding_digest);
    hash.update(&source_seal_receipt_digest);
    hash.update(&transcript.transcript_digest());
    hash.update(&transcript.statement_digest());
    hash.update(&transcript.operational_context_digest());
    hash.update(&transcript.governed_roster_digest());
    hash.update(&transcript.public_ciphertext_digest());
    hash.update(&public_identity_manifest.manifest_digest());
    for ((ordinal, record), public_record_digest) in records
        .iter()
        .enumerate()
        .zip(public_identity_manifest.record_digests_v1())
    {
        hash.update(&ciphertext_correspondence_record_digest_v2(
            u8::try_from(ordinal)
                .map_err(|_| ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord)?,
            public_record_digest,
            *record,
        ));
    }
    Ok(hash.finalize())
}

fn ciphertext_correspondence_record_digest_v2(
    ordinal: u8,
    public_record_digest: [u8; 32],
    record: ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(CIPHERTEXT_CORRESPONDENCE_RECORD_DOMAIN_V2);
    hash.update(&[CORRIDOR_VERSION_V2, ordinal, record.record_index]);
    hash.update(&public_record_digest);
    hash.update(&record.receipt_digest);
    hash.update(&record.ciphertext_digest);
    hash.update(&record.decryption_provider_identity);
    hash.update(&record.decryption_snapshot_identity);
    hash.update(&record.ordered_share_set_digest);
    hash.update(&record.maximum_residual_bits.to_be_bytes());
    hash.finalize()
}

fn ciphertext_equality_input_digest_v2(
    input: &ZkAmsMkheRnsNativeCiphertextEqualityInputV2,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(CIPHERTEXT_EQUALITY_INPUT_DOMAIN_V2);
    hash.update(&[input.version]);
    hash.update(&input.profile_authority_digest);
    hash.update(&input.source_binding_digest);
    hash.update(&input.source_receipt_digest);
    hash.update(&input.source_seal_receipt_digest);
    hash.update(&input.transcript_digest);
    hash.update(&input.statement_digest);
    hash.update(&input.operational_context_digest);
    hash.update(&input.governed_roster_digest);
    hash.update(&input.public_ciphertext_digest);
    hash.update(&input.public_identity_manifest_digest);
    hash.update(&input.ordered_record_root);
    hash.update(&input.ordered_ciphertext_correspondence_root);
    hash.update(&[
        input.record_count,
        input.ciphertext_equations_verified.into(),
        input.persistent_equality_verified.into(),
        input.release_available.into(),
    ]);
    hash.finalize()
}

/// Move-only sealed snapshot retaining the exact 43-record receipt.
#[must_use = "dropping this owner closes the confidential source snapshot"]
pub struct ZkAmsMkheRnsNativeSplitDecryptionSourceSnapshotV2<S: ZkAmsMkheRnsNativeSourceSnapshotV1>
{
    snapshot: S,
    receipt: ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2,
}

impl<S: ZkAmsMkheRnsNativeSourceSnapshotV1> ZkAmsMkheRnsNativeSplitDecryptionSourceSnapshotV2<S> {
    /// Borrow the non-authorizing exact-43 seal receipt.
    #[must_use]
    pub const fn seal_receipt(&self) -> &ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2 {
        &self.receipt
    }
}

impl<S: ZkAmsMkheRnsNativeSourceSnapshotV1> ZkAmsMkheRnsNativeSourceSnapshotV1
    for ZkAmsMkheRnsNativeSplitDecryptionSourceSnapshotV2<S>
{
    type Chunk = S::Chunk;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.snapshot.layout()
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
        self.snapshot.snapshot_digest(arena)
    }

    fn read_slot(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        self.snapshot.read_slot(arena, slot)
    }
}

impl<S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1> ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1
    for ZkAmsMkheRnsNativeSplitDecryptionSourceSnapshotV2<S>
{
}

/// Return the sole phase-0 authority accepted by this corridor.
///
/// This convenience remains non-authorizing and returns a record whose release
/// bit is false.
///
/// # Errors
///
/// Returns [`ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch`]
/// if the corrected phase-0 authority cannot be reconstructed exactly.
pub fn zk_ams_mkhe_rns_native_split_decryption_authority_v2()
-> Result<ZkAmsMkheRnsNativeProfileAuthorityV2, ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2> {
    resolve_zk_ams_mkhe_rns_native_profile_authority_v2(
        ZkAmsMkheRnsNativeProfileGenerationV2::Corrected40LimbV2,
    )
    .map_err(|_| ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::ProfileGenerationMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::zk_ams::mkhe::{
        manifest::release_profile_v1,
        rns_native_profile::{
            zk_ams_mkhe_rns_native_profile_v1, zk_ams_mkhe_rns_native_release_candidate_digest_v1,
            zk_ams_mkhe_rns_native_topology_v1,
        },
    };

    struct ReceiptChunkV2 {
        bytes: [u8; 0],
    }

    impl ZkAmsMkheRnsNativeSecretChunkV1 for ReceiptChunkV2 {
        fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1 {
            ZkAmsMkheRnsNativeSourceArenaV1::Main
        }

        fn as_slice(&self) -> &[u8] {
            &self.bytes
        }

        fn as_mut_slice(&mut self) -> &mut [u8] {
            &mut self.bytes
        }
    }

    struct ReceiptSnapshotV2 {
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    }

    impl ZkAmsMkheRnsNativeSourceSnapshotV1 for ReceiptSnapshotV2 {
        type Chunk = ReceiptChunkV2;

        fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
            self.layout
        }

        fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
            match arena {
                ZkAmsMkheRnsNativeSourceArenaV1::Main => [0x81; 32],
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce => [0x82; 32],
            }
        }

        fn read_slot(
            &mut self,
            _arena: ZkAmsMkheRnsNativeSourceArenaV1,
            _slot: u64,
        ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
            Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication)
        }
    }

    struct SemanticChunkV2 {
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        bytes: Vec<u8>,
    }

    impl ZkAmsMkheRnsNativeSecretChunkV1 for SemanticChunkV2 {
        fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1 {
            self.arena
        }

        fn as_slice(&self) -> &[u8] {
            &self.bytes
        }

        fn as_mut_slice(&mut self) -> &mut [u8] {
            &mut self.bytes
        }
    }

    struct SemanticSnapshotV2 {
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    }

    impl ZkAmsMkheRnsNativeSourceSnapshotV1 for SemanticSnapshotV2 {
        type Chunk = SemanticChunkV2;

        fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
            self.layout
        }

        fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
            match arena {
                ZkAmsMkheRnsNativeSourceArenaV1::Main => [0xa1; 32],
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce => [0xa2; 32],
            }
        }

        fn read_slot(
            &mut self,
            _arena: ZkAmsMkheRnsNativeSourceArenaV1,
            _slot: u64,
        ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
            Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication)
        }
    }

    struct SemanticWriterV2 {
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        next_main_slot: u64,
        nonce_written: bool,
    }

    impl ZkAmsMkheRnsNativeSourceWriterV1 for SemanticWriterV2 {
        type Chunk = SemanticChunkV2;
        type Snapshot = SemanticSnapshotV2;

        fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
            self.layout
        }

        fn allocate_chunk(
            &self,
            arena: ZkAmsMkheRnsNativeSourceArenaV1,
        ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
            Ok(SemanticChunkV2 {
                arena,
                bytes: vec![0; arena.plaintext_bytes() as usize],
            })
        }

        fn write_slot(
            &mut self,
            arena: ZkAmsMkheRnsNativeSourceArenaV1,
            slot: u64,
            chunk: Self::Chunk,
        ) -> Result<(), ZkAmsMkheRnsNativeSourceErrorV1> {
            if chunk.arena() != arena || chunk.as_slice().len() != arena.plaintext_bytes() as usize
            {
                return Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication);
            }
            match arena {
                ZkAmsMkheRnsNativeSourceArenaV1::Main if slot == self.next_main_slot => {
                    self.next_main_slot += 1;
                    Ok(())
                }
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce if slot == 0 && !self.nonce_written => {
                    self.nonce_written = true;
                    Ok(())
                }
                _ => Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication),
            }
        }

        fn seal(self) -> Result<Self::Snapshot, ZkAmsMkheRnsNativeSourceErrorV1> {
            Err(ZkAmsMkheRnsNativeSourceErrorV1::Incomplete)
        }
    }

    fn witness_stage_v2(
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
    ) -> ZkAmsMkheRnsNativeEncryptionWitnessWriterV2<SemanticWriterV2> {
        ZkAmsMkheRnsNativeEncryptionWitnessWriterV2 {
            live: Some(LiveWitnessWriterV2 {
                writer: SemanticWriterV2 {
                    layout,
                    next_main_slot: u64::from(CANONICAL_BLOCKS_PER_RECORD_V2),
                    nonce_written: false,
                },
                layout,
                authority,
                record_index: 0,
                next_witness_block: 0,
                ephemeral_nonzero: false,
                ordered_record_hash: {
                    let mut hash = Keccak256::new();
                    hash.update(ORDERED_RECORD_ROOT_DOMAIN_V2);
                    hash.update(&[CORRIDOR_VERSION_V2]);
                    hash.update(&authority.authority_digest());
                    hash.update(&layout.source_binding_digest());
                    hash
                },
                pending: PendingDecryptionRecordV2 {
                    ciphertext_digest: [0xb1; 32],
                    decryption_provider_identity: [0xb2; 32],
                    decryption_snapshot_identity: [0xb3; 32],
                    ordered_share_set_digest: [0xb4; 32],
                    maximum_residual_bits: ZK_AMS_MKHE_RNS_NATIVE_RESIDUAL_BITS_V1,
                },
            }),
        }
    }

    fn write_complete_witness_v2(
        stage: &mut ZkAmsMkheRnsNativeEncryptionWitnessWriterV2<SemanticWriterV2>,
        nonzero_ephemeral: bool,
    ) {
        for absolute in 0..WITNESS_BLOCKS_PER_RECORD_V2 {
            let (kind, block) =
                ZkAmsMkheRnsNativeEncryptionWitnessKindV2::from_absolute_block_v2(absolute)
                    .expect("fixed witness coordinate");
            let mut chunk = stage.allocate_witness_chunk_v2().expect("witness chunk");
            if nonzero_ephemeral && absolute == 0 {
                chunk.as_mut_slice()[..8].copy_from_slice(&1_i64.to_be_bytes());
            }
            stage
                .write_next_witness_block_v2(kind, block, chunk)
                .expect("bounded witness block");
        }
    }

    fn corrected_layout_v2() -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        let profile = zk_ams_mkhe_rns_native_profile_v1().expect("corrected profile");
        let topology = zk_ams_mkhe_rns_native_topology_v1().expect("corrected topology");
        ZkAmsMkheRnsNativeSourceLayoutV1::new(
            profile.profile_digest,
            topology.topology_digest,
            zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("candidate digest"),
            [0x83; 32],
            [0x84; 32],
        )
        .expect("corrected source layout")
    }

    fn valid_record_receipt_v2(
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        authority: ZkAmsMkheRnsNativeProfileAuthorityV2,
    ) -> ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2 {
        let mut receipt = ZkAmsMkheRnsNativeSplitDecryptionRecordReceiptV2 {
            version: CORRIDOR_VERSION_V2,
            profile_authority_digest: authority.authority_digest(),
            source_binding_digest: layout.source_binding_digest(),
            record_index: 7,
            ciphertext_digest: [0x91; 32],
            decryption_provider_identity: [0x92; 32],
            decryption_snapshot_identity: [0x93; 32],
            ordered_share_set_digest: [0x94; 32],
            maximum_residual_bits: ZK_AMS_MKHE_RNS_NATIVE_RESIDUAL_BITS_V1,
            canonical_block_count: CANONICAL_BLOCKS_PER_RECORD_V2,
            witness_block_count: WITNESS_BLOCKS_PER_RECORD_V2,
            canonical_plaintext_verified: true,
            witness_ranges_verified: true,
            ephemeral_nonzero: true,
            nonce_written: true,
            nonce_nonzero: true,
            persistent_equality_verified: false,
            release_available: false,
            receipt_digest: [0; 32],
        };
        receipt.receipt_digest = record_receipt_digest_v2(receipt);
        receipt
    }

    #[test]
    fn current_runtime_profile_cannot_be_relabelled_as_corrected_v2() {
        let authority = zk_ams_mkhe_rns_native_split_decryption_authority_v2()
            .expect("phase-0 corrected authority");
        assert_ne!(
            release_profile_v1().digest().expect("legacy digest"),
            authority.profile_digest()
        );
        assert!(!authority.release_available());
    }

    #[test]
    fn exact_record_geometry_is_frozen_at_43_times_896_plus_nonce() {
        assert_eq!(ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1, 43);
        assert_eq!(CANONICAL_BLOCKS_PER_RECORD_V2, 512);
        assert_eq!(WITNESS_BLOCKS_PER_RECORD_V2, 384);
        assert_eq!(
            u64::from(CANONICAL_BLOCKS_PER_RECORD_V2 + WITNESS_BLOCKS_PER_RECORD_V2),
            ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1
        );
    }

    #[test]
    fn plaintext_witness_and_nonce_source_domain_checks_reject_mutations() {
        let mut plaintext = vec![[0_u8; 32]; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
        assert!(plaintext_coefficients_are_canonical_v2(&plaintext));
        plaintext[ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 1] = VEGA_T256_SCALAR_MODULUS_BE_V1;
        assert!(!plaintext_coefficients_are_canonical_v2(&plaintext));
        assert!(!plaintext_coefficients_are_canonical_v2(&plaintext[..1]));

        let mut witness =
            vec![0_u8; ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize];
        assert_eq!(
            validate_witness_chunk_v2(
                ZkAmsMkheRnsNativeEncryptionWitnessKindV2::Ephemeral,
                &witness,
            ),
            Ok(false)
        );
        witness[..8].copy_from_slice(&(-1_i64).to_be_bytes());
        assert_eq!(
            validate_witness_chunk_v2(
                ZkAmsMkheRnsNativeEncryptionWitnessKindV2::Ephemeral,
                &witness,
            ),
            Ok(true)
        );
        witness[..8].copy_from_slice(&2_i64.to_be_bytes());
        assert_eq!(
            validate_witness_chunk_v2(
                ZkAmsMkheRnsNativeEncryptionWitnessKindV2::Ephemeral,
                &witness,
            ),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness)
        );
        witness[..8].copy_from_slice(&(-2_i64).to_be_bytes());
        assert_eq!(
            validate_witness_chunk_v2(
                ZkAmsMkheRnsNativeEncryptionWitnessKindV2::ErrorZero,
                &witness,
            ),
            Ok(true)
        );
        witness[..8].copy_from_slice(&3_i64.to_be_bytes());
        assert_eq!(
            validate_witness_chunk_v2(
                ZkAmsMkheRnsNativeEncryptionWitnessKindV2::ErrorOne,
                &witness,
            ),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness)
        );
        assert_eq!(
            validate_witness_chunk_v2(
                ZkAmsMkheRnsNativeEncryptionWitnessKindV2::ErrorOne,
                &witness[..8],
            ),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness)
        );

        assert_eq!(
            validate_nonce_chunk_v2(&[0_u8; 32]),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness)
        );
        assert_eq!(validate_nonce_chunk_v2(&[1_u8; 32]), Ok(()));
        assert_eq!(
            validate_nonce_chunk_v2(&[1_u8; 31]),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness)
        );
    }

    #[test]
    fn witness_state_machine_is_ordered_poisoning_and_semantic() {
        let authority = zk_ams_mkhe_rns_native_split_decryption_authority_v2()
            .expect("phase-0 corrected authority");
        let layout = corrected_layout_v2();

        let mut out_of_order = witness_stage_v2(layout, authority);
        let chunk = out_of_order
            .allocate_witness_chunk_v2()
            .expect("witness chunk");
        assert_eq!(
            out_of_order.write_next_witness_block_v2(
                ZkAmsMkheRnsNativeEncryptionWitnessKindV2::ErrorZero,
                0,
                chunk,
            ),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::UnexpectedWitness)
        );
        assert!(matches!(
            out_of_order.allocate_witness_chunk_v2(),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::Incomplete)
        ));

        let mut out_of_range = witness_stage_v2(layout, authority);
        let mut chunk = out_of_range
            .allocate_witness_chunk_v2()
            .expect("witness chunk");
        chunk.as_mut_slice()[..8].copy_from_slice(&2_i64.to_be_bytes());
        assert_eq!(
            out_of_range.write_next_witness_block_v2(
                ZkAmsMkheRnsNativeEncryptionWitnessKindV2::Ephemeral,
                0,
                chunk,
            ),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness)
        );
        assert!(matches!(
            out_of_range.allocate_witness_chunk_v2(),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::Incomplete)
        ));

        let mut all_zero_ephemeral = witness_stage_v2(layout, authority);
        write_complete_witness_v2(&mut all_zero_ephemeral, false);
        let mut nonce = SemanticChunkV2 {
            arena: ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
            bytes: vec![0; ZkAmsMkheRnsNativeSourceArenaV1::Nonce.plaintext_bytes() as usize],
        };
        nonce.as_mut_slice()[0] = 1;
        assert!(matches!(
            all_zero_ephemeral.finish_record_v2(nonce),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness)
        ));

        let mut zero_nonce = witness_stage_v2(layout, authority);
        write_complete_witness_v2(&mut zero_nonce, true);
        let nonce = SemanticChunkV2 {
            arena: ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
            bytes: vec![0; ZkAmsMkheRnsNativeSourceArenaV1::Nonce.plaintext_bytes() as usize],
        };
        assert!(matches!(
            zero_nonce.finish_record_v2(nonce),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidWitness)
        ));

        let mut valid = witness_stage_v2(layout, authority);
        write_complete_witness_v2(&mut valid, true);
        let mut nonce = SemanticChunkV2 {
            arena: ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
            bytes: vec![0; ZkAmsMkheRnsNativeSourceArenaV1::Nonce.plaintext_bytes() as usize],
        };
        nonce.as_mut_slice()[31] = 1;
        let (writer, receipt) = valid.finish_record_v2(nonce).expect("semantic record");
        assert_eq!(writer.next_record, 1);
        receipt
            .validate(layout, authority)
            .expect("semantic receipt validates");
    }

    #[test]
    fn record_receipt_binds_source_domain_checks_and_stays_non_authorizing() {
        let authority = zk_ams_mkhe_rns_native_split_decryption_authority_v2()
            .expect("phase-0 corrected authority");
        let layout = corrected_layout_v2();
        let baseline = valid_record_receipt_v2(layout, authority);
        baseline
            .validate(layout, authority)
            .expect("semantic record receipt");
        assert!(baseline.source_domain_checks_verified());
        assert!(!baseline.persistent_equality_verified());
        assert!(!baseline.release_available());

        macro_rules! assert_semantic_false_rejected {
            ($field:ident) => {{
                let mut mutated = baseline;
                mutated.$field = false;
                mutated.receipt_digest = record_receipt_digest_v2(mutated);
                assert_eq!(
                    mutated.validate(layout, authority),
                    Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord),
                    stringify!($field),
                );
            }};
        }
        assert_semantic_false_rejected!(canonical_plaintext_verified);
        assert_semantic_false_rejected!(witness_ranges_verified);
        assert_semantic_false_rejected!(ephemeral_nonzero);
        assert_semantic_false_rejected!(nonce_written);
        assert_semantic_false_rejected!(nonce_nonzero);

        let mut forged = baseline;
        forged.persistent_equality_verified = true;
        forged.receipt_digest = record_receipt_digest_v2(forged);
        assert_eq!(
            forged.validate(layout, authority),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord)
        );
        let mut forged = baseline;
        forged.release_available = true;
        forged.receipt_digest = record_receipt_digest_v2(forged);
        assert_eq!(
            forged.validate(layout, authority),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord)
        );
        let mut corrupted = baseline;
        corrupted.receipt_digest[0] ^= 1;
        assert_eq!(
            corrupted.validate(layout, authority),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord)
        );
    }

    #[test]
    fn ciphertext_correspondence_record_pairs_public_and_native_identities() {
        let authority = zk_ams_mkhe_rns_native_split_decryption_authority_v2()
            .expect("phase-0 corrected authority");
        let layout = corrected_layout_v2();
        let record = valid_record_receipt_v2(layout, authority);
        let public_record_digest = [0xd1; 32];
        let baseline = ciphertext_correspondence_record_digest_v2(
            record.record_index(),
            public_record_digest,
            record,
        );

        let mut substituted_public = public_record_digest;
        substituted_public[0] ^= 1;
        assert_ne!(
            baseline,
            ciphertext_correspondence_record_digest_v2(
                record.record_index(),
                substituted_public,
                record,
            )
        );

        let mut substituted_native = record;
        substituted_native.ciphertext_digest[0] ^= 1;
        substituted_native.receipt_digest = record_receipt_digest_v2(substituted_native);
        assert_ne!(
            baseline,
            ciphertext_correspondence_record_digest_v2(
                substituted_native.record_index(),
                public_record_digest,
                substituted_native,
            )
        );
        assert_ne!(
            baseline,
            ciphertext_correspondence_record_digest_v2(
                record.record_index().wrapping_add(1),
                public_record_digest,
                record,
            )
        );

        assert!(!record.persistent_equality_verified());
        assert!(!record.release_available());
    }

    #[test]
    fn exact_43_record_seal_receipt_is_structural_and_non_authorizing() {
        let authority = zk_ams_mkhe_rns_native_split_decryption_authority_v2()
            .expect("phase-0 corrected authority");
        let layout = corrected_layout_v2();
        let snapshot = ReceiptSnapshotV2 { layout };
        let source_receipt = snapshot.structural_receipt().expect("source receipt");
        let mut ordered = Keccak256::new();
        ordered.update(ORDERED_RECORD_ROOT_DOMAIN_V2);
        ordered.update(&[CORRIDOR_VERSION_V2]);
        ordered.update(&authority.authority_digest());
        ordered.update(&layout.source_binding_digest());
        for record in 0..ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 {
            ordered.update(&[record]);
            ordered.update(&[record.wrapping_add(1); 32]);
        }
        let receipt = ZkAmsMkheRnsNativeSplitDecryptionSourceSealReceiptV2::new_v2(
            layout,
            authority,
            ordered.finalize(),
            source_receipt,
        )
        .expect("exact-43 structural receipt");
        receipt
            .validate(layout, authority, source_receipt)
            .expect("receipt validates");
        assert_eq!(receipt.record_count(), 43);
        assert!(receipt.split_decryption_records_complete);
        assert!(receipt.record_source_domain_checks_verified());
        assert!(receipt.source_writer_complete);
        assert!(!receipt.persistent_equality_verified());
        assert!(!receipt.release_available());
        assert_ne!(receipt.receipt_digest(), [0; 32]);

        let mut forged = receipt;
        forged.record_source_domain_checks_verified = false;
        forged.receipt_digest = seal_receipt_digest_v2(forged);
        assert_eq!(
            forged.validate(layout, authority, source_receipt),
            Err(ZkAmsMkheRnsNativeSplitDecryptionSourceErrorV2::InvalidRecord)
        );
    }
}
