//! Canonical host-to-secure-device messages for the KAGEMUSHA V1 mint inbox.
//!
//! These bounded messages carry only public authorization and finalized-credit bytes. They do
//! not expose private reservation openings, device key handles, hardware journal snapshots, or
//! complete Guard certificates. Canonical decoding and shape validation never grant monetary
//! authority; Core and a qualified non-forking hardware provider must perform that work.

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{
    KAGEMUSHA_DEVICE_LIFECYCLE_VERSION_V1, KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1,
    KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1, KagemushaMintAuthorizationV1, KagemushaMintCreditV1,
    KagemushaValidationErrorV1,
};

/// Maximum canonical bytes accepted for an operation-16 mint-stage body.
///
/// Both nested frames retain their tighter independent 7,936-byte limits. The larger outer bound
/// matches the already frozen secure-device command payload cap and permits no unbounded decode.
pub const KAGEMUSHA_DEVICE_MINT_STAGE_COMMAND_MAX_BYTES_V1: usize = 64 * 1024;
/// Maximum canonical bytes accepted for the fixed operation-16 public result.
pub const KAGEMUSHA_DEVICE_MINT_STAGE_RESULT_MAX_BYTES_V1: usize = 128;
/// A previously unseen finalized credit was durably staged.
pub const KAGEMUSHA_DEVICE_MINT_STAGE_DISPOSITION_STAGED_V1: u8 = 0;
/// The same canonical credit was already pending or consumed.
pub const KAGEMUSHA_DEVICE_MINT_STAGE_DISPOSITION_EXACT_DUPLICATE_V1: u8 = 1;

/// Public operation-16 input delivered to a qualified secure-device service.
///
/// The nested values remain canonical independent archives so hardware can validate their exact
/// byte identities before it mutates its rollback-resistant inbox.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaDeviceMintStageCommandV1 {
    /// Secure-device lifecycle version.
    pub version: u16,
    /// Exact canonical pre-debit authorization bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub canonical_authorization: Vec<u8>,
    /// Exact canonical finalized mint-credit bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub canonical_mint_credit: Vec<u8>,
}

/// Public operation-16 result returned only after durable hardware completion.
///
/// The complete hardware Guard certificate remains in native authenticated storage. The bounded
/// lifecycle response authenticator separately binds this result and must be verified by the
/// qualified platform adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct KagemushaDeviceMintStageResultV1 {
    /// Secure-device lifecycle version.
    pub version: u16,
    /// Zero for a new stage, one for an exact pending/consumed duplicate.
    pub disposition: u8,
    /// Nonzero identity of the supplied finalized credit.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_id: [u8; 32],
}

impl KagemushaDeviceMintStageCommandV1 {
    /// Construct and validate an operation-16 command from exact nested archives.
    ///
    /// # Errors
    ///
    /// Returns an error for a noncanonical, malformed, oversized, or mismatched nested value.
    pub fn from_canonical_inputs(
        canonical_authorization: Vec<u8>,
        canonical_mint_credit: Vec<u8>,
    ) -> Result<Self, KagemushaValidationErrorV1> {
        let value = Self {
            version: KAGEMUSHA_DEVICE_LIFECYCLE_VERSION_V1,
            canonical_authorization,
            canonical_mint_credit,
        };
        value.validated_inputs()?;
        value.require_encoded_size()?;
        Ok(value)
    }

    /// Decode both nested archives and enforce their complete public binding.
    ///
    /// # Errors
    ///
    /// Returns an error for the wrong version or any invalid authorization/credit pair.
    pub fn validated_inputs(
        &self,
    ) -> Result<(KagemushaMintAuthorizationV1, KagemushaMintCreditV1), KagemushaValidationErrorV1>
    {
        if self.version != KAGEMUSHA_DEVICE_LIFECYCLE_VERSION_V1 {
            return Err(invalid("kagemusha.device.mint_stage.command.version"));
        }
        if self.canonical_authorization.len() > KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1 {
            return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
                actual: self.canonical_authorization.len(),
                max: KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1,
            });
        }
        if self.canonical_mint_credit.len() > KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1 {
            return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
                actual: self.canonical_mint_credit.len(),
                max: KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1,
            });
        }
        let authorization = KagemushaMintAuthorizationV1::decode_canonical_shape_exact(
            &self.canonical_authorization,
        )?;
        let credit =
            KagemushaMintCreditV1::decode_canonical_shape_exact(&self.canonical_mint_credit)?;
        credit.validate_shape_against_authorization(&authorization)?;
        Ok((authorization, credit))
    }

    /// Validate and encode this command as one bounded canonical Norito archive.
    ///
    /// # Errors
    ///
    /// Returns an error when nested validation, encoding, or the outer bound fails.
    pub fn encode_canonical_shape(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        self.validated_inputs()?;
        let bytes = norito::encode_canonical(self)?;
        require_size(&bytes, KAGEMUSHA_DEVICE_MINT_STAGE_COMMAND_MAX_BYTES_V1)?;
        Ok(bytes)
    }

    /// Decode one exact, bounded canonical operation-16 command.
    ///
    /// # Errors
    ///
    /// Returns an error before unbounded allocation or for any noncanonical or invalid input.
    pub fn decode_canonical_shape_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let value: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_DEVICE_MINT_STAGE_COMMAND_MAX_BYTES_V1)?;
        value.validated_inputs()?;
        value.require_encoded_size()?;
        Ok(value)
    }

    fn require_encoded_size(&self) -> Result<(), KagemushaValidationErrorV1> {
        let bytes = norito::encode_canonical(self)?;
        require_size(&bytes, KAGEMUSHA_DEVICE_MINT_STAGE_COMMAND_MAX_BYTES_V1)
    }
}

impl KagemushaDeviceMintStageResultV1 {
    /// Construct the public result of a newly staged credit.
    ///
    /// # Errors
    ///
    /// Returns an error for the reserved all-zero credit identity.
    pub fn staged(credit_id: [u8; 32]) -> Result<Self, KagemushaValidationErrorV1> {
        Self::new(KAGEMUSHA_DEVICE_MINT_STAGE_DISPOSITION_STAGED_V1, credit_id)
    }

    /// Construct the public result of an exact pending or consumed duplicate.
    ///
    /// # Errors
    ///
    /// Returns an error for the reserved all-zero credit identity.
    pub fn exact_duplicate(credit_id: [u8; 32]) -> Result<Self, KagemushaValidationErrorV1> {
        Self::new(
            KAGEMUSHA_DEVICE_MINT_STAGE_DISPOSITION_EXACT_DUPLICATE_V1,
            credit_id,
        )
    }

    /// Validate the closed result discriminant and nonzero credit identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown version, disposition, zero identity, or oversized archive.
    pub fn validate_shape(&self) -> Result<(), KagemushaValidationErrorV1> {
        if self.version != KAGEMUSHA_DEVICE_LIFECYCLE_VERSION_V1 {
            return Err(invalid("kagemusha.device.mint_stage.result.version"));
        }
        if !matches!(
            self.disposition,
            KAGEMUSHA_DEVICE_MINT_STAGE_DISPOSITION_STAGED_V1
                | KAGEMUSHA_DEVICE_MINT_STAGE_DISPOSITION_EXACT_DUPLICATE_V1
        ) {
            return Err(invalid("kagemusha.device.mint_stage.result.disposition"));
        }
        if self.credit_id == [0; 32] {
            return Err(invalid("kagemusha.device.mint_stage.result.credit_id"));
        }
        let bytes = norito::encode_canonical(self)?;
        require_size(&bytes, KAGEMUSHA_DEVICE_MINT_STAGE_RESULT_MAX_BYTES_V1)
    }

    /// Bind this public result to the exact finalized credit carried by a command.
    ///
    /// This remains a structural check. It is not a substitute for response authentication or
    /// qualified hardware journal verification.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid command/result or a substituted credit identity.
    pub fn validate_shape_against_command(
        &self,
        command: &KagemushaDeviceMintStageCommandV1,
    ) -> Result<(), KagemushaValidationErrorV1> {
        self.validate_shape()?;
        let (_, credit) = command.validated_inputs()?;
        if self.credit_id != credit.statement.lifecycle.credit_id {
            return Err(invalid("kagemusha.device.mint_stage.result.credit_id"));
        }
        Ok(())
    }

    /// Encode this result as one bounded canonical Norito archive.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid result or codec failure.
    pub fn encode_canonical_shape(&self) -> Result<Vec<u8>, KagemushaValidationErrorV1> {
        self.validate_shape()?;
        Ok(norito::encode_canonical(self)?)
    }

    /// Decode one exact, bounded canonical operation-16 result.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid bytes.
    pub fn decode_canonical_shape_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationErrorV1> {
        let value: Self =
            decode_bounded_canonical(bytes, KAGEMUSHA_DEVICE_MINT_STAGE_RESULT_MAX_BYTES_V1)?;
        value.validate_shape()?;
        Ok(value)
    }

    fn new(disposition: u8, credit_id: [u8; 32]) -> Result<Self, KagemushaValidationErrorV1> {
        let value = Self {
            version: KAGEMUSHA_DEVICE_LIFECYCLE_VERSION_V1,
            disposition,
            credit_id,
        };
        value.validate_shape()?;
        Ok(value)
    }
}

fn decode_bounded_canonical<T>(bytes: &[u8], max: usize) -> Result<T, KagemushaValidationErrorV1>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    require_size(bytes, max)?;
    let limits = norito::canonical_decode_limits(bytes.len());
    Ok(norito::decode_canonical_with_limits(bytes, limits)?)
}

fn require_size(bytes: &[u8], max: usize) -> Result<(), KagemushaValidationErrorV1> {
    if bytes.len() > max {
        return Err(KagemushaValidationErrorV1::EncodedSizeExceeded {
            actual: bytes.len(),
            max,
        });
    }
    Ok(())
}

fn invalid(field: &'static str) -> KagemushaValidationErrorV1 {
    KagemushaValidationErrorV1::InvalidField { field }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_stage_result_roundtrips_and_rejects_mutation() {
        for value in [
            KagemushaDeviceMintStageResultV1::staged([1; 32]).unwrap(),
            KagemushaDeviceMintStageResultV1::exact_duplicate([2; 32]).unwrap(),
        ] {
            let bytes = value.encode_canonical_shape().unwrap();
            assert!(bytes.len() <= KAGEMUSHA_DEVICE_MINT_STAGE_RESULT_MAX_BYTES_V1);
            assert_eq!(
                KagemushaDeviceMintStageResultV1::decode_canonical_shape_exact(&bytes).unwrap(),
                value
            );

            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(
                KagemushaDeviceMintStageResultV1::decode_canonical_shape_exact(&trailing).is_err()
            );
        }

        assert!(KagemushaDeviceMintStageResultV1::staged([0; 32]).is_err());
        assert!(
            KagemushaDeviceMintStageResultV1 {
                version: 2,
                disposition: KAGEMUSHA_DEVICE_MINT_STAGE_DISPOSITION_STAGED_V1,
                credit_id: [1; 32],
            }
            .validate_shape()
            .is_err()
        );
        assert!(
            KagemushaDeviceMintStageResultV1 {
                version: 1,
                disposition: 2,
                credit_id: [1; 32],
            }
            .validate_shape()
            .is_err()
        );
    }

    #[test]
    fn mint_stage_command_rejects_unbounded_or_invalid_nested_frames() {
        let oversized = KagemushaDeviceMintStageCommandV1 {
            version: 1,
            canonical_authorization: vec![0; KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1 + 1],
            canonical_mint_credit: vec![1],
        };
        assert!(oversized.validated_inputs().is_err());
        assert!(oversized.encode_canonical_shape().is_err());

        let malformed = KagemushaDeviceMintStageCommandV1 {
            version: 1,
            canonical_authorization: vec![1],
            canonical_mint_credit: vec![2],
        };
        assert!(malformed.validated_inputs().is_err());
        assert!(
            KagemushaDeviceMintStageCommandV1::from_canonical_inputs(vec![1], vec![2]).is_err()
        );

        let wrong_version = KagemushaDeviceMintStageCommandV1 {
            version: 2,
            canonical_authorization: Vec::new(),
            canonical_mint_credit: Vec::new(),
        };
        assert!(wrong_version.validated_inputs().is_err());

        let bytes = norito::encode_canonical(&malformed).unwrap();
        assert!(KagemushaDeviceMintStageCommandV1::decode_canonical_shape_exact(&bytes).is_err());
        assert!(
            decode_bounded_canonical::<KagemushaDeviceMintStageCommandV1>(
                &vec![0; KAGEMUSHA_DEVICE_MINT_STAGE_COMMAND_MAX_BYTES_V1 + 1],
                KAGEMUSHA_DEVICE_MINT_STAGE_COMMAND_MAX_BYTES_V1,
            )
            .is_err()
        );
    }
}
