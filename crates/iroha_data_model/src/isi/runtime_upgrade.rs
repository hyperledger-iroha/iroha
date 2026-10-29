//! Instructions of the live-upgrade lifecycle that are not governance proposals
//! (`specs/live_runtime_upgrades.md` §6 and §7.1).
//!
//! Change sets are proposed and cancelled through SORA Parliament
//! ([`crate::isi::governance::ProposeRuntimeUpgradeProposal`]). A validator states with
//! [`AttestRuntimeSupportV1`] that its release executes a scheduled change set's exact
//! successor selection; activation needs the attestation of every validator. The runtime
//! emergency panel closes admission of definitions with [`ContainRuntimeV1`].
use super::*;
use crate::runtime::lifecycle::{ContainmentV1, EndorsementV1, SignedSupportAttestationV1};

use norito::json::{FastJsonWrite, JsonSerialize};
isi! {
    /// Record one validator's signed support for a scheduled change set.
    ///
    /// Any account may submit it; the signature of a current validator's key authenticates it,
    /// and a newer attestation of the same validator replaces its earlier one.
    #[norito_schema(name = "iroha_data_model::isi::runtime_upgrade::AttestRuntimeSupportV1")]
    pub struct AttestRuntimeSupportV1 {
        /// The validator's signed attestation.
        pub attestation: SignedSupportAttestationV1,
    }
}
impl crate::seal::Instruction for AttestRuntimeSupportV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for AttestRuntimeSupportV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        let mut offset = 0usize;
        let attestation = super::decode_aos_canonical_field::<SignedSupportAttestationV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { attestation }, offset))
    }
}
impl AttestRuntimeSupportV1 {
    /// Norito wire identifier.
    pub const WIRE_ID: &'static str = "iroha.runtime.support.attest.v1";
}

isi! {
    /// Contain runtime definitions in an emergency: close their admission from the next block
    /// until the lease ends, unless ordinary governance ratifies or replaces the restriction.
    ///
    /// Any account may submit it; the endorsements of the committed runtime emergency panel
    /// authorize it.
    #[norito_schema(name = "iroha_data_model::isi::runtime_upgrade::ContainRuntimeV1")]
    pub struct ContainRuntimeV1 {
        /// The containment.
        pub containment: ContainmentV1,
        /// Panel members' endorsements of its identity.
        pub endorsements: Vec<EndorsementV1>,
    }
}
impl crate::seal::Instruction for ContainRuntimeV1 {}
impl<'a> norito::core::DecodeFromSlice<'a> for ContainRuntimeV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        let mut offset = 0usize;
        let containment = super::decode_aos_canonical_field::<ContainmentV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let endorsements = super::decode_aos_canonical_field::<Vec<EndorsementV1>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                containment,
                endorsements,
            },
            offset,
        ))
    }
}
impl ContainRuntimeV1 {
    /// Norito wire identifier.
    pub const WIRE_ID: &'static str = "iroha.runtime.contain.v1";
}

impl FastJsonWrite for ContainRuntimeV1 {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"containment\":");
        JsonSerialize::json_serialize(&self.containment, out);
        out.push_str(",\"endorsements\":");
        JsonSerialize::json_serialize(&self.endorsements, out);
        out.push('}');
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        out.begin_container()?;
        let result = (|| -> Result<(), norito::json::BoundedJsonError> {
            out.push_str("{\"containment\":")?;
            JsonSerialize::json_serialize_to(&self.containment, out)?;
            out.push_str(",\"endorsements\":")?;
            JsonSerialize::json_serialize_to(&self.endorsements, out)?;
            out.push('}')?;
            Ok(())
        })();
        out.end_container();
        result?;
        Ok(())
    }
}

impl FastJsonWrite for AttestRuntimeSupportV1 {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"attestation\":");
        JsonSerialize::json_serialize(&self.attestation, out);
        out.push('}');
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        out.begin_container()?;
        let result = (|| -> Result<(), norito::json::BoundedJsonError> {
            out.push_str("{\"attestation\":")?;
            JsonSerialize::json_serialize_to(&self.attestation, out)?;
            out.push('}')?;
            Ok(())
        })();
        out.end_container();
        result?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{
        definition::SelectionId,
        lifecycle::{ChangeSetId, RootScopeV1, SupportAttestationV1},
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use norito::core::DecodeFromSlice;

    fn attest() -> AttestRuntimeSupportV1 {
        let key = KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519);
        let attestation = SupportAttestationV1 {
            network: crate::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::prehashed([0x11; 32]),
            )),
            root: RootScopeV1::Universal,
            change_set: ChangeSetId([1; 32]),
            predecessor: SelectionId([2; 32]),
            successor: SelectionId([3; 32]),
            committee: [4; 32],
            tested_checkpoint: 10,
            valid_from: 11,
            valid_until: 20,
            release: [5; 32],
            validator: key.public_key().clone(),
        }
        .sign(key.private_key())
        .expect("sign");
        AttestRuntimeSupportV1 { attestation }
    }

    #[test]
    fn attestation_json_has_an_exact_checked_bound() {
        let value = attest();
        let json = norito::json::to_json(&value).expect("JSON");
        assert_eq!(
            norito::json::to_json_bounded(&value, json.len()).expect("exact bound"),
            json
        );
        assert_eq!(
            norito::json::to_json_bounded(&value, json.len() - 1),
            Err(norito::json::BoundedJsonError::BodyTooLarge)
        );
    }

    fn contain() -> ContainRuntimeV1 {
        let key = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
        let containment = crate::runtime::lifecycle::ContainmentV1 {
            network: crate::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::prehashed([0x11; 32]),
            )),
            root: RootScopeV1::Universal,
            incident: [0x1c; 32],
            targets: vec![crate::runtime::definition::DefinitionId([3; 32])],
            kind: crate::runtime::lifecycle::ContainmentKindV1::AdmissionClosed,
            lease_until_ms: 1_000_000,
        };
        let endorsement = crate::runtime::lifecycle::EndorsementV1::sign(
            containment.id().expect("identity"),
            10,
            key.public_key().clone(),
            key.private_key(),
        )
        .expect("endorse");
        ContainRuntimeV1 {
            containment,
            endorsements: vec![endorsement],
        }
    }

    #[test]
    fn containment_round_trips_through_codec_slice_and_json() {
        let value = contain();
        let bytes = norito::to_bytes(&value).expect("encode");
        let decoded: ContainRuntimeV1 = norito::decode_from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, value);
        let bare = value.encode();
        let (sliced, used) = ContainRuntimeV1::decode_from_slice(&bare).expect("slice");
        assert_eq!(used, bare.len());
        assert_eq!(sliced, value);
        let json = norito::json::to_json(&value).expect("JSON");
        assert_eq!(
            norito::json::to_json_bounded(&value, json.len()).expect("exact bound"),
            json
        );
    }

    #[test]
    fn attestation_round_trips_through_codec_slice_and_registry() {
        let value = attest();
        let bytes = norito::to_bytes(&value).expect("encode");
        let decoded: AttestRuntimeSupportV1 = norito::decode_from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, value);
        let bare = value.encode();
        let (sliced, used) = AttestRuntimeSupportV1::decode_from_slice(&bare).expect("slice");
        assert_eq!(used, bare.len());
        assert_eq!(sliced, value);

        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id::<AttestRuntimeSupportV1>(AttestRuntimeSupportV1::WIRE_ID);
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<AttestRuntimeSupportV1>(&payload, flags)
                .expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            AttestRuntimeSupportV1::WIRE_ID,
            &framed,
        )
        .expect("registered")
        .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }
}
