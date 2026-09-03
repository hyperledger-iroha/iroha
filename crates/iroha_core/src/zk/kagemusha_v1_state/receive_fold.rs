//! Canonical host preparation for one KAGEMUSHA `ReceiveFold` credit.

use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::{CreditIdV1, DigestV1};

/// Domain separating the canonical KAGEMUSHA V1 receive-credit transcript.
pub const KAGEMUSHA_RECEIVE_FOLD_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:receive-fold\0";
/// Exact byte length of one canonical receive-credit transcript.
pub const KAGEMUSHA_RECEIVE_FOLD_CREDIT_BYTES_V1: usize = 16 + 32 + 32 + 32 + 32 + 32 + 32;

/// One exact credit consumed by a KAGEMUSHA V1 `ReceiveFold` transition.
///
/// The canonical transcript is `amount:u128-le || credit_id || lane ||
/// incoming_proof_binding || receiver_binding_digest || payment_output_digest ||
/// envelope_digest`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiveFoldCreditV1 {
    /// Positive credit amount in atomic units.
    pub amount: u128,
    /// Unique receiver-bound credit identity.
    pub credit_id: CreditIdV1,
    /// Recipient lane committed by the incoming proof.
    pub recipient_lane_id: DigestV1,
    /// Binding of the exact incoming proof and its private history.
    pub incoming_proof_binding_digest: DigestV1,
    /// Digest binding the receiver hardware credential.
    pub receiver_binding_digest: DigestV1,
    /// Digest of the sender's compact terminal public payment output.
    pub payment_output_digest: DigestV1,
    /// Digest of the exact canonical payment envelope.
    pub envelope_digest: DigestV1,
}

impl ReceiveFoldCreditV1 {
    /// Validate every mandatory field of the singular received credit.
    pub fn validate(self) -> Result<(), ReceiveFoldErrorV1> {
        if self.amount == 0 {
            return Err(ReceiveFoldErrorV1::ZeroAmount);
        }
        for (digest, error) in [
            (self.credit_id.0, ReceiveFoldErrorV1::ZeroCreditId),
            (self.recipient_lane_id, ReceiveFoldErrorV1::ZeroLane),
            (
                self.incoming_proof_binding_digest,
                ReceiveFoldErrorV1::ZeroIncomingProofBinding,
            ),
            (
                self.receiver_binding_digest,
                ReceiveFoldErrorV1::ZeroReceiverBindingDigest,
            ),
            (
                self.payment_output_digest,
                ReceiveFoldErrorV1::ZeroPaymentOutputDigest,
            ),
            (self.envelope_digest, ReceiveFoldErrorV1::ZeroEnvelopeDigest),
        ] {
            if digest == [0; 32] {
                return Err(error);
            }
        }
        Ok(())
    }

    /// Encode the exact canonical receive-credit transcript.
    #[must_use]
    pub fn canonical_transcript_bytes(self) -> [u8; KAGEMUSHA_RECEIVE_FOLD_CREDIT_BYTES_V1] {
        let mut bytes = [0_u8; KAGEMUSHA_RECEIVE_FOLD_CREDIT_BYTES_V1];
        bytes[..16].copy_from_slice(&self.amount.to_le_bytes());
        bytes[16..48].copy_from_slice(&self.credit_id.0);
        bytes[48..80].copy_from_slice(&self.recipient_lane_id);
        bytes[80..112].copy_from_slice(&self.incoming_proof_binding_digest);
        bytes[112..144].copy_from_slice(&self.receiver_binding_digest);
        bytes[144..176].copy_from_slice(&self.payment_output_digest);
        bytes[176..208].copy_from_slice(&self.envelope_digest);
        bytes
    }
}

/// Exact input for the singular consumed-credit replay-root update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiveFoldReplayRootUpdateInputV1 {
    /// Unique sparse-tree key inserted by this update.
    pub credit_id: CreditIdV1,
    /// Exact envelope digest committed by the inserted present leaf.
    pub envelope_digest: DigestV1,
}

/// Validated fixed-shape input for one `ReceiveFold` transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiveFoldV1 {
    credit: ReceiveFoldCreditV1,
}

impl ReceiveFoldV1 {
    /// Validate and construct one singular receive fold.
    pub fn try_new(credit: ReceiveFoldCreditV1) -> Result<Self, ReceiveFoldErrorV1> {
        credit.validate()?;
        Ok(Self { credit })
    }

    /// Return the exact received credit.
    #[must_use]
    pub const fn credit(&self) -> ReceiveFoldCreditV1 {
        self.credit
    }

    /// Return the positive amount added to the balance.
    #[must_use]
    pub const fn amount(&self) -> u128 {
        self.credit.amount
    }

    /// Encode the fixed receive-credit transcript.
    #[must_use]
    pub fn canonical_body_bytes(&self) -> [u8; KAGEMUSHA_RECEIVE_FOLD_CREDIT_BYTES_V1] {
        self.credit.canonical_transcript_bytes()
    }

    /// Hash the domain and singular credit transcript.
    #[must_use]
    pub fn canonical_transcript_digest(&self) -> DigestV1 {
        let mut hasher = Sha256::new();
        hasher.update(KAGEMUSHA_RECEIVE_FOLD_DOMAIN_V1);
        hasher.update(self.canonical_body_bytes());
        hasher.finalize().into()
    }

    /// Return the only replay-root update consumed by this operation.
    #[must_use]
    pub const fn replay_root_update_input(&self) -> ReceiveFoldReplayRootUpdateInputV1 {
        ReceiveFoldReplayRootUpdateInputV1 {
            credit_id: self.credit.credit_id,
            envelope_digest: self.credit.envelope_digest,
        }
    }
}

/// Validation failures for a singular receive fold.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ReceiveFoldErrorV1 {
    /// The received amount was zero.
    #[error("KAGEMUSHA receive-fold credit has a zero amount")]
    ZeroAmount,
    /// The credit ID was zero.
    #[error("KAGEMUSHA receive-fold credit has a zero credit ID")]
    ZeroCreditId,
    /// The recipient lane was zero.
    #[error("KAGEMUSHA receive-fold credit has a zero lane")]
    ZeroLane,
    /// The incoming-proof binding was zero.
    #[error("KAGEMUSHA receive-fold credit has a zero incoming-proof binding")]
    ZeroIncomingProofBinding,
    /// The receiver-binding digest was zero.
    #[error("KAGEMUSHA receive-fold credit has a zero receiver-binding digest")]
    ZeroReceiverBindingDigest,
    /// The payment-output digest was zero.
    #[error("KAGEMUSHA receive-fold credit has a zero payment-output digest")]
    ZeroPaymentOutputDigest,
    /// The envelope digest was zero.
    #[error("KAGEMUSHA receive-fold credit has a zero envelope digest")]
    ZeroEnvelopeDigest,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credit() -> ReceiveFoldCreditV1 {
        ReceiveFoldCreditV1 {
            amount: 7,
            credit_id: CreditIdV1([1; 32]),
            recipient_lane_id: [2; 32],
            incoming_proof_binding_digest: [3; 32],
            receiver_binding_digest: [4; 32],
            payment_output_digest: [5; 32],
            envelope_digest: [6; 32],
        }
    }

    #[test]
    fn singular_transcript_is_fixed_and_bound() {
        let fold = ReceiveFoldV1::try_new(credit()).expect("valid credit");
        assert_eq!(fold.canonical_body_bytes().len(), 208);
        assert_ne!(fold.canonical_transcript_digest(), [0; 32]);
        assert_eq!(fold.replay_root_update_input().credit_id, CreditIdV1([1; 32]));
    }

    #[test]
    fn every_required_field_fails_closed() {
        let mut invalid = credit();
        invalid.amount = 0;
        assert_eq!(
            ReceiveFoldV1::try_new(invalid),
            Err(ReceiveFoldErrorV1::ZeroAmount)
        );
        let mut invalid = credit();
        invalid.credit_id = CreditIdV1([0; 32]);
        assert_eq!(
            ReceiveFoldV1::try_new(invalid),
            Err(ReceiveFoldErrorV1::ZeroCreditId)
        );
    }
}
