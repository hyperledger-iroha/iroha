//! Exact field-neutral public-instance ABI for Offline Cash V1 STATE proofs.
//!
//! The final recursive `State` wrapper exposes 229 canonical little-endian
//! `u32` words (33 packed cells). Its `StateLeaf` relation exposes only the
//! first 93 semantic words (14 packed cells); the wrapper alone owns the final
//! 136-word reciprocal-audit binding. This split prevents a proof-transcript
//! fixed point in which a leaf proof would depend on an audit derived from that
//! same proof. Seven words are packed into one 224-bit field element without
//! field reduction on either Pasta parity.

use core::fmt;

use super::{
    OfflineCashHalo2ParityV1,
    protocol::{
        OFFLINE_CASH_STATE_ABI_WORDS_V1, OFFLINE_CASH_STATE_INSTANCE_CELLS_MAX_V1,
        OFFLINE_CASH_STATE_INSTANCE_CELLS_V1, OFFLINE_CASH_STATE_LEAF_INSTANCE_CELLS_V1,
        OFFLINE_CASH_STATE_SEMANTIC_ABI_WORDS_V1, OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1,
        OfflineCashHalo2CircuitRoleV1, offline_cash_halo2_protocol_identity_v1,
    },
    state_relation::offline_cash_receive_semantic_digest_v1,
    state_transition::{OfflineCashStateContextV1, ReceiveFoldOutputV1},
};
use halo2_proofs::halo2curves::ff::PrimeField;
use iroha_data_model::offline::{
    KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2, OFFLINE_CASH_HALO2_K_V1,
    OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1, OFFLINE_CASH_WIRE_VERSION_V1,
    OfflineCashRecursivePairBindingV1, OfflineCashRecursivePairTopologyV1,
    OfflineCashTransferStatementV1,
};

const ABI_VERSION: u32 = 1;
const FIXED_PARENT_COUNT: u32 = 2;
const DIGEST_WORDS: usize = 8;
const DIGEST_FIELDS: usize = 10;
const RECURSIVE_PAIR_BINDING_WORDS: usize = OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1;
const PACKED_CELL_BYTES: usize = 28;

pub(super) const STATE_ABI_WORDS: usize = OFFLINE_CASH_STATE_ABI_WORDS_V1 as usize;
pub(super) const STATE_WORDS_PER_INSTANCE: usize =
    OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1 as usize;
pub(super) const STATE_INSTANCE_CELLS: usize = OFFLINE_CASH_STATE_INSTANCE_CELLS_V1 as usize;
pub(super) const STATE_INSTANCE_CELLS_MAX: usize =
    OFFLINE_CASH_STATE_INSTANCE_CELLS_MAX_V1 as usize;
pub(super) const STATE_LEAF_ABI_WORDS: usize = OFFLINE_CASH_STATE_SEMANTIC_ABI_WORDS_V1 as usize;
pub(super) const STATE_LEAF_INSTANCE_CELLS: usize =
    OFFLINE_CASH_STATE_LEAF_INSTANCE_CELLS_V1 as usize;

pub(super) const STATE_PARITY_WORD: usize = 3;
pub(super) const STATE_OPERATION_WORD: usize = 4;
pub(super) const STATE_PROTOCOL_WORD_START: usize = 16;

pub(super) const RELEASE_WORD_START: usize = 8;
pub(super) const SEMANTIC_WORD_START: usize = 24;
pub(super) const CONTEXT_WORD_START: usize = 32;
pub(super) const REQUEST_WORD_START: usize = 40;
pub(super) const PARENT_0_WORD_START: usize = 48;
pub(super) const PARENT_1_WORD_START: usize = 56;
pub(super) const RESULT_WORD_START: usize = 64;
pub(super) const LINK_WORD_START: usize = 72;
pub(super) const TRANSITION_WORD_START: usize = 80;
pub(super) const AMOUNT_WORD_START: usize = 88;
pub(super) const SCALE_WORD: usize = 92;
pub(super) const RECURSIVE_PAIR_BINDING_WORD_START: usize = STATE_LEAF_ABI_WORDS;

const _: () = assert!(DIGEST_WORDS * DIGEST_FIELDS == 80);
const _: () =
    assert!(RECURSIVE_PAIR_BINDING_WORD_START + RECURSIVE_PAIR_BINDING_WORDS == STATE_ABI_WORDS);
const _: () = assert!(STATE_WORDS_PER_INSTANCE == PACKED_CELL_BYTES / 4);
const _: () = assert!(STATE_INSTANCE_CELLS == STATE_ABI_WORDS.div_ceil(STATE_WORDS_PER_INSTANCE));
const _: () = assert!(STATE_INSTANCE_CELLS <= STATE_INSTANCE_CELLS_MAX);
const _: () = assert!(STATE_LEAF_INSTANCE_CELLS == 14);

/// Exact STATE operation selected by the public words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(super) enum OfflineCashStateOperationV1 {
    /// Split one sender balance into a remainder and receiver-bound credit.
    SendSplit = 1,
    /// Fold one verified receiver credit into its current balance.
    ReceiveFold = 2,
}

/// Host-side rejection while constructing the exact STATE public ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashStateAbiErrorV1 {
    /// A send statement failed its canonical wire relation.
    InvalidSendStatement,
    /// The private state context differs from the send statement.
    ContextMismatch,
    /// A receive output is not a canonical positive fixed-two-parent relation.
    InvalidReceiveOutput,
    /// The shared 136-word recursive-pair binding is malformed.
    InvalidRecursivePairBinding,
    /// A structural header, digest, amount, protocol, or operation is invalid.
    InvalidLayout,
    /// A private balance/credit opening does not satisfy the selected public relation.
    InvalidPrivateWitness,
    /// Packed cells do not have the exact count or canonical final zero padding.
    NonCanonicalPacking,
    /// A parity-specific circuit was given the other parity's ABI.
    ParityMismatch,
}

impl fmt::Display for OfflineCashStateAbiErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSendStatement => "invalid offline-cash STATE send statement",
            Self::ContextMismatch => "offline-cash STATE context mismatch",
            Self::InvalidReceiveOutput => "invalid offline-cash STATE receive output",
            Self::InvalidRecursivePairBinding => {
                "invalid offline-cash STATE recursive-pair binding"
            }
            Self::InvalidLayout => "invalid offline-cash STATE instance layout",
            Self::InvalidPrivateWitness => "invalid offline-cash STATE private witness",
            Self::NonCanonicalPacking => "non-canonical offline-cash STATE instance packing",
            Self::ParityMismatch => "offline-cash STATE parity mismatch",
        })
    }
}

impl std::error::Error for OfflineCashStateAbiErrorV1 {}

/// Canonical semantic words and their selected Pasta parity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OfflineCashStatePublicInstancesV1 {
    parity: OfflineCashHalo2ParityV1,
    words: [u32; STATE_ABI_WORDS],
}

/// Fixed-point-free public input of the private `StateLeaf` relation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OfflineCashStateLeafPublicInstancesV1 {
    parity: OfflineCashHalo2ParityV1,
    words: [u32; STATE_LEAF_ABI_WORDS],
}

/// Exact public fields consumed by the private STATE relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OfflineCashStateRelationPublicV1 {
    pub(super) operation: OfflineCashStateOperationV1,
    pub(super) release_id: [u8; 32],
    pub(super) semantic_digest: [u8; 32],
    pub(super) context_digest: [u8; 32],
    pub(super) request_digest: [u8; 32],
    pub(super) parent_0: [u8; 32],
    pub(super) parent_1: [u8; 32],
    pub(super) result: [u8; 32],
    pub(super) link: [u8; 32],
    pub(super) transition_digest: [u8; 32],
    pub(super) transfer: u128,
    pub(super) scale: u32,
}

impl OfflineCashStateLeafPublicInstancesV1 {
    /// Build the fixed-point-free leaf relation for a sender split.
    pub(super) fn send_split(
        context: &OfflineCashStateContextV1,
        statement: &OfflineCashTransferStatementV1,
        parity: OfflineCashHalo2ParityV1,
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        statement
            .validate()
            .map_err(|_| OfflineCashStateAbiErrorV1::InvalidSendStatement)?;
        if !context.matches_statement(statement) {
            return Err(OfflineCashStateAbiErrorV1::ContextMismatch);
        }
        let semantic_digest = statement
            .canonical_digest()
            .map_err(|_| OfflineCashStateAbiErrorV1::InvalidSendStatement)?;
        Self::build(
            parity,
            OfflineCashStateOperationV1::SendSplit,
            statement.release_id,
            semantic_digest,
            context.digest(),
            statement.request_digest,
            statement.sender_before,
            statement.receiver_before,
            statement.sender_after,
            statement.credit_commitment,
            statement.transition_digest,
            statement.amount,
            statement.scale,
        )
    }

    /// Build the fixed-point-free leaf relation for a receiver fold.
    pub(super) fn receive_fold(
        output: &ReceiveFoldOutputV1,
        parity: OfflineCashHalo2ParityV1,
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        validate_receive_output(output)?;
        Self::build(
            parity,
            OfflineCashStateOperationV1::ReceiveFold,
            output.release_id,
            receive_semantic_digest(output),
            output.context_digest,
            output.request_digest,
            output.balance_parent,
            output.credit_parent,
            output.next_head,
            output.send_transition_digest,
            output.receive_transition_digest,
            output.amount,
            output.scale,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        parity: OfflineCashHalo2ParityV1,
        operation: OfflineCashStateOperationV1,
        release_id: [u8; 32],
        semantic_digest: [u8; 32],
        context_digest: [u8; 32],
        request_digest: [u8; 32],
        parent_0: [u8; 32],
        parent_1: [u8; 32],
        result: [u8; 32],
        link: [u8; 32],
        transition_digest: [u8; 32],
        amount: u128,
        scale: u32,
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        if amount == 0 || scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 {
            return Err(OfflineCashStateAbiErrorV1::InvalidLayout);
        }
        let protocol_digest =
            offline_cash_halo2_protocol_identity_v1(parity, OfflineCashHalo2CircuitRoleV1::State)
                .digest();
        let mut words = [0_u32; STATE_LEAF_ABI_WORDS];
        words[..8].copy_from_slice(&[
            ABI_VERSION,
            u32::from(OFFLINE_CASH_WIRE_VERSION_V1),
            OFFLINE_CASH_HALO2_K_V1,
            parity as u32,
            operation as u32,
            FIXED_PARENT_COUNT,
            DIGEST_WORDS as u32,
            RECURSIVE_PAIR_BINDING_WORDS as u32,
        ]);
        for (offset, digest) in [
            (RELEASE_WORD_START, release_id),
            (STATE_PROTOCOL_WORD_START, protocol_digest),
            (SEMANTIC_WORD_START, semantic_digest),
            (CONTEXT_WORD_START, context_digest),
            (REQUEST_WORD_START, request_digest),
            (PARENT_0_WORD_START, parent_0),
            (PARENT_1_WORD_START, parent_1),
            (RESULT_WORD_START, result),
            (LINK_WORD_START, link),
            (TRANSITION_WORD_START, transition_digest),
        ] {
            write_digest_words(&mut words, offset, digest);
        }
        for (target, chunk) in words[AMOUNT_WORD_START..AMOUNT_WORD_START + 4]
            .iter_mut()
            .zip(amount.to_le_bytes().chunks_exact(4))
        {
            *target = u32::from_le_bytes(chunk.try_into().expect("four-byte amount limb"));
        }
        words[SCALE_WORD] = scale;
        let instances = Self { parity, words };
        instances.validate_structure()?;
        Ok(instances)
    }

    /// Selected Pasta parity.
    pub(super) const fn parity(&self) -> OfflineCashHalo2ParityV1 {
        self.parity
    }

    /// Exact 93 public semantic words.
    pub(super) const fn words(&self) -> &[u32; STATE_LEAF_ABI_WORDS] {
        &self.words
    }

    /// Recover the public fields constrained by the private relation.
    pub(super) fn relation_public(
        &self,
    ) -> Result<OfflineCashStateRelationPublicV1, OfflineCashStateAbiErrorV1> {
        relation_public_from_words(&self.words)
    }

    /// Convert the canonical 93 words to the exact 14-cell leaf instance column.
    pub(super) fn field_instances<F: PrimeField>(&self) -> [F; STATE_LEAF_INSTANCE_CELLS] {
        std::array::from_fn(|cell_index| {
            let start = cell_index * STATE_WORDS_PER_INSTANCE;
            let end = start
                .saturating_add(STATE_WORDS_PER_INSTANCE)
                .min(STATE_LEAF_ABI_WORDS);
            pack_words_as_field::<F>(&self.words[start..end])
        })
    }

    fn validate_structure(&self) -> Result<(), OfflineCashStateAbiErrorV1> {
        validate_semantic_structure(self.parity, &self.words)
    }
}

impl OfflineCashStatePublicInstancesV1 {
    /// Build the exact public relation for a sender split.
    pub(super) fn send_split(
        context: &OfflineCashStateContextV1,
        statement: &OfflineCashTransferStatementV1,
        parity: OfflineCashHalo2ParityV1,
        recursive_pair_binding: &OfflineCashRecursivePairBindingV1,
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        let leaf = OfflineCashStateLeafPublicInstancesV1::send_split(context, statement, parity)?;
        Self::from_leaf(leaf, recursive_pair_binding)
    }

    /// Build the exact public relation for a receiver fold produced by Core.
    pub(super) fn receive_fold(
        output: &ReceiveFoldOutputV1,
        parity: OfflineCashHalo2ParityV1,
        recursive_pair_binding: &OfflineCashRecursivePairBindingV1,
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        let leaf = OfflineCashStateLeafPublicInstancesV1::receive_fold(output, parity)?;
        Self::from_leaf(leaf, recursive_pair_binding)
    }

    pub(super) fn from_leaf(
        leaf: OfflineCashStateLeafPublicInstancesV1,
        recursive_pair_binding: &OfflineCashRecursivePairBindingV1,
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        if recursive_pair_binding
            .topology()
            .map_err(|_| OfflineCashStateAbiErrorV1::InvalidRecursivePairBinding)?
            != OfflineCashRecursivePairTopologyV1::State
        {
            return Err(OfflineCashStateAbiErrorV1::InvalidRecursivePairBinding);
        }
        let recursive_pair_words = recursive_pair_binding
            .canonical_words()
            .map_err(|_| OfflineCashStateAbiErrorV1::InvalidRecursivePairBinding)?;
        let mut words = [0_u32; STATE_ABI_WORDS];
        words[..STATE_LEAF_ABI_WORDS].copy_from_slice(leaf.words());
        words[RECURSIVE_PAIR_BINDING_WORD_START..].copy_from_slice(&recursive_pair_words);
        let instances = Self {
            parity: leaf.parity(),
            words,
        };
        instances.validate_structure()?;
        Ok(instances)
    }

    /// Selected parity.
    pub(super) const fn parity(&self) -> OfflineCashHalo2ParityV1 {
        self.parity
    }

    /// Exact semantic `u32` words in canonical order.
    pub(super) const fn words(&self) -> &[u32; STATE_ABI_WORDS] {
        &self.words
    }

    /// Project the exact fixed-point-free 93-word input proved by `StateLeaf`.
    pub(super) fn state_leaf(&self) -> OfflineCashStateLeafPublicInstancesV1 {
        let mut words = [0_u32; STATE_LEAF_ABI_WORDS];
        words.copy_from_slice(&self.words[..STATE_LEAF_ABI_WORDS]);
        OfflineCashStateLeafPublicInstancesV1 {
            parity: self.parity,
            words,
        }
    }

    /// Exact operation encoded in word four.
    pub(super) fn operation(
        &self,
    ) -> Result<OfflineCashStateOperationV1, OfflineCashStateAbiErrorV1> {
        operation_from_words(&self.words)
    }

    /// Recover the exact public subset constrained by the private relation.
    pub(super) fn relation_public(
        &self,
    ) -> Result<OfflineCashStateRelationPublicV1, OfflineCashStateAbiErrorV1> {
        relation_public_from_words(&self.words)
    }

    /// Canonical field-neutral 28-byte cells before conversion to Fp or Fq.
    pub(super) fn packed_cell_bytes(&self) -> [[u8; PACKED_CELL_BYTES]; STATE_INSTANCE_CELLS] {
        std::array::from_fn(|cell_index| {
            let mut bytes = [0_u8; PACKED_CELL_BYTES];
            let start = cell_index * STATE_WORDS_PER_INSTANCE;
            let end = start
                .saturating_add(STATE_WORDS_PER_INSTANCE)
                .min(STATE_ABI_WORDS);
            for (lane, word) in self.words[start..end].iter().enumerate() {
                bytes[lane * 4..lane * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
            bytes
        })
    }

    /// Convert canonical packed cells to one exact Pasta instance column.
    pub(super) fn field_instances<F: PrimeField>(&self) -> [F; STATE_INSTANCE_CELLS] {
        std::array::from_fn(|cell_index| {
            let start = cell_index * STATE_WORDS_PER_INSTANCE;
            let end = start
                .saturating_add(STATE_WORDS_PER_INSTANCE)
                .min(STATE_ABI_WORDS);
            pack_words_as_field::<F>(&self.words[start..end])
        })
    }

    /// Recover the exact shared recursive-pair binding bound into this ABI.
    pub(super) fn recursive_pair_binding(
        &self,
    ) -> Result<OfflineCashRecursivePairBindingV1, OfflineCashStateAbiErrorV1> {
        recursive_pair_binding(&self.words)
    }

    /// Strictly recover semantic words from the field-neutral packed bytes.
    pub(super) fn unpack_cell_bytes(
        cells: &[[u8; PACKED_CELL_BYTES]],
    ) -> Result<[u32; STATE_ABI_WORDS], OfflineCashStateAbiErrorV1> {
        if cells.len() != STATE_INSTANCE_CELLS {
            return Err(OfflineCashStateAbiErrorV1::NonCanonicalPacking);
        }
        let used_in_last = STATE_ABI_WORDS % STATE_WORDS_PER_INSTANCE;
        if used_in_last != 0
            && cells[STATE_INSTANCE_CELLS - 1][used_in_last * 4..]
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(OfflineCashStateAbiErrorV1::NonCanonicalPacking);
        }
        let mut words = [0_u32; STATE_ABI_WORDS];
        for (word_index, target) in words.iter_mut().enumerate() {
            let cell = &cells[word_index / STATE_WORDS_PER_INSTANCE];
            let offset = word_index % STATE_WORDS_PER_INSTANCE * 4;
            *target = u32::from_le_bytes(
                cell[offset..offset + 4]
                    .try_into()
                    .expect("four-byte packed limb"),
            );
        }
        Ok(words)
    }

    fn validate_structure(&self) -> Result<(), OfflineCashStateAbiErrorV1> {
        validate_semantic_structure(self.parity, &self.words[..STATE_LEAF_ABI_WORDS])?;
        recursive_pair_binding(&self.words).map(|_| ())
    }
}

pub(super) fn fixed_state_word_v1(parity: OfflineCashHalo2ParityV1, index: usize) -> Option<u32> {
    match index {
        0 => Some(ABI_VERSION),
        1 => Some(u32::from(OFFLINE_CASH_WIRE_VERSION_V1)),
        2 => Some(OFFLINE_CASH_HALO2_K_V1),
        STATE_PARITY_WORD => Some(parity as u32),
        5 => Some(FIXED_PARENT_COUNT),
        6 => Some(DIGEST_WORDS as u32),
        7 => Some(RECURSIVE_PAIR_BINDING_WORDS as u32),
        STATE_PROTOCOL_WORD_START..=23 => {
            let digest = offline_cash_halo2_protocol_identity_v1(
                parity,
                OfflineCashHalo2CircuitRoleV1::State,
            )
            .digest();
            Some(u32::from_le_bytes(
                digest[(index - STATE_PROTOCOL_WORD_START) * 4
                    ..(index - STATE_PROTOCOL_WORD_START + 1) * 4]
                    .try_into()
                    .expect("four-byte protocol limb"),
            ))
        }
        _ => None,
    }
}

pub(super) fn pack_words_as_field<F: PrimeField>(words: &[u32]) -> F {
    assert!(
        words.len() <= STATE_WORDS_PER_INSTANCE,
        "Offline Cash STATE packed cell exceeds seven u32 limbs"
    );
    let radix = F::from(1_u64 << 32);
    words.iter().rev().fold(F::ZERO, |accumulator, word| {
        accumulator * radix + F::from(u64::from(*word))
    })
}

fn write_digest_words(words: &mut [u32], offset: usize, digest: [u8; 32]) {
    for (target, chunk) in words[offset..offset + DIGEST_WORDS]
        .iter_mut()
        .zip(digest.chunks_exact(4))
    {
        *target = u32::from_le_bytes(chunk.try_into().expect("four-byte digest limb"));
    }
}

fn read_digest_words(words: &[u32], offset: usize) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    for (chunk, word) in digest
        .chunks_exact_mut(4)
        .zip(&words[offset..offset + DIGEST_WORDS])
    {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    digest
}

fn operation_from_words(
    words: &[u32],
) -> Result<OfflineCashStateOperationV1, OfflineCashStateAbiErrorV1> {
    match words[STATE_OPERATION_WORD] {
        value if value == OfflineCashStateOperationV1::SendSplit as u32 => {
            Ok(OfflineCashStateOperationV1::SendSplit)
        }
        value if value == OfflineCashStateOperationV1::ReceiveFold as u32 => {
            Ok(OfflineCashStateOperationV1::ReceiveFold)
        }
        _ => Err(OfflineCashStateAbiErrorV1::InvalidLayout),
    }
}

fn relation_public_from_words(
    words: &[u32],
) -> Result<OfflineCashStateRelationPublicV1, OfflineCashStateAbiErrorV1> {
    let mut amount = [0_u8; 16];
    for (chunk, word) in amount
        .chunks_exact_mut(4)
        .zip(&words[AMOUNT_WORD_START..AMOUNT_WORD_START + 4])
    {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    Ok(OfflineCashStateRelationPublicV1 {
        operation: operation_from_words(words)?,
        release_id: read_digest_words(words, RELEASE_WORD_START),
        semantic_digest: read_digest_words(words, SEMANTIC_WORD_START),
        context_digest: read_digest_words(words, CONTEXT_WORD_START),
        request_digest: read_digest_words(words, REQUEST_WORD_START),
        parent_0: read_digest_words(words, PARENT_0_WORD_START),
        parent_1: read_digest_words(words, PARENT_1_WORD_START),
        result: read_digest_words(words, RESULT_WORD_START),
        link: read_digest_words(words, LINK_WORD_START),
        transition_digest: read_digest_words(words, TRANSITION_WORD_START),
        transfer: u128::from_le_bytes(amount),
        scale: words[SCALE_WORD],
    })
}

fn validate_semantic_structure(
    parity: OfflineCashHalo2ParityV1,
    words: &[u32],
) -> Result<(), OfflineCashStateAbiErrorV1> {
    if words.len() != STATE_LEAF_ABI_WORDS
        || words[..8]
            != [
                ABI_VERSION,
                u32::from(OFFLINE_CASH_WIRE_VERSION_V1),
                OFFLINE_CASH_HALO2_K_V1,
                parity as u32,
                words[STATE_OPERATION_WORD],
                FIXED_PARENT_COUNT,
                DIGEST_WORDS as u32,
                RECURSIVE_PAIR_BINDING_WORDS as u32,
            ]
        || operation_from_words(words).is_err()
        || words[AMOUNT_WORD_START..AMOUNT_WORD_START + 4]
            .iter()
            .all(|word| *word == 0)
        || words[SCALE_WORD] > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
    {
        return Err(OfflineCashStateAbiErrorV1::InvalidLayout);
    }
    let expected_protocol =
        offline_cash_halo2_protocol_identity_v1(parity, OfflineCashHalo2CircuitRoleV1::State)
            .digest();
    if read_digest_words(words, STATE_PROTOCOL_WORD_START) != expected_protocol {
        return Err(OfflineCashStateAbiErrorV1::InvalidLayout);
    }
    for offset in [
        RELEASE_WORD_START,
        STATE_PROTOCOL_WORD_START,
        SEMANTIC_WORD_START,
        CONTEXT_WORD_START,
        REQUEST_WORD_START,
        PARENT_0_WORD_START,
        PARENT_1_WORD_START,
        RESULT_WORD_START,
        LINK_WORD_START,
        TRANSITION_WORD_START,
    ] {
        if words[offset..offset + DIGEST_WORDS]
            .iter()
            .all(|word| *word == 0)
        {
            return Err(OfflineCashStateAbiErrorV1::InvalidLayout);
        }
    }
    Ok(())
}

fn validate_receive_output(output: &ReceiveFoldOutputV1) -> Result<(), OfflineCashStateAbiErrorV1> {
    if output.release_id == [0; 32]
        || output.context_digest == [0; 32]
        || output.request_digest == [0; 32]
        || output.balance_parent == [0; 32]
        || output.credit_parent == [0; 32]
        || output.next_head == [0; 32]
        || output.send_transition_digest == [0; 32]
        || output.receive_transition_digest == [0; 32]
        || output.amount == 0
        || output.scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
        || output.balance_parent == output.credit_parent
        || output.next_head == output.balance_parent
        || output.next_head == output.credit_parent
    {
        return Err(OfflineCashStateAbiErrorV1::InvalidReceiveOutput);
    }
    Ok(())
}

fn recursive_pair_binding(
    words: &[u32; STATE_ABI_WORDS],
) -> Result<OfflineCashRecursivePairBindingV1, OfflineCashStateAbiErrorV1> {
    let pair_words: [u32; OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1] = words
        [RECURSIVE_PAIR_BINDING_WORD_START..]
        .try_into()
        .expect("STATE recursive-pair word count is fixed");
    OfflineCashRecursivePairBindingV1::from_canonical_words(pair_words)
        .map_err(|_| OfflineCashStateAbiErrorV1::InvalidRecursivePairBinding)
}

fn receive_semantic_digest(output: &ReceiveFoldOutputV1) -> [u8; 32] {
    offline_cash_receive_semantic_digest_v1(
        &output.release_id,
        &output.context_digest,
        &output.request_digest,
        &output.balance_parent,
        &output.credit_parent,
        &output.next_head,
        &output.send_transition_digest,
        &output.receive_transition_digest,
        output.amount,
        output.scale,
    )
}
