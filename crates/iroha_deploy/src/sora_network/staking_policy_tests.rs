//! Localnet bootstrap consumes the original policy without monetary defaults or overlays.

use super::*;
use iroha_data_model::block::consensus::PublicLaneStakingPolicyError;

#[test]
fn localnet_selects_the_complete_compiled_recipe_before_custody_authoring() {
    use iroha_config::profile::{Profile, ProfileId};
    let key = KeyPair::from_seed(vec![0x58; 32], iroha_crypto::Algorithm::Ed25519);
    for selected in [
        None,
        Some(SoraProfile::Dataspace),
        Some(SoraProfile::PrivateSbp),
        Some(SoraProfile::PrivateCbuae),
        Some(SoraProfile::PrivateBpng),
        Some(SoraProfile::Nexus),
    ] {
        for sccp in [false, true] {
            let id = if selected.is_some() || sccp {
                ProfileId::SoraNexusV1
            } else {
                ProfileId::IrohaDevV1
            };
            let mut expected = Profile::compiled(id)
                .unwrap()
                .genesis_recipe()
                .public_lane_staking
                .clone();
            expected.stake_escrow_account_id = localnet_gas_account_id(key.public_key());
            expected.slash_sink_account_id = expected.stake_escrow_account_id.clone();
            if sccp {
                expected.unbonding_delay_ms = crate::genesis::sccp::UNBONDING_DELAY_MS;
                expected.max_slash_bps = crate::genesis::sccp::MAX_SLASH_BPS;
            }
            assert_eq!(
                localnet_staking_policy(key.public_key(), selected, sccp).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn nondefault_recipe_terms_reach_original_genesis_without_reconstruction() {
    let _author = ChainDiscriminantGuard::enter(369);
    let key = KeyPair::from_seed(vec![0x59; 32], iroha_crypto::Algorithm::Ed25519);
    let (mut recipe, _) = policy_and_manifest(SumeragiConsensusMode::Npos);
    recipe.min_validator_stake = 37_u64.into();
    recipe.max_stake_shares_per_validator = 13_u32.try_into().unwrap();
    recipe.max_pending_unbonds_per_share = 9_u32.try_into().unwrap();
    recipe.unbonding_delay_ms = 987_654;
    recipe.max_slash_bps = 997;
    for sccp in [false, true] {
        let mut expected = recipe.clone();
        expected.stake_escrow_account_id = localnet_gas_account_id(key.public_key());
        expected.slash_sink_account_id = expected.stake_escrow_account_id.clone();
        if sccp {
            expected.unbonding_delay_ms = crate::genesis::sccp::UNBONDING_DELAY_MS;
            expected.max_slash_bps = crate::genesis::sccp::MAX_SLASH_BPS;
        }
        let authored =
            localnet_staking_policy_from_recipe(recipe.clone(), key.public_key(), sccp).unwrap();
        assert_eq!(authored, expected);
        let raw = generate_raw_genesis(
            key.public_key(),
            SumeragiConsensusMode::Npos,
            DEFAULT_CHAIN_ID,
            authored,
        )
        .unwrap()
        .with_chain_discriminant(369);
        assert_eq!(localnet_bootstrap_staking_policy(&raw).unwrap(), expected);
    }
    let mut invalid = recipe.clone();
    invalid.max_slash_bps = 10_001;
    assert!(
        matches!(
            localnet_staking_policy_from_recipe(invalid, key.public_key(), true)
                .unwrap_err()
                .downcast_ref::<PublicLaneStakingPolicyError>(),
            Some(PublicLaneStakingPolicyError::InvalidSlashFraction)
        ),
        "SCCP cannot repair an invalid input recipe"
    );
    recipe.stake_asset_id =
        AssetDefinitionId::parse_address_literal("61CtjvNd9T3THAR65GsMVHr82Bjc").unwrap();
    assert!(
        localnet_staking_policy_from_recipe(recipe, key.public_key(), false)
            .unwrap_err()
            .to_string()
            .contains("selected localnet recipe")
    );
}

fn policy_and_manifest(
    mode: SumeragiConsensusMode,
) -> (PublicLaneStakingPolicy, RawGenesisTransaction) {
    let _author = ChainDiscriminantGuard::enter(369);
    let key = KeyPair::from_seed(vec![0x52; 32], iroha_crypto::Algorithm::Ed25519);
    let mut policy = localnet_staking_policy(key.public_key(), None, false).unwrap();
    policy.min_validator_stake = 19_u64.into();
    policy.max_stake_shares_per_validator = 7_u32.try_into().unwrap();
    policy.max_pending_unbonds_per_share = 3_u32.try_into().unwrap();
    policy.unbonding_delay_ms = 43_210;
    policy.max_slash_bps = 1_234;
    policy.stake_escrow_account_id = AccountId::new(
        KeyPair::from_seed(vec![0x53; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    policy.slash_sink_account_id = AccountId::new(
        KeyPair::from_seed(vec![0x54; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let raw = generate_raw_genesis(
        key.public_key(),
        mode,
        DEFAULT_CHAIN_ID,
        localnet_staking_policy(key.public_key(), None, false).unwrap(),
    )
    .unwrap()
    .with_chain_discriminant(369);
    let mut value = norito::json::to_value(&raw).unwrap();
    let payload = original_policy_payload(&mut value);
    *payload = norito::json::to_value(&policy).unwrap();
    (policy, norito::json::from_value(value).unwrap())
}

fn original_policy_payload(value: &mut norito::json::Value) -> &mut norito::json::Value {
    let transactions = value
        .get_mut("transactions")
        .unwrap()
        .as_array_mut()
        .unwrap();
    let mut originals = transactions.iter_mut().filter_map(|transaction| {
        transaction
            .get_mut("parameters")?
            .get_mut("custom")?
            .get_mut(PublicLaneStakingPolicy::PARAMETER_ID_STR)?
            .get_mut("payload")
    });
    let original = originals.next().expect("one original policy");
    assert!(originals.next().is_none(), "no replacement policy snapshot");
    original
}

#[test]
fn localnet_original_policy_preserves_all_terms_and_restores_caller_context() {
    let _caller = ChainDiscriminantGuard::enter(753);
    for mode in [
        SumeragiConsensusMode::Permissioned,
        SumeragiConsensusMode::Npos,
    ] {
        let (policy, raw) = policy_and_manifest(mode);
        let original = norito::json::to_json(&raw).unwrap();
        assert_eq!(localnet_bootstrap_staking_policy(&raw).unwrap(), policy);
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            753
        );
        assert_eq!(norito::json::to_json(&raw).unwrap(), original);
    }
}

#[test]
fn localnet_bootstrap_refuses_missing_invalid_or_mismatched_original_policy() {
    let _author = ChainDiscriminantGuard::enter(369);
    for mode in [
        SumeragiConsensusMode::Permissioned,
        SumeragiConsensusMode::Npos,
    ] {
        let (_, raw) = policy_and_manifest(mode);
        for mutation in ["missing", "invalid", "different_asset"] {
            let mut value = norito::json::to_value(&raw).unwrap();
            if mutation == "missing" {
                let mut removed = 0;
                for transaction in value
                    .get_mut("transactions")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                {
                    if let Some(custom) = transaction
                        .get_mut("parameters")
                        .and_then(|p| p.get_mut("custom"))
                        .and_then(norito::json::Value::as_object_mut)
                    {
                        removed += usize::from(
                            custom
                                .remove(PublicLaneStakingPolicy::PARAMETER_ID_STR)
                                .is_some(),
                        );
                    }
                }
                assert_eq!(removed, 1);
            } else {
                let payload = original_policy_payload(&mut value).as_object_mut().unwrap();
                if mutation == "invalid" {
                    payload.insert("max_slash_bps".into(), 10_001_u32.into());
                } else {
                    payload.insert(
                        "stake_asset_id".into(),
                        "61CtjvNd9T3THAR65GsMVHr82Bjc".into(),
                    );
                }
            }
            let changed: RawGenesisTransaction = norito::json::from_value(value).unwrap();
            if mutation == "different_asset" && matches!(mode, SumeragiConsensusMode::Permissioned)
            {
                // Permissioned support has no staking transfers or NPoS currency pin.
                assert_eq!(
                    localnet_bootstrap_staking_policy(&changed)
                        .unwrap()
                        .stake_asset_id
                        .to_string(),
                    "61CtjvNd9T3THAR65GsMVHr82Bjc"
                );
                continue;
            }
            let error = localnet_bootstrap_staking_policy(&changed).unwrap_err();
            match mutation {
                "missing" => assert!(matches!(
                    error.downcast_ref::<PublicLaneStakingPolicyError>(),
                    Some(PublicLaneStakingPolicyError::MissingParameter)
                )),
                "invalid" => assert!(matches!(
                    error.downcast_ref::<PublicLaneStakingPolicyError>(),
                    Some(PublicLaneStakingPolicyError::InvalidSlashFraction)
                )),
                "different_asset" => assert!(error.to_string().contains("original NPoS XOR pin")),
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn localnet_bootstrap_uses_original_custody_without_increasing_declared_stake() {
    let _author = ChainDiscriminantGuard::enter(369);
    let (policy, raw) = policy_and_manifest(SumeragiConsensusMode::Npos);
    let peers = build_peers(4, Some(b"original-policy-bootstrap"), 31_080, 31_337).unwrap();
    let gas = AccountId::new(
        KeyPair::from_seed(vec![0x55; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let authority = AccountId::new(
        KeyPair::from_seed(vec![0x52; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let declared = Quantity::from(3_u64);
    assert!(declared < policy.min_validator_stake);
    let prepared = append_localnet_npos_bootstrap(
        raw,
        &LocalnetNposBootstrapContext {
            peers: &peers,
            gas_account_id: &gas,
            stake_amount: &declared,
            sora_profile: None,
            genesis_account_id: &authority,
            client_account_id: &authority,
            onboarding_account_id: &authority,
            taira: false,
        },
    )
    .unwrap();
    assert_eq!(
        localnet_bootstrap_staking_policy(&prepared).unwrap(),
        policy
    );
    let registrations = BootstrapRegistrations::from_manifest(&prepared);
    for account in [
        &gas,
        &policy.stake_escrow_account_id,
        &policy.slash_sink_account_id,
    ] {
        assert!(registrations.accounts.contains(account));
        assert_eq!(prepared.instructions().filter(|instruction| matches!(instruction.as_any().downcast_ref::<RegisterBox>(), Some(RegisterBox::Account(register)) if register.object().id() == account)).count(), 1);
    }
    let validators: Vec<_> = prepared
        .instructions()
        .filter_map(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
        })
        .collect();
    assert_eq!(validators.len(), 4);
    for registration in validators {
        let destination = AssetId::new(
            policy.stake_asset_id.clone(),
            registration.stake_account.clone(),
        );
        let minted = prepared
            .instructions()
            .filter_map(|instruction| {
                match instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::MintBox>()
                {
                    Some(iroha_data_model::isi::MintBox::Asset(mint))
                        if mint.destination() == &destination =>
                    {
                        Some(mint.object().clone())
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            minted,
            vec![declared.clone(), declared.clone()],
            "one fixed stake mint and one fixed fee mint; neither repairs the minimum"
        );
        assert_eq!(
            registration.initial_stake, declared,
            "minimum is validated during execution, never repaired by minting"
        );
        assert_eq!(
            registration.monetary_plan,
            PublicLaneMonetaryPlanV1::genesis_registration(
                AssetId::new(
                    policy.stake_asset_id.clone(),
                    registration.stake_account.clone()
                ),
                AssetId::new(
                    policy.stake_asset_id.clone(),
                    policy.stake_escrow_account_id.clone()
                ),
                declared.clone()
            )
        );
    }
}

#[test]
fn native_localnet_staging_refuses_original_minimum_above_unchanged_bootstrap_stakes() {
    use iroha_core::block::{BlockValidationError, InvalidGenesisError};

    let directory = crate::sora_network::localnet_test_helpers::private_tempdir()
        .expect("private generated localnet runtime");
    let seed = "original-minimum-native-rejection";
    let options = NetworkOptions {
        sora_profile: SoraProfile::Dataspace,
        peers: NonZeroU16::new(4).unwrap(),
        seed: Some(seed.to_owned()),
        bind_host: DEFAULT_BIND_HOST.to_owned(),
        public_host: DEFAULT_PUBLIC_HOST.to_owned(),
        base_api_port: 31_580,
        base_p2p_port: 31_837,
        out_dir: directory.path().to_path_buf(),
        block_cadence_ms: None,
    };
    generate_network(&options, &mut BufWriter::new(Vec::new()))
        .expect("original minimum admits the actual generated native genesis");
    let manifest = RawGenesisTransaction::from_path(directory.path().join("genesis.json"))
        .expect("generated complete original manifest");
    let signed = read_signed_genesis(&directory.path().join("genesis.signed.nrt"))
        .expect("original executed signed genesis");
    assert!(
        signed
            .output_results()
            .all(|result| result.as_ref().is_ok())
    );
    let _chain = crate::genesis::staging::staged_genesis_chain_discriminant(&manifest);
    let config_path = directory.path().join("peer0.toml");
    let config = parse_localnet_peer_config(
        &fs::read_to_string(&config_path).unwrap(),
        Some(&config_path),
    )
    .expect("exact generated peer configuration");
    let (_, private) = generate_genesis_key_pair(Some(seed.as_bytes()), GENESIS_SEED).unwrap();
    let signer = KeyPair::from_private_key(private.0).unwrap();
    let registrations: Vec<_> = manifest
        .instructions()
        .filter_map(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
        })
        .collect();
    assert_eq!(registrations.len(), 4);
    let minimum = registrations
        .iter()
        .map(|registration| &registration.initial_stake)
        .max()
        .unwrap()
        .checked_add(&Quantity::from(1_u64))
        .unwrap();
    assert!(
        registrations
            .iter()
            .all(|registration| registration.initial_stake < minimum)
    );
    let original_policy = localnet_bootstrap_staking_policy(&manifest).unwrap();
    let mut expected_policy = original_policy.clone();
    expected_policy.min_validator_stake = minimum.clone();
    let mut value = norito::json::to_value(&manifest).unwrap();
    let payload = original_policy_payload(&mut value).as_object_mut().unwrap();
    assert!(
        payload
            .insert(
                "min_validator_stake".into(),
                norito::json::to_value(&minimum).unwrap()
            )
            .is_some()
    );
    let changed: RawGenesisTransaction = norito::json::from_value(value).unwrap();
    assert_eq!(
        localnet_bootstrap_staking_policy(&changed).unwrap(),
        expected_policy
    );
    let encode_instructions = |source: &RawGenesisTransaction| {
        source
            .instructions()
            .map(norito::encode_canonical)
            .collect::<Result<Vec<_>, _>>()
    };
    assert_eq!(
        encode_instructions(&changed).unwrap(),
        encode_instructions(&manifest).unwrap(),
        "all original registrations, transfers and mints remain byte-identical"
    );
    let stage = |source: RawGenesisTransaction| {
        crate::genesis::staging::bind_and_sign_staged_sumeragi_context(
            source,
            &signer,
            Some(&config),
            Some(resolve_localnet_da_proof_policies(&config)),
            iroha_core::state::default_genesis_confidential_policy_hash(),
            None,
        )
    };
    let error = stage(changed).expect_err("native registration must enforce the original minimum");
    let Some(BlockValidationError::InvalidGenesis(InvalidGenesisError::RejectedOutput(rejection))) =
        error.downcast_ref::<BlockValidationError>()
    else {
        panic!("expected concrete native genesis output rejection, got {error:#}");
    };
    assert!(
        format!("{:?}", rejection.reason).contains("initial stake below committed minimum amount"),
        "a different staging failure cannot satisfy the monetary rejection: {error:#}"
    );
    let (retry, retried) = stage(manifest).expect("original admitted source remains retryable");
    assert_eq!(
        localnet_bootstrap_staking_policy(&retry).unwrap(),
        original_policy
    );
    assert!(
        retried
            .0
            .output_results()
            .all(|result| result.as_ref().is_ok())
    );
}
