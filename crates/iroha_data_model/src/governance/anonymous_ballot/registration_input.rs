//! The credential registration of an anonymous Parliament binding seat (design §3.2).
//!
//! [`RegisterParliamentCredentialV1`] is the instruction of the anonymous family's credential
//! interface, added after the frozen baseline by the
//! `iroha.governance.parliament.anonymous.v1` release addition. Its wire ID is registered
//! tolerantly, so until a change set activates the addition its envelope resolves as
//! `definition_inactive`, exactly as on a release that keeps it opaque. It changes no
//! existing Parliament enum or frozen instruction layout and grants no qualification or
//! enactment authority.

use super::CredentialRegistrationV1;
use crate::governance::types::{BodyElectionAttemptId, GovernanceAttemptId};

/// A binding seat's proof-carrying registration; State derives the member and assignment.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
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

impl crate::seal::Instruction for RegisterParliamentCredentialV1 {}

impl RegisterParliamentCredentialV1 {
    /// Stable wire identity of the credential interface.
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
    fn credential_registration_is_a_bounded_canonical_instruction() {
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

        // The typed instruction travels under its wire ID and decodes back to itself.
        let boxed = InstructionBox::from(input.clone());
        assert_eq!(
            crate::isi::instruction_wire_id(&boxed),
            Some(RegisterParliamentCredentialV1::WIRE_ID)
        );
        let wire = norito::encode_canonical(&boxed).unwrap();
        let decoded = norito::decode_canonical::<InstructionBox>(&wire).unwrap();
        assert_eq!(
            decoded
                .as_any()
                .downcast_ref::<RegisterParliamentCredentialV1>(),
            Some(&input)
        );
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), wire);

        // A frame the decoder refuses stays an opaque envelope, as on a release without the
        // instruction.
        let mut trailing = bytes;
        trailing.push(0);
        let envelope: InstructionBox =
            OpaqueInstructionV1::new(RegisterParliamentCredentialV1::WIRE_ID, trailing)
                .unwrap()
                .into();
        let wire = norito::encode_canonical(&envelope).unwrap();
        let decoded = norito::decode_canonical::<InstructionBox>(&wire).unwrap();
        assert!(decoded.as_opaque().is_some());
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), wire);
    }
}
