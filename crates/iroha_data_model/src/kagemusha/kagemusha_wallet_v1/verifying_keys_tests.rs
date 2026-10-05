//! Verifying-key allowlist tests (owner answers Q6 and Q11).

use iroha_crypto::Hash;

use super::*;
use crate::kagemusha::kagemusha_wallet_v1::{
    KAGEMUSHA_WALLET_PAYMENT_FIXED_BYTES_V1, KAGEMUSHA_WALLET_VERSION_V1,
    KagemushaWalletValidationErrorV1,
    codec_tests::norito_tag,
    identity::{
        KAGEMUSHA_WALLET_CONTROL_BLACKLIST_V1, KAGEMUSHA_WALLET_CONTROL_QUOTAS_V1,
        KAGEMUSHA_WALLET_CONTROLS_DEFINED_MASK_V1, KagemushaWalletEvidenceKindV1,
        identity_tests::identity_fixture, kagemusha_wallet_provider_contract_v1,
        kagemusha_wallet_relation_id_v1,
    },
    state::{
        KagemushaWalletEffectV1, KagemushaWalletLifecycleV1, KagemushaWalletLineageSlotV1,
        state_tests::{
            LINEAGE_PROOF_LEN, bootstrap_statement, send_effect, signed_package, stand_in_proof,
            transition_statement,
        },
    },
};

/// σ length of every stand-in entry.
const SIGMA: u32 = 48;

#[track_caller]
fn assert_invalid<T: core::fmt::Debug>(result: WalletResult<T>, expected: &str) {
    match result {
        Err(KagemushaWalletValidationErrorV1::InvalidField { field }) if field == expected => {}
        other => panic!("expected invalid `{expected}`, got {other:?}"),
    }
}

fn entry(
    kind: KagemushaWalletOperationKindV1,
    enabled_controls: u32,
    proof_bytes: u32,
) -> KagemushaWalletVerifyingKeyEntryV1 {
    KagemushaWalletVerifyingKeyEntryV1 {
        kind,
        enabled_controls,
        verifying_key_digest: [kind.tag() ^ u8::try_from(enabled_controls).expect("mask"); 32]
            .map(|byte| byte | 0x80),
        proof_bytes,
    }
}

/// One entry per operation, plus Send with the blacklist and quota masks.
pub(in crate::kagemusha::kagemusha_wallet_v1) fn sample_allowlist()
-> KagemushaWalletVerifyingKeyAllowlistV1 {
    let mut steps = Vec::new();
    for kind in KagemushaWalletOperationKindV1::ALL {
        steps.push(entry(kind, 0, SIGMA));
        if kind == KagemushaWalletOperationKindV1::Send {
            steps.push(entry(
                kind,
                KAGEMUSHA_WALLET_CONTROL_BLACKLIST_V1,
                SIGMA + 1,
            ));
            steps.push(entry(
                kind,
                KAGEMUSHA_WALLET_CONTROL_BLACKLIST_V1 | KAGEMUSHA_WALLET_CONTROL_QUOTAS_V1,
                SIGMA + 2,
            ));
        }
    }
    KagemushaWalletVerifyingKeyAllowlistV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        steps,
        lineage_verifying_key_digest: [0x4c; 32],
        lineage_proof_bytes: u32::try_from(LINEAGE_PROOF_LEN).expect("len"),
    }
}

#[test]
fn kagemusha_wallet_v1_verifying_key_allowlist_layout_and_digest() {
    let allowlist = sample_allowlist();
    allowlist.validate().expect("allowlist");
    assert_eq!(KAGEMUSHA_WALLET_VERIFYING_KEY_ENTRY_TRANSCRIPT_BYTES_V1, 41);
    assert_eq!(KAGEMUSHA_WALLET_VERIFYING_KEY_ENTRIES_MAX_V1, 15);
    let transcript = allowlist.transcript().expect("transcript");
    let mut expected = 1_u16.to_le_bytes().to_vec();
    expected.extend_from_slice(&10_u32.to_le_bytes());
    for step in &allowlist.steps {
        expected.push(step.kind.tag());
        expected.extend_from_slice(&step.enabled_controls.to_le_bytes());
        expected.extend_from_slice(&step.verifying_key_digest);
        expected.extend_from_slice(&step.proof_bytes.to_le_bytes());
    }
    expected.extend_from_slice(&[0x4c; 32]);
    expected.extend_from_slice(&40_u32.to_le_bytes());
    assert_eq!(transcript, expected);
    assert_eq!(transcript.len(), 2 + 4 + 10 * 41 + 32 + 4);
    assert_eq!(
        allowlist.verifying_key_set_digest().ok(),
        Some(kagemusha_wallet_digest_v1(Role::VerifyingKeySet, &expected))
    );
    // Selectors ascend by (tag, mask): Send's masks follow its empty mask.
    let selectors: Vec<(u8, u32)> = allowlist
        .steps
        .iter()
        .map(KagemushaWalletVerifyingKeyEntryV1::selector)
        .collect();
    assert_eq!(selectors[2..5], [(3, 0), (3, 1), (3, 3)]);
    assert_eq!(norito_tag(&allowlist.steps[2].kind), 3);

    // Canonical frame round trip and the bound.
    let frame = allowlist.to_canonical_bytes().expect("frame");
    assert!(frame.len() <= KAGEMUSHA_WALLET_VERIFYING_KEY_ALLOWLIST_MAX_BYTES_V1);
    let decoded: KagemushaWalletVerifyingKeyAllowlistV1 = decode_frame_v1(
        &frame,
        KAGEMUSHA_WALLET_VERIFYING_KEY_ALLOWLIST_MAX_BYTES_V1,
    )
    .expect("decode");
    assert_eq!(decoded, allowlist);
}

#[test]
fn kagemusha_wallet_v1_verifying_key_allowlist_rules() {
    use KagemushaWalletOperationKindV1 as Kind;
    let reject = |mutate: &dyn Fn(&mut KagemushaWalletVerifyingKeyAllowlistV1), field: &str| {
        let mut allowlist = sample_allowlist();
        mutate(&mut allowlist);
        assert_invalid(allowlist.validate(), field);
        assert_invalid(allowlist.verifying_key_set_digest(), field);
    };
    reject(&|a| a.steps.swap(0, 1), "verifying_keys.order");
    reject(
        &|a| {
            let duplicate = a.steps[2];
            a.steps.insert(2, duplicate);
        },
        "verifying_keys.order",
    );
    reject(
        &|a| a.steps.retain(|e| e.kind != Kind::Retiring),
        "verifying_keys.missing",
    );
    reject(
        &|a| a.steps.retain(|e| e.selector() != (3, 0)),
        "verifying_keys.missing",
    );
    reject(
        &|a| a.steps[0].enabled_controls = 1,
        "verifying_keys.enabled_controls",
    );
    reject(
        &|a| a.steps[4].enabled_controls = KAGEMUSHA_WALLET_CONTROLS_DEFINED_MASK_V1 + 1,
        "verifying_keys.enabled_controls",
    );
    reject(
        &|a| a.steps[1].verifying_key_digest = [0; 32],
        "verifying_keys.verifying_key_digest",
    );
    reject(
        &|a| a.steps[1].proof_bytes = 0,
        "verifying_keys.proof_bytes",
    );
    reject(
        &|a| a.steps[1].proof_bytes = 10_001,
        "verifying_keys.proof_bytes",
    );
    reject(
        &|a| a.lineage_verifying_key_digest = [0; 32],
        "verifying_keys.lineage_verifying_key_digest",
    );
    reject(
        &|a| a.lineage_proof_bytes = 0,
        "verifying_keys.lineage_proof_bytes",
    );
    reject(
        &|a| {
            let more = a.steps[3];
            a.steps.extend(core::iter::repeat_n(more, 6));
        },
        "verifying_keys.steps",
    );
    let mut version = sample_allowlist();
    version.version = 2;
    assert!(matches!(
        version.validate(),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion { .. })
    ));

    // R9 (owner answer Q6): |Ω| + the largest σ_send fits 10,000 − F_payment.
    assert_eq!(
        KAGEMUSHA_WALLET_PAYMENT_PROOF_BUDGET_V1,
        10_000 - KAGEMUSHA_WALLET_PAYMENT_FIXED_BYTES_V1
    );
    let budget = u32::try_from(KAGEMUSHA_WALLET_PAYMENT_PROOF_BUDGET_V1).expect("budget");
    let mut at_budget = sample_allowlist();
    at_budget.steps[4].proof_bytes = 3_296;
    at_budget.lineage_proof_bytes = budget - 3_296;
    at_budget
        .validate()
        .expect("joint proofs exactly at the budget");
    let mut over = at_budget.clone();
    over.lineage_proof_bytes += 1;
    assert_invalid(over.validate(), "verifying_keys.budget");
    // The budget binds Send only; another relation's σ may be longer.
    let mut receive = at_budget;
    assert_eq!(receive.steps[5].kind, Kind::Receive);
    receive.steps[5].proof_bytes = 9_000;
    receive
        .validate()
        .expect("σ_recv outside the Payment budget");
}

#[test]
fn kagemusha_wallet_v1_verifying_key_selection_and_lengths() {
    use KagemushaWalletOperationKindV1 as Kind;
    let allowlist = sample_allowlist();
    let send = allowlist.entry(Kind::Send, 1).expect("send blacklist");
    assert_eq!(send.proof_bytes, SIGMA + 1);
    assert_eq!(
        allowlist.entry(Kind::Load, 0).expect("load").proof_bytes,
        SIGMA
    );
    assert_invalid(allowlist.entry(Kind::Load, 1), "verifying_keys.selector");
    assert_invalid(allowlist.entry(Kind::Send, 2), "verifying_keys.selector");

    let sigma = stand_in_proof(usize::try_from(SIGMA).expect("len"));
    assert_eq!(
        allowlist.check_step_proof(Kind::Receive, 0, &sigma).ok(),
        Some(entry(Kind::Receive, 0, SIGMA).verifying_key_digest)
    );
    assert_invalid(
        allowlist.check_step_proof(Kind::Send, 1, &sigma),
        "step_proof.length",
    );
    assert_invalid(
        allowlist.check_step_proof(Kind::Receive, 0, &stand_in_proof(47)),
        "step_proof.length",
    );

    // A Send package selects by its mask and checks Ω(pred)'s exact transport length.
    let f = identity_fixture(KagemushaWalletEvidenceKindV1::AndroidKeyMintTee, 0x4b);
    let statement =
        transition_statement(&f, 3, 0, KagemushaWalletLifecycleV1::Active, send_effect());
    let package = signed_package(&f, &f.credential, &statement, sigma.clone());
    assert!(matches!(
        package.lineage,
        KagemushaWalletLineageSlotV1::Present { .. }
    ));
    assert_eq!(
        allowlist.check_package(&package).ok(),
        Some(entry(Kind::Send, 0, SIGMA).verifying_key_digest)
    );
    let mut short_omega = allowlist.clone();
    short_omega.lineage_proof_bytes -= 1;
    assert_invalid(short_omega.check_package(&package), "lineage.proof_length");
    let bootstrap = signed_package(&f, &f.credential, &bootstrap_statement(&f), sigma);
    assert!(matches!(
        bootstrap.statement.effect,
        KagemushaWalletEffectV1::Bootstrap { .. }
    ));
    assert_eq!(
        allowlist.check_package(&bootstrap).ok(),
        Some(entry(Kind::Bootstrap, 0, SIGMA).verifying_key_digest)
    );
}

#[test]
fn kagemusha_wallet_v1_verifying_key_allowlist_binds_the_manifest() {
    let allowlist = sample_allowlist();
    let digest = allowlist.verifying_key_set_digest().expect("digest");
    let (eq, ep, native, inventory) = ([0x21; 32], [0x22; 32], [0x23; 32], [0x25; 32]);
    let manifest = KagemushaWalletArtifactManifestBodyV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        network_id: *Hash::prehashed([11; 32]).as_ref(),
        relation_id: kagemusha_wallet_relation_id_v1(&eq, &ep, &native, &digest, &inventory),
        eq_protocol_digest: eq,
        ep_protocol_digest: ep,
        native_profile_digest: native,
        verifying_key_set_digest: digest,
        artifact_inventory_digest: inventory,
        provider_contract: kagemusha_wallet_provider_contract_v1(),
        signer_certificate: [0x31; 32],
    };
    allowlist.require_manifest(&manifest).expect("manifest");
    let frame = allowlist.to_canonical_bytes().expect("frame");
    assert_eq!(
        KagemushaWalletVerifyingKeyAllowlistV1::decode_canonical(&frame, &manifest)
            .expect("decode"),
        allowlist
    );
    let mut other = allowlist.clone();
    other.steps[0].proof_bytes += 1;
    assert_invalid(
        other.require_manifest(&manifest),
        "artifact_manifest.verifying_key_set_digest",
    );
    let other_frame = other.to_canonical_bytes().expect("frame");
    assert_invalid(
        KagemushaWalletVerifyingKeyAllowlistV1::decode_canonical(&other_frame, &manifest),
        "artifact_manifest.verifying_key_set_digest",
    );
    let mut oversized = frame.clone();
    oversized.resize(KAGEMUSHA_WALLET_VERIFYING_KEY_ALLOWLIST_MAX_BYTES_V1 + 1, 0);
    assert!(matches!(
        KagemushaWalletVerifyingKeyAllowlistV1::decode_canonical(&oversized, &manifest),
        Err(KagemushaWalletValidationErrorV1::EncodedSizeExceeded { .. })
    ));
}
