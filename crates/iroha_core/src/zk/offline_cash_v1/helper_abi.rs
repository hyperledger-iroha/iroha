//! Exact field-neutral public ABI for Offline Cash V1 hardware helpers.
//!
//! Every helper role repeats one common semantic statement so the authenticated
//! `GuardBundle` composition boundary joins child proofs by exact public equality rather
//! than by a host-selected subset. The layout is 184 canonical little-endian
//! `u32` words, packed seven words per 224-bit field element into one 27-cell
//! instance column on either Pasta field. The last cell contains two words and
//! five mandatory zero words. No field reduction is permitted.

use core::fmt;

use halo2_proofs::halo2curves::ff::PrimeField;
use iroha_data_model::offline::{OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_WIRE_VERSION_V1};

use super::{
    OfflineCashHalo2ParityV1,
    protocol::{
        OFFLINE_CASH_HELPER_ABI_WORDS_V1, OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1,
        OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1, OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1,
        OfflineCashHalo2CircuitRoleV1, offline_cash_halo2_protocol_identity_v1,
    },
};

const ABI_VERSION: u32 = 1;
const DIGEST_WORDS: usize = 8;
const DIGEST_FIELDS: usize = 21;
const PACKED_CELL_BYTES: usize = 28;

pub(super) const HELPER_ABI_WORDS: usize = OFFLINE_CASH_HELPER_ABI_WORDS_V1 as usize;
pub(super) const HELPER_WORDS_PER_INSTANCE: usize =
    OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1 as usize;
pub(super) const HELPER_INSTANCE_CELLS: usize = OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1 as usize;
pub(super) const HELPER_INSTANCE_CELLS_MAX: usize =
    OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1 as usize;

pub(super) const HELPER_PARITY_WORD: usize = 3;
pub(super) const HELPER_ROLE_WORD: usize = 4;
pub(super) const HELPER_OPERATION_WORD: usize = 5;
pub(super) const HELPER_ANDROID_PRESENT_WORD: usize = 6;
pub(super) const HELPER_FROM_LOW_WORD: usize = 8;
pub(super) const HELPER_FROM_HIGH_WORD: usize = 9;
pub(super) const HELPER_TO_LOW_WORD: usize = 10;
pub(super) const HELPER_TO_HIGH_WORD: usize = 11;

pub(super) const HELPER_PROTOCOL_WORD_START: usize = 16;
pub(super) const RELEASE_WORD_START: usize = 24;
pub(super) const CONTEXT_WORD_START: usize = 32;
pub(super) const CURRENT_HEAD_WORD_START: usize = 40;
pub(super) const CURRENT_LINEAGE_WORD_START: usize = 48;
pub(super) const TRANSITION_WORD_START: usize = 56;
pub(super) const WALLET_WORD_START: usize = 64;
pub(super) const POLICY_WORD_START: usize = 72;
pub(super) const DEVICE_WORD_START: usize = 80;
pub(super) const CURRENT_GUARD_WORD_START: usize = 88;
pub(super) const NEXT_GUARD_WORD_START: usize = 96;
pub(super) const PLATFORM_KEY_WORD_START: usize = 104;
pub(super) const PLATFORM_MESSAGE_WORD_START: usize = 112;
pub(super) const GUARD_USE_CLAIM_WORD_START: usize = 120;
pub(super) const PLATFORM_BIND_CLAIM_WORD_START: usize = 128;
pub(super) const ANDROID_CERTIFICATE_WORD_START: usize = 136;
pub(super) const ANDROID_TBS_WORD_START: usize = 144;
pub(super) const ANDROID_ISSUER_KEY_WORD_START: usize = 152;
pub(super) const ANDROID_ATTESTATION_WORD_START: usize = 160;
pub(super) const ANDROID_CLAIM_WORD_START: usize = 168;
pub(super) const BUNDLE_WORD_START: usize = 176;

pub(super) const REQUIRED_DIGEST_OFFSETS: [usize; 16] = [
    HELPER_PROTOCOL_WORD_START,
    RELEASE_WORD_START,
    CONTEXT_WORD_START,
    CURRENT_HEAD_WORD_START,
    CURRENT_LINEAGE_WORD_START,
    TRANSITION_WORD_START,
    WALLET_WORD_START,
    POLICY_WORD_START,
    DEVICE_WORD_START,
    CURRENT_GUARD_WORD_START,
    NEXT_GUARD_WORD_START,
    PLATFORM_KEY_WORD_START,
    PLATFORM_MESSAGE_WORD_START,
    GUARD_USE_CLAIM_WORD_START,
    PLATFORM_BIND_CLAIM_WORD_START,
    BUNDLE_WORD_START,
];
pub(super) const ANDROID_DIGEST_OFFSETS: [usize; 5] = [
    ANDROID_CERTIFICATE_WORD_START,
    ANDROID_TBS_WORD_START,
    ANDROID_ISSUER_KEY_WORD_START,
    ANDROID_ATTESTATION_WORD_START,
    ANDROID_CLAIM_WORD_START,
];

const _: () = assert!(DIGEST_WORDS * DIGEST_FIELDS == HELPER_ABI_WORDS - 16);
const _: () = assert!(HELPER_WORDS_PER_INSTANCE == PACKED_CELL_BYTES / 4);
const _: () =
    assert!(HELPER_INSTANCE_CELLS == HELPER_ABI_WORDS.div_ceil(HELPER_WORDS_PER_INSTANCE));
const _: () = assert!(HELPER_INSTANCE_CELLS <= HELPER_INSTANCE_CELLS_MAX);

/// Exact monetary operation authorized by a helper statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(super) enum OfflineCashHelperOperationV1 {
    /// Replace a sender balance with its deterministic remainder.
    SendSplit = 1,
    /// Fold one request-bound credit into the receiver balance.
    ReceiveFold = 2,
}

/// Host-side failure while constructing or decoding the helper ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashHelperAbiErrorV1 {
    /// Header, role, operation, sequence, digest, or optionality is invalid.
    InvalidLayout,
    /// A helper role was given the other Pasta parity.
    ParityMismatch,
    /// Packed cells have the wrong count or nonzero terminal padding.
    NonCanonicalPacking,
    /// The private helper witness does not satisfy the common statement.
    InvalidPrivateWitness,
}

impl fmt::Display for OfflineCashHelperAbiErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLayout => "invalid offline-cash helper instance layout",
            Self::ParityMismatch => "offline-cash helper parity mismatch",
            Self::NonCanonicalPacking => "non-canonical offline-cash helper instance packing",
            Self::InvalidPrivateWitness => "invalid offline-cash helper private witness",
        })
    }
}

impl std::error::Error for OfflineCashHelperAbiErrorV1 {}

/// Common public statement repeated by every helper role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OfflineCashHelperStatementV1 {
    pub(super) operation: OfflineCashHelperOperationV1,
    pub(super) android_key_cert_present: bool,
    pub(super) from_sequence: u64,
    pub(super) to_sequence: u64,
    pub(super) release_id: [u8; 32],
    pub(super) context_digest: [u8; 32],
    pub(super) current_head: [u8; 32],
    pub(super) current_lineage_digest: [u8; 32],
    pub(super) transition_digest: [u8; 32],
    pub(super) wallet_binding: [u8; 32],
    pub(super) hardware_policy_id: [u8; 32],
    pub(super) guard_device_id: [u8; 32],
    pub(super) current_guard_binding: [u8; 32],
    pub(super) next_guard_binding: [u8; 32],
    pub(super) platform_key_digest: [u8; 32],
    pub(super) platform_message_digest: [u8; 32],
    pub(super) guard_use_claim_digest: [u8; 32],
    pub(super) platform_bind_claim_digest: [u8; 32],
    pub(super) android_certificate_digest: [u8; 32],
    pub(super) android_tbs_digest: [u8; 32],
    pub(super) android_issuer_key_digest: [u8; 32],
    pub(super) android_attestation_digest: [u8; 32],
    pub(super) android_key_cert_claim_digest: [u8; 32],
    pub(super) guard_bundle_digest: [u8; 32],
}

impl OfflineCashHelperStatementV1 {
    fn validate_layout(self) -> Result<(), OfflineCashHelperAbiErrorV1> {
        let required = [
            self.release_id,
            self.context_digest,
            self.current_head,
            self.current_lineage_digest,
            self.transition_digest,
            self.wallet_binding,
            self.hardware_policy_id,
            self.guard_device_id,
            self.current_guard_binding,
            self.next_guard_binding,
            self.platform_key_digest,
            self.platform_message_digest,
            self.guard_use_claim_digest,
            self.platform_bind_claim_digest,
            self.guard_bundle_digest,
        ];
        let android = [
            self.android_certificate_digest,
            self.android_tbs_digest,
            self.android_issuer_key_digest,
            self.android_attestation_digest,
            self.android_key_cert_claim_digest,
        ];
        if required.into_iter().any(|digest| digest == [0; 32])
            || self.from_sequence.checked_add(1) != Some(self.to_sequence)
            || self.current_guard_binding == self.next_guard_binding
            || self.current_head == self.transition_digest
            || (self.android_key_cert_present
                && android.into_iter().any(|digest| digest == [0; 32]))
            || (!self.android_key_cert_present
                && android.into_iter().any(|digest| digest != [0; 32]))
        {
            return Err(OfflineCashHelperAbiErrorV1::InvalidLayout);
        }
        Ok(())
    }
}

/// Canonical helper words for one role and Pasta parity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashHelperPublicInstancesV1 {
    parity: OfflineCashHalo2ParityV1,
    role: OfflineCashHalo2CircuitRoleV1,
    words: [u32; HELPER_ABI_WORDS],
}

impl OfflineCashHelperPublicInstancesV1 {
    /// Encode one already validated common helper statement.
    pub(super) fn new(
        statement: OfflineCashHelperStatementV1,
        parity: OfflineCashHalo2ParityV1,
        role: OfflineCashHalo2CircuitRoleV1,
    ) -> Result<Self, OfflineCashHelperAbiErrorV1> {
        statement.validate_layout()?;
        if matches!(
            role,
            OfflineCashHalo2CircuitRoleV1::State
                | OfflineCashHalo2CircuitRoleV1::StateLeaf
                | OfflineCashHalo2CircuitRoleV1::P256V3
        ) {
            return Err(OfflineCashHelperAbiErrorV1::InvalidLayout);
        }
        let mut words = [0_u32; HELPER_ABI_WORDS];
        words[..16].copy_from_slice(&[
            ABI_VERSION,
            u32::from(OFFLINE_CASH_WIRE_VERSION_V1),
            OFFLINE_CASH_HALO2_K_V1,
            parity as u32,
            role as u32,
            statement.operation as u32,
            u32::from(statement.android_key_cert_present),
            DIGEST_WORDS as u32,
            statement.from_sequence as u32,
            (statement.from_sequence >> 32) as u32,
            statement.to_sequence as u32,
            (statement.to_sequence >> 32) as u32,
            DIGEST_FIELDS as u32,
            HELPER_WORDS_PER_INSTANCE as u32,
            HELPER_INSTANCE_CELLS as u32,
            0,
        ]);
        for (offset, digest) in [
            (
                HELPER_PROTOCOL_WORD_START,
                offline_cash_halo2_protocol_identity_v1(parity, role).digest(),
            ),
            (RELEASE_WORD_START, statement.release_id),
            (CONTEXT_WORD_START, statement.context_digest),
            (CURRENT_HEAD_WORD_START, statement.current_head),
            (CURRENT_LINEAGE_WORD_START, statement.current_lineage_digest),
            (TRANSITION_WORD_START, statement.transition_digest),
            (WALLET_WORD_START, statement.wallet_binding),
            (POLICY_WORD_START, statement.hardware_policy_id),
            (DEVICE_WORD_START, statement.guard_device_id),
            (CURRENT_GUARD_WORD_START, statement.current_guard_binding),
            (NEXT_GUARD_WORD_START, statement.next_guard_binding),
            (PLATFORM_KEY_WORD_START, statement.platform_key_digest),
            (
                PLATFORM_MESSAGE_WORD_START,
                statement.platform_message_digest,
            ),
            (GUARD_USE_CLAIM_WORD_START, statement.guard_use_claim_digest),
            (
                PLATFORM_BIND_CLAIM_WORD_START,
                statement.platform_bind_claim_digest,
            ),
            (
                ANDROID_CERTIFICATE_WORD_START,
                statement.android_certificate_digest,
            ),
            (ANDROID_TBS_WORD_START, statement.android_tbs_digest),
            (
                ANDROID_ISSUER_KEY_WORD_START,
                statement.android_issuer_key_digest,
            ),
            (
                ANDROID_ATTESTATION_WORD_START,
                statement.android_attestation_digest,
            ),
            (
                ANDROID_CLAIM_WORD_START,
                statement.android_key_cert_claim_digest,
            ),
            (BUNDLE_WORD_START, statement.guard_bundle_digest),
        ] {
            write_digest_words(&mut words, offset, digest);
        }
        let instances = Self {
            parity,
            role,
            words,
        };
        instances.validate_structure()?;
        Ok(instances)
    }

    pub(super) const fn parity(&self) -> OfflineCashHalo2ParityV1 {
        self.parity
    }

    pub(super) const fn role(&self) -> OfflineCashHalo2CircuitRoleV1 {
        self.role
    }

    pub(super) const fn words(&self) -> &[u32; HELPER_ABI_WORDS] {
        &self.words
    }

    #[cfg(test)]
    pub(super) fn overwrite_word_for_test(&mut self, index: usize, value: u32) {
        self.words[index] = value;
    }

    pub(super) fn statement(
        &self,
    ) -> Result<OfflineCashHelperStatementV1, OfflineCashHelperAbiErrorV1> {
        self.validate_structure()?;
        self.decode_statement()
    }

    fn decode_statement(
        &self,
    ) -> Result<OfflineCashHelperStatementV1, OfflineCashHelperAbiErrorV1> {
        Ok(OfflineCashHelperStatementV1 {
            operation: match self.words[HELPER_OPERATION_WORD] {
                value if value == OfflineCashHelperOperationV1::SendSplit as u32 => {
                    OfflineCashHelperOperationV1::SendSplit
                }
                value if value == OfflineCashHelperOperationV1::ReceiveFold as u32 => {
                    OfflineCashHelperOperationV1::ReceiveFold
                }
                _ => return Err(OfflineCashHelperAbiErrorV1::InvalidLayout),
            },
            android_key_cert_present: self.words[HELPER_ANDROID_PRESENT_WORD] == 1,
            from_sequence: u64::from(self.words[HELPER_FROM_LOW_WORD])
                | (u64::from(self.words[HELPER_FROM_HIGH_WORD]) << 32),
            to_sequence: u64::from(self.words[HELPER_TO_LOW_WORD])
                | (u64::from(self.words[HELPER_TO_HIGH_WORD]) << 32),
            release_id: read_digest_words(&self.words, RELEASE_WORD_START),
            context_digest: read_digest_words(&self.words, CONTEXT_WORD_START),
            current_head: read_digest_words(&self.words, CURRENT_HEAD_WORD_START),
            current_lineage_digest: read_digest_words(&self.words, CURRENT_LINEAGE_WORD_START),
            transition_digest: read_digest_words(&self.words, TRANSITION_WORD_START),
            wallet_binding: read_digest_words(&self.words, WALLET_WORD_START),
            hardware_policy_id: read_digest_words(&self.words, POLICY_WORD_START),
            guard_device_id: read_digest_words(&self.words, DEVICE_WORD_START),
            current_guard_binding: read_digest_words(&self.words, CURRENT_GUARD_WORD_START),
            next_guard_binding: read_digest_words(&self.words, NEXT_GUARD_WORD_START),
            platform_key_digest: read_digest_words(&self.words, PLATFORM_KEY_WORD_START),
            platform_message_digest: read_digest_words(&self.words, PLATFORM_MESSAGE_WORD_START),
            guard_use_claim_digest: read_digest_words(&self.words, GUARD_USE_CLAIM_WORD_START),
            platform_bind_claim_digest: read_digest_words(
                &self.words,
                PLATFORM_BIND_CLAIM_WORD_START,
            ),
            android_certificate_digest: read_digest_words(
                &self.words,
                ANDROID_CERTIFICATE_WORD_START,
            ),
            android_tbs_digest: read_digest_words(&self.words, ANDROID_TBS_WORD_START),
            android_issuer_key_digest: read_digest_words(
                &self.words,
                ANDROID_ISSUER_KEY_WORD_START,
            ),
            android_attestation_digest: read_digest_words(
                &self.words,
                ANDROID_ATTESTATION_WORD_START,
            ),
            android_key_cert_claim_digest: read_digest_words(&self.words, ANDROID_CLAIM_WORD_START),
            guard_bundle_digest: read_digest_words(&self.words, BUNDLE_WORD_START),
        })
    }

    /// Canonical field-neutral packed cells.
    pub(super) fn packed_cell_bytes(&self) -> [[u8; PACKED_CELL_BYTES]; HELPER_INSTANCE_CELLS] {
        std::array::from_fn(|cell_index| {
            let mut bytes = [0_u8; PACKED_CELL_BYTES];
            let start = cell_index * HELPER_WORDS_PER_INSTANCE;
            let end = (start + HELPER_WORDS_PER_INSTANCE).min(HELPER_ABI_WORDS);
            for (lane, word) in self.words[start..end].iter().enumerate() {
                bytes[lane * 4..lane * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
            bytes
        })
    }

    /// Convert the canonical cells to one exact Pasta instance column.
    pub(super) fn field_instances<F: PrimeField>(&self) -> [F; HELPER_INSTANCE_CELLS] {
        std::array::from_fn(|cell_index| {
            let start = cell_index * HELPER_WORDS_PER_INSTANCE;
            let end = (start + HELPER_WORDS_PER_INSTANCE).min(HELPER_ABI_WORDS);
            pack_words_as_field::<F>(&self.words[start..end])
        })
    }

    /// Strictly recover words from field-neutral cells.
    pub(super) fn unpack_cell_bytes(
        cells: &[[u8; PACKED_CELL_BYTES]],
    ) -> Result<[u32; HELPER_ABI_WORDS], OfflineCashHelperAbiErrorV1> {
        if cells.len() != HELPER_INSTANCE_CELLS {
            return Err(OfflineCashHelperAbiErrorV1::NonCanonicalPacking);
        }
        let used_in_last = HELPER_ABI_WORDS % HELPER_WORDS_PER_INSTANCE;
        if used_in_last != 0
            && cells[HELPER_INSTANCE_CELLS - 1][used_in_last * 4..]
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(OfflineCashHelperAbiErrorV1::NonCanonicalPacking);
        }
        let mut words = [0_u32; HELPER_ABI_WORDS];
        for (word_index, target) in words.iter_mut().enumerate() {
            let cell = &cells[word_index / HELPER_WORDS_PER_INSTANCE];
            let offset = word_index % HELPER_WORDS_PER_INSTANCE * 4;
            *target = u32::from_le_bytes(
                cell[offset..offset + 4]
                    .try_into()
                    .expect("four-byte helper packed limb"),
            );
        }
        Ok(words)
    }

    fn validate_structure(&self) -> Result<(), OfflineCashHelperAbiErrorV1> {
        if matches!(
            self.role,
            OfflineCashHalo2CircuitRoleV1::State
                | OfflineCashHalo2CircuitRoleV1::StateLeaf
                | OfflineCashHalo2CircuitRoleV1::P256V3
        ) || self.words[..16]
            != [
                ABI_VERSION,
                u32::from(OFFLINE_CASH_WIRE_VERSION_V1),
                OFFLINE_CASH_HALO2_K_V1,
                self.parity as u32,
                self.role as u32,
                self.words[HELPER_OPERATION_WORD],
                self.words[HELPER_ANDROID_PRESENT_WORD],
                DIGEST_WORDS as u32,
                self.words[HELPER_FROM_LOW_WORD],
                self.words[HELPER_FROM_HIGH_WORD],
                self.words[HELPER_TO_LOW_WORD],
                self.words[HELPER_TO_HIGH_WORD],
                DIGEST_FIELDS as u32,
                HELPER_WORDS_PER_INSTANCE as u32,
                HELPER_INSTANCE_CELLS as u32,
                0,
            ]
            || !matches!(
                self.words[HELPER_OPERATION_WORD],
                value if value == OfflineCashHelperOperationV1::SendSplit as u32
                    || value == OfflineCashHelperOperationV1::ReceiveFold as u32
            )
            || self.words[HELPER_ANDROID_PRESENT_WORD] > 1
            || read_digest_words(&self.words, HELPER_PROTOCOL_WORD_START)
                != offline_cash_halo2_protocol_identity_v1(self.parity, self.role).digest()
        {
            return Err(OfflineCashHelperAbiErrorV1::InvalidLayout);
        }
        self.decode_statement()?.validate_layout()
    }
}

/// Exact same-proof-parity public-equality boundary enforced by one
/// authenticated `GuardBundle` scalar-verifier half. The reciprocal partner
/// enforces this half's deferred curve equations over the other Pasta field.
///
/// This value validates public-instance identity and role topology; the sibling
/// proof owner constructs it only after each mapped ordinary child proof has
/// been constrained and its carried lineage joined. Keeping the equality logic independently typed
/// prevents any adapter from joining a host-selected subset of the 184-word
/// statement or silently omitting the optional Android child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OfflineCashGuardBundleChildPublicEqualityV1 {
    parent_parity: OfflineCashHalo2ParityV1,
    child_parity: OfflineCashHalo2ParityV1,
    statement: OfflineCashHelperStatementV1,
    android_child_present: bool,
}

impl OfflineCashGuardBundleChildPublicEqualityV1 {
    /// Close the exact same-proof-parity fixed-slot `GuardUse + PlatformBind +
    /// AndroidKeyCert + GuardBundleLeaf -> GuardBundle` public topology.
    pub(super) fn new(
        guard_bundle: &OfflineCashHelperPublicInstancesV1,
        guard_use: &OfflineCashHelperPublicInstancesV1,
        platform_bind: &OfflineCashHelperPublicInstancesV1,
        android_key_cert: Option<&OfflineCashHelperPublicInstancesV1>,
        guard_bundle_leaf: &OfflineCashHelperPublicInstancesV1,
    ) -> Result<Self, OfflineCashHelperAbiErrorV1> {
        let android_key_cert =
            android_key_cert.ok_or(OfflineCashHelperAbiErrorV1::InvalidLayout)?;
        // The reviewed split verifier runs transcript/scalar arithmetic in the
        // proof curve's scalar field: Eq::Scalar=Fp and Ep::Scalar=Fq. Hence
        // each wrapper scalar-verifies same-parity children; its reciprocal
        // sibling enforces the deferred group equations.
        let child_parity = guard_bundle.parity();
        if guard_bundle.role() != OfflineCashHalo2CircuitRoleV1::GuardBundle
            || guard_use.role() != OfflineCashHalo2CircuitRoleV1::GuardUse
            || platform_bind.role() != OfflineCashHalo2CircuitRoleV1::PlatformBind
            || guard_use.parity() != child_parity
            || platform_bind.parity() != child_parity
            || android_key_cert.role() != OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
            || android_key_cert.parity() != child_parity
            || guard_bundle_leaf.role() != OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf
            || guard_bundle_leaf.parity() != child_parity
        {
            return Err(OfflineCashHelperAbiErrorV1::InvalidLayout);
        }

        let statement = guard_bundle.statement()?;
        if guard_use.statement()? != statement
            || platform_bind.statement()? != statement
            || android_key_cert.statement()? != statement
            || guard_bundle_leaf.statement()? != statement
            || !helper_child_semantic_words_equal(guard_bundle, guard_use)
            || !helper_child_semantic_words_equal(guard_bundle, platform_bind)
            || !helper_child_semantic_words_equal(guard_bundle, android_key_cert)
            || !helper_child_semantic_words_equal(guard_bundle, guard_bundle_leaf)
        {
            return Err(OfflineCashHelperAbiErrorV1::InvalidLayout);
        }

        Ok(Self {
            parent_parity: guard_bundle.parity(),
            child_parity,
            statement,
            android_child_present: statement.android_key_cert_present,
        })
    }

    pub(super) const fn parent_parity(self) -> OfflineCashHalo2ParityV1 {
        self.parent_parity
    }

    pub(super) const fn child_parity(self) -> OfflineCashHalo2ParityV1 {
        self.child_parity
    }

    pub(super) const fn statement(self) -> OfflineCashHelperStatementV1 {
        self.statement
    }

    pub(super) const fn android_child_present(self) -> bool {
        self.android_child_present
    }
}

fn helper_child_semantic_words_equal(
    guard_bundle: &OfflineCashHelperPublicInstancesV1,
    child: &OfflineCashHelperPublicInstancesV1,
) -> bool {
    (0..HELPER_ABI_WORDS).all(|index| {
        index == HELPER_PARITY_WORD
            || index == HELPER_ROLE_WORD
            || (HELPER_PROTOCOL_WORD_START..RELEASE_WORD_START).contains(&index)
            || guard_bundle.words()[index] == child.words()[index]
    })
}

pub(super) fn fixed_helper_word_v1(
    parity: OfflineCashHalo2ParityV1,
    role: OfflineCashHalo2CircuitRoleV1,
    index: usize,
) -> Option<u32> {
    match index {
        0 => Some(ABI_VERSION),
        1 => Some(u32::from(OFFLINE_CASH_WIRE_VERSION_V1)),
        2 => Some(OFFLINE_CASH_HALO2_K_V1),
        HELPER_PARITY_WORD => Some(parity as u32),
        HELPER_ROLE_WORD => Some(role as u32),
        7 => Some(DIGEST_WORDS as u32),
        12 => Some(DIGEST_FIELDS as u32),
        13 => Some(HELPER_WORDS_PER_INSTANCE as u32),
        14 => Some(HELPER_INSTANCE_CELLS as u32),
        15 => Some(0),
        HELPER_PROTOCOL_WORD_START..=23 => {
            let digest = offline_cash_halo2_protocol_identity_v1(parity, role).digest();
            Some(u32::from_le_bytes(
                digest[(index - HELPER_PROTOCOL_WORD_START) * 4
                    ..(index - HELPER_PROTOCOL_WORD_START + 1) * 4]
                    .try_into()
                    .expect("four-byte helper protocol limb"),
            ))
        }
        _ => None,
    }
}

pub(super) fn pack_words_as_field<F: PrimeField>(words: &[u32]) -> F {
    assert!(
        words.len() <= HELPER_WORDS_PER_INSTANCE,
        "Offline Cash helper packed cell exceeds seven u32 limbs"
    );
    let radix = F::from(1_u64 << 32);
    words.iter().rev().fold(F::ZERO, |accumulator, word| {
        accumulator * radix + F::from(u64::from(*word))
    })
}

fn write_digest_words(words: &mut [u32; HELPER_ABI_WORDS], offset: usize, digest: [u8; 32]) {
    for (target, chunk) in words[offset..offset + DIGEST_WORDS]
        .iter_mut()
        .zip(digest.chunks_exact(4))
    {
        *target = u32::from_le_bytes(chunk.try_into().expect("four-byte helper digest limb"));
    }
}

fn read_digest_words(words: &[u32; HELPER_ABI_WORDS], offset: usize) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    for (chunk, word) in digest
        .chunks_exact_mut(4)
        .zip(&words[offset..offset + DIGEST_WORDS])
    {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    digest
}
