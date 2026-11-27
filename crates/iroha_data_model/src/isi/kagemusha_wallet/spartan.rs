//! Governed verifier selection, wallet activation and charged Unload for the Spartan profile.
use super::*;

isi! {
    /// The consenting reserve selects one exact signed verifier installation for a fresh scheme.
    #[derive(DeriveJsonSerialize, DeriveJsonDeserialize)]
    #[norito(decode_from_slice)]
    #[norito_schema(name = "iroha.kagemusha.spartan.InstallPrograms.v1")]
    pub struct InstallKagemushaSpartanProgramsV1 {
        /// Registered scheme and its root authority.
        pub scheme: [u8; 32],
        /// Registered asset of the consenting reserve.
        pub asset: [u8; 32],
        /// Independently selected kgwparm1 digest of the canonical manifest.
        pub manifest_digest: [u8; 32],
        /// Exact parameter manifest signed by the scheme-certified Artifact key.
        pub manifest: Vec<u8>,
        /// Fixed-width P-256 signature of the parameter manifest digest.
        pub signature: [u8; 64],
        /// Canonical bounded bundle of all eight public programs.
        pub programs: Vec<u8>,
    }
}
impl crate::seal::Instruction for InstallKagemushaSpartanProgramsV1 {}
impl InstallKagemushaSpartanProgramsV1 {
    /// Governed wire identity, absent from the original ledger interface.
    pub const WIRE_ID: &'static str = "iroha.kagemusha.spartan.programs.install.v1";
    /// Maximum complete installation transport; manifest, signature and bundle share the cap.
    pub const MAX_BYTES: usize = 10 * 1024 * 1024;
    /// Maximum signed manifest frame admitted before decoding any program.
    pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;
}

isi! {
    /// Pay the registered reserve's proven Control Unload claim to its authenticated destinations.
    #[derive(DeriveJsonSerialize, DeriveJsonDeserialize)]
    #[norito(decode_from_slice)]
    #[norito_schema(name = "iroha.kagemusha.spartan.Unload.v1")]
    pub struct UnloadKagemushaSpartanV1 {
        /// Registered scheme.
        pub scheme: [u8; 32],
        /// Registered asset selected independently of the proof.
        pub asset: [u8; 32],
        /// Digest of the installed signed manifest, guarded on every dispatch.
        pub manifest_digest: [u8; 32],
        /// Exact canonical completed Control claim original.
        pub claim: Vec<u8>,
    }
}
impl crate::seal::Instruction for UnloadKagemushaSpartanV1 {}
impl UnloadKagemushaSpartanV1 {
    /// Governed wire identity, absent from the original ledger interface.
    pub const WIRE_ID: &'static str = "iroha.kagemusha.spartan.unload.v1";
}

isi! {
    /// Activate one enrolled wallet using its signed control and completed Bootstrap proof.
    #[derive(DeriveJsonSerialize, DeriveJsonDeserialize)]
    #[norito(decode_from_slice)]
    #[norito_schema(name = "iroha.kagemusha.spartan.Activate.v1")]
    pub struct ActivateKagemushaSpartanV1 {
        /// Registered scheme.
        pub scheme: [u8; 32],
        /// Registered asset scope.
        pub asset: [u8; 32],
        /// Independently selected signed manifest digest.
        pub manifest_digest: [u8; 32],
        /// Enrolled wallet incarnation authenticated again by the Bootstrap verifier.
        pub wallet: [u8; 32],
        /// Exact canonical public Bootstrap claim, including the signed Activate control.
        pub activation: Vec<u8>,
    }
}
impl crate::seal::Instruction for ActivateKagemushaSpartanV1 {}
impl ActivateKagemushaSpartanV1 {
    /// Governed wire identity of the Spartan Activate instruction.
    pub const WIRE_ID: &'static str = "iroha.kagemusha.spartan.activate.v1";
    /// Maximum signed Bootstrap claim in a native ledger instruction.
    pub const MAX_ACTIVATION_BYTES: usize = 32 * 1024;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spartan_activate_uses_the_typed_account_transaction_wire() {
        let activate = ActivateKagemushaSpartanV1 {
            scheme: [1; 32],
            asset: [2; 32],
            manifest_digest: [3; 32],
            wallet: [4; 32],
            activation: vec![0x4b, 0x47, 4],
        };
        let frame = norito::to_bytes(&activate).unwrap();
        let decoded =
            crate::isi::decode_instruction_envelope(ActivateKagemushaSpartanV1::WIRE_ID, &frame)
                .unwrap();
        assert_eq!(decoded.as_any().downcast_ref(), Some(&activate));
        assert_eq!(
            norito::json::from_str::<ActivateKagemushaSpartanV1>(
                &norito::json::to_json(&activate).unwrap()
            )
            .unwrap(),
            activate
        );
        assert_eq!(
            hex::encode(frame),
            "4e52543000009a1a14c7f2632fd04b30d02624e1f63a0090000000000000004326b61176e2052b022001010101010101010101010101010101010101010101010101010101010101012002020202020202020202020202020202020202020202020202020202020202022003030303030303030303030303030303030303030303030303030303030303032004040404040404040404040404040404040404040404040404040404040404040b03000000000000004b4704"
        );
    }

    #[test]
    fn both_spartan_originals_roundtrip_exactly_in_norito_and_json() {
        let install = InstallKagemushaSpartanProgramsV1 {
            scheme: [1; 32],
            asset: [2; 32],
            manifest_digest: [3; 32],
            manifest: vec![0, 255, 4],
            signature: [5; 64],
            programs: vec![6, 0, 7, 255],
        };
        let unload = UnloadKagemushaSpartanV1 {
            scheme: install.scheme,
            asset: install.asset,
            manifest_digest: install.manifest_digest,
            claim: vec![0x4b, 0x47, 4],
        };
        let install_frame = norito::to_bytes(&install).unwrap();
        let unload_frame = norito::to_bytes(&unload).unwrap();
        // The Swift and Kotlin ordinary account transaction builders reproduce this complete frame.
        assert_eq!(
            hex::encode(&unload_frame),
            "4e5254300000634e443f1c2be14836147b0888245923006f0000000000000065c2f63fa1effb5b022001010101010101010101010101010101010101010101010101010101010101012002020202020202020202020202020202020202020202020202020202020202022003030303030303030303030303030303030303030303030303030303030303030b03000000000000004b4704"
        );
        let from_frame = crate::isi::decode_instruction_envelope(
            InstallKagemushaSpartanProgramsV1::WIRE_ID,
            &install_frame,
        )
        .unwrap();
        assert_eq!(from_frame.as_any().downcast_ref(), Some(&install));
        let from_frame = crate::isi::decode_instruction_envelope(
            UnloadKagemushaSpartanV1::WIRE_ID,
            &unload_frame,
        )
        .unwrap();
        assert_eq!(from_frame.as_any().downcast_ref(), Some(&unload));
        assert_eq!(
            norito::json::from_str::<InstallKagemushaSpartanProgramsV1>(
                &norito::json::to_json(&install).unwrap()
            )
            .unwrap(),
            install
        );
        assert_eq!(
            norito::json::from_str::<UnloadKagemushaSpartanV1>(
                &norito::json::to_json(&unload).unwrap()
            )
            .unwrap(),
            unload
        );
        for (wire, frame) in [
            (InstallKagemushaSpartanProgramsV1::WIRE_ID, unload_frame),
            (UnloadKagemushaSpartanV1::WIRE_ID, install_frame),
        ] {
            assert!(
                crate::isi::decode_instruction_envelope(wire, &frame)
                    .unwrap()
                    .as_opaque()
                    .is_some()
            );
        }
    }
}
