//! Rust-owner conformance guard for shared Musubi V1 instruction wire fixtures.

use std::{
    any::type_name,
    collections::BTreeSet,
    fmt::{Debug, Write as _},
};

use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
use iroha_data_model::{
    account::AccountId,
    id::ChainId,
    isi::{
        InstructionBox, decode_instruction_from_pair, framed_instruction_payload,
        instruction_wire_id,
        musubi::{
            AcceptMusubiPackageMaintainerV1, AddMusubiArchiveLocationV1,
            AssertMusubiReleaseDigestV1, InviteMusubiPackageMaintainerV1, PublishMusubiReleaseV1,
            RecoverMusubiPackageV1, RegisterMusubiAliasV1, RegisterMusubiArchiveV1,
            RegisterMusubiNamespaceBindingV1, RemoveMusubiPackageMaintainerV1,
            RetargetMusubiAliasV1, RetireMusubiArchiveLocationV1,
            RevokeMusubiPackageMaintainerInvitationV1, SetMusubiArtifactTakedownV1,
            SetMusubiPackageMaintainerRoleV1, SetMusubiPackageMetadataV1,
            SetMusubiRegistryPolicyV1, SetMusubiReleaseYankV1,
        },
    },
    musubi::{
        ArchiveId, MUSUBI_MAX_CAR_BYTES_V1, MUSUBI_MAX_CHUNKS_V1, MUSUBI_MAX_FILES_V1,
        MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1, MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1,
        MusubiAliasNameV1, MusubiAliasPricingPolicyV1, MusubiArchiveCommitmentV1,
        MusubiArchiveLocationIdV1, MusubiContentDigestV1, MusubiDependencyKindV1,
        MusubiDependencyReqV1, MusubiDescriptionV1, MusubiDocumentRefV1,
        MusubiExactDependencyEdgeV1, MusubiGovernanceDecisionV1, MusubiInviteIdV1, MusubiKeywordV1,
        MusubiKotodamaEditionV1, MusubiMaintainerPermissionsV1, MusubiNamespaceBindingV1,
        MusubiNamespaceDelegationApprovalV1, MusubiNamespaceDelegationPayloadV1,
        MusubiNamespaceDelegationV1, MusubiNamespaceV1, MusubiPackageIdV1, MusubiPackageNameV1,
        MusubiPackageRoleV1, MusubiPackageScopeV1, MusubiParliamentActionV1,
        MusubiProviderBundleVerificationApprovalV1, MusubiProviderBundleVerificationAttestationV1,
        MusubiProviderBundleVerificationBindingV1, MusubiProviderBundleVerificationPayloadV1,
        MusubiPublicationV1, MusubiReasonV1, MusubiRecoverPackageOwnersV1,
        MusubiRegistryAdmissionModeV1, MusubiRegistryPolicyV1, MusubiRegistrySnapshotV1,
        MusubiReleaseDigestV1, MusubiReleaseIdV1, MusubiReleaseManifestV1, MusubiReleaseMetadataV1,
        MusubiResolutionProofV1, MusubiRetargetAliasV1, MusubiSeedIngressReceiptApprovalV1,
        MusubiSeedIngressReceiptBindingV1, MusubiSeedIngressReceiptPayloadV1,
        MusubiSeedIngressReceiptV1, MusubiSetRegistryPolicyActionV1,
        MusubiTakedownArtifactActionV1, MusubiVerificationLockV1, MusubiVerificationNodeV1,
        MusubiVersionReqV1, MusubiVersionV1,
    },
    name::Name,
    nexus::DataSpaceId,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            ChunkerProfileHandle, ManifestDigest, ManifestRootCid,
            ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
            ProviderIngestFinalizedAnchorV1, ReplicationOrderId,
        },
    },
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{DecodeFlagsGuard, DecodeFromSlice, Header},
    json::{self, JsonDeserialize, JsonSerialize, Value},
};

const FIXTURE: &str = include_str!("../../../fixtures/musubi/instructions_v1.json");
const INSTRUCTION_BOX_SCHEMA_NAME: &str = "(alloc::string::String, alloc::vec::Vec<u8>)";
const INSTRUCTION_BOX_SCHEMA_HASH: &str = "862a7d77075d4d23ff6c1261db027811";

fn object(value: &Value) -> &norito::json::Map {
    value.as_object().expect("fixture value is an object")
}

fn keys(value: &Value) -> BTreeSet<&str> {
    object(value).keys().map(String::as_str).collect()
}

fn required_string<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("fixture `{key}` is a string"))
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    output
}

fn required_hex(value: &Value, key: &str) -> Vec<u8> {
    let encoded = required_string(value, key);
    assert_eq!(encoded.len() % 2, 0, "fixture `{key}` has even hex length");
    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = core::str::from_utf8(pair).expect("fixture hex is ASCII");
            u8::from_str_radix(pair, 16).unwrap_or_else(|_| panic!("fixture `{key}` is hex"))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        encode_hex(&bytes),
        encoded,
        "fixture `{key}` is lowercase hex"
    );
    bytes
}

fn package(home_dataspace: u64, scope: MusubiPackageScopeV1, name: &str) -> MusubiPackageIdV1 {
    MusubiPackageIdV1::new(
        DataSpaceId::new(home_dataspace),
        scope,
        MusubiPackageNameV1::new(name).expect("fixture package name"),
    )
}

fn keypair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("fixture seed derives an Ed25519 keypair")
}

fn account(seed: u8) -> AccountId {
    AccountId::new(keypair(seed).public_key().clone())
}

fn metadata(
    description: &str,
    readme: &str,
    license: &str,
    repository: &str,
    keywords: &[&str],
) -> MusubiReleaseMetadataV1 {
    let value = MusubiReleaseMetadataV1 {
        description: Some(
            description
                .parse::<MusubiDescriptionV1>()
                .expect("fixture description"),
        ),
        readme: Some(
            readme
                .parse::<MusubiDocumentRefV1>()
                .expect("fixture readme"),
        ),
        license: Some(
            license
                .parse::<MusubiDocumentRefV1>()
                .expect("fixture license"),
        ),
        repository: Some(
            repository
                .parse::<MusubiDocumentRefV1>()
                .expect("fixture repository"),
        ),
        keywords: keywords
            .iter()
            .map(|keyword| keyword.parse::<MusubiKeywordV1>().expect("fixture keyword"))
            .collect(),
    };
    value.validate().expect("fixture metadata is canonical");
    value
}

fn case<'a>(root: &'a Value, id: &str) -> &'a Value {
    root.get("cases")
        .and_then(Value::as_array)
        .expect("fixture cases")
        .iter()
        .find(|case| case.get("id").and_then(Value::as_str) == Some(id))
        .unwrap_or_else(|| panic!("fixture case `{id}`"))
}

fn assert_case<T>(case: &Value, value: T, expected_wire_id: &str)
where
    T: Clone
        + Debug
        + Eq
        + Into<InstructionBox>
        + JsonDeserialize
        + JsonSerialize
        + NoritoSerialize
        + 'static,
    for<'a> T: NoritoDeserialize<'a>,
{
    assert_eq!(
        keys(case),
        BTreeSet::from([
            "bare_payload_hex",
            "concrete_frame_hex",
            "concrete_schema_hash",
            "concrete_schema_name",
            "header_flags",
            "id",
            "instruction_box_pair_hex",
            "semantic",
            "standalone_instruction_box_frame_hex",
            "wire_id",
        ])
    );

    let semantic = case.get("semantic").expect("semantic fixture input");
    assert_eq!(
        json::to_value(&value).expect("encode semantic input"),
        semantic.clone(),
        "fixture semantic input is canonical Norito JSON"
    );
    let semantic_decoded: T =
        json::from_value(semantic.clone()).expect("decode semantic input with Norito JSON");
    assert_eq!(semantic_decoded, value);

    assert_eq!(required_string(case, "wire_id"), expected_wire_id);
    assert_eq!(
        required_string(case, "concrete_schema_name"),
        type_name::<T>()
    );
    assert_eq!(
        required_string(case, "concrete_schema_hash"),
        encode_hex(&<T as NoritoSerialize>::schema_hash())
    );
    let expected_flags = case
        .get("header_flags")
        .and_then(Value::as_u64)
        .and_then(|flags| u8::try_from(flags).ok())
        .expect("header flags fit u8");

    let expected_bare_payload = required_hex(case, "bare_payload_hex");
    let (bare_payload, concrete_flags) = norito::codec::encode_with_header_flags(&value);
    assert_eq!(concrete_flags, expected_flags);
    assert_eq!(bare_payload, expected_bare_payload);

    let expected_concrete_frame = required_hex(case, "concrete_frame_hex");
    let concrete_frame =
        norito::core::frame_bare_with_header_flags::<T>(&bare_payload, concrete_flags)
            .expect("frame concrete instruction");
    assert_eq!(concrete_frame[Header::SIZE - 1], expected_flags);
    assert_eq!(concrete_frame, expected_concrete_frame);
    assert_eq!(
        norito::core::to_bytes(&value).expect("encode concrete instruction frame"),
        concrete_frame
    );

    let concrete_decoded: T =
        norito::decode_from_bytes(&concrete_frame).expect("decode concrete instruction frame");
    assert_eq!(concrete_decoded, value);
    assert_eq!(
        norito::core::to_bytes(&concrete_decoded).expect("re-encode concrete instruction frame"),
        concrete_frame
    );

    let boxed: InstructionBox = value.clone().into();
    assert_eq!(instruction_wire_id(&boxed), Some(expected_wire_id));
    let (embedded_wire_id, embedded_concrete_frame) =
        framed_instruction_payload(&boxed).expect("registered instruction payload");
    assert_eq!(embedded_wire_id, expected_wire_id);
    assert_eq!(embedded_concrete_frame, concrete_frame);
    let pair_decoded = decode_instruction_from_pair(expected_wire_id, &concrete_frame)
        .expect("decode registered concrete frame");
    assert_eq!(
        pair_decoded
            .as_any()
            .downcast_ref::<T>()
            .expect("decoded registered concrete type"),
        &value
    );

    let expected_instruction_box_pair = required_hex(case, "instruction_box_pair_hex");
    let (instruction_box_pair, instruction_box_flags) =
        norito::codec::encode_with_header_flags(&boxed);
    assert_eq!(instruction_box_flags, expected_flags);
    assert_eq!(instruction_box_pair, expected_instruction_box_pair);
    let pair_decoded = {
        let _guard = DecodeFlagsGuard::enter(instruction_box_flags);
        let (decoded, used) = InstructionBox::decode_from_slice(&instruction_box_pair)
            .expect("decode bare InstructionBox pair");
        assert_eq!(used, instruction_box_pair.len());
        decoded
    };
    assert_eq!(
        pair_decoded
            .as_any()
            .downcast_ref::<T>()
            .expect("bare pair concrete type"),
        &value
    );
    let (reencoded_pair, reencoded_pair_flags) =
        norito::codec::encode_with_header_flags(&pair_decoded);
    assert_eq!(reencoded_pair_flags, instruction_box_flags);
    assert_eq!(reencoded_pair, instruction_box_pair);

    let expected_instruction_box_frame = required_hex(case, "standalone_instruction_box_frame_hex");
    let instruction_box_frame = norito::core::frame_bare_with_header_flags::<InstructionBox>(
        &instruction_box_pair,
        instruction_box_flags,
    )
    .expect("frame InstructionBox pair");
    assert_eq!(instruction_box_frame[Header::SIZE - 1], expected_flags);
    assert_eq!(instruction_box_frame, expected_instruction_box_frame);
    assert_eq!(
        norito::core::to_bytes(&boxed).expect("encode standalone InstructionBox frame"),
        instruction_box_frame
    );

    let decoded_box: InstructionBox = norito::decode_from_bytes(&instruction_box_frame)
        .expect("decode standalone InstructionBox frame");
    assert_eq!(
        decoded_box
            .as_any()
            .downcast_ref::<T>()
            .expect("standalone frame concrete type"),
        &value
    );
    assert_eq!(
        norito::core::to_bytes(&decoded_box).expect("re-encode standalone InstructionBox frame"),
        instruction_box_frame
    );
}

#[test]
fn shared_musubi_instruction_fixture_locks_every_wire_layer() {
    let root: Value = json::from_str(FIXTURE).expect("parse Musubi instruction fixture");
    assert_eq!(
        keys(&root),
        BTreeSet::from([
            "cases",
            "fixture_version",
            "format",
            "instruction_box_schema_hash",
            "instruction_box_schema_name",
            "rust_owner",
        ])
    );
    assert_eq!(
        root.get("format").and_then(Value::as_str),
        Some("iroha-musubi-instructions-v1")
    );
    assert_eq!(root.get("fixture_version").and_then(Value::as_u64), Some(1));
    assert_eq!(
        root.get("rust_owner").and_then(Value::as_str),
        Some("iroha_data_model::isi::musubi")
    );
    assert_eq!(
        root.get("instruction_box_schema_name")
            .and_then(Value::as_str),
        Some(INSTRUCTION_BOX_SCHEMA_NAME)
    );
    assert_eq!(
        type_name::<(String, Vec<u8>)>(),
        INSTRUCTION_BOX_SCHEMA_NAME
    );
    assert_eq!(
        root.get("instruction_box_schema_hash")
            .and_then(Value::as_str),
        Some(INSTRUCTION_BOX_SCHEMA_HASH)
    );
    assert_eq!(
        encode_hex(&norito::core::type_name_schema_hash::<(String, Vec<u8>)>()),
        INSTRUCTION_BOX_SCHEMA_HASH
    );
    assert_eq!(
        <InstructionBox as NoritoSerialize>::schema_hash(),
        norito::core::type_name_schema_hash::<(String, Vec<u8>)>()
    );

    let cases = root
        .get("cases")
        .and_then(Value::as_array)
        .expect("instruction cases");
    assert_eq!(cases.len(), 18);
    assert_eq!(
        cases
            .iter()
            .map(|case| required_string(case, "id"))
            .collect::<Vec<_>>(),
        vec![
            "accept-root-max-revision",
            "revoke-domain-invitation",
            "register-alias-domain-target",
            "assert-prerelease-digest",
            "retire-location-max-revision",
            "unyank-domain-release-high-revision",
            "remove-root-maintainer-high-revision",
            "register-domain-namespace-max-generation",
            "invite-domain-maintainer-max-expiry",
            "promote-root-member-to-owner-high-revision",
            "recover-domain-package-three-owners",
            "retarget-one-character-alias-high-revision",
            "takedown-max-major-prerelease",
            "register-archive-max-bounds-signed-receipt",
            "add-location-three-signed-providers",
            "publish-delegated-domain-release",
            "replace-domain-metadata-high-revision",
            "set-allowlisted-policy-repriced-aliases",
        ]
    );

    let accept = AcceptMusubiPackageMaintainerV1 {
        package: package(7, MusubiPackageScopeV1::DataspaceRoot, "math-utils"),
        invite_id: MusubiInviteIdV1::new([0x11; 32]),
        expected_governance_revision: u64::MAX,
    };
    let revoke = RevokeMusubiPackageMaintainerInvitationV1 {
        package: package(
            42,
            MusubiPackageScopeV1::Domain("finance".parse::<Name>().expect("domain name")),
            "oracle-kit",
        ),
        invite_id: MusubiInviteIdV1::new([0x22; 32]),
        expected_governance_revision: 9,
    };
    let alias = RegisterMusubiAliasV1::new(
        "oracle-tools".parse::<MusubiAliasNameV1>().expect("alias"),
        package(
            u64::MAX,
            MusubiPackageScopeV1::Domain("defi".parse::<Name>().expect("domain name")),
            "price-feed",
        ),
        17,
    );
    let assertion = AssertMusubiReleaseDigestV1::new(
        MusubiReleaseIdV1::new(
            package(99, MusubiPackageScopeV1::DataspaceRoot, "compiler-core"),
            "1.2.3-rc.7"
                .parse::<MusubiVersionV1>()
                .expect("prerelease version"),
        ),
        MusubiReleaseDigestV1::new([0x33; 32]),
    );
    let retire = RetireMusubiArchiveLocationV1 {
        archive_id: ArchiveId::new([0xA5; 32]),
        location_id: MusubiArchiveLocationIdV1::new([0x5A; 32]),
        expected_location_revision: u64::MAX,
        reason: MusubiReasonV1::new(
            "Provider lease retired after cross-provider readback failed at epoch 9.",
        )
        .expect("retirement reason"),
    };
    let unyank = SetMusubiReleaseYankV1::new(
        MusubiReleaseIdV1::new(
            package(
                u64::MAX - 1,
                MusubiPackageScopeV1::Domain("operations".parse::<Name>().expect("domain name")),
                "ledger-proof",
            ),
            "0.0.1-alpha.0"
                .parse::<MusubiVersionV1>()
                .expect("prerelease version"),
        ),
        false,
        MusubiReasonV1::new("Replica quorum restored after independent bundle verification.")
            .expect("unyank reason"),
        u64::MAX - 1,
    );
    let remove = RemoveMusubiPackageMaintainerV1 {
        package: package(0, MusubiPackageScopeV1::DataspaceRoot, "access-control"),
        account: AccountId::new(
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .expect("public key"),
        ),
        expected_governance_revision: u64::MAX - 2,
    };
    let register_namespace = RegisterMusubiNamespaceBindingV1::new(
        MusubiNamespaceBindingV1 {
            namespace: "governance.universal"
                .parse::<MusubiNamespaceV1>()
                .expect("domain namespace"),
            home_dataspace: DataSpaceId::new(u64::MAX - 6),
            scope: MusubiPackageScopeV1::Domain("governance".parse::<Name>().expect("domain name")),
            generation: u64::MAX,
        },
        u64::MAX - 5,
    );
    let invite = InviteMusubiPackageMaintainerV1 {
        package: package(
            5_124_095_576_030_430,
            MusubiPackageScopeV1::Domain("security".parse::<Name>().expect("domain name")),
            "key-rotation",
        ),
        invite_id: MusubiInviteIdV1::new(core::array::from_fn(|index| {
            u8::try_from(index).expect("invite-id index fits u8")
        })),
        invited_account: AccountId::new(
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key"),
        ),
        role: MusubiPackageRoleV1::Maintainer(MusubiMaintainerPermissionsV1 {
            publish: true,
            yank: false,
            metadata: true,
            archive_locations: false,
        }),
        expires_at_height: u64::MAX,
        expected_governance_revision: u64::MAX - 3,
    };
    let promote = SetMusubiPackageMaintainerRoleV1 {
        package: package(1, MusubiPackageScopeV1::DataspaceRoot, "consensus-tools"),
        account: AccountId::new(
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                .parse()
                .expect("public key"),
        ),
        role: MusubiPackageRoleV1::Owner,
        expected_governance_revision: u64::MAX - 4,
    };
    let recovery_package = package(
        u64::MAX,
        MusubiPackageScopeV1::Domain("recovery".parse::<Name>().expect("domain name")),
        "registry-guard",
    );
    let mut recovery_owners = vec![
        AccountId::new(
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key"),
        ),
        AccountId::new(
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                .parse()
                .expect("public key"),
        ),
        AccountId::new(
            "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016"
                .parse()
                .expect("public key"),
        ),
    ];
    recovery_owners.sort();
    recovery_owners.dedup();
    let recovery_action =
        MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package: recovery_package.clone(),
            owners: recovery_owners.clone(),
            expected_revision: u64::MAX - 6,
        });
    let recover = RecoverMusubiPackageV1 {
        decision: MusubiGovernanceDecisionV1 {
            decision_id: [0xC1; 32],
            action_digest: recovery_action.action_digest(),
            enacted_at_height: 1,
            execute_after_height: 2,
        },
        package: recovery_package,
        owners: recovery_owners,
        expected_governance_revision: u64::MAX - 6,
    };
    let retarget_alias = "x".parse::<MusubiAliasNameV1>().expect("global alias");
    let retarget_target = package(
        0,
        MusubiPackageScopeV1::Domain("parliament".parse::<Name>().expect("domain name")),
        "emergency-target",
    );
    let retarget_action = MusubiParliamentActionV1::RetargetAlias(MusubiRetargetAliasV1 {
        alias: retarget_alias.clone(),
        target: retarget_target.clone(),
        expected_revision: u64::MAX - 7,
    });
    let retarget = RetargetMusubiAliasV1 {
        decision: MusubiGovernanceDecisionV1 {
            decision_id: core::array::from_fn(|index| {
                0x40_u8 + u8::try_from(index).expect("decision-id index fits u8")
            }),
            action_digest: retarget_action.action_digest(),
            enacted_at_height: u64::MAX - 2,
            execute_after_height: u64::MAX - 1,
        },
        alias: retarget_alias,
        target: retarget_target,
        expected_history_revision: u64::MAX - 7,
    };
    let takedown_release = MusubiReleaseIdV1::new(
        package(
            u64::MAX - 9,
            MusubiPackageScopeV1::DataspaceRoot,
            "supply-chain",
        ),
        "18446744073709551615.0.0-incident.1"
            .parse::<MusubiVersionV1>()
            .expect("maximum-major prerelease version"),
    );
    let takedown_reason = MusubiReasonV1::new(
        "Parliament ordered takedown after reproducible signature-forgery evidence.",
    )
    .expect("takedown reason");
    let takedown_action =
        MusubiParliamentActionV1::TakedownArtifact(MusubiTakedownArtifactActionV1 {
            release: takedown_release.clone(),
            reason: takedown_reason.clone(),
            expected_artifact_governance_revision: u64::MAX - 8,
        });
    let takedown = SetMusubiArtifactTakedownV1 {
        decision: MusubiGovernanceDecisionV1 {
            decision_id: [0xFE; 32],
            action_digest: takedown_action.action_digest(),
            enacted_at_height: u64::MAX - 1,
            execute_after_height: u64::MAX,
        },
        release: takedown_release,
        reason: takedown_reason,
        expected_artifact_governance_revision: u64::MAX - 8,
    };
    let commitment = MusubiArchiveCommitmentV1 {
        root_cid: ManifestRootCid::from_blake3_digest([0x61; 32]).expect("root CID"),
        chunker: ChunkerProfileHandle {
            profile_id: u32::MAX,
            namespace: "sorafs".to_owned(),
            name: "musubi-v1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1f,
        },
        chunk_plan_digest: MusubiContentDigestV1::new([0x62; 32]),
        por_root: MusubiContentDigestV1::new([0x63; 32]),
        content_length: MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1,
        car_digest: MusubiContentDigestV1::new([0x64; 32]),
        car_size: MUSUBI_MAX_CAR_BYTES_V1,
        bundle_digest: MusubiContentDigestV1::new([0x65; 32]),
        source_tree_digest: MusubiContentDigestV1::new([0x66; 32]),
        descriptor_digest: MusubiContentDigestV1::new([0x67; 32]),
        file_count: MUSUBI_MAX_FILES_V1,
        chunk_count: MUSUBI_MAX_CHUNKS_V1,
    };
    commitment
        .validate()
        .expect("fixture archive commitment is valid");
    let root_package = package(
        0x0123_4567_89AB_CDEF,
        MusubiPackageScopeV1::Domain("fixture".parse::<Name>().expect("domain name")),
        "deterministic-publisher",
    );
    let root_release = MusubiReleaseIdV1::new(
        root_package.clone(),
        "2.5.8-beta.13"
            .parse::<MusubiVersionV1>()
            .expect("fixture root version"),
    );
    let dependency_package = package(17, MusubiPackageScopeV1::DataspaceRoot, "vector-math");
    let dependency_release = MusubiReleaseIdV1::new(
        dependency_package.clone(),
        "1.4.2"
            .parse::<MusubiVersionV1>()
            .expect("fixture dependency version"),
    );
    let dependency_requirement = ">=1.2.0,<2.0.0"
        .parse::<MusubiVersionReqV1>()
        .expect("fixture dependency requirement");
    let exact_dependency = MusubiExactDependencyEdgeV1 {
        alias: "math".parse().expect("dependency alias"),
        kind: MusubiDependencyKindV1::Normal,
        package: dependency_package.clone(),
        requirement: dependency_requirement.clone(),
        selected: dependency_release.clone(),
    };
    let verification_lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: root_release.clone(),
        root_dependencies: vec![exact_dependency],
        nodes: vec![MusubiVerificationNodeV1 {
            release: dependency_release,
            release_digest: MusubiReleaseDigestV1::new([0x91; 32]),
            archive_id: ArchiveId::new([0x92; 32]),
            source_digest: MusubiContentDigestV1::new([0x93; 32]),
            interface_digest: MusubiContentDigestV1::new([0x94; 32]),
            abi: MusubiAbiBindingV1::new([0x95; 32]).expect("dependency ABI"),
            dependencies: Vec::new(),
        }],
    };
    verification_lock
        .validate()
        .expect("fixture verification lock is valid");
    let release_manifest = MusubiReleaseManifestV1 {
        release: root_release,
        edition: MusubiKotodamaEditionV1::V1,
        abi: MusubiAbiBindingV1::new([0xA1; 32]).expect("root ABI"),
        dependencies: vec![MusubiDependencyReqV1 {
            alias: "math".parse().expect("dependency alias"),
            package: dependency_package,
            requirement: dependency_requirement,
        }],
        exports: vec![
            "compile".parse().expect("export name"),
            "verify".parse().expect("export name"),
        ],
        interface_digest: MusubiContentDigestV1::new([0xA2; 32]),
        metadata: metadata(
            "Deterministic publication fixture with a signed namespace delegation.",
            "README.md",
            "Apache-2.0",
            "https://example.invalid/musubi/deterministic-publisher",
            &["deterministic", "kotodama", "registry"],
        ),
        archive_id: commitment.archive_id(),
        verification_lock_digest: verification_lock.digest(),
    };
    let publication = MusubiPublicationV1 {
        manifest: release_manifest,
        resolution: MusubiResolutionProofV1 {
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: u64::MAX - 20,
                finalized_block_hash: [0xA3; 32],
                index_revision: u64::MAX - 21,
            },
            lock: verification_lock,
        },
    };
    publication
        .validate()
        .expect("fixture publication binds its exact graph");
    let publisher = account(0x81);
    let broker_keypair = keypair(0x82);
    let receipt_binding = MusubiSeedIngressReceiptBindingV1 {
        chain_id: ChainId::from("musubi-fixture-all-18"),
        genesis_block_hash: [0xB1; 32],
        publisher: publisher.clone(),
        ingress_broker: AccountId::new(broker_keypair.public_key().clone()),
        seed_provider: ProviderId::new([0xB2; 32]),
        semantic_release_manifest_digest: publication.manifest.semantic_digest(),
        archive_id: commitment.archive_id(),
        car_body_digest: commitment.car_digest,
        car_body_length: commitment.car_size,
        nonce: core::array::from_fn(|index| {
            0xC0_u8 + u8::try_from(index).expect("nonce index fits u8")
        }),
    };
    let receipt_payload = MusubiSeedIngressReceiptPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding: receipt_binding.clone(),
        issued_at_ms: 1_700_000_000_000,
        expires_at_ms: 1_700_086_400_000,
    };
    let staging_receipt = MusubiSeedIngressReceiptV1 {
        approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
            public_key: broker_keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                broker_keypair.private_key(),
                receipt_payload.signing_hash(),
            )
            .expect("sign staging receipt"),
        }],
        payload: receipt_payload,
    };
    staging_receipt
        .verify(&receipt_binding, 1_700_000_000_001)
        .expect("fixture staging receipt verifies");
    let register_archive =
        RegisterMusubiArchiveV1::new(commitment.clone(), staging_receipt, u64::MAX - 30);
    let replication_order = ReplicationOrderId::new([0xC2; 32]);
    let provider_attestations = [
        (0xD1, 0x90, 0xE1, 0xF1, 0x71),
        (0xD2, 0x91, 0xE2, 0xF2, 0x72),
        (0xD3, 0x92, 0xE3, 0xF3, 0x73),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(index, (provider_byte, key_seed, policy_byte, digest_byte, block_byte))| {
            let owner_keypair = keypair(key_seed);
            let provider_id = ProviderId::new([provider_byte; 32]);
            let owner = AccountId::new(owner_keypair.public_key().clone());
            let binding = MusubiProviderBundleVerificationBindingV1 {
                chain_id: receipt_binding.chain_id.clone(),
                genesis_block_hash: receipt_binding.genesis_block_hash,
                provider_id,
                completed_by: owner.clone(),
                completion_authority: ProviderIngestCompletionAuthorityV1::new(
                    owner,
                    ProviderIngestCompletionSignerPolicyV1 {
                        policy_id: [policy_byte; 32],
                        revision: 2,
                        predecessor_digest: Some([policy_byte.wrapping_add(8); 32]),
                        policy_digest: [digest_byte; 32],
                    },
                ),
                replication_order,
                assignment_revision: u64::MAX
                    - 40
                    - u64::try_from(index).expect("provider index fits u64"),
                completion_epoch: 600 + u64::try_from(index).expect("provider index fits u64"),
                finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                    height: 700 + u64::try_from(index).expect("provider index fits u64"),
                    block_hash: [block_byte; 32],
                },
                archive_id: commitment.archive_id(),
                bundle_digest: commitment.bundle_digest,
                descriptor_digest: commitment.descriptor_digest,
                semantic_release_manifest_digest: publication.manifest.semantic_digest(),
                verification_lock_digest: publication.manifest.verification_lock_digest,
                source_tree_digest: commitment.source_tree_digest,
            };
            let payload = MusubiProviderBundleVerificationPayloadV1 {
                version: MUSUBI_REGISTRY_VERSION_V1,
                binding: binding.clone(),
            };
            let attestation = MusubiProviderBundleVerificationAttestationV1 {
                approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                    public_key: owner_keypair.public_key().clone(),
                    signature: SignatureOf::try_from_hash(
                        owner_keypair.private_key(),
                        payload.signing_hash(),
                    )
                    .expect("sign provider attestation"),
                }],
                payload,
            };
            attestation
                .verify(&binding)
                .expect("fixture provider attestation verifies");
            attestation
        },
    )
    .collect::<Vec<_>>();
    let add_location = AddMusubiArchiveLocationV1 {
        archive_id: commitment.archive_id(),
        location_id: MusubiArchiveLocationIdV1::new([0xC3; 32]),
        pin_manifest: ManifestDigest::new([0xC1; 32]),
        replication_order,
        provider_attestations,
        renew_after_epoch: 1_000,
        expires_at_epoch: 2_000,
        expected_location_revision: u64::MAX - 31,
    };
    let namespace_binding = MusubiNamespaceBindingV1 {
        namespace: "fixture.universal"
            .parse::<MusubiNamespaceV1>()
            .expect("fixture namespace"),
        home_dataspace: root_package.home_dataspace,
        scope: root_package.scope.clone(),
        generation: u64::MAX - 50,
    };
    let namespace_owner_keypair = keypair(0x80);
    let namespace_owner = AccountId::new(namespace_owner_keypair.public_key().clone());
    let delegation_payload = MusubiNamespaceDelegationPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        namespace_binding: namespace_binding.digest(),
        owner_generation: namespace_binding.generation,
        owner: namespace_owner.clone(),
        delegate: publisher.clone(),
        expires_at_height: u64::MAX - 1,
    };
    let namespace_delegation = MusubiNamespaceDelegationV1 {
        approvals: vec![MusubiNamespaceDelegationApprovalV1 {
            public_key: namespace_owner_keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                namespace_owner_keypair.private_key(),
                delegation_payload.signing_hash(),
            )
            .expect("sign namespace delegation"),
        }],
        payload: delegation_payload,
    };
    namespace_delegation
        .verify(
            &namespace_binding,
            &namespace_owner,
            namespace_binding.generation,
            &publisher,
            u64::MAX - 2,
        )
        .expect("fixture namespace delegation verifies");
    let publish = PublishMusubiReleaseV1::new(
        namespace_binding.namespace.clone(),
        publication,
        Some(namespace_delegation),
        u64::MAX - 29,
        None,
    );
    let set_metadata = SetMusubiPackageMetadataV1 {
        package: root_package,
        metadata: metadata(
            "Package metadata replaced after independent source and interface review.",
            "docs/overview.md",
            "LICENSES/Apache-2.0.txt",
            "https://example.invalid/musubi/reviewed-package",
            &["audit-ready", "sorafs", "supply-chain"],
        ),
        expected_metadata_revision: u64::MAX - 28,
    };
    let current_policy = MusubiRegistryPolicyV1::default();
    let replacement_policy = MusubiRegistryPolicyV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        revision: 2,
        mode: MusubiRegistryAdmissionModeV1::Allowlisted,
        allowlisted_dataspaces: vec![
            DataSpaceId::new(0),
            DataSpaceId::new(0x0123_4567_89AB_CDEF),
            DataSpaceId::new(u64::MAX),
        ],
        alias_pricing: MusubiAliasPricingPolicyV1 {
            revision: 2,
            length_1_xor: 2_000,
            length_2_xor: 400,
            length_3_xor: 80,
            length_4_xor: 16,
            length_5_to_32_xor: 2,
        },
    };
    replacement_policy
        .validate_successor(&current_policy)
        .expect("fixture policy is the exact successor");
    let policy_action =
        MusubiParliamentActionV1::SetRegistryPolicy(MusubiSetRegistryPolicyActionV1 {
            policy: replacement_policy.clone(),
            expected_revision: current_policy.revision,
        });
    policy_action
        .validate()
        .expect("fixture policy action is valid");
    let set_policy = SetMusubiRegistryPolicyV1 {
        decision: MusubiGovernanceDecisionV1 {
            decision_id: [0xF7; 32],
            action_digest: policy_action.action_digest(),
            enacted_at_height: u64::MAX - 1,
            execute_after_height: u64::MAX,
        },
        policy: replacement_policy,
        expected_policy_revision: current_policy.revision,
    };

    assert_ne!(accept.invite_id, revoke.invite_id);
    assert_ne!(
        accept.invite_id.as_bytes(),
        assertion.expected_digest.as_bytes()
    );
    assert_ne!(
        revoke.invite_id.as_bytes(),
        assertion.expected_digest.as_bytes()
    );
    assert!(assertion.release.version.is_prerelease());
    assert!(!unyank.yanked);
    assert_eq!(retire.expected_location_revision, u64::MAX);
    assert_eq!(unyank.expected_yank_revision, u64::MAX - 1);
    assert_eq!(remove.expected_governance_revision, u64::MAX - 2);
    register_namespace
        .binding
        .validate_authority_generation(u64::MAX)
        .expect("namespace binding generation is current");
    assert_eq!(
        register_namespace.binding.namespace.domain_segment(),
        Some("governance")
    );
    assert_eq!(register_namespace.expected_policy_revision, u64::MAX - 5);
    assert!(!invite.invite_id.is_zero());
    assert_eq!(invite.expires_at_height, u64::MAX);
    assert_eq!(invite.expected_governance_revision, u64::MAX - 3);
    let MusubiPackageRoleV1::Maintainer(permissions) = invite.role else {
        panic!("fixture invitation offers a maintainer role");
    };
    assert!(!permissions.is_empty());
    assert!(permissions.publish);
    assert!(!permissions.yank);
    assert!(permissions.metadata);
    assert!(!permissions.archive_locations);
    assert_ne!(invite.invited_account, promote.account);
    assert_eq!(promote.role, MusubiPackageRoleV1::Owner);
    assert_eq!(promote.expected_governance_revision, u64::MAX - 4);
    for decision in [
        recover.decision,
        retarget.decision,
        takedown.decision,
        set_policy.decision,
    ] {
        decision.validate().expect("valid enacted decision anchors");
        assert!(decision.enacted_at_height < decision.execute_after_height);
    }
    assert_eq!(
        BTreeSet::from([
            recover.decision.decision_id,
            retarget.decision.decision_id,
            takedown.decision.decision_id,
            set_policy.decision.decision_id,
        ])
        .len(),
        4
    );
    assert_eq!(recover.owners.len(), 3);
    assert!(recover.owners.windows(2).all(|pair| pair[0] < pair[1]));
    recovery_action
        .validate()
        .expect("valid package recovery action");
    retarget_action
        .validate()
        .expect("valid alias retarget action");
    takedown_action
        .validate()
        .expect("valid artifact takedown action");
    let MusubiParliamentActionV1::RecoverPackageOwners(recovery_payload) = &recovery_action else {
        panic!("fixture recovery action has the expected variant");
    };
    assert_eq!(recovery_payload.package, recover.package);
    assert_eq!(recovery_payload.owners, recover.owners);
    assert_eq!(
        recovery_payload.expected_revision,
        recover.expected_governance_revision
    );
    assert_eq!(
        recover.decision.action_digest,
        recovery_action.action_digest()
    );
    let MusubiParliamentActionV1::RetargetAlias(retarget_payload) = &retarget_action else {
        panic!("fixture alias action has the expected variant");
    };
    assert_eq!(retarget_payload.alias, retarget.alias);
    assert_eq!(retarget_payload.target, retarget.target);
    assert_eq!(
        retarget_payload.expected_revision,
        retarget.expected_history_revision
    );
    assert_eq!(
        retarget.decision.action_digest,
        retarget_action.action_digest()
    );
    let MusubiParliamentActionV1::TakedownArtifact(takedown_payload) = &takedown_action else {
        panic!("fixture takedown action has the expected variant");
    };
    assert_eq!(takedown_payload.release, takedown.release);
    assert_eq!(takedown_payload.reason, takedown.reason);
    assert_eq!(
        takedown_payload.expected_artifact_governance_revision,
        takedown.expected_artifact_governance_revision
    );
    assert_eq!(
        takedown.decision.action_digest,
        takedown_action.action_digest()
    );
    assert_eq!(recover.expected_governance_revision, u64::MAX - 6);
    assert_eq!(retarget.expected_history_revision, u64::MAX - 7);
    assert_eq!(takedown.expected_artifact_governance_revision, u64::MAX - 8);
    assert!(takedown.release.version.is_prerelease());
    register_archive
        .commitment
        .validate()
        .expect("fixture archive commitment remains valid");
    register_archive
        .staging_receipt
        .verify(&receipt_binding, 1_700_000_000_001)
        .expect("fixture staging receipt remains bound and signed");
    assert_eq!(
        register_archive.commitment.archive_id(),
        add_location.archive_id
    );
    assert_eq!(
        register_archive.commitment.archive_id(),
        publish.publication.manifest.archive_id
    );
    assert_eq!(
        register_archive.staging_receipt.payload.binding.archive_id,
        add_location.archive_id
    );
    assert_eq!(
        register_archive
            .staging_receipt
            .payload
            .binding
            .semantic_release_manifest_digest,
        publish.publication.manifest.semantic_digest()
    );
    assert_eq!(register_archive.expected_policy_revision, u64::MAX - 30);
    assert_eq!(add_location.provider_attestations.len(), 3);
    assert!(
        add_location
            .provider_attestations
            .windows(2)
            .all(|pair| pair[0].payload.binding.provider_id < pair[1].payload.binding.provider_id)
    );
    assert!(add_location.renew_after_epoch < add_location.expires_at_epoch);
    assert_eq!(add_location.expected_location_revision, u64::MAX - 31);
    for attestation in &add_location.provider_attestations {
        let binding = &attestation.payload.binding;
        attestation
            .verify(binding)
            .expect("fixture provider attestation remains bound and signed");
        assert_eq!(binding.archive_id, add_location.archive_id);
        assert_eq!(binding.replication_order, add_location.replication_order);
        assert_eq!(binding.chain_id, receipt_binding.chain_id);
        assert_eq!(
            binding.semantic_release_manifest_digest,
            publish.publication.manifest.semantic_digest()
        );
        assert_eq!(
            binding.verification_lock_digest,
            publish.publication.manifest.verification_lock_digest
        );
        assert_eq!(
            binding.bundle_digest,
            register_archive.commitment.bundle_digest
        );
        assert_eq!(
            binding.descriptor_digest,
            register_archive.commitment.descriptor_digest
        );
        assert_eq!(
            binding.source_tree_digest,
            register_archive.commitment.source_tree_digest
        );
    }
    publish
        .publication
        .validate()
        .expect("fixture publication remains bound to its exact graph");
    assert_eq!(publish.namespace, namespace_binding.namespace);
    assert_eq!(publish.expected_policy_revision, u64::MAX - 29);
    assert_eq!(publish.expected_governance_revision, None);
    let delegation = publish
        .namespace_delegation
        .as_ref()
        .expect("fixture publication carries a namespace delegation");
    delegation
        .verify(
            &namespace_binding,
            &namespace_owner,
            namespace_binding.generation,
            &publisher,
            u64::MAX - 2,
        )
        .expect("fixture namespace delegation remains bound and signed");
    assert_eq!(delegation.payload.delegate, receipt_binding.publisher);
    set_metadata
        .metadata
        .validate()
        .expect("fixture replacement metadata remains canonical");
    assert_eq!(set_metadata.expected_metadata_revision, u64::MAX - 28);
    set_policy
        .policy
        .validate_successor(&current_policy)
        .expect("fixture replacement policy remains the exact successor");
    let MusubiParliamentActionV1::SetRegistryPolicy(policy_payload) = &policy_action else {
        panic!("fixture registry policy action has the expected variant");
    };
    assert_eq!(policy_payload.policy, set_policy.policy);
    assert_eq!(
        policy_payload.expected_revision,
        set_policy.expected_policy_revision
    );
    assert_eq!(set_policy.expected_policy_revision, current_policy.revision);
    assert_eq!(
        set_policy.decision.action_digest,
        policy_action.action_digest()
    );

    assert_case(
        case(&root, "accept-root-max-revision"),
        accept,
        AcceptMusubiPackageMaintainerV1::WIRE_ID,
    );
    assert_case(
        case(&root, "revoke-domain-invitation"),
        revoke,
        RevokeMusubiPackageMaintainerInvitationV1::WIRE_ID,
    );
    assert_case(
        case(&root, "register-alias-domain-target"),
        alias,
        RegisterMusubiAliasV1::WIRE_ID,
    );
    assert_case(
        case(&root, "assert-prerelease-digest"),
        assertion,
        AssertMusubiReleaseDigestV1::WIRE_ID,
    );
    assert_case(
        case(&root, "retire-location-max-revision"),
        retire,
        RetireMusubiArchiveLocationV1::WIRE_ID,
    );
    assert_case(
        case(&root, "unyank-domain-release-high-revision"),
        unyank,
        SetMusubiReleaseYankV1::WIRE_ID,
    );
    assert_case(
        case(&root, "remove-root-maintainer-high-revision"),
        remove,
        RemoveMusubiPackageMaintainerV1::WIRE_ID,
    );
    assert_case(
        case(&root, "register-domain-namespace-max-generation"),
        register_namespace,
        RegisterMusubiNamespaceBindingV1::WIRE_ID,
    );
    assert_case(
        case(&root, "invite-domain-maintainer-max-expiry"),
        invite,
        InviteMusubiPackageMaintainerV1::WIRE_ID,
    );
    assert_case(
        case(&root, "promote-root-member-to-owner-high-revision"),
        promote,
        SetMusubiPackageMaintainerRoleV1::WIRE_ID,
    );
    assert_case(
        case(&root, "recover-domain-package-three-owners"),
        recover,
        RecoverMusubiPackageV1::WIRE_ID,
    );
    assert_case(
        case(&root, "retarget-one-character-alias-high-revision"),
        retarget,
        RetargetMusubiAliasV1::WIRE_ID,
    );
    assert_case(
        case(&root, "takedown-max-major-prerelease"),
        takedown,
        SetMusubiArtifactTakedownV1::WIRE_ID,
    );
    assert_case(
        case(&root, "register-archive-max-bounds-signed-receipt"),
        register_archive,
        RegisterMusubiArchiveV1::WIRE_ID,
    );
    assert_case(
        case(&root, "add-location-three-signed-providers"),
        add_location,
        AddMusubiArchiveLocationV1::WIRE_ID,
    );
    assert_case(
        case(&root, "publish-delegated-domain-release"),
        publish,
        PublishMusubiReleaseV1::WIRE_ID,
    );
    assert_case(
        case(&root, "replace-domain-metadata-high-revision"),
        set_metadata,
        SetMusubiPackageMetadataV1::WIRE_ID,
    );
    assert_case(
        case(&root, "set-allowlisted-policy-repriced-aliases"),
        set_policy,
        SetMusubiRegistryPolicyV1::WIRE_ID,
    );
}
