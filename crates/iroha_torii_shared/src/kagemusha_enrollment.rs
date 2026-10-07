//! Exact authenticated enrollment operation envelopes; parsing grants no enrollment authority.

use norito::{Decode, Encode, NoritoSchema};

/// Exact canonical POST target. Queries and alternative targets are not accepted.
pub const ENROLLMENT_SERVICE_ROUTE_V1: &str = "/v1/kagemusha/enrollment";
/// Native pre-key dispatch original bound, checked again by the native owner.
pub const ENROLLMENT_DISPATCH_MAX_BYTES_V1: usize = 16_384;
/// Native account-signed E5 original bound, checked again by the native owner.
pub const ENROLLMENT_EVIDENCE_REQUEST_MAX_BYTES_V1: usize = 524_288;
/// Complete request frame bound including the dispatch, E5 and bounded Norito framing.
pub const ENROLLMENT_SERVICE_REQUEST_MAX_BYTES_V1: usize = 544 * 1024;
/// Native E6 original bound; this is an online enrollment message, not an offline Payment.
pub const ENROLLMENT_RESULT_MAX_BYTES_V1: usize = 262_144;
/// Complete response envelope bound including the original and Norito framing.
pub const ENROLLMENT_SERVICE_RESPONSE_MAX_BYTES_V1: usize = 264 * 1024;

/// One closed issuer action. The entire envelope is covered by native Torii account authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito_schema(name = "iroha.torii.kagemusha.enrollment.action.v1")]
pub enum EnrollmentServiceActionV1 {
    /// Obtain or recover the exact durable pre-key permit before mobile key generation.
    #[codec(index = 0)]
    PreKey,
    /// Submit/recover the exact account-signed evidence request; never reset a consumed attempt.
    #[codec(index = 1)]
    Evidence,
    /// Issue/recover the exact credential after retained platform evidence and fresh eligibility.
    #[codec(index = 2)]
    Issue,
    /// Deliver the exact retained E6 after fresh current eligibility.
    #[codec(index = 3)]
    Deliver,
}

/// Public request DATA; there are no key paths, worker selectors or caller verdict fields.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito_schema(name = "iroha.torii.kagemusha.enrollment.request.v1")]
pub struct EnrollmentServiceRequestV1 {
    /// Exactly one for the sole first-release format.
    pub version: u16,
    /// Exact operation covered by the account signature.
    pub action: EnrollmentServiceActionV1,
    /// Exact native PreKeyDispatchV1 original used at every recovery boundary.
    pub dispatch_original: Vec<u8>,
    /// Exact native account-signed RequestV1, required only for Evidence.
    pub evidence_original: Vec<u8>,
}
fn invalid() -> norito::Error {
    norito::Error::Message("invalid enrollment service envelope".into())
}
impl EnrollmentServiceRequestV1 {
    /// Validate finite envelope shape only; native originals still require their complete checks.
    /// # Errors
    /// Unknown version, missing/oversized originals or evidence on the wrong action.
    pub fn validate(&self) -> Result<(), norito::Error> {
        if self.version != 1
            || self.dispatch_original.is_empty()
            || self.dispatch_original.len() > ENROLLMENT_DISPATCH_MAX_BYTES_V1
            || self.evidence_original.len() > ENROLLMENT_EVIDENCE_REQUEST_MAX_BYTES_V1
            || (self.action == EnrollmentServiceActionV1::Evidence)
                != !self.evidence_original.is_empty()
        {
            return Err(invalid());
        }
        Ok(())
    }
    /// Encode exact bytes for canonical Torii account signing; there is no fallback format.
    /// # Errors
    /// Invalid shape, codec failure or complete frame beyond the fixed online bound.
    pub fn canonical_wire(&self) -> Result<Vec<u8>, norito::Error> {
        self.validate()?;
        if norito::canonical_frame_len(self)? > ENROLLMENT_SERVICE_REQUEST_MAX_BYTES_V1 {
            return Err(invalid());
        }
        norito::encode_canonical(self)
    }
    /// Decode exactly one bounded canonical operation envelope.
    /// # Errors
    /// Malformed, noncanonical, trailing or excessive bytes or invalid field shape.
    pub fn decode_canonical(original: &[u8]) -> Result<Self, norito::Error> {
        if original.is_empty() || original.len() > ENROLLMENT_SERVICE_REQUEST_MAX_BYTES_V1 {
            return Err(invalid());
        }
        let value: Self = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(original.len()),
        )?;
        value.validate()?;
        Ok(value)
    }
}

/// Explicit operation result. A pending response never authorizes a new verification attempt.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito_schema(name = "iroha.torii.kagemusha.enrollment.response.v1")]
pub enum EnrollmentServiceResponseV1 {
    /// Exact durably retained canonical pre-key permit.
    #[codec(index = 0)]
    Permit(Vec<u8>),
    /// Exact platform evidence is durable; subsequent Issue remains separately authorized.
    #[codec(index = 1)]
    EvidenceReady,
    /// The exact attempt remains consumed; retry only recovers that operation.
    #[codec(index = 2)]
    Pending,
    /// E6 is durably retained; Deliver checks fresh eligibility before returning it.
    #[codec(index = 3)]
    CredentialReady,
    /// Exact original E6, never regenerated or refreshed on retry.
    #[codec(index = 4)]
    Credential(Vec<u8>),
}
impl EnrollmentServiceResponseV1 {
    fn validate(&self) -> Result<(), norito::Error> {
        match self {
            Self::Permit(original)
                if original.is_empty()
                    || original.len()
                        > iroha_data_model::kagemusha::KAGEMUSHA_ENROLLMENT_PERMIT_MAX_BYTES_V1 =>
            {
                Err(invalid())
            }
            Self::Credential(original)
                if original.is_empty() || original.len() > ENROLLMENT_RESULT_MAX_BYTES_V1 =>
            {
                Err(invalid())
            }
            _ => Ok(()),
        }
    }
    /// Encode bounded exact service output; callers separately verify signed originals.
    /// # Errors
    /// Missing/oversized originals, codec failure or excessive total frame.
    pub fn canonical_wire(&self) -> Result<Vec<u8>, norito::Error> {
        self.validate()?;
        if norito::canonical_frame_len(self)? > ENROLLMENT_SERVICE_RESPONSE_MAX_BYTES_V1 {
            return Err(invalid());
        }
        norito::encode_canonical(self)
    }
    /// Decode only the current complete canonical response format.
    /// # Errors
    /// Invalid shape, malformed/noncanonical bytes, trailing input or excessive frame.
    pub fn decode_canonical(original: &[u8]) -> Result<Self, norito::Error> {
        if original.is_empty() || original.len() > ENROLLMENT_SERVICE_RESPONSE_MAX_BYTES_V1 {
            return Err(invalid());
        }
        let value: Self = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(original.len()),
        )?;
        value.validate()?;
        Ok(value)
    }
}
#[cfg(test)]
mod tests;
