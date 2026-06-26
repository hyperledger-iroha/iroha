//! Bridge proof ingestion instructions.

use super::*;
use iroha_primitives::json::Json;

/// Route-bound browser prover manifest reference used by SCCP route-manifest ISIs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SccpRouteBrowserProverManifestRef {
    /// Browser-safe prover module URL.
    pub module_url: String,
    /// Optional package/module specifier for reproducible builds.
    pub module_specifier: Option<String>,
    /// Hex-encoded SHA-256 digest of the browser module bytes.
    pub module_hash: String,
    /// Hex-encoded SHA-256 digest of the public browser prover manifest.
    pub manifest_hash: String,
    /// Expected exported symbols in the browser module.
    pub expected_exports: Vec<String>,
    /// Hex-encoded route/deployment hash this prover manifest is bound to.
    pub bound_route_hash: String,
    /// Hex-encoded proof/material hash this prover manifest is bound to.
    pub bound_proof_hash: String,
}

/// SCCP route manifest payload managed by on-chain route-manifest ISIs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SccpRouteManifest {
    /// Material format version.
    pub version: u8,
    /// Stable route identifier.
    pub route_id: String,
    /// Stable asset key within the route.
    pub asset_key: String,
    /// Legacy network key retained for route-manifest compatibility.
    pub tron_network: String,
    /// Canonical counterparty chain key.
    pub chain: String,
    /// Counterparty chain id hex.
    pub chain_id_hex: String,
    /// Canonical counterparty explorer base URL.
    pub explorer_url: Option<String>,
    /// Canonical counterparty explorer host.
    pub explorer_host: Option<String>,
    /// SCCP counterparty account codec id.
    pub counterparty_account_codec: Option<u8>,
    /// Stable logical key for the counterparty account codec.
    pub counterparty_account_codec_key: Option<String>,
    /// SCCP counterparty domain identifier.
    pub counterparty_domain: u32,
    /// Destination verifier target name.
    pub verifier_target: String,
    /// Whether this route is production-ready.
    pub production_ready: bool,
    /// Disabled reason surfaced when the route is not production-ready.
    pub disabled_reason: Option<String>,
    /// Hex-encoded destination network id used in binding evidence.
    pub network_id_hex: String,
    /// Counterparty TairaXOR token contract address.
    pub taira_xor_token_address: String,
    /// Counterparty TairaXOR bridge contract address.
    pub taira_xor_bridge_address: String,
    /// SCCP source bridge contract address.
    pub sccp_tron_source_bridge_address: String,
    /// Destination verifier contract address.
    pub tron_verifier_address: String,
    /// Hex-encoded verifier code digest.
    pub verifier_code_hash: String,
    /// Hex-encoded verifier key digest.
    pub verifier_key_hash: String,
    /// Optional hex-encoded browser/local prover artifact digest.
    pub proof_artifact_hash: Option<String>,
    /// Optional hex-encoded proving key digest.
    pub proving_key_hash: Option<String>,
    /// Optional hex-encoded native EVM prover bundle digest.
    pub native_evm_prover_bundle_hash: Option<String>,
    /// Optional canonical native EVM prover bundle JSON.
    pub native_evm_prover_bundle: Option<Json>,
    /// Optional route-bound TAIRA-to-counterparty browser prover manifest reference.
    pub destination_browser_prover: Option<SccpRouteBrowserProverManifestRef>,
    /// Optional route-bound counterparty-to-TAIRA browser prover manifest reference.
    pub source_browser_prover: Option<SccpRouteBrowserProverManifestRef>,
    /// Optional hash of the normalized deployment evidence used to build this route.
    pub deployment_evidence_sha256: Option<String>,
    /// Canonical destination binding key.
    pub destination_binding_key: String,
    /// Hex-encoded canonical destination binding hash.
    pub destination_binding_hash: String,
    /// Canonical TAIRA settlement asset definition id.
    pub taira_burn_record_settlement_asset_definition_id: String,
    /// Base64-encoded TAIRA burn-record contract artifact.
    pub taira_burn_record_contract_artifact_b64: String,
    /// Hex-encoded SHA-256 digest of the TAIRA burn-record artifact.
    pub taira_burn_record_artifact_sha256: String,
    /// Hex-encoded TAIRA burn-record contract code hash.
    pub taira_burn_record_code_hash: String,
    /// TAIRA burn-record verifier backend.
    pub taira_burn_record_vk_backend: String,
    /// TAIRA burn-record verifier key name.
    pub taira_burn_record_vk_name: String,
    /// TAIRA burn-record settlement gas limit.
    pub taira_burn_record_gas_limit: u64,
    /// Optional settlement contract address.
    pub settlement_contract_address: Option<String>,
    /// Optional settlement contract alias.
    pub settlement_contract_alias: Option<String>,
    /// Whether post-deploy route evidence is complete.
    pub post_deploy_full_toml_ready: Option<bool>,
    /// Hex-encoded source bridge config hash.
    pub post_deploy_source_bridge_config_hash: Option<String>,
    /// Hex-encoded source event transaction id.
    pub post_deploy_source_event_transaction_id: Option<String>,
    /// Canonical explorer URL for the source event transaction.
    pub post_deploy_source_event_explorer_url: Option<String>,
    /// Hex-encoded route canary evidence hash.
    pub post_deploy_route_canary_evidence_hash: Option<String>,
    /// Hex-encoded route canary transaction id.
    pub post_deploy_route_canary_transaction_id: Option<String>,
    /// Canonical explorer URL for the route canary transaction.
    pub post_deploy_route_canary_explorer_url: Option<String>,
    /// Hex-encoded offline full TOML SHA-256 digest.
    pub post_deploy_offline_full_toml_sha256: Option<String>,
}

isi! {
    /// Submit a bridge proof artifact for verification and registry retention.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SubmitBridgeProof {
        /// Bridge proof payload (ICS or transparent ZK).
        pub proof: crate::bridge::BridgeProof,
    }
}

impl crate::seal::Instruction for SubmitBridgeProof {}

impl SubmitBridgeProof {
    /// Construct a new submission wrapping the provided proof.
    pub fn new(proof: crate::bridge::BridgeProof) -> Self {
        Self { proof }
    }
}

isi! {
    /// Record a bridge receipt and emit a typed bridge event.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RecordBridgeReceipt {
        /// Bridge receipt payload to record.
        pub receipt: crate::bridge::BridgeReceipt,
    }
}

impl crate::seal::Instruction for RecordBridgeReceipt {}

impl RecordBridgeReceipt {
    /// Construct a new record instruction for the provided receipt.
    pub fn new(receipt: crate::bridge::BridgeReceipt) -> Self {
        Self { receipt }
    }
}

isi! {
    /// Upsert an on-chain SCCP route manifest.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct UpsertSccpRouteManifest {
        /// Route manifest to insert or replace.
        pub manifest: SccpRouteManifest,
    }
}

impl crate::seal::Instruction for UpsertSccpRouteManifest {}

impl UpsertSccpRouteManifest {
    /// Construct a new SCCP route manifest upsert.
    pub fn new(manifest: SccpRouteManifest) -> Self {
        Self { manifest }
    }
}

isi! {
    /// Remove an on-chain SCCP route manifest by canonical route key.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RemoveSccpRouteManifest {
        /// Stable route identifier.
        pub route_id: String,
        /// Stable asset key within the route.
        pub asset_key: String,
        /// SCCP counterparty domain identifier.
        pub counterparty_domain: u32,
        /// Counterparty chain id hex.
        pub chain_id_hex: String,
    }
}

impl crate::seal::Instruction for RemoveSccpRouteManifest {}

impl RemoveSccpRouteManifest {
    /// Construct a new SCCP route manifest removal.
    pub fn new(
        route_id: String,
        asset_key: String,
        counterparty_domain: u32,
        chain_id_hex: String,
    ) -> Self {
        Self {
            route_id,
            asset_key,
            counterparty_domain,
            chain_id_hex,
        }
    }
}

fn bridge_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

impl<'a> norito::core::DecodeFromSlice<'a> for SubmitBridgeProof {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = bridge_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let proof = super::decode_aos_canonical_field::<crate::bridge::BridgeProof>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { proof }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RecordBridgeReceipt {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = bridge_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let receipt = super::decode_aos_canonical_field::<crate::bridge::BridgeReceipt>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { receipt }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for UpsertSccpRouteManifest {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = bridge_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let manifest = super::decode_aos_canonical_field::<SccpRouteManifest>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { manifest }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RemoveSccpRouteManifest {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = bridge_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let route_id = super::decode_aos_canonical_field::<String>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let asset_key = super::decode_aos_canonical_field::<String>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let counterparty_domain = super::decode_aos_canonical_field::<u32>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let chain_id_hex = super::decode_aos_canonical_field::<String>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                route_id,
                asset_key,
                counterparty_domain,
                chain_id_hex,
            },
            offset,
        ))
    }
}

isi! {
    /// Record an SCCP message payload for block-level commitment anchoring.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RecordSccpMessage {
        /// Canonical SCCP payload bytes.
        pub payload_bytes: Vec<u8>,
    }
}

impl crate::seal::Instruction for RecordSccpMessage {}

impl RecordSccpMessage {
    /// Construct a new SCCP message record instruction for the provided payload bytes.
    pub fn new(payload_bytes: Vec<u8>) -> Self {
        Self { payload_bytes }
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RecordSccpMessage {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let payload_bytes = super::decode_aos_slice_field::<Vec<u8>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { payload_bytes }, offset))
    }
}

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        bridge::{
            BridgeProof, BridgeProofPayload, BridgeProofRange, BridgeReceipt,
            BridgeTransparentProof,
        },
        nexus::LaneId,
        proof::ProofBox,
    };

    fn proof() -> BridgeProof {
        BridgeProof {
            range: BridgeProofRange {
                start_height: 7,
                end_height: 9,
            },
            manifest_hash: [0xAB; 32],
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                proof: ProofBox::new("halo2/mock".into(), vec![0xDE, 0xAD, 0xBE, 0xEF]),
                recursion_depth: Some(2),
            }),
            pinned: true,
        }
    }

    fn receipt() -> BridgeReceipt {
        BridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: Some([0x22; 32]),
            proof_hash: [0x33; 32],
            amount: 42,
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        }
    }

    fn browser_prover_ref(seed: u8) -> SccpRouteBrowserProverManifestRef {
        let hex = |byte| format!("0x{}", hex::encode([byte; 32]));
        SccpRouteBrowserProverManifestRef {
            module_url: "/sccp-bsc/taira-bsc-xor-prover.js".to_owned(),
            module_specifier: Some("@sora/sccp-bsc-prover".to_owned()),
            module_hash: hex(seed),
            manifest_hash: hex(seed.saturating_add(1)),
            expected_exports: vec!["bscSccpProve".to_owned(), "bscSccpSelfTest".to_owned()],
            bound_route_hash: hex(seed.saturating_add(2)),
            bound_proof_hash: hex(seed.saturating_add(3)),
        }
    }

    fn route_manifest() -> SccpRouteManifest {
        let hex = |byte| format!("0x{}", hex::encode([byte; 32]));
        SccpRouteManifest {
            version: 1,
            route_id: "taira_bsc_xor".to_owned(),
            asset_key: "xor".to_owned(),
            tron_network: "bsc-testnet".to_owned(),
            chain: "bsc-testnet".to_owned(),
            chain_id_hex: "0x61".to_owned(),
            explorer_url: Some("https://testnet.bscscan.com".to_owned()),
            explorer_host: Some("testnet.bscscan.com".to_owned()),
            counterparty_account_codec: Some(2),
            counterparty_account_codec_key: Some("evm_hex".to_owned()),
            counterparty_domain: 2,
            verifier_target: "EvmContract".to_owned(),
            production_ready: true,
            disabled_reason: None,
            network_id_hex: hex(0x61),
            taira_xor_token_address: "0x1111111111111111111111111111111111111111".to_owned(),
            taira_xor_bridge_address: "0x2222222222222222222222222222222222222222".to_owned(),
            sccp_tron_source_bridge_address: "0x3333333333333333333333333333333333333333"
                .to_owned(),
            tron_verifier_address: "0x4444444444444444444444444444444444444444".to_owned(),
            verifier_code_hash: hex(0x45),
            verifier_key_hash: hex(0x46),
            proof_artifact_hash: Some(hex(0x47)),
            proving_key_hash: Some(hex(0x48)),
            native_evm_prover_bundle_hash: Some(hex(0x49)),
            native_evm_prover_bundle: Some(Json::new(norito::json!({
                "schema": "sccp-bsc-native-evm-prover-bundle/v1",
                "routeId": "taira_bsc_xor",
                "assetKey": "xor"
            }))),
            destination_browser_prover: Some(browser_prover_ref(0x50)),
            source_browser_prover: Some(browser_prover_ref(0x60)),
            deployment_evidence_sha256: Some(hex(0x4a)),
            destination_binding_key: "evm:0:2:test-binding".to_owned(),
            destination_binding_hash: hex(0x4b),
            taira_burn_record_settlement_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
                .to_owned(),
            taira_burn_record_contract_artifact_b64: "QUJDREVGRw==".to_owned(),
            taira_burn_record_artifact_sha256: hex(0x4c),
            taira_burn_record_code_hash: hex(0x4d),
            taira_burn_record_vk_backend: "halo2/ipa".to_owned(),
            taira_burn_record_vk_name: "taira_bsc_xor_burn_record_v1".to_owned(),
            taira_burn_record_gas_limit: 2_000_000,
            settlement_contract_address: None,
            settlement_contract_alias: Some("taira_xor_burn_record".to_owned()),
            post_deploy_full_toml_ready: Some(true),
            post_deploy_source_bridge_config_hash: Some(hex(0x4e)),
            post_deploy_source_event_transaction_id: Some(hex(0x4f)),
            post_deploy_source_event_explorer_url: Some(format!(
                "https://testnet.bscscan.com/tx/{}",
                hex(0x4f)
            )),
            post_deploy_route_canary_evidence_hash: Some(hex(0x51)),
            post_deploy_route_canary_transaction_id: Some(hex(0x52)),
            post_deploy_route_canary_explorer_url: Some(format!(
                "https://testnet.bscscan.com/tx/{}",
                hex(0x52)
            )),
            post_deploy_offline_full_toml_sha256: Some(hex(0x53)),
        }
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(registry: &crate::isi::InstructionRegistry, value: T)
    where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let wire_id = std::any::type_name::<T>();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn bridge_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(SubmitBridgeProof::new(proof()));
        assert_slice_roundtrip(RecordBridgeReceipt::new(receipt()));
        assert_slice_roundtrip(UpsertSccpRouteManifest::new(route_manifest()));
        assert_slice_roundtrip(RemoveSccpRouteManifest::new(
            "taira_bsc_xor".to_owned(),
            "xor".to_owned(),
            2,
            "0x61".to_owned(),
        ));
        assert_slice_roundtrip(RecordSccpMessage::new(vec![0xCA, 0xFE]));
    }

    #[test]
    fn bridge_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<SubmitBridgeProof>()
            .register_slice::<RecordBridgeReceipt>()
            .register_slice::<UpsertSccpRouteManifest>()
            .register_slice::<RemoveSccpRouteManifest>()
            .register_slice::<RecordSccpMessage>();

        assert_registry_decodes(&registry, SubmitBridgeProof::new(proof()));
        assert_registry_decodes(&registry, RecordBridgeReceipt::new(receipt()));
        assert_registry_decodes(&registry, UpsertSccpRouteManifest::new(route_manifest()));
        assert_registry_decodes(
            &registry,
            RemoveSccpRouteManifest::new(
                "taira_bsc_xor".to_owned(),
                "xor".to_owned(),
                2,
                "0x61".to_owned(),
            ),
        );
        assert_registry_decodes(&registry, RecordSccpMessage::new(vec![0xCA, 0xFE]));
    }
}
