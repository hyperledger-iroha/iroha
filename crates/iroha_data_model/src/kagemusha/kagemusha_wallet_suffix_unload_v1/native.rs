//! Additive Native suffix review and scheduling DATA; no decoded value grants authority.
//!
//! These are separate canonical schemas. Ordinary wallet encodings and financial review
//! projections remain unchanged. The one-use capability stays in the native owner.

use super::*;
use crate::kagemusha::kagemusha_wallet_v1::KagemushaDevicePublicKeyV1;

/// Maximum canonical financial display projection.
pub const SUFFIX_UNLOAD_REVIEW_MAX_BYTES_V1: usize = 12_288;
/// Maximum canonical progress frame.
pub const SUFFIX_UNLOAD_PROGRESS_MAX_BYTES_V1: usize = 256;
/// Maximum canonical scheduling status or release summary.
pub const SUFFIX_UNLOAD_STATUS_MAX_BYTES_V1: usize = 512;
/// Maximum canonical suffix instruction (separate from the ordinary instruction cap).
pub const SUFFIX_UNLOAD_INSTRUCTION_MAX_BYTES_V1: usize =
    KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1 + 16_384;
/// Maximum exact signed suffix transaction, including outer envelope and signatures.
pub const SUFFIX_UNLOAD_SIGNED_WIRE_MAX_BYTES_V1: usize =
    KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1 + 65_536;

fn tag(version: u16, discriminator: u16, expected: u16) -> SuffixResult<()> {
    require_version("suffix.native.version", version)?;
    if discriminator != expected {
        return Err(invalid("suffix.native.discriminator"));
    }
    Ok(())
}
fn counts(steps: u128, chunks: u128, instructions: u128) -> SuffixResult<()> {
    if chunks == 0
        || chunks > KAGEMUSHA_WALLET_SUFFIX_CHUNKS_MAX_V1 as u128
        || steps < chunks
        || steps > chunks * KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1 as u128
        || instructions != chunks + 2
    {
        return Err(invalid("suffix.native.counts"));
    }
    Ok(())
}
fn account_original(bytes: &[u8], digest: &[u8; 32]) -> SuffixResult<()> {
    let account: AccountId = decode_frame_v1(bytes, 4096)?;
    if kagemusha_wallet_account_digest_v1(&account)? != *digest {
        return Err(invalid("suffix.native.destination"));
    }
    Ok(())
}

/// Exact Native-selected financial terms displayed before one hardware approval.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::SuffixUnloadReviewProjectionV1"
)]
pub struct SuffixUnloadReviewProjectionV1 {
    /// Exactly one.
    pub format_version: u16,
    /// Exactly thirteen.
    pub discriminator: u16,
    /// Request selected by Native; nonzero.
    pub request_id: [u8; 32],
    /// Admitted scheme identity.
    pub scheme_id: [u8; 32],
    /// Enrolled wallet incarnation.
    pub wallet_id: [u8; 32],
    /// Exact immutable Begin/header authority digest.
    pub authority_digest: [u8; 32],
    /// Released operation which supplies the monetary predecessor.
    pub source_operation_id: [u8; 32],
    /// Exact retained predecessor capsule.
    pub source_capsule_digest: [u8; 32],
    /// Opened predecessor state commitment.
    pub source_state_commitment: [u8; 32],
    /// Released predecessor sequence.
    pub source_sequence: u128,
    /// Genuine folded anchor sequence.
    pub anchor_sequence: u128,
    /// Number of completely verified semantic steps.
    pub suffix_step_count: u128,
    /// Number of immutable Append chunks.
    pub chunk_count: u128,
    /// Begin, all Appends and Finalize; exactly chunk_count plus two.
    pub instruction_count: u128,
    /// Positive offline face amount.
    pub amount: u128,
    /// Authenticated online charge.
    pub charge: u128,
    /// Exact offline debit; equals amount.
    pub gross_debit: u128,
    /// Account payout; equals amount minus charge.
    pub net_destination_amount: u128,
    /// Digest of the exact admitted account original.
    pub destination_account_digest: [u8; 32],
    /// Canonical admitted AccountId frame, at most 4,096 bytes.
    pub destination_account_original: Vec<u8>,
    /// Zero precisely when no charge quote is selected.
    pub charge_quote_digest: [u8; 32],
    /// Exact canonical charge-beneficiary account, present precisely with a charge quote.
    pub charge_beneficiary_original: Option<Vec<u8>>,
    /// Current issuer-authenticated credential.
    pub credential_digest: [u8; 32],
    /// Actual installed signed artifact manifest.
    pub artifact_manifest_digest: [u8; 32],
    /// Admitted nonexportable hardware payment key.
    pub payment_key: KagemushaDevicePublicKeyV1,
}
impl SuffixUnloadReviewProjectionV1 {
    /// Check bounded, canonical display DATA, never a proof or approval.
    /// # Errors
    /// Invalid version, identities, account originals, counts or arithmetic.
    pub fn validate(&self) -> SuffixResult<()> {
        tag(self.format_version, self.discriminator, 13)?;
        for value in [
            self.request_id,
            self.scheme_id,
            self.wallet_id,
            self.authority_digest,
            self.source_operation_id,
            self.source_capsule_digest,
            self.source_state_commitment,
            self.destination_account_digest,
            self.credential_digest,
            self.artifact_manifest_digest,
        ] {
            require_nonzero("suffix.review.identity", &value)?;
        }
        counts(
            self.suffix_step_count,
            self.chunk_count,
            self.instruction_count,
        )?;
        if self.anchor_sequence.checked_add(self.suffix_step_count) != Some(self.source_sequence)
            || self.amount == 0
            || self.gross_debit != self.amount
            || self.amount.checked_sub(self.charge) != Some(self.net_destination_amount)
        {
            return Err(invalid("suffix.review.terms"));
        }
        if !kagemusha_wallet_is_canonical_field_v1(&self.authority_digest) {
            return Err(invalid("suffix.review.authority"));
        }
        account_original(
            &self.destination_account_original,
            &self.destination_account_digest,
        )?;
        match (
            &self.charge_beneficiary_original,
            self.charge_quote_digest == [0; 32],
        ) {
            (None, true) if self.charge == 0 => {}
            (Some(bytes), false) => {
                let _: AccountId = decode_frame_v1(bytes, 4096)?;
            }
            _ => return Err(invalid("suffix.review.charge_presence")),
        }
        self.payment_key.validate()?;
        Ok(())
    }
    /// Encode the complete bounded canonical projection.
    /// # Errors
    /// Invalid projection or resource refusal.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, SUFFIX_UNLOAD_REVIEW_MAX_BYTES_V1)
    }
    /// Decode and validate the exact bounded projection.
    /// # Errors
    /// Noncanonical encoding, invalid shape or resource refusal.
    pub fn decode_canonical(bytes: &[u8]) -> SuffixResult<Self> {
        let value: Self = decode_frame_v1(bytes, SUFFIX_UNLOAD_REVIEW_MAX_BYTES_V1)?;
        value.validate()?;
        Ok(value)
    }
}

/// Bounded verification progress, not a persisted proof-verdict capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::SuffixUnloadProgressV1"
)]
pub struct SuffixUnloadProgressV1 {
    /// Exactly one.
    pub format_version: u16,
    /// Exactly fourteen.
    pub discriminator: u16,
    /// Exact Native-selected request identity.
    pub request_id: [u8; 32],
    /// Completely processed steps in this transient verification pass.
    pub completed_steps: u128,
    /// Total selected semantic steps, at most 4,096.
    pub total_steps: u128,
}
impl SuffixUnloadProgressV1 {
    /// Validate progress counters without granting authority.
    /// # Errors
    /// Wrong tag, zero request, invalid bound or counter order.
    pub fn validate(&self) -> SuffixResult<()> {
        tag(self.format_version, self.discriminator, 14)?;
        require_nonzero("suffix.progress.request", &self.request_id)?;
        if self.total_steps == 0
            || self.total_steps > KAGEMUSHA_WALLET_SUFFIX_STEPS_MAX_V1 as u128
            || self.completed_steps > self.total_steps
        {
            return Err(invalid("suffix.progress.counts"));
        }
        Ok(())
    }
    /// Encode exact bounded progress DATA.
    /// # Errors
    /// Invalid progress or resource refusal.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, SUFFIX_UNLOAD_PROGRESS_MAX_BYTES_V1)
    }
    /// Decode exact bounded progress DATA.
    /// # Errors
    /// Noncanonical, invalid or oversized progress.
    pub fn decode_canonical(bytes: &[u8]) -> SuffixResult<Self> {
        let value: Self = decode_frame_v1(bytes, SUFFIX_UNLOAD_PROGRESS_MAX_BYTES_V1)?;
        value.validate()?;
        Ok(value)
    }
}

/// Cheap authenticated scheduling status. Polling this DATA never scans or proves a suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::SuffixUnloadStatusV1"
)]
pub struct SuffixUnloadStatusV1 {
    /// Exactly one.
    pub format_version: u16,
    /// Exactly fifteen.
    pub discriminator: u16,
    /// 0 initializing, 1 not applicable, 2 eligible, 3 verifying, 4 ready, 5 pending,
    /// 6 released, 7 exact request absent (named resume only).
    pub state: u16,
    /// 1 no anchor, 2 unsupported step, 3 no suffix; zero except for state one.
    pub reason: u16,
    /// Current released monetary sequence, if initialized.
    pub current_sequence: Option<u128>,
    /// Latest verified ordinary fold sequence, if present.
    pub folded_sequence: Option<u128>,
    /// Exact selected request for states three through six, or queried absent request for seven.
    pub request_id: Option<[u8; 32]>,
    /// Completed steps in the selected pass.
    pub completed_steps: u128,
    /// Total selected steps.
    pub total_steps: u128,
}
impl SuffixUnloadStatusV1 {
    /// Validate finite status codes and consistent counters.
    /// # Errors
    /// Unknown codes, inconsistent presence, sequences or counters.
    pub fn validate(&self) -> SuffixResult<()> {
        tag(self.format_version, self.discriminator, 15)?;
        if self.state > 7
            || (self.state == 1 && !(1..=3).contains(&self.reason))
            || (self.state != 1 && self.reason != 0)
            || self
                .folded_sequence
                .is_some_and(|folded| self.current_sequence.is_none_or(|current| folded > current))
        {
            return Err(invalid("suffix.status.state"));
        }
        if self.state == 7 {
            let request = self
                .request_id
                .ok_or_else(|| invalid("suffix.status.absent_request"))?;
            require_nonzero("suffix.status.absent_request", &request)?;
            if self.current_sequence.is_none() || self.completed_steps != 0 || self.total_steps != 0
            {
                return Err(invalid("suffix.status.absent_geometry"));
            }
        } else if self.state >= 3 {
            let request = self
                .request_id
                .ok_or_else(|| invalid("suffix.status.request"))?;
            SuffixUnloadProgressV1 {
                format_version: 1,
                discriminator: 14,
                request_id: request,
                completed_steps: self.completed_steps,
                total_steps: self.total_steps,
            }
            .validate()?;
            if self.state >= 4 && self.completed_steps != self.total_steps {
                return Err(invalid("suffix.status.complete"));
            }
        } else if self.request_id.is_some() || self.completed_steps != 0 || self.total_steps != 0 {
            return Err(invalid("suffix.status.empty"));
        }
        Ok(())
    }
    /// Encode exact bounded status DATA.
    /// # Errors
    /// Invalid status or resource refusal.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, SUFFIX_UNLOAD_STATUS_MAX_BYTES_V1)
    }
    /// Decode exact bounded status DATA.
    /// # Errors
    /// Noncanonical, invalid or oversized status.
    pub fn decode_canonical(bytes: &[u8]) -> SuffixResult<Self> {
        let value: Self = decode_frame_v1(bytes, SUFFIX_UNLOAD_STATUS_MAX_BYTES_V1)?;
        value.validate()?;
        Ok(value)
    }
}

/// Exact released suffix summary. Only certified Finalize confirms the online payout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::SuffixUnloadReleasedV1"
)]
pub struct SuffixUnloadReleasedV1 {
    /// Exactly one.
    pub format_version: u16,
    /// Exactly sixteen.
    pub discriminator: u16,
    /// Immutable request identity.
    pub request_id: [u8; 32],
    /// Immutable authority/header digest.
    pub authority_digest: [u8; 32],
    /// Irreversibly debited offline face amount.
    pub amount: u128,
    /// Verified semantic steps.
    pub step_count: u128,
    /// Immutable Append count.
    pub chunk_count: u128,
    /// Begin, Appends and Finalize count.
    pub instruction_count: u128,
}
impl SuffixUnloadReleasedV1 {
    /// Validate this release summary as DATA only.
    /// # Errors
    /// Invalid tags, identities, amount or counts.
    pub fn validate(&self) -> SuffixResult<()> {
        tag(self.format_version, self.discriminator, 16)?;
        require_nonzero("suffix.released.request", &self.request_id)?;
        require_nonzero("suffix.released.authority", &self.authority_digest)?;
        if self.amount == 0 || !kagemusha_wallet_is_canonical_field_v1(&self.authority_digest) {
            return Err(invalid("suffix.released.amount_or_authority"));
        }
        counts(self.step_count, self.chunk_count, self.instruction_count)
    }
    /// Encode exact bounded release DATA.
    /// # Errors
    /// Invalid summary or resource refusal.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, SUFFIX_UNLOAD_STATUS_MAX_BYTES_V1)
    }
    /// Decode exact bounded release DATA.
    /// # Errors
    /// Noncanonical, invalid or oversized summary.
    pub fn decode_canonical(bytes: &[u8]) -> SuffixResult<Self> {
        let value: Self = decode_frame_v1(bytes, SUFFIX_UNLOAD_STATUS_MAX_BYTES_V1)?;
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scheduling_frames_round_trip_and_reject_impossible_states() {
        let progress = SuffixUnloadProgressV1 {
            format_version: 1,
            discriminator: 14,
            request_id: [1; 32],
            completed_steps: 17,
            total_steps: 50,
        };
        let bytes = progress.to_canonical_bytes().unwrap();
        assert_eq!(
            SuffixUnloadProgressV1::decode_canonical(&bytes).unwrap(),
            progress
        );
        for value in [
            SuffixUnloadProgressV1 {
                discriminator: 15,
                ..progress
            },
            SuffixUnloadProgressV1 {
                completed_steps: 51,
                ..progress
            },
            SuffixUnloadProgressV1 {
                total_steps: 4097,
                ..progress
            },
            SuffixUnloadProgressV1 {
                request_id: [0; 32],
                ..progress
            },
        ] {
            assert!(value.to_canonical_bytes().is_err());
        }
        let status = SuffixUnloadStatusV1 {
            format_version: 1,
            discriminator: 15,
            state: 3,
            reason: 0,
            current_sequence: Some(99),
            folded_sequence: Some(49),
            request_id: Some([1; 32]),
            completed_steps: 17,
            total_steps: 50,
        };
        assert_eq!(
            SuffixUnloadStatusV1::decode_canonical(&status.to_canonical_bytes().unwrap()).unwrap(),
            status
        );
        for value in [
            SuffixUnloadStatusV1 { state: 7, ..status },
            SuffixUnloadStatusV1 { state: 4, ..status },
            SuffixUnloadStatusV1 {
                request_id: None,
                ..status
            },
            SuffixUnloadStatusV1 {
                folded_sequence: Some(100),
                ..status
            },
            SuffixUnloadStatusV1 {
                reason: 1,
                ..status
            },
            SuffixUnloadStatusV1 { state: 2, ..status },
        ] {
            assert!(value.to_canonical_bytes().is_err());
        }
    }
    #[test]
    fn release_summary_has_exact_counts_and_no_payout_semantics() {
        let mut authority = [0; 32];
        authority[0] = 7;
        let released = SuffixUnloadReleasedV1 {
            format_version: 1,
            discriminator: 16,
            request_id: [1; 32],
            authority_digest: authority,
            amount: 500,
            step_count: 50,
            chunk_count: 7,
            instruction_count: 9,
        };
        let bytes = released.to_canonical_bytes().unwrap();
        assert_eq!(
            SuffixUnloadReleasedV1::decode_canonical(&bytes).unwrap(),
            released
        );
        assert!(
            SuffixUnloadReleasedV1 {
                instruction_count: 8,
                ..released
            }
            .validate()
            .is_err()
        );
        assert!(
            SuffixUnloadReleasedV1 {
                chunk_count: 0,
                ..released
            }
            .validate()
            .is_err()
        );
        assert!(
            SuffixUnloadReleasedV1 {
                amount: 0,
                ..released
            }
            .validate()
            .is_err()
        );
        assert!(
            SuffixUnloadReleasedV1 {
                step_count: 57,
                ..released
            }
            .validate()
            .is_err()
        );
        assert!(
            SuffixUnloadReleasedV1 {
                authority_digest: [255; 32],
                ..released
            }
            .validate()
            .is_err()
        );
        let mut extra = bytes;
        extra.push(0);
        assert!(SuffixUnloadReleasedV1::decode_canonical(&extra).is_err());
        assert_eq!(std::mem::align_of::<SuffixUnloadReviewProjectionV1>(), 16);
        assert_eq!(std::mem::align_of::<SuffixUnloadProgressV1>(), 16);
        assert_eq!(std::mem::align_of::<SuffixUnloadStatusV1>(), 16);
        assert_eq!(std::mem::align_of::<SuffixUnloadReleasedV1>(), 16);
    }
}

#[cfg(test)]
mod absence_tests {
    use super::*;

    fn absent() -> SuffixUnloadStatusV1 {
        SuffixUnloadStatusV1 {
            format_version: 1,
            discriminator: 15,
            state: 7,
            reason: 0,
            current_sequence: Some((1_u128 << 64) + 3),
            folded_sequence: Some(0),
            request_id: Some([1; 32]),
            completed_steps: 0,
            total_steps: 0,
        }
    }

    #[test]
    fn named_request_absence_roundtrips_wide_and_unfolded_selected_heads() {
        let wide = absent();
        let mut unfolded = absent();
        unfolded.current_sequence = Some(0);
        unfolded.folded_sequence = None;
        for value in [wide, unfolded] {
            let bytes = value.to_canonical_bytes().unwrap();
            assert_eq!(
                SuffixUnloadStatusV1::decode_canonical(&bytes).unwrap(),
                value
            );
        }
    }

    #[test]
    fn named_request_absence_rejects_invalid_identity_and_progress() {
        let base = absent();
        let mut mutations = vec![];
        let mut value = base;
        value.current_sequence = None;
        value.folded_sequence = None;
        mutations.push(value);
        let mut value = base;
        value.request_id = None;
        mutations.push(value);
        let mut value = base;
        value.request_id = Some([0; 32]);
        mutations.push(value);
        let mut value = base;
        value.reason = 1;
        mutations.push(value);
        let mut value = base;
        value.total_steps = 1;
        mutations.push(value);
        let mut value = base;
        value.completed_steps = 1;
        mutations.push(value);
        let mut value = base;
        value.folded_sequence = Some(base.current_sequence.unwrap() + 1);
        mutations.push(value);
        let mut value = base;
        value.state = 8;
        mutations.push(value);
        for value in mutations {
            assert!(value.validate().is_err());
            let bytes = norito::encode_canonical(&value).unwrap();
            assert!(SuffixUnloadStatusV1::decode_canonical(&bytes).is_err());
        }
    }
}
