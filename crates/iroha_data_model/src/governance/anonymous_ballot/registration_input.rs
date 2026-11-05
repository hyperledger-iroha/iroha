//! Additive original input for candidate credential preparation.
//!
//! This payload is deliberately not registered as an executable instruction. It is
//! carried by the ordinary opaque envelope until a governed successor publishes its
//! interface and complete State transition. It changes no existing Parliament enum
//! or frozen instruction layout and grants no qualification or enactment authority.

use super::CredentialRegistrationV1;
use crate::governance::types::{BodyElectionAttemptId, GovernanceAttemptId};

/// A binding seat's proof-carrying registration; State derives the member and assignment.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::RegisterParliamentCredentialV1"
)]
pub struct RegisterParliamentCredentialV1 {
    /// Existing committed governance attempt; not caller-provided attempt contents.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Existing invitation election; State derives its body, window and assignment.
    pub election_attempt_id: BodyElectionAttemptId,
    /// Actual registration relation for the signing authority's derived assignment.
    pub registration: CredentialRegistrationV1,
}

impl RegisterParliamentCredentialV1 {
    /// Reserved additive wire name; no current registry or selection activates it.
    pub const WIRE_ID: &'static str = "iroha.governance.parliament.credential.register.v1";
    /// Fixed proof plus typed IDs, commitment, authorization and framing overhead.
    pub const MAX_FRAME_BYTES: usize = 164 * 1024;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        governance::anonymous_ballot::{
            CredentialCommitmentV1, RegistrationAuthorizationV1, RegistrationProofBytesV1,
        },
        isi::{InstructionBox, OpaqueInstructionV1},
    };

    #[test]
    fn credential_registration_input_is_bounded_canonical_and_remains_opaque() {
        // Structural codec fixture only. Real proof acceptance is tested by Core.
        let input = RegisterParliamentCredentialV1 {
            governance_attempt_id: GovernanceAttemptId::new([1; 32]),
            election_attempt_id: BodyElectionAttemptId::new([2; 32]),
            registration: CredentialRegistrationV1 {
                commitment: CredentialCommitmentV1::new([3; 128]),
                authorization: RegistrationAuthorizationV1::new([4; 64]),
                proof: RegistrationProofBytesV1::new(vec![5; 160_148]).unwrap(),
            },
        };
        let bytes = norito::encode_canonical(&input).unwrap();
        assert!(bytes.len() <= RegisterParliamentCredentialV1::MAX_FRAME_BYTES);
        assert_eq!(
            norito::decode_canonical::<RegisterParliamentCredentialV1>(&bytes).unwrap(),
            input
        );
        let json = norito::json::to_json(&input).unwrap();
        assert_eq!(
            norito::json::from_str::<RegisterParliamentCredentialV1>(&json).unwrap(),
            input
        );
        let envelope: InstructionBox =
            OpaqueInstructionV1::new(RegisterParliamentCredentialV1::WIRE_ID, bytes)
                .unwrap()
                .into();
        let wire = norito::encode_canonical(&envelope).unwrap();
        let decoded = norito::decode_canonical::<InstructionBox>(&wire).unwrap();
        assert!(
            decoded.as_opaque().is_some(),
            "no registry or dispatch activation"
        );
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), wire);
    }
}
