//! Pre-burn route, exact horizon, controller and original-set regression checks.

use super::*;
use iroha::data_model::sccp::{
    deployment::{SccpCommitteePinV1, SccpDeploymentV1, SccpEvmDeploymentV1},
    light_client::{SccpLcHeadV1, SccpLcPointV1, SccpLightClientParamsV1},
};
use iroha_crypto::Algorithm;

fn light_client(network: SccpNetworkV1) -> SccpLightClientV1 {
    SccpLightClientV1 {
        params: SccpLightClientParamsV1::defaults_for(network).unwrap(),
        head: SccpLcHeadV1 {
            latest_set_id: 1,
            latest_finalized: SccpLcPointV1 {
                source_height: 2,
                block_hash: [3; 32],
                source_time_ms: 1000,
            },
            last_progress_taira_ms: 1000,
        },
        frozen: None,
        state_hash: [4; 32],
    }
}

#[test]
fn burn_route_accepts_inbound_drain_but_never_hold_or_quarantine() {
    let mut revision = SccpRouteRevisionV1::staged(
        1,
        SccpDeploymentV1::Evm(SccpEvmDeploymentV1 {
            address: [1; 20],
            runtime_code_hash: [2; 32],
        }),
        SccpCommitteePinV1 {
            generation: 1,
            committee_root: [3; 32],
            start_height: 1,
            until_ms: 1000,
        },
        100,
        1,
    );
    assert!(require_route(&revision, None).is_err());
    revision.activation = SccpRouteActivationV1::Bidirectional;
    assert!(require_route(&revision, None).is_ok());
    revision.activation = SccpRouteActivationV1::InboundOnly;
    assert!(require_route(&revision, None).is_ok());
    assert!(require_route(&revision, Some(revision.forgery_floor)).is_err());
    revision.quarantine = Some(iroha::data_model::sccp::native::SccpQuarantineV1 {
        certificate_id: [7; 32],
        since_height: 1,
    });
    assert!(require_route(&revision, None).is_err());
    revision.quarantine = None;
    revision.activation = SccpRouteActivationV1::Paused;
    assert!(require_route(&revision, None).is_err());
}

#[test]
fn burn_horizons_round_the_quarter_up_and_refuse_the_exact_profile_margin() {
    let client = light_client(SccpNetworkV1::BscMainnet);
    let supported = Some(100 + PROFILE_MARGIN_MS + 1);
    assert!(require_horizon(&client, 100, 103, 9, supported).is_ok());
    assert!(require_horizon(&client, 100, 102, 9, supported).is_err());
    assert!(require_horizon(&client, 104, 103, 9, supported).is_err());
    assert!(require_horizon(&client, 100, 103, 9, Some(100 + PROFILE_MARGIN_MS)).is_err());
    assert!(require_horizon(&client, 100, 103, 9, None).is_err());
    assert!(require_horizon(&client, 100, 103, 0, supported).is_err());
    assert!(require_horizon(&client, u64::MAX, u64::MAX, 9, Some(u64::MAX)).is_err());
    let mut frozen = client;
    frozen.frozen = Some(
        iroha::data_model::sccp::light_client::SccpLcFreezeReasonV1::Parliament(
            iroha::data_model::sccp::light_client::SccpLcParliamentFreezeV1 {
                proposal_id: [1; 32],
            },
        ),
    );
    assert!(require_horizon(&frozen, 100, 103, 9, supported).is_err());
}

fn policy(algorithms: Vec<Algorithm>) -> CryptoAdmissionPolicyV1 {
    CryptoAdmissionPolicyV1::new(algorithms).unwrap()
}

#[test]
fn burn_controller_requires_an_admitted_algorithm() {
    let key = iroha_crypto::KeyPair::from_seed(vec![19; 32], Algorithm::Ed25519);
    let account = AccountId::new(key.public_key().clone());
    let bytes = AccountAddress::from_account_id(&account)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let good = policy(vec![Algorithm::Ed25519]);
    assert_eq!(require_controller(&bytes, &good).unwrap(), account);
    assert!(require_controller(&bytes, &policy(vec![Algorithm::Secp256k1])).is_err());
    assert!(require_controller(&[0; 1025], &good).is_err());
    let mut malformed = bytes.clone();
    malformed.push(0);
    assert!(require_controller(&malformed, &good).is_err());
}

#[test]
fn burn_multisig_checks_every_member_even_below_the_signing_threshold() {
    use iroha::data_model::account::controller::{MultisigMember, MultisigPolicy};

    let ed = iroha_crypto::KeyPair::from_seed(vec![21; 32], Algorithm::Ed25519);
    let secp = iroha_crypto::KeyPair::from_seed(vec![22; 32], Algorithm::Secp256k1);
    let account = AccountId::new_multisig(
        MultisigPolicy::new(
            1,
            vec![
                MultisigMember::new(ed.public_key().clone(), 1).unwrap(),
                MultisigMember::new(secp.public_key().clone(), 1).unwrap(),
            ],
        )
        .unwrap(),
    );
    let bytes = AccountAddress::from_account_id(&account)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let permitted = policy(vec![Algorithm::Ed25519, Algorithm::Secp256k1]);
    assert_eq!(require_controller(&bytes, &permitted).unwrap(), account);
    assert!(
        require_controller(&bytes, &policy(vec![Algorithm::Ed25519])).is_err(),
        "one permitted signer cannot hide an inadmissible controller member"
    );
}

#[test]
fn burn_bls_requires_committed_membership_without_exempting_other_members() {
    use iroha::data_model::account::controller::{MultisigMember, MultisigPolicy};

    let ed_permitted = policy(vec![Algorithm::Ed25519]);
    let ed = iroha_crypto::KeyPair::from_seed(vec![24; 32], Algorithm::Ed25519);
    for algorithm in [Algorithm::BlsNormal, Algorithm::BlsSmall] {
        let bls = iroha_crypto::KeyPair::from_seed(vec![23; 32], algorithm);
        let single = AccountId::new(bls.public_key().clone());
        let bytes = AccountAddress::from_account_id(&single)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert!(require_controller(&bytes, &ed_permitted).is_err());
        let bls_permitted = policy(vec![algorithm]);
        assert_eq!(require_controller(&bytes, &bls_permitted).unwrap(), single);
        let mixed = AccountId::new_multisig(
            MultisigPolicy::new(
                1,
                vec![
                    MultisigMember::new(bls.public_key().clone(), 1).unwrap(),
                    MultisigMember::new(ed.public_key().clone(), 1).unwrap(),
                ],
            )
            .unwrap(),
        );
        let bytes = AccountAddress::from_account_id(&mixed)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert!(require_controller(&bytes, &bls_permitted).is_err());
        assert!(require_controller(&bytes, &ed_permitted).is_err());
        let both = policy(vec![Algorithm::Ed25519, algorithm]);
        assert_eq!(require_controller(&bytes, &both).unwrap(), mixed);
    }
}

/// A certified cut whose complete `world.parameters` cell is `parameters`.
fn parameters_cut(parameters: &Parameters) -> origin::AuthenticatedOrigin {
    use iroha::data_model::{
        sumeragi_finality::{
            WorldStateElementKindV1, WorldStateSnapshotEntryV1, WorldStateSnapshotV1,
            world_state_value_hash_v1,
        },
        testing::native_finality::NativeFinalityFixture,
    };
    let snapshot = WorldStateSnapshotV1 {
        schema_hash: iroha_crypto::Hash::new(b"burn admission parameters"),
        entries: vec![WorldStateSnapshotEntryV1 {
            field_id: "world.parameters".into(),
            kind: WorldStateElementKindV1::Cell,
            key_hash: None,
            value_hash: world_state_value_hash_v1(parameters).unwrap(),
        }],
    };
    let mut native = NativeFinalityFixture::start("sccp-burn-admission");
    let block = native.block_with_submitted_work(native.next_header());
    let proof = native.certify_with_world_root(block, snapshot.root().unwrap());
    origin::test_cut(&native, &snapshot, &proof)
}

fn parameters_with(policy: Option<&CryptoAdmissionPolicyV1>) -> Parameters {
    use iroha::data_model::parameter::Parameter;
    let mut parameters = Parameters::default();
    if let Some(policy) = policy {
        let Parameter::Custom(custom) = policy.clone().into_parameter().unwrap() else {
            unreachable!("the policy is a custom parameter")
        };
        parameters.custom.insert(custom);
    }
    parameters
}

#[test]
fn burn_admission_policy_is_the_cut_s_own_parameter_never_an_endpoint_report() {
    let committed = policy(vec![Algorithm::Ed25519]);
    let parameters = parameters_with(Some(&committed));
    let cut = parameters_cut(&parameters);
    assert_eq!(cut.crypto_admission_policy(&parameters).unwrap(), committed);
    // An endpoint widening the policy cannot match the certified cell.
    let wider = policy(vec![Algorithm::Ed25519, Algorithm::Secp256k1]);
    assert!(
        cut.crypto_admission_policy(&parameters_with(Some(&wider)))
            .is_err()
    );
    // Nor can it omit the policy, or change any other parameter served beside it.
    assert!(cut.crypto_admission_policy(&parameters_with(None)).is_err());
    let mut changed = parameters.clone();
    changed
        .custom
        .insert(iroha::data_model::parameter::CustomParameter::new(
            "sccp_burn_fixture".parse().unwrap(),
            iroha_primitives::json::Json::new(1_u8),
        ));
    assert!(cut.crypto_admission_policy(&changed).is_err());
    // A cut whose own parameters lack the policy admits no recipient.
    let bare = parameters_with(None);
    assert!(
        parameters_cut(&bare)
            .crypto_admission_policy(&bare)
            .is_err()
    );
}

#[test]
fn ton_bound_uses_original_stake_retention_minus_the_active_profile_margin() {
    let profiles = SccpChainProfilesV1::genesis();
    let mut epoch = iroha_sccp::light_client::ton::TonEpochV1 {
        key_block_seqno: 1,
        key_block_root_hash: [1; 32],
        validators: iroha_sccp::TonValidatorConfigV1 {
            valid_since: 1,
            valid_until: 2,
            main_validator_count: 0,
            shuffle_masterchain_validators: false,
            validators: vec![],
        },
        stake_held_for: 100_000,
    };
    let set = |epoch| SccpLcConsensusSetV1 {
        set_id: 1,
        valid_from_source_height: 1,
        superseded_at_source_ms: None,
        set_bytes: SccpLcSetDataV1::Ton(epoch).to_frame().unwrap(),
    };
    let client = light_client(SccpNetworkV1::TonMainnet);
    assert_eq!(
        weak_subjectivity_bound(profiles, &client, &set(epoch.clone())).unwrap(),
        100_000_000 - profiles.ton.freshness_margin_ms
    );
    epoch.stake_held_for = 0;
    assert!(weak_subjectivity_bound(profiles, &client, &set(epoch.clone())).is_err());
    epoch.stake_held_for = 100_000;
    epoch.key_block_seqno = 2;
    assert!(weak_subjectivity_bound(profiles, &client, &set(epoch)).is_err());
}
