//! Sole discriminated incoming source data. Decoding never admits a source or request key.
use super::*;
use crate::kagemusha_v1_recursion::KAGEMUSHA_ORDINARY_CASH_OUTGOING_ORIGINAL_MAX_BYTES_V1;
use crate::kagemusha_v1_state::KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1;

const MINT_SOURCE_MAX: usize = KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1
    + KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1
    + KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1
    + KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_MAX_BYTES_V1
    + KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1
    + 16 * 1024;
const RECEIVE_SOURCE_MAX: usize = KAGEMUSHA_ORDINARY_CASH_OUTGOING_ORIGINAL_MAX_BYTES_V1
    + KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1
    + KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1
    + KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1
    + KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_MAX_BYTES_V1
    + 2 * KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1
    + 16 * 1024;
pub(super) const SOURCE_MAX: usize = if MINT_SOURCE_MAX > RECEIVE_SOURCE_MAX {
    MINT_SOURCE_MAX
} else {
    RECEIVE_SOURCE_MAX
};

/// First-release complete source original. The discriminant must match the actual incoming
/// selection; Receive never decodes a Mint original or promotes a Native financial borrower.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::KagemushaOrdinaryIncomingSourceOriginalsV1")]
pub enum KagemushaOrdinaryIncomingSourceOriginalsV1 {
    /// Actual finalized debit, dedicated Mint113 originals and neutral MintAuthority credit.
    Mint(Box<KagemushaOrdinaryIncomingMintOriginalsV1>),
    /// Actual sender Wrapper output and independently admitted immutable Core Commit envelope.
    Receive(Box<KagemushaOrdinaryIncomingReceiveOriginalsV1>),
}
/// Complete mathematical Mint source data; authentic source custody is independently required.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::KagemushaOrdinaryIncomingMintOriginalsV1")]
pub struct KagemushaOrdinaryIncomingMintOriginalsV1 {
    /// Full unsigned request, signed pre-debit decision and genuine finalized receipt/membership.
    pub finalized_mint_original: Vec<u8>,
    /// Complete canonical neutral MintCredit including both proofs and entire histories.
    pub mint_credit_original: Vec<u8>,
    /// Actual historical Mint preparation FI-control original, distinct from fresh incoming W2.
    pub mint_preparation_financial_control_original: Vec<u8>,
    /// Actual old Mint selected PI original; it is not replaced by fresh incoming PI.
    pub mint_selected_integrity_original: Option<Vec<u8>>,
    /// Complete original signed Mint preparation clock cut.
    pub mint_preparation_clock_signed_original: Vec<u8>,
}
/// Complete Receive source data. The private previous request floor and request key remain
/// in actual Native custody/checkpoint; this carrier supplies no journal counter authority.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::KagemushaOrdinaryIncomingReceiveOriginalsV1")]
pub struct KagemushaOrdinaryIncomingReceiveOriginalsV1 {
    /// Entire sender outgoing carrier, including actual Wrapper and sender capture clock.
    pub sender_outgoing_original: Vec<u8>,
    /// Entire immutable Core envelope: signed result, full DATA row and historical finality.
    pub received_assertion_transport_original: Vec<u8>,
    /// Exact issuer-admitted receiver FI certificate retained for the original request.
    pub receiver_financial_certificate_original: Vec<u8>,
    /// Exact full original signed receiver C; no current replacement is permitted.
    pub receiver_credential_original: Vec<u8>,
    /// Exact PI selected for that original request, distinct from current incoming W2/W1 PI.
    pub receiver_request_integrity_original: Option<Vec<u8>>,
    /// Public signed C enrollment minimum only. This never authenticates a private prior floor.
    pub request_enrollment_counter_minimum: Option<u32>,
    /// Actual original request-signature CaptureAck context, backed by the full cut below.
    pub request_signature_capture_context: KagemushaOrdinaryCashClockContextV1,
    /// Two full signed clock originals in request creation then signature-capture order.
    pub request_signed_clock_originals: [Vec<u8>; 2],
}
impl KagemushaOrdinaryIncomingSourceOriginalsV1 {
    pub(super) fn validate_data(
        &self,
        selection: &KagemushaOrdinaryIncomingSourceSelectionV1,
    ) -> Result<()> {
        selection.validate_shape()?;
        match (self, selection) {
            (Self::Mint(value), KagemushaOrdinaryIncomingSourceSelectionV1::Mint { .. }) => {
                for (raw, max) in [
                    (
                        &value.finalized_mint_original,
                        KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1,
                    ),
                    (
                        &value.mint_credit_original,
                        KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1,
                    ),
                    (
                        &value.mint_preparation_financial_control_original,
                        KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
                    ),
                    (
                        &value.mint_preparation_clock_signed_original,
                        KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
                    ),
                ] {
                    require_raw(raw, max)?;
                }
                require_lease(value.mint_selected_integrity_original.as_deref())?;
            }
            (Self::Receive(value), KagemushaOrdinaryIncomingSourceSelectionV1::Receive { .. }) => {
                for (raw, max) in [
                    (
                        &value.sender_outgoing_original,
                        KAGEMUSHA_ORDINARY_CASH_OUTGOING_ORIGINAL_MAX_BYTES_V1,
                    ),
                    (
                        &value.received_assertion_transport_original,
                        KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1,
                    ),
                    (
                        &value.receiver_financial_certificate_original,
                        KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
                    ),
                    (
                        &value.receiver_credential_original,
                        KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1,
                    ),
                ] {
                    require_raw(raw, max)?;
                }
                require_lease(value.receiver_request_integrity_original.as_deref())?;
                value.request_signature_capture_context.validate_shape()?;
                for raw in &value.request_signed_clock_originals {
                    require_raw(
                        raw,
                        KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
                    )?;
                }
            }
            _ => return reject(),
        }
        Ok(())
    }
    pub(super) fn old_mint_control_original(&self) -> Option<&[u8]> {
        match self {
            Self::Mint(v) => Some(&v.mint_preparation_financial_control_original),
            Self::Receive(_) => None,
        }
    }
}
fn require_raw(raw: &[u8], max: usize) -> Result<()> {
    if raw.is_empty() || raw.len() > max {
        return reject();
    }
    Ok(())
}
fn require_lease(raw: Option<&[u8]>) -> Result<()> {
    if let Some(raw) = raw {
        require_raw(raw, KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_MAX_BYTES_V1)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incoming_source_discriminant_cannot_fall_through_to_other_source() {
        // Pure bounded data vectors; no proof, clock, FI, request key or source cap is constructed.
        let mint = KagemushaOrdinaryIncomingSourceOriginalsV1::Mint(Box::new(
            KagemushaOrdinaryIncomingMintOriginalsV1 {
                finalized_mint_original: vec![1],
                mint_credit_original: vec![2],
                mint_preparation_financial_control_original: vec![3],
                mint_selected_integrity_original: None,
                mint_preparation_clock_signed_original: vec![4],
            },
        ));
        let select_mint = KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
            topup_request_original_sha256: [1; 32],
        };
        let select_receive = KagemushaOrdinaryIncomingSourceSelectionV1::Receive {
            sender_commit_transport_original_sha256: [2; 32],
            sender_outgoing_original_sha256: [3; 32],
            recipient_request_original_digest: [4; 32],
            encrypted_credit_original_sha256: [5; 32],
        };
        assert!(mint.validate_data(&select_mint).is_ok());
        assert!(mint.validate_data(&select_receive).is_err());
        let receive = KagemushaOrdinaryIncomingSourceOriginalsV1::Receive(Box::new(
            KagemushaOrdinaryIncomingReceiveOriginalsV1 {
                sender_outgoing_original: vec![1],
                received_assertion_transport_original: vec![2],
                receiver_financial_certificate_original: vec![3],
                receiver_credential_original: vec![4],
                receiver_request_integrity_original: None,
                request_enrollment_counter_minimum: None,
                request_signature_capture_context: KagemushaOrdinaryCashClockContextV1 {
                    version: 1,
                    request_nonce: [5; 32],
                    signed_observations_original_digest: [6; 32],
                    lower_at_ms: 7,
                    upper_at_ms: 8,
                },
                request_signed_clock_originals: [vec![5], vec![6]],
            },
        ));
        assert!(receive.validate_data(&select_receive).is_ok());
        assert!(receive.validate_data(&select_mint).is_err());
        assert!(require_raw(&[], 16).is_err());
        assert!(require_raw(&[1; 17], 16).is_err());
        assert_eq!(SOURCE_MAX, MINT_SOURCE_MAX.max(RECEIVE_SOURCE_MAX));
    }
}
