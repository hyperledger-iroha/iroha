use std::{collections::BTreeSet, io::Cursor, sync::Arc};

use halo2_proofs::halo2curves::{
    group::prime::PrimeCurveAffine as _,
    pasta::{EpAffine, EqAffine, Fp, Fq},
};
use iroha_data_model::offline::{
    OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_PARAMS_BYTES_V1, OfflineCashArtifactBindingV1,
    OfflineCashArtifactRoleV1, OfflineCashAuthenticatedReleaseV1,
};
use sha2::{Digest as _, Sha256};

use super::*;

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

fn canonical_eq_history() -> Vec<u8> {
    let history = super::halo2_primitives::test_support::history_from_eq_parts(
        std::array::from_fn(|index| Fp::from((index + 1) as u64)),
        EqAffine::generator(),
    )
    .expect("canonical Eq history");
    super::halo2_primitives::test_support::encode_history(&history).to_vec()
}

fn canonical_ep_history() -> Vec<u8> {
    let history = super::halo2_primitives::test_support::history_from_ep_parts(
        std::array::from_fn(|index| Fq::from((index + 1) as u64)),
        EpAffine::generator(),
    )
    .expect("canonical Ep history");
    super::halo2_primitives::test_support::encode_history(&history).to_vec()
}

#[test]
fn profile_pins_k_finite_roles_caps_and_distinct_protocols() {
    assert_eq!(OFFLINE_CASH_HALO2_K_V1, 16);
    assert_eq!(
        hex::encode(offline_cash_halo2_profile_digest_v1()),
        "2efa29961b13b00a106dcb9ffbcf491f0fefd9013063659a8b5a92eb1cc823af"
    );
    assert_eq!(OfflineCashHalo2ParityV1::ALL.len(), 2);
    assert_eq!(OfflineCashHalo2CircuitRoleV1::ALL.len(), 5);

    let expected = [
        "67bad7b6145ef44e1bcb06ab01a1e0c5efd688292b9f652e6b8f4f86971a6c7d",
        "a0ac04d998f0e0f258d49c246c91e0642716219f3194cdb02bcaf39a7e58056f",
        "bea057eb94775db5cccfa58a422cd837d823aa98756956136eafecc6a92b9b67",
        "dc2aae8a1bac738c7c165c6610799015c2198394bbe1d83f7eb0424ecea1aba8",
        "919ecb15bb10adb53676c222bf95b099c6d16a236557a7987e3708625042936f",
        "4956489041a0d65a3cf8e4c172817e77ee04ab975916915e1228c57b59c68f7a",
        "e07865b759cf9b3dc5b367144a73ec7e64fd404db8885d5f6a8f1cea2f163bf9",
        "956d9c0a4284dcf2ea263342b4f0199802a1e7de0fa257c22eda0abdff720799",
        "f6f0cd2e1bc1d7b493a0b2568220d1536448977dad24ea834dc6b8b232aef667",
        "993041665fa8e420ca8509b70cd8e460f65018fda09cd348bd7ad6ef756c6f7a",
    ];
    let mut identities = BTreeSet::new();
    let mut identity_index = 0;
    for parity in OfflineCashHalo2ParityV1::ALL {
        for role in OfflineCashHalo2CircuitRoleV1::ALL {
            let identity = offline_cash_halo2_protocol_identity_v1(parity, role);
            assert_eq!(identity.parity(), parity);
            assert_eq!(identity.circuit_role(), role);
            assert_ne!(identity.digest(), [0; 32]);
            assert!(identities.insert(identity.digest()));
            assert_eq!(hex::encode(identity.digest()), expected[identity_index]);
            identity_index += 1;
        }
    }
    assert_eq!(identities.len(), 10);
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_ABI_WORDS_V1, 229);
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V1, 7);
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_INSTANCE_COLUMNS_V1, 1);
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_INSTANCE_CELLS_V1, 33);
    assert_eq!(
        super::protocol::OFFLINE_CASH_STATE_INSTANCE_CELLS_MAX_V1,
        50
    );
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_SHA_LANES_V1, 5);
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_SHA_JOBS_V1, 10);
    assert_eq!(
        super::protocol::OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1,
        [6, 6, 5, 6, 6, 2, 2, 5, 6, 7]
    );
    assert_eq!(super::protocol::OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1, 51);
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
    let backend = OfflineCashHalo2VerifierBackendV1::from_artifact_source(source)
        .expect("hash-authenticated verifier artifacts");
    let manifest = backend.artifact_manifest();
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
            OfflineCashHalo2CircuitRoleV1::GuardBundle,
            OfflineCashArtifactRoleV1::GuardBundleVkEq,
        ),
        (
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::GuardBundle,
            OfflineCashArtifactRoleV1::GuardBundleVkEp,
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
        .find(|(role, _)| *role == OfflineCashArtifactRoleV1::GuardBundleVkEp)
        .expect("Ep GuardBundle VK");
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
            OfflineCashHalo2CircuitRoleV1::GuardBundle,
            corrupt_artifacts
                .manifest()
                .artifact(OfflineCashArtifactRoleV1::GuardBundleVkEp),
            corrupt_artifacts.manifest().protocol_digest(
                OfflineCashHalo2ParityV1::Ep,
                OfflineCashHalo2CircuitRoleV1::GuardBundle,
            ),
        ),
        Err(OfflineCashHalo2ArtifactErrorV1::DigestMismatch)
    );
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
fn wrong_role_protocol_and_all_proofs_remain_rejected() {
    let source = source_with_behavior(SourceBehavior::Normal);
    let backend = OfflineCashHalo2VerifierBackendV1::from_artifact_source(source)
        .expect("hash-authenticated verifier artifacts");
    let manifest = backend.artifact_manifest();
    let eq_history = canonical_eq_history();
    let ep_history = canonical_ep_history();
    let proof = [0x32; 128];
    let semantic = [0x33; 32];
    let eq_vk = manifest.artifact(OfflineCashArtifactRoleV1::StateVkEq);
    let eq_protocol = manifest.state_protocol_digest(OfflineCashHalo2ParityV1::Eq);
    let ep_vk = manifest.artifact(OfflineCashArtifactRoleV1::StateVkEp);
    let ep_protocol = manifest.state_protocol_digest(OfflineCashHalo2ParityV1::Ep);

    let unavailable = backend
        .verify_eq_current(eq_vk, eq_protocol, semantic, &proof, &eq_history)
        .expect_err("skeleton must never accept a proof");
    assert!(unavailable.contains("verification is unavailable"));
    assert!(
        backend
            .verify_ep_current(ep_vk, ep_protocol, semantic, &proof, &ep_history)
            .expect_err("Ep skeleton must never accept a proof")
            .contains("verification is unavailable")
    );
    assert!(
        backend
            .decide_eq_history(eq_vk, eq_protocol, &eq_history)
            .expect_err("Eq history skeleton must never decide")
            .contains("verification is unavailable")
    );
    assert!(
        backend
            .decide_ep_history(ep_vk, ep_protocol, &ep_history)
            .expect_err("Ep history skeleton must never decide")
            .contains("verification is unavailable")
    );

    assert!(
        backend
            .verify_eq_current(ep_vk, eq_protocol, semantic, &proof, &eq_history)
            .expect_err("role substitution")
            .contains("manifest")
    );
    let mut wrong_protocol = eq_protocol;
    wrong_protocol[0] ^= 1;
    assert!(
        backend
            .verify_eq_current(eq_vk, wrong_protocol, semantic, &proof, &eq_history)
            .expect_err("protocol substitution")
            .contains("protocol")
    );
    assert!(
        backend
            .verify_eq_current(eq_vk, eq_protocol, [0; 32], &proof, &eq_history)
            .expect_err("zero semantic identity")
            .contains("proof shape")
    );
}

#[test]
fn terminal_boundary_cannot_mint_receipt_from_backend_skeleton() {
    let source = source_with_behavior(SourceBehavior::Normal);
    let backend = OfflineCashHalo2VerifierBackendV1::from_artifact_source(source.clone())
        .expect("hash-authenticated verifier artifacts");
    let request = super::terminal_tests::request(source.authenticated_release());
    let payment = super::terminal_tests::payment(source.authenticated_release(), &request);
    let verifier = OfflineCashTerminalVerifierV1::new(source.authenticated_release(), &backend);
    assert!(matches!(
        verifier.verify_payment(&request, &payment, 2_000),
        Err(OfflineCashVerificationErrorV1::Cryptographic {
            stage: OfflineCashVerificationStageV1::EqCurrent,
            ..
        })
    ));
}

#[test]
fn staged_warning_allowance_is_confined_and_proof_authority_stays_core_private() {
    const STAGING_REASON: &str = "staged offline-cash boundary remains disconnected until exact STATE circuits and activation wiring land";

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
            assert!(
                !(externally_public && acceptance_authority),
                "offline-cash proof-acceptance authority must remain Core-private: {line}"
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
    assert!(backend.contains("FULL_STATE_TYPED_PUBLIC_INSTANCES_REQUIRED_BEFORE_ACTIVATION"));
    assert!(!backend.contains("OfflineCashStatePublicInstancesV1"));
    assert!(!compact_backend.contains("pubstructOfflineCashHalo2VerifierBackendV1"));
    assert!(!compact_root.contains("pubusehalo2_backend::OfflineCashHalo2VerifierBackendV1"));
}
