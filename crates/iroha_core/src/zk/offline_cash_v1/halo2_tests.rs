use std::{
    collections::BTreeSet,
    io::{Cursor, Seek as _, SeekFrom, Write as _},
    sync::Arc,
};

use iroha_data_model::offline::{
    OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_PARAMS_BYTES_V1, OfflineCashArtifactBindingV1,
    OfflineCashArtifactRoleV1, OfflineCashAuthenticatedReleaseV1,
};
use sha2::{Digest as _, Sha256};

use super::{halo2_backend::OfflineCashHalo2VerifierBackendV1, *};

#[derive(Clone, Copy)]
enum SourceBehavior {
    Normal,
    OmitCallback,
    RepeatCallback,
    SwallowCallbackError,
}

struct TestArtifactSource {
    release: OfflineCashAuthenticatedReleaseV1,
    payloads: Vec<(OfflineCashArtifactRoleV1, Vec<u8>)>,
    behavior: SourceBehavior,
}

impl super::artifacts::sealed::Sealed for TestArtifactSource {}

impl OfflineCashHalo2ArtifactSourceV1 for TestArtifactSource {
    fn authenticated_release(&self) -> &OfflineCashAuthenticatedReleaseV1 {
        &self.release
    }

    fn with_artifact(
        &self,
        role: OfflineCashArtifactRoleV1,
        consume: &mut dyn FnMut(&mut dyn std::io::Read) -> Result<(), String>,
    ) -> Result<(), String> {
        if matches!(self.behavior, SourceBehavior::OmitCallback) {
            return Ok(());
        }
        let payload = self
            .payloads
            .iter()
            .find_map(|(candidate, payload)| (*candidate == role).then_some(payload))
            .ok_or_else(|| "missing test artifact".to_owned())?;
        let mut reader = Cursor::new(payload);
        let first = consume(&mut reader);
        if matches!(self.behavior, SourceBehavior::RepeatCallback) {
            let mut repeated = Cursor::new(payload);
            consume(&mut repeated)?;
        }
        if matches!(self.behavior, SourceBehavior::SwallowCallbackError) {
            Ok(())
        } else {
            first
        }
    }
}

fn artifact_payloads() -> Vec<(OfflineCashArtifactRoleV1, Vec<u8>)> {
    OfflineCashArtifactRoleV1::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, role)| {
            let len = match role {
                OfflineCashArtifactRoleV1::ParamsEq | OfflineCashArtifactRoleV1::ParamsEp => {
                    usize::try_from(OFFLINE_CASH_PARAMS_BYTES_V1).expect("params length fits usize")
                }
                _ => 64 + index,
            };
            let byte = u8::try_from(index + 1).expect("artifact index fits u8");
            (role, vec![byte; len])
        })
        .collect()
}

fn artifact_bindings(
    payloads: &[(OfflineCashArtifactRoleV1, Vec<u8>)],
) -> Vec<OfflineCashArtifactBindingV1> {
    payloads
        .iter()
        .map(|(role, payload)| OfflineCashArtifactBindingV1 {
            role: *role,
            sha256: Sha256::digest(payload).into(),
            byte_len: u64::try_from(payload.len()).expect("artifact length fits u64"),
        })
        .collect()
}

fn source_with_behavior(behavior: SourceBehavior) -> Arc<TestArtifactSource> {
    let payloads = artifact_payloads();
    let release =
        super::terminal_tests::authenticated_release_for_artifacts(artifact_bindings(&payloads));
    Arc::new(TestArtifactSource {
        release,
        payloads,
        behavior,
    })
}

fn artifact_files(
    payloads: &[(OfflineCashArtifactRoleV1, Vec<u8>)],
) -> Vec<(OfflineCashArtifactRoleV1, std::fs::File)> {
    payloads
        .iter()
        .map(|(role, payload)| {
            let mut file = tempfile::tempfile().expect("create anonymous regular artifact file");
            file.write_all(payload)
                .expect("write complete artifact file");
            file.seek(SeekFrom::Start(0)).expect("rewind artifact file");
            (*role, file)
        })
        .collect()
}

#[test]
fn profile_pins_public_contract_k_finite_roles_caps_and_distinct_protocols() {
    assert_eq!(OFFLINE_CASH_HALO2_K_V1, 16);
    assert_eq!(
        hex::encode(offline_cash_halo2_profile_digest_v1()),
        "c73faa73caa6159316f87947816ecfd43c1bd2b586dd150891b9e99373d60152"
    );
    assert_eq!(OfflineCashHalo2ParityV1::ALL.len(), 2);
    assert_eq!(OfflineCashHalo2CircuitRoleV1::ALL.len(), 8);

    let expected = [
        "ff078de121d1a59afe992fd49fe71966a328de59795e5d0892fed5ec4c0040dc",
        "86f2b4257464dfbf7a8756a98d359adfed51b37122e281214001ff8481ff929f",
        "d479e97fe1ba9bb996689b73ba5f081b8ecd2442b1081caae679dbd42dc3484b",
        "782e7c21f52e666714769f5db6554d4313b81034616738b1743b05393832ac33",
        "34ebc9bf9f81943a2b5900041e66592235b2ede15fb0b55df6661ea2b6abb14c",
        "85cd051adb52110b8a5bdb3a3c87d4857b5ebd3ef6d2035119cac169269a408e",
        "7841ab2596d3385e01f0a53d379d525e9357cc339da5a150edbc11aa5024648d",
        "6b73120690837aa75a8f49cd5be11dd05a7cdc9bd3f65ba9c0bbcb0cfe5f6217",
        "f9eca727b3657e1794bd0fb7062396899a1013efef4f9fe48c1121d74b876133",
        "9cadf62765eb7551c8d56e12d9c863a5aef25dc1a53ac19637fcf81242e7b223",
        "cd65ef1b1576c1c115603175b0f5b1fb5643ade0bd4fa52e38c3829686e4d00a",
        "b0f28d619d5a5299b12badf4ae4efceed5a3de0ea1107d014a8ac17e641fac54",
        "80b6fc92e32ad117610d42841968a7e7959365cceded60e2a03a7f6671d8333e",
        "aa7671ca094a41db951a0a825350921cd1966330b8a6b9f1bf36c768c6d9c4d8",
        "330bc59c9ccef9434fd8e60bfc046e2a5c4a4fa9ef64fddb30ac0927e5c5c76e",
        "a20d1659d66d8ae2ae5152590bdb27ac311c598bdf7317fc3d7d31360748a012",
    ];
    let mut identities = BTreeSet::new();
    let mut actual = Vec::with_capacity(expected.len());
    for parity in OfflineCashHalo2ParityV1::ALL {
        for role in OfflineCashHalo2CircuitRoleV1::ALL {
            let identity = offline_cash_halo2_protocol_identity_v1(parity, role);
            assert_eq!(identity.parity(), parity);
            assert_eq!(identity.circuit_role(), role);
            assert_ne!(identity.digest(), [0; 32]);
            assert!(identities.insert(identity.digest()));
            actual.push(hex::encode(identity.digest()));
        }
    }
    assert_eq!(actual, expected);
    assert_eq!(identities.len(), 16);
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_ABI_WORDS_V1, 229);
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1, 7);
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_INSTANCE_COLUMNS_V1, 2);
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_INSTANCE_CELLS_V1, 33);
    assert_eq!(
        super::protocol::OFFLINE_CASH_STATE_INSTANCE_CELLS_MAX_V1,
        50
    );
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_SHA_LANES_V1, 5);
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_SHA_JOBS_V1, 13);
    assert_eq!(
        super::protocol::OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1,
        [6, 6, 5, 6, 6, 2, 2, 5, 6, 7, 5, 8, 7]
    );
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1, 71);
    assert_eq!(super::protocol::OFFLINE_CASH_HELPER_ABI_WORDS_V1, 184);
    assert_eq!(
        super::protocol::OFFLINE_CASH_HELPER_WORDS_PER_INSTANCE_V1,
        7
    );
    assert_eq!(super::protocol::OFFLINE_CASH_HELPER_INSTANCE_COLUMNS_V1, 1);
    assert_eq!(super::protocol::OFFLINE_CASH_HELPER_INSTANCE_CELLS_V1, 27);
    assert_eq!(
        super::protocol::OFFLINE_CASH_HELPER_INSTANCE_CELLS_MAX_V1,
        32
    );

    for role in OfflineCashArtifactRoleV1::ALL {
        let (minimum, maximum) = super::protocol::offline_cash_artifact_length_bounds_v1(role);
        assert!(minimum != 0 && minimum <= maximum);
        if matches!(
            role,
            OfflineCashArtifactRoleV1::ParamsEq | OfflineCashArtifactRoleV1::ParamsEp
        ) {
            assert_eq!(
                (minimum, maximum),
                (OFFLINE_CASH_PARAMS_BYTES_V1, OFFLINE_CASH_PARAMS_BYTES_V1)
            );
        } else {
            assert!(super::protocol::offline_cash_artifact_protocol_v1(role).is_some());
        }
    }
}

#[test]
fn manifest_rejects_authenticated_profile_and_protocol_substitution() {
    let payloads = artifact_payloads();
    let bindings = artifact_bindings(&payloads);
    let eq = offline_cash_halo2_protocol_identity_v1(
        OfflineCashHalo2ParityV1::Eq,
        OfflineCashHalo2CircuitRoleV1::State,
    )
    .digest();
    let ep = offline_cash_halo2_protocol_identity_v1(
        OfflineCashHalo2ParityV1::Ep,
        OfflineCashHalo2CircuitRoleV1::State,
    )
    .digest();

    let wrong_profile = super::terminal_tests::authenticated_release_for_artifacts_and_protocol(
        bindings.clone(),
        [0xA1; 32],
        eq,
        ep,
    );
    assert_eq!(
        OfflineCashHalo2ArtifactManifestV1::from_authenticated_release(&wrong_profile),
        Err(OfflineCashHalo2ArtifactErrorV1::ProfileMismatch)
    );

    let wrong_protocol = super::terminal_tests::authenticated_release_for_artifacts_and_protocol(
        bindings,
        offline_cash_halo2_profile_digest_v1(),
        [0xA2; 32],
        ep,
    );
    assert_eq!(
        OfflineCashHalo2ArtifactManifestV1::from_authenticated_release(&wrong_protocol),
        Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch)
    );
}

#[test]
fn manifest_rejects_cross_role_artifact_digest_aliasing() {
    let payloads = artifact_payloads();
    let mut bindings = artifact_bindings(&payloads);
    bindings[1].sha256 = bindings[0].sha256;
    let release = super::terminal_tests::authenticated_release_for_artifacts(bindings);
    assert_eq!(
        OfflineCashHalo2ArtifactManifestV1::from_authenticated_release(&release),
        Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest)
    );
}

#[test]
fn artifact_source_authenticates_all_required_verifier_bytes() {
    let source = source_with_behavior(SourceBehavior::Normal);
    let artifacts = OfflineCashAuthenticatedVerifierArtifactsV1::load(source)
        .expect("hash-authenticated verifier artifacts");
    let manifest = artifacts.manifest();
    assert_eq!(
        manifest.state_protocol_digest(OfflineCashHalo2ParityV1::Eq),
        offline_cash_halo2_protocol_identity_v1(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::State,
        )
        .digest()
    );
    assert_eq!(
        manifest.artifact(OfflineCashArtifactRoleV1::StateVkEp).role,
        OfflineCashArtifactRoleV1::StateVkEp
    );
}

#[test]
fn authenticated_file_set_pins_and_authenticates_all_34_roles() {
    let payloads = artifact_payloads();
    let release =
        super::terminal_tests::authenticated_release_for_artifacts(artifact_bindings(&payloads));
    let expected_release_id = release.release_id();
    let expected_manifest_digest = release.manifest_digest();
    let source = OfflineCashAuthenticatedArtifactFileSetV1::new(release, artifact_files(&payloads))
        .expect("complete canonical file set authenticates");
    assert_eq!(
        source.artifact_count(),
        OfflineCashArtifactRoleV1::ALL.len()
    );
    assert_eq!(source.release_id(), expected_release_id);
    assert_eq!(source.manifest_digest(), expected_manifest_digest);

    OfflineCashAuthenticatedVerifierArtifactsV1::load(Arc::new(source))
        .expect("pinned file set remains valid for the first-party verifier loader");
}

#[test]
fn authenticated_file_set_rejects_role_order_and_post_install_mutation() {
    let payloads = artifact_payloads();
    let release =
        super::terminal_tests::authenticated_release_for_artifacts(artifact_bindings(&payloads));
    let mut wrong_order = artifact_files(&payloads);
    wrong_order.swap(0, 1);
    assert!(matches!(
        OfflineCashAuthenticatedArtifactFileSetV1::new(release, wrong_order),
        Err(OfflineCashArtifactFileSetErrorV1::InvalidInventory)
    ));

    let release =
        super::terminal_tests::authenticated_release_for_artifacts(artifact_bindings(&payloads));
    let files = artifact_files(&payloads);
    let state_vk_index = OfflineCashArtifactRoleV1::ALL
        .iter()
        .position(|role| *role == OfflineCashArtifactRoleV1::StateVkEq)
        .expect("StateVkEq belongs to the finite inventory");
    let mut external = files[state_vk_index]
        .1
        .try_clone()
        .expect("clone test file handle");
    let source = OfflineCashAuthenticatedArtifactFileSetV1::new(release, files)
        .expect("initial exact bytes authenticate");
    external
        .seek(SeekFrom::Start(0))
        .expect("seek external file handle");
    external
        .write_all(&[0xFF])
        .expect("mutate one byte through the external handle");
    external.flush().expect("flush external mutation");

    assert!(matches!(
        OfflineCashAuthenticatedVerifierArtifactsV1::load(Arc::new(source)),
        Err(OfflineCashHalo2ArtifactErrorV1::DigestMismatch)
    ));
}

#[test]
fn helper_artifact_boundary_is_exact_authenticated_and_non_authorizing() {
    let source = source_with_behavior(SourceBehavior::Normal);
    let artifacts = OfflineCashAuthenticatedVerifierArtifactsV1::load(source)
        .expect("STATE artifact bootstrap remains authenticated");
    let cases = [
        (
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
            OfflineCashArtifactRoleV1::GuardUseVkEq,
        ),
        (
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
            OfflineCashArtifactRoleV1::GuardUseVkEp,
        ),
        (
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::PlatformBind,
            OfflineCashArtifactRoleV1::PlatformBindVkEq,
        ),
        (
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::PlatformBind,
            OfflineCashArtifactRoleV1::PlatformBindVkEp,
        ),
        (
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::AndroidKeyCert,
            OfflineCashArtifactRoleV1::AndroidKeyCertVkEq,
        ),
        (
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::AndroidKeyCert,
            OfflineCashArtifactRoleV1::AndroidKeyCertVkEp,
        ),
        (
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf,
            OfflineCashArtifactRoleV1::GuardBundleLeafVkEq,
        ),
        (
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf,
            OfflineCashArtifactRoleV1::GuardBundleLeafVkEp,
        ),
    ];
    for (parity, role, verifier_role) in cases {
        let verifier = artifacts.manifest().artifact(verifier_role);
        let protocol = artifacts.manifest().protocol_digest(parity, role);
        assert_eq!(
            protocol,
            offline_cash_halo2_protocol_identity_v1(parity, role).digest()
        );
        artifacts
            .authenticate_helper_verifier(parity, role, verifier, protocol)
            .expect("exact helper parameters, VK, and compiled protocol authenticate");
    }

    let guard_use_protocol = artifacts.manifest().protocol_digest(
        OfflineCashHalo2ParityV1::Eq,
        OfflineCashHalo2CircuitRoleV1::GuardUse,
    );
    assert_eq!(
        artifacts.authenticate_helper_verifier(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
            artifacts
                .manifest()
                .artifact(OfflineCashArtifactRoleV1::GuardUsePkEq),
            guard_use_protocol,
        ),
        Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest)
    );
    let mut wrong_protocol = guard_use_protocol;
    wrong_protocol[0] ^= 1;
    assert_eq!(
        artifacts.authenticate_helper_verifier(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
            artifacts
                .manifest()
                .artifact(OfflineCashArtifactRoleV1::GuardUseVkEq),
            wrong_protocol,
        ),
        Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch)
    );
    assert_eq!(
        artifacts.authenticate_helper_verifier(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::State,
            artifacts
                .manifest()
                .artifact(OfflineCashArtifactRoleV1::StateVkEq),
            artifacts
                .manifest()
                .state_protocol_digest(OfflineCashHalo2ParityV1::Eq),
        ),
        Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest)
    );

    let payloads = artifact_payloads();
    let bindings = artifact_bindings(&payloads);
    let release = super::terminal_tests::authenticated_release_for_artifacts(bindings);
    let mut corrupt = payloads;
    let (_, helper_vk) = corrupt
        .iter_mut()
        .find(|(role, _)| *role == OfflineCashArtifactRoleV1::GuardBundleLeafVkEp)
        .expect("Ep GuardBundleLeaf VK");
    helper_vk[0] ^= 1;
    let corrupt_source = Arc::new(TestArtifactSource {
        release,
        payloads: corrupt,
        behavior: SourceBehavior::Normal,
    });
    let corrupt_artifacts = OfflineCashAuthenticatedVerifierArtifactsV1::load(corrupt_source)
        .expect("helper VKs remain lazy and non-authorizing at bootstrap");
    assert_eq!(
        corrupt_artifacts.authenticate_helper_verifier(
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf,
            corrupt_artifacts
                .manifest()
                .artifact(OfflineCashArtifactRoleV1::GuardBundleLeafVkEp),
            corrupt_artifacts.manifest().protocol_digest(
                OfflineCashHalo2ParityV1::Ep,
                OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf,
            ),
        ),
        Err(OfflineCashHalo2ArtifactErrorV1::DigestMismatch)
    );
}

#[test]
fn public_abi22_and_kagemusha_v4_contract_are_profile_bound() {
    use iroha_data_model::offline::{
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V5,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4, KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
    };

    let canonical = offline_cash_halo2_profile_digest_v1();
    assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4, 22);
    assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4, 4);
    assert_eq!(KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4, 4);
    assert_eq!(
        canonical,
        super::protocol::offline_cash_halo2_profile_digest_for_public_contract_test_v1(
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        )
    );

    let substitutions = [
        (
            21,
            KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        ),
        (
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            3,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        ),
        (
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V5,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
        ),
        (
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
            5,
        ),
    ];
    for (bridge_abi, data_wire, manifest_schema, manifest_version) in substitutions {
        let substituted =
            super::protocol::offline_cash_halo2_profile_digest_for_public_contract_test_v1(
                bridge_abi,
                data_wire,
                manifest_schema,
                manifest_version,
            );
        assert_ne!(substituted, canonical);

        let payloads = artifact_payloads();
        let release = super::terminal_tests::authenticated_release_for_artifacts_and_protocol(
            artifact_bindings(&payloads),
            substituted,
            offline_cash_halo2_protocol_identity_v1(
                OfflineCashHalo2ParityV1::Eq,
                OfflineCashHalo2CircuitRoleV1::State,
            )
            .digest(),
            offline_cash_halo2_protocol_identity_v1(
                OfflineCashHalo2ParityV1::Ep,
                OfflineCashHalo2CircuitRoleV1::State,
            )
            .digest(),
        );
        assert_eq!(
            OfflineCashHalo2ArtifactManifestV1::from_authenticated_release(&release).unwrap_err(),
            OfflineCashHalo2ArtifactErrorV1::ProfileMismatch,
            "public contract substitution must fail before artifact parsing"
        );
    }
}

#[test]
fn artifact_corruption_and_callback_contracts_fail_closed() {
    let payloads = artifact_payloads();
    let bindings = artifact_bindings(&payloads);
    let release = super::terminal_tests::authenticated_release_for_artifacts(bindings);
    let mut corrupt = payloads;
    let (_, state_vk_eq) = corrupt
        .iter_mut()
        .find(|(role, _)| *role == OfflineCashArtifactRoleV1::StateVkEq)
        .expect("Eq STATE VK");
    state_vk_eq[0] ^= 1;
    let source = Arc::new(TestArtifactSource {
        release,
        payloads: corrupt,
        behavior: SourceBehavior::SwallowCallbackError,
    });
    assert_eq!(
        OfflineCashHalo2VerifierBackendV1::from_artifact_source(source).unwrap_err(),
        OfflineCashHalo2ArtifactErrorV1::DigestMismatch
    );

    let payloads = artifact_payloads();
    let bindings = artifact_bindings(&payloads);
    let release = super::terminal_tests::authenticated_release_for_artifacts(bindings);
    let mut truncated = payloads;
    truncated
        .iter_mut()
        .find(|(role, _)| *role == OfflineCashArtifactRoleV1::StateVkEp)
        .expect("Ep STATE VK")
        .1
        .pop();
    let source = Arc::new(TestArtifactSource {
        release,
        payloads: truncated,
        behavior: SourceBehavior::Normal,
    });
    assert_eq!(
        OfflineCashHalo2VerifierBackendV1::from_artifact_source(source).unwrap_err(),
        OfflineCashHalo2ArtifactErrorV1::LengthMismatch
    );

    assert_eq!(
        OfflineCashHalo2VerifierBackendV1::from_artifact_source(source_with_behavior(
            SourceBehavior::OmitCallback,
        ))
        .unwrap_err(),
        OfflineCashHalo2ArtifactErrorV1::SourceContractViolation
    );
    assert_eq!(
        OfflineCashHalo2VerifierBackendV1::from_artifact_source(source_with_behavior(
            SourceBehavior::RepeatCallback,
        ))
        .unwrap_err(),
        OfflineCashHalo2ArtifactErrorV1::SourceContractViolation
    );
}

#[test]
fn malformed_semantic_artifacts_and_role_substitution_fail_before_verification() {
    let source = source_with_behavior(SourceBehavior::Normal);
    assert_eq!(
        OfflineCashHalo2VerifierBackendV1::from_artifact_source(source.clone()).unwrap_err(),
        OfflineCashHalo2ArtifactErrorV1::InvalidParameterArtifact
    );

    let artifacts = OfflineCashAuthenticatedVerifierArtifactsV1::load(source)
        .expect("hash-authenticated verifier artifacts");
    let manifest = artifacts.manifest();
    let eq_vk = manifest.artifact(OfflineCashArtifactRoleV1::StateVkEq);
    let eq_protocol = manifest.state_protocol_digest(OfflineCashHalo2ParityV1::Eq);
    let ep_vk = manifest.artifact(OfflineCashArtifactRoleV1::StateVkEp);
    assert_eq!(
        artifacts.authenticate_state_verifier(OfflineCashHalo2ParityV1::Eq, ep_vk, eq_protocol,),
        Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest)
    );
    let mut wrong_protocol = eq_protocol;
    wrong_protocol[0] ^= 1;
    assert_eq!(
        artifacts.authenticate_state_verifier(OfflineCashHalo2ParityV1::Eq, eq_vk, wrong_protocol,),
        Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch)
    );
}

#[test]
fn terminal_boundary_cannot_mint_receipt_from_unparsed_artifact_bytes() {
    let source = source_with_behavior(SourceBehavior::Normal);
    assert_eq!(
        OfflineCashHalo2VerifierBackendV1::from_artifact_source(source).unwrap_err(),
        OfflineCashHalo2ArtifactErrorV1::InvalidParameterArtifact
    );
}

#[test]
fn shared_pasta_profile_is_fixed_and_offline_cash_preflight_is_non_activating() {
    use crate::zk::pasta_ipa_recursion::{
        PASTA_IPA_POSEIDON_FULL_ROUNDS_V1, PASTA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
        PASTA_IPA_POSEIDON_RATE_V1, PASTA_IPA_POSEIDON_SECURE_MDS_V1, PASTA_IPA_POSEIDON_WIDTH_V1,
        pasta_ipa_direct_instance_compile_config_v1,
    };

    assert_eq!(PASTA_IPA_POSEIDON_WIDTH_V1, 3);
    assert_eq!(PASTA_IPA_POSEIDON_RATE_V1, 2);
    assert_eq!(PASTA_IPA_POSEIDON_FULL_ROUNDS_V1, 8);
    assert_eq!(PASTA_IPA_POSEIDON_PARTIAL_ROUNDS_V1, 57);
    assert_eq!(PASTA_IPA_POSEIDON_SECURE_MDS_V1, 0);
    let config = format!("{:?}", pasta_ipa_direct_instance_compile_config_v1(27));
    assert!(config.contains("zk: true"));
    assert!(config.contains("query_instance: false"));
    assert!(config.contains("num_proof: 1"));
    assert!(config.contains("num_instance: [27]"));

    let shared = include_str!("../pasta_ipa_recursion.rs");
    assert!(shared.contains("final BGH19 folded generator contributes one extra"));
    assert!(shared.contains("PastaIpaInstanceQueryV1::Direct => 0"));
    assert!(shared.contains("PastaIpaInstanceQueryV1::Queried => cs.instance_queries().len()"));

    let adapter = include_str!("../kagemusha_recursion_adapter.rs");
    assert!(adapter.contains("pasta_ipa_direct_instance_compile_config_v1(public_len)"));
    assert!(adapter.contains("pasta_ipa_augmented_proof_shape_v1("));
    assert!(adapter.contains("PastaIpaInstanceQueryV1::Direct"));
    let accumulation = include_str!("../kagemusha_accumulation.rs");
    assert!(accumulation.contains("PASTA_IPA_POSEIDON_WIDTH_V1"));
    assert!(accumulation.contains("PASTA_IPA_POSEIDON_PARTIAL_ROUNDS_V1"));

    let protocol = include_str!("protocol.rs");
    assert!(protocol.contains("preflight_offline_cash_recursion_activation_v1"));
    assert!(protocol.contains("Passing this gate is deliberately not proof authority"));
    assert!(protocol.contains("snark-verifier/PoseidonTranscript(width3,rate2,full8,partial57"));
    assert!(protocol.contains("state-compact100-wire+guard-bundle-canonical68-domain-sha256-join"));
    assert!(!protocol.contains(concat!("compact", "68-wire")));
    let backend = include_str!("halo2_backend.rs");
    assert!(backend.contains("terminal_verify_eq_outer_and_carried_v1"));
    assert!(backend.contains("terminal_verify_ep_outer_and_carried_v1"));
    assert!(!backend.contains(concat!("verify_augmented_", "ipa_proof_v1")));
    assert!(!backend.contains(concat!("OfflineCashIpa", "HistoryV1")));
    assert!(backend.contains("authorize_verified_credit"));
    assert!(!backend.contains("preflight_offline_cash_recursion_activation_v1"));
}

#[test]
fn staged_warning_is_confined_and_public_verifier_facade_is_unforgeable() {
    const STAGING_REASON: &str = "offline-cash STATE verification is connected but production authority remains blocked on the governed proof cap, reviewed recursion, and secure-device activation";

    fn without_whitespace(source: &str) -> String {
        source
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect()
    }

    let zk_source = include_str!("../../zk.rs");
    assert_eq!(zk_source.matches(STAGING_REASON).count(), 1);
    let compact_zk = without_whitespace(zk_source);
    let compact_reason = without_whitespace(STAGING_REASON);
    assert!(compact_zk.contains(&format!(
        "#[allow(dead_code,reason=\"{compact_reason}\")]pubmodoffline_cash_v1;"
    )));

    let root = include_str!("../offline_cash_v1.rs");
    let backend = include_str!("halo2_backend.rs");
    let staged_sources = [
        root,
        include_str!("artifacts.rs"),
        backend,
        include_str!("halo2_primitives.rs"),
        include_str!("helper_abi.rs"),
        include_str!("helper_circuit.rs"),
        include_str!("helper_relation.rs"),
        include_str!("protocol.rs"),
        include_str!("state_abi.rs"),
        include_str!("state_circuit.rs"),
        include_str!("state_relation.rs"),
        include_str!("state_relation_circuit.rs"),
        include_str!("state_sha.rs"),
        include_str!("state_transition.rs"),
        include_str!("state_transition/balance.rs"),
        include_str!("state_transition/context.rs"),
        include_str!("state_transition/credit.rs"),
        include_str!("state_transition/guard.rs"),
        include_str!("state_transition/pending.rs"),
        include_str!("state_transition/receive.rs"),
        include_str!("state_transition/send.rs"),
    ];
    for source in staged_sources {
        assert!(
            !without_whitespace(source).contains("allow(dead_code"),
            "dead-code allowances must remain at the offline_cash_v1 module boundary"
        );
        for line in source.lines().map(str::trim_start) {
            let externally_public = ["pub trait ", "pub struct ", "pub enum ", "pub use "]
                .iter()
                .any(|prefix| line.starts_with(prefix));
            let acceptance_authority = ["Verifier", "Proof", "VerifiedOfflineCashCredit"]
                .iter()
                .any(|name| line.contains(name));
            let allowed_opaque_facade = [
                "pub enum OfflineCashHalo2ArtifactErrorV1",
                "pub enum OfflineCashVerificationStageV1",
                "pub enum OfflineCashVerificationErrorV1",
                "pub struct VerifiedOfflineCashCreditV1",
                "pub struct OfflineCashVerifierV1",
            ]
            .iter()
            .any(|declaration| line.starts_with(declaration));
            assert!(
                !(externally_public && acceptance_authority && !allowed_opaque_facade),
                "only the exact opaque Core verifier facade may be public: {line}"
            );
        }
    }

    let compact_root = without_whitespace(root);
    let compact_backend = without_whitespace(backend);
    assert!(compact_root.contains(
        "pub(crate)traitOfflineCashPairedProofVerifierV1:paired_verifier_sealed::Sealed"
    ));
    assert!(compact_root.contains("pub(crate)modpaired_verifier_sealed{"));
    assert!(compact_root.contains("pub(crate)traitSealed{}"));
    assert!(!compact_root.contains("pubtraitOfflineCashPairedProofVerifierV1"));
    assert!(!compact_root.contains("pubmodpaired_verifier_sealed"));
    assert!(compact_backend.contains("pub(crate)structOfflineCashHalo2VerifierBackendV1"));
    assert!(backend.contains("OfflineCashStatePublicInstancesV1"));
    assert!(backend.contains("parse_processed_verifier_key_v1"));
    assert!(backend.contains("PRODUCTION_ACTIVATION_BLOCKER_V1"));
    assert!(!compact_backend.contains("pubstructOfflineCashHalo2VerifierBackendV1"));
    assert!(!compact_root.contains("pubusehalo2_backend::OfflineCashHalo2VerifierBackendV1"));
    assert!(compact_root.contains("pubstructOfflineCashVerifierV1{"));
    assert!(compact_root.contains("pubstructVerifiedOfflineCashCreditV1{"));
    assert!(compact_root.contains(
        "pubfnfrom_authenticated_artifact_file_set(source:OfflineCashAuthenticatedArtifactFileSetV1,)->Result<Self,OfflineCashHalo2ArtifactErrorV1>"
    ));
    assert!(!compact_root.contains("pubfnnew(release:&OfflineCashAuthenticatedReleaseV1"));
    assert!(!compact_root.contains("pubfnfrom_artifact_source"));
}
