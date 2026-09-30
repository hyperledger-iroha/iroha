//! Parent registration and compact quorum-certified anchoring of independent private roots.

use super::*;
use iroha_model_base::topology::DataSpaceId;

isi! {
    /// Bind one private child genesis to the active SNS dataspace owner and ownership generation.
    /// The parent checks its committed admission policy, lease, quotas and exact network binding.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::private_dataspace::RegisterPrivateDataspace")]
    pub struct RegisterPrivateDataspace {
        /// Canonical active SNS dataspace alias.
        pub alias: String,
        /// Compare-and-swap guard against alias transfer or lease replacement.
        pub expected_ownership_generation: u64,
        /// Bounded canonical [`crate::private_dataspace::PrivateDataspaceRegistration`] frame.
        /// Contains public committee credentials and commitments, never the private genesis body.
        #[norito(with = "crate::json_helpers::base64_vec",
            bounded_with = "crate::json_helpers::base64_vec::serialize_bounded")]
        pub registration: Vec<u8>,
    }
}

isi! {
    /// Extend a registered private root with the next genuine quorum-certified decision.
    /// Relayers pay normal fees; current SNS ownership and exact registered child authority apply.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::private_dataspace::AnchorPrivateDataspace")]
    pub struct AnchorPrivateDataspace {
        /// Exact full-width private dataspace identity.
        pub dataspace_id: DataSpaceId,
        /// Bounded canonical [`crate::private_dataspace::PrivateDataspaceAnchor`] frame.
        #[norito(with = "crate::json_helpers::base64_vec",
            bounded_with = "crate::json_helpers::base64_vec::serialize_bounded")]
        pub anchor: Vec<u8>,
    }
}

impl RegisterPrivateDataspace {
    /// Canonical wire identifier.
    pub const WIRE_ID: &'static str = "iroha.private_dataspace.register.v1";
}
impl AnchorPrivateDataspace {
    /// Canonical wire identifier.
    pub const WIRE_ID: &'static str = "iroha.private_dataspace.anchor.v1";
}
impl crate::seal::Instruction for RegisterPrivateDataspace {}
impl crate::seal::Instruction for AnchorPrivateDataspace {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_dataspace_instructions_use_native_registry_and_full_u64_identity() {
        let registration = RegisterPrivateDataspace {
            alias: "acme".into(),
            expected_ownership_generation: u64::MAX,
            registration: vec![1, 2],
        };
        let anchor = AnchorPrivateDataspace {
            dataspace_id: DataSpaceId::new(u64::MAX),
            anchor: vec![3, 4],
        };
        for (wire_id, instruction) in [
            (
                RegisterPrivateDataspace::WIRE_ID,
                InstructionBox::from(registration.clone()),
            ),
            (
                AnchorPrivateDataspace::WIRE_ID,
                InstructionBox::from(anchor.clone()),
            ),
        ] {
            assert!(crate::instruction_registry::default().contains(wire_id));
            let bytes = norito::encode_canonical(&instruction).unwrap();
            assert_eq!(
                norito::decode_canonical::<InstructionBox>(&bytes).unwrap(),
                instruction
            );
        }
        let json = norito::json::to_json(&registration).unwrap();
        assert_eq!(
            norito::json::from_str::<RegisterPrivateDataspace>(&json).unwrap(),
            registration
        );
        let json = norito::json::to_json(&anchor).unwrap();
        assert_eq!(
            norito::json::from_str::<AnchorPrivateDataspace>(&json).unwrap(),
            anchor
        );
    }
}
