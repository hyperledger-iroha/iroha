//! Incremental canonical frame sizing for one growing transfer occurrence.
//!
//! This counts framing only: it does not validate arithmetic, chronology, a
//! digest's value, source authority, or resource admission. Each supplied delta
//! is counted once; earlier deltas and their private paths are not retained.
//! The checked child pairs sizing with incremental quantity validation.
//! TODO: Wire the State-owned reservation/rollback adapter before using it for runtime quotas.

use fastpq_prover::gadgets::public_transfer_statement::encode_quantity_units_v1;
use iroha_crypto::Hash;
use iroha_data_model::fastpq::{
    FastpqOperationKind, FastpqPublicInputs, FastpqPublicTransferDeltaV1,
    FastpqPublicTransferStatementV1, FastpqPublicTransferTranscriptV1, FastpqQuantityUnits,
    FastpqStateTransition, TransferDeltaTranscript, TransferTranscript, transfer_balance_key,
};
use iroha_primitives::numeric::{MAX_DECIMAL_SCALE, Quantity};
use norito::core::{DecodeFlagsGuard, SequencePayloadLength, header_flags};

/// Explicit inclusive sizing limits; no production defaults or E ownership.
#[derive(Clone, Copy)]
pub(crate) struct PrefixLengthLimits {
    /// Maximum original deltas in this single occurrence.
    pub(crate) max_deltas: usize,
    /// Maximum complete canonical original transcript frame, including paths.
    pub(crate) max_input_frame_bytes: usize,
    /// Maximum complete canonical quantity public-statement frame.
    pub(crate) max_public_statement_frame_bytes: usize,
}

/// Sizes of the supplied prefix, without a semantic or authority certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PrefixFrameLengths {
    /// Complete original delta count; the nonempty prefix represents one T.
    pub(crate) deltas: usize,
    /// Complete original private transcript frame I.
    pub(crate) input_frame_bytes: usize,
    /// Complete quantity statement frame, contributing M=S for this occurrence.
    pub(crate) public_statement_frame_bytes: usize,
}

/// A failed sizing attempt; none of these variants authorizes entry deferral.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PrefixLengthError {
    /// This schema-coupled sizer has not been reviewed for another canonical layout.
    #[error("incremental source sizing requires canonical COMPACT_LEN layout")]
    UnsupportedLayout,
    /// A first prefix needs Some(digest); every longer prefix needs None.
    #[error("source prefix digest presence does not match its delta count")]
    DigestShape,
    /// A complete public participant-row count cannot be represented as u32.
    #[error("source prefix participant row count exceeds u32")]
    RowCount,
    /// Exact size arithmetic cannot be represented on this host.
    #[error("source prefix frame size overflows")]
    Overflow,
    /// The exact occurrence exceeds its caller's delta ceiling.
    #[error("source prefix has {actual} deltas, exceeding {maximum}")]
    Deltas {
        /// Prospective complete prefix count.
        actual: usize,
        /// Caller-supplied inclusive ceiling.
        maximum: usize,
    },
    /// The original transcript frame exceeds its caller's sizing ceiling.
    #[error("source prefix input frame has {actual} bytes, exceeding {maximum}")]
    Input {
        /// Prospective complete original frame length.
        actual: usize,
        /// Caller-supplied inclusive ceiling.
        maximum: usize,
    },
    /// The complete public statement frame exceeds its caller's sizing ceiling.
    #[error("source prefix public frame has {actual} bytes, exceeding {maximum}")]
    Public {
        /// Prospective complete public frame length.
        actual: usize,
        /// Caller-supplied inclusive ceiling.
        maximum: usize,
    },
    /// Canonical encoding of a real supplied value or fixed framing exemplar failed.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Canonical encoding of the fixed quantity-value shape failed.
    #[error(transparent)]
    QuantityCodec(#[from] fastpq_prover::Error),
    /// The current full quantity domain no longer contains the fixed zero exemplar.
    #[error("source quantity frame exemplar is outside the supported domain")]
    QuantityShape,
}

fn canonical_flags(flags: u8) -> Result<u8, PrefixLengthError> {
    // This is an explicit codec/layout dependency, not a layout heuristic. A
    // future canonical change must update the sizing derivation and parity tests.
    if flags != header_flags::COMPACT_LEN {
        return Err(PrefixLengthError::UnsupportedLayout);
    }
    Ok(flags)
}

fn add(left: usize, right: usize) -> Result<usize, PrefixLengthError> {
    left.checked_add(right).ok_or(PrefixLengthError::Overflow)
}

fn subtract(left: usize, right: usize) -> Result<usize, PrefixLengthError> {
    left.checked_sub(right).ok_or(PrefixLengthError::Overflow)
}

fn field_span(payload: usize, flags: u8) -> Result<usize, PrefixLengthError> {
    add(
        payload,
        norito::core::len_prefix_len_with_flags(payload, flags),
    )
}

fn checked_row_count(deltas: usize) -> Result<u32, PrefixLengthError> {
    u32::try_from(deltas)
        .ok()
        .and_then(|count| count.checked_mul(2))
        .ok_or(PrefixLengthError::RowCount)
}

/// Constant-retained-state sizing of one occurrence's complete growing prefix.
///
/// Clone is a sizing checkpoint, with only counters, fixed baselines and one
/// fixed quantity frame. It is not a WSV, semantic-validator or reservation
/// checkpoint. The caller must append each accepted original delta exactly once.
/// Later changes to prior public facts or private paths invalidate these counts;
/// the adapter must preserve the measured snapshot or explicitly remeasure it.
#[derive(Clone)]
pub(crate) struct SourcePrefixFrameSizer {
    flags: u8,
    limits: PrefixLengthLimits,
    private_deltas: SequencePayloadLength,
    public_deltas: SequencePayloadLength,
    rows: SequencePayloadLength,
    empty_sequence_bytes: usize,
    input_fixed: [usize; 2],
    public_claim_fixed: [usize; 2],
    statement_fixed: usize,
    quantity_frame: Vec<u8>,
    latest: Option<PrefixFrameLengths>,
}

impl SourcePrefixFrameSizer {
    /// Measure fixed framing once for exact immutable occurrence headers.
    ///
    /// Real empty model values supply all fixed fields and frame alignment.
    /// The only algebra below substitutes generic sequence payloads into their
    /// existing length-prefixed fields under the explicitly checked V1 layout.
    pub(crate) fn new(
        batch_hash: Hash,
        authority_digest: Hash,
        limits: PrefixLengthLimits,
    ) -> Result<Self, PrefixLengthError> {
        let flags = canonical_flags(norito::core::default_encode_flags())?;
        let _canonical = DecodeFlagsGuard::enter(flags);
        let sequence = SequencePayloadLength::new(flags)?;
        let empty_sequence_bytes = sequence.len();
        let empty_field = field_span(empty_sequence_bytes, flags)?;
        let mut input_fixed = [0; 2];
        let mut public_claim_fixed = [0; 2];
        for (index, digest) in [None, Some(batch_hash)].into_iter().enumerate() {
            let original = TransferTranscript {
                batch_hash,
                deltas: Vec::new(),
                authority_digest,
                poseidon_preimage_digest: digest,
            };
            input_fixed[index] =
                subtract(norito::core::encoded_frame_len(&original)?, empty_field)?;
            let public = FastpqPublicTransferTranscriptV1::from(&original);
            public_claim_fixed[index] =
                subtract(norito::core::encoded_payload_len(&public)?, empty_field)?;
        }
        let empty_statement = FastpqPublicTransferStatementV1 {
            public_inputs: FastpqPublicInputs {
                dsid: [0; 16],
                slot: 0,
                old_root: [0; 32],
                new_root: [0; 32],
                perm_root: [0; 32],
                tx_set_hash: [0; 32],
            },
            ordering_hash: [0; 32],
            transitions: Vec::new(),
            transcripts: Vec::new(),
        };
        let statement_fixed = subtract(
            norito::core::encoded_frame_len(&empty_statement)?,
            add(empty_field, empty_field)?,
        )?;
        // QuantityValueV1 retains u32 scale plus all nineteen u32 limbs. Under
        // canonical V1 their contents change bytes, never length. No exemplar
        // row, ordering digest, statement or root escapes this sizing helper.
        let quantity = FastpqQuantityUnits::from_quantity(&Quantity::zero(), MAX_DECIMAL_SCALE)
            .ok_or(PrefixLengthError::QuantityShape)?;
        let quantity_frame = encode_quantity_units_v1(&quantity)?;
        Ok(Self {
            flags,
            limits,
            private_deltas: sequence,
            public_deltas: sequence,
            rows: sequence,
            empty_sequence_bytes,
            input_fixed,
            public_claim_fixed,
            statement_fixed,
            quantity_frame,
            latest: None,
        })
    }

    /// Count one actual appended delta and publish new counters only on success.
    ///
    /// The optional digest's presence is checked; its value is not verified.
    /// Private input framing is checked before copying bounded public fields.
    /// Row ordering and selected quantity scales do not affect frame length;
    /// they still require full semantic validation and exact final commitments.
    pub(crate) fn append(
        &mut self,
        delta: &TransferDeltaTranscript,
        poseidon_preimage_digest: Option<Hash>,
    ) -> Result<PrefixFrameLengths, PrefixLengthError> {
        let deltas = add(self.private_deltas.count(), 1)?;
        if poseidon_preimage_digest.is_some() != (deltas == 1) {
            return Err(PrefixLengthError::DigestShape);
        }
        if deltas > self.limits.max_deltas {
            return Err(PrefixLengthError::Deltas {
                actual: deltas,
                maximum: self.limits.max_deltas,
            });
        }
        checked_row_count(deltas)?;
        let _canonical = DecodeFlagsGuard::enter(self.flags);
        let mut next = self.clone();
        next.private_deltas.push(delta)?;
        let digest_index = usize::from(deltas == 1);
        let input_frame_bytes = add(
            self.input_fixed[digest_index],
            field_span(next.private_deltas.len(), self.flags)?,
        )?;
        if input_frame_bytes > self.limits.max_input_frame_bytes {
            return Err(PrefixLengthError::Input {
                actual: input_frame_bytes,
                maximum: self.limits.max_input_frame_bytes,
            });
        }
        next.public_deltas
            .push(&FastpqPublicTransferDeltaV1::from(delta))?;
        for account in [&delta.from_account, &delta.to_account] {
            let row = FastpqStateTransition {
                key: transfer_balance_key(&delta.asset_definition, account)?,
                pre_value: self.quantity_frame.clone(),
                post_value: self.quantity_frame.clone(),
                operation: FastpqOperationKind::Transfer,
            };
            next.rows.push(&row)?;
        }
        let public_claim = add(
            self.public_claim_fixed[digest_index],
            field_span(next.public_deltas.len(), self.flags)?,
        )?;
        // The whole statement contains exactly this one transcript occurrence.
        // A generic one-element sequence adds its count and the element prefix.
        let transcript_sequence = add(
            self.empty_sequence_bytes,
            field_span(public_claim, self.flags)?,
        )?;
        let public_statement_frame_bytes = add(
            add(
                self.statement_fixed,
                field_span(next.rows.len(), self.flags)?,
            )?,
            field_span(transcript_sequence, self.flags)?,
        )?;
        if public_statement_frame_bytes > self.limits.max_public_statement_frame_bytes {
            return Err(PrefixLengthError::Public {
                actual: public_statement_frame_bytes,
                maximum: self.limits.max_public_statement_frame_bytes,
            });
        }
        let result = PrefixFrameLengths {
            deltas,
            input_frame_bytes,
            public_statement_frame_bytes,
        };
        next.latest = Some(result);
        *self = next;
        Ok(result)
    }

    /// Last complete successfully counted prefix; empty sizing state has no occurrence.
    pub(crate) const fn latest(&self) -> Option<PrefixFrameLengths> {
        self.latest
    }
}

#[path = "source_prefix_lengths/checked.rs"]
pub(crate) mod checked;

#[cfg(test)]
#[path = "source_prefix_lengths/tests.rs"]
mod tests;
