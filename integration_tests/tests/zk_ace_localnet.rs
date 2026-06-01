//! Local-node E2E coverage for ZK-ACE transparent-transfer authorization.
#![cfg(feature = "zk-stark")]

use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr as _, ensure};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::KeyPair,
    data_model::{
        account::{Account, AccountId},
        asset::AssetDefinition,
        confidential::ConfidentialStatus,
        isi::{
            Grant, InstructionBox, Mint, Register, verifying_keys,
            zk::{
                RegisterZkAceIdentityCommitment, RevokeZkAceIdentityCommitment,
                RotateZkAceIdentityCommitment, SubmitZkAceAuthorizedTransfer,
            },
        },
        permission::Permission,
        prelude::{
            AssetDefinitionId, AssetId, DomainId, FindAssets, Identifiable, Json, Numeric,
            QueryBuilderExt,
        },
        proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
        zk::{
            BackendTag, ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND, ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG, ZkAceWitnessV1,
            derive_zk_ace_identity_commitment, zk_ace_public_inputs_schema_hash_v1,
        },
    },
};
use iroha_executor_data_model::permission::zk_ace::CanManageZkAceIdentityForAccount;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, BOB_ID, BOB_KEYPAIR, gen_account_in};
use zk_ace_prover::{
    build_zk_ace_transfer_authorization_v1, zk_ace_stark_fri_params_v1, zk_ace_verifier_key_id,
    zk_ace_verifying_key_record_v1,
};

const POLICY_HASH: [u8; 32] = [0x42; 32];

fn has_test_network_feature(feature: &str) -> bool {
    std::env::var("TEST_NETWORK_IROHAD_FEATURES")
        .ok()
        .map(|value| {
            value
                .split([',', ' ', '\t', '\n'])
                .any(|item| item.trim() == feature)
        })
        .unwrap_or(false)
}

fn require_test_network_feature(feature: &str, test_name: &str) -> Result<()> {
    ensure!(
        has_test_network_feature(feature),
        "{test_name}: TEST_NETWORK_IROHAD_FEATURES must include `{feature}` to execute the runtime path"
    );
    Ok(())
}

fn alt_client(signatory: (AccountId, KeyPair), base_client: &Client) -> Client {
    Client {
        account: signatory.0,
        key_pair: signatory.1,
        add_transaction_nonce: true,
        ..base_client.clone()
    }
}

fn witness(seed: u8) -> ZkAceWitnessV1 {
    ZkAceWitnessV1 {
        identity_root: [seed; 32],
        identity_blinding: [seed.wrapping_add(1); 32],
        replay_secret: [seed.wrapping_add(2); 32],
    }
}

fn identity_commitment(witness: &ZkAceWitnessV1) -> [u8; 32] {
    derive_zk_ace_identity_commitment(
        &witness.identity_root,
        &witness.identity_blinding,
        ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
    )
}

fn stark_verifier_record(
    id: &VerifyingKeyId,
    circuit_id: &str,
    version: u32,
    status: ConfidentialStatus,
) -> Result<VerifyingKeyRecord> {
    let params = zk_ace_stark_fri_params_v1();
    let payload = iroha_core::zk_stark::StarkFriVerifyingKeyV1 {
        version: 1,
        circuit_id: circuit_id.to_owned(),
        n_log2: params.n_log2,
        blowup_log2: params.blowup_log2,
        fold_arity: params.fold_arity,
        queries: params.queries,
        merkle_arity: params.merkle_arity,
        hash_fn: params.hash_fn,
    };
    let key = VerifyingKeyBox::new(
        id.backend.clone(),
        norito::to_bytes(&payload).wrap_err("encode STARK verifier payload")?,
    );
    let commitment = iroha_core::zk::hash_vk(&key);
    let mut record = VerifyingKeyRecord::new(
        version,
        circuit_id,
        BackendTag::Stark,
        "goldilocks",
        zk_ace_public_inputs_schema_hash_v1(),
        commitment,
    );
    record.namespace = "zk-ace".to_owned();
    record.vk_len = u32::try_from(key.bytes.len()).unwrap_or(u32::MAX);
    record.max_proof_bytes = 256 * 1024;
    record.gas_schedule_id = Some("zk_ace_stark_default".to_owned());
    record.key = Some(key);
    record.status = status;
    Ok(record)
}

fn asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").expect("default domain id"),
        "zkace_e2e".parse().expect("asset name"),
    )
}

#[allow(clippy::too_many_arguments)]
fn authorized_transfer(
    from: AccountId,
    to: AccountId,
    asset: AssetDefinitionId,
    amount: u128,
    chain_id: iroha::data_model::ChainId,
    witness: ZkAceWitnessV1,
    verifier_key_id: VerifyingKeyId,
    vk_commitment: [u8; 32],
) -> Result<SubmitZkAceAuthorizedTransfer> {
    let authorization = build_zk_ace_transfer_authorization_v1(
        from.clone(),
        to.clone(),
        asset.clone(),
        amount,
        chain_id,
        witness,
        POLICY_HASH,
        verifier_key_id,
        vk_commitment,
    )
    .wrap_err("build ZK-ACE transfer authorization")?;
    let public_inputs = authorization.public_inputs;
    Ok(SubmitZkAceAuthorizedTransfer::new(
        from,
        to,
        asset,
        amount,
        public_inputs.identity_commitment,
        public_inputs.tx_digest,
        public_inputs.chain_id,
        public_inputs.domain_tag,
        public_inputs.action_class,
        public_inputs.replay_nullifier,
        public_inputs.policy_hash,
        authorization.proof,
    ))
}

fn submit_and_wait<I>(client: &Client, instruction: I, context: &str) -> Result<()>
where
    I: Into<InstructionBox>,
{
    client
        .submit_blocking(instruction)
        .wrap_err_with(|| format!("submit {context}"))?;
    Ok(())
}

fn submit_expect_err<I>(client: &Client, instruction: I, context: &str) -> String
where
    I: Into<InstructionBox>,
{
    let err = client.submit_blocking(instruction).expect_err(context);
    let mut text = format!("{err:?}");
    for cause in err.chain() {
        text.push_str(" | ");
        text.push_str(&cause.to_string());
    }
    text
}

fn assert_rejected_contains<I>(client: &Client, instruction: I, context: &str, expected: &str)
where
    I: Into<InstructionBox>,
{
    let text = submit_expect_err(client, instruction, context);
    assert!(
        text.contains(expected),
        "unexpected rejection for {context}; expected `{expected}` in {text}"
    );
}

fn wait_for_balance(
    client: &Client,
    asset_definition_id: &AssetDefinitionId,
    account_id: &AccountId,
    expected: u64,
    context: &str,
) -> Result<()> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);

    let expected_value = Numeric::from(expected);
    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "assets were not queried".to_owned();

    while Instant::now() < deadline {
        match client.query(FindAssets::new()).execute_all() {
            Ok(assets) => {
                let observed = assets
                    .iter()
                    .find(|asset| {
                        asset.id().definition() == asset_definition_id
                            && asset.id().account() == account_id
                    })
                    .map(|asset| asset.value().clone());
                last_observed = format!("{observed:?}");
                if observed.as_ref() == Some(&expected_value) {
                    return Ok(());
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }
        sleep(POLL_INTERVAL);
    }

    eyre::bail!(
        "timed out waiting for {context}; account={account_id}, expected={expected_value:?}, last_observed={last_observed}"
    )
}

#[test]
fn zk_ace_authorized_transfer_local_node_lifecycle() -> Result<()> {
    const TEST_NAME: &str = "zk_ace_authorized_transfer_local_node_lifecycle";
    require_test_network_feature("zk-stark", TEST_NAME)?;

    let asset_definition_id = asset_definition_id();
    let alice_asset = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
    let (delegate_id, delegate_keypair) = gen_account_in("wonderland");
    let builder = NetworkBuilder::new()
        .with_genesis_instruction(Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone())
                .with_name(asset_definition_id.name().to_string()),
        ))
        .with_genesis_instruction(Register::account(Account::new(delegate_id.clone())))
        .with_genesis_instruction(Mint::asset_numeric(100_u64, alice_asset))
        .with_genesis_instruction(Grant::account_permission(
            Permission::new("CanManageVerifyingKeys".into(), Json::new(())),
            ALICE_ID.clone(),
        ))
        .with_config_layer(|layer| {
            layer.write(["zk", "stark", "enabled"], true);
        });

    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(builder, TEST_NAME)? else {
        return Ok(());
    };
    let mut client = network.client();
    client.add_transaction_nonce = true;
    let bob_client = alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &client);
    let delegate_client = alt_client((delegate_id.clone(), delegate_keypair), &client);
    let chain_id = network.chain_id();

    let vk_id = zk_ace_verifier_key_id(ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID);
    let vk_record = zk_ace_verifying_key_record_v1(1).wrap_err("build ZK-ACE verifier record")?;
    let vk_commitment = vk_record.commitment;
    submit_and_wait(
        &client,
        verifying_keys::RegisterVerifyingKey {
            id: vk_id.clone(),
            record: vk_record,
        },
        "register ZK-ACE verifier key",
    )?;

    let inactive_vk_id = VerifyingKeyId::new(
        ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
        "inactive_zk_ace_pq_authorization_v0",
    );
    let mut inactive_vk_record =
        zk_ace_verifying_key_record_v1(2).wrap_err("build inactive ZK-ACE verifier record")?;
    inactive_vk_record.status = ConfidentialStatus::Proposed;
    submit_and_wait(
        &client,
        verifying_keys::RegisterVerifyingKey {
            id: inactive_vk_id.clone(),
            record: inactive_vk_record,
        },
        "register inactive ZK-ACE verifier key",
    )?;

    let wrong_circuit_vk_id = VerifyingKeyId::new(
        ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
        "wrong_circuit_zk_ace_pq_authorization_v0",
    );
    submit_and_wait(
        &client,
        verifying_keys::RegisterVerifyingKey {
            id: wrong_circuit_vk_id.clone(),
            record: stark_verifier_record(
                &wrong_circuit_vk_id,
                "wrong_zk_ace_pq_authorization_v0",
                3,
                ConfidentialStatus::Active,
            )?,
        },
        "register wrong-circuit ZK-ACE verifier key",
    )?;

    let wrong_schema_vk_id = VerifyingKeyId::new(
        ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
        "wrong_schema_zk_ace_pq_authorization_v0",
    );
    let mut wrong_schema_vk_record =
        zk_ace_verifying_key_record_v1(6).wrap_err("build wrong-schema ZK-ACE verifier record")?;
    wrong_schema_vk_record.public_inputs_schema_hash[0] ^= 1;
    submit_and_wait(
        &client,
        verifying_keys::RegisterVerifyingKey {
            id: wrong_schema_vk_id.clone(),
            record: wrong_schema_vk_record,
        },
        "register wrong-schema ZK-ACE verifier key",
    )?;

    let withdrawn_vk_id = VerifyingKeyId::new(
        ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
        "withdrawn_zk_ace_pq_authorization_v0",
    );
    let withdrawn_initial_record =
        zk_ace_verifying_key_record_v1(4).wrap_err("build withdrawable ZK-ACE verifier record")?;
    let withdrawn_vk_commitment = withdrawn_initial_record.commitment;
    submit_and_wait(
        &client,
        verifying_keys::RegisterVerifyingKey {
            id: withdrawn_vk_id.clone(),
            record: withdrawn_initial_record,
        },
        "register withdrawable ZK-ACE verifier key",
    )?;

    let unauthorized_witness = witness(0x31);
    assert_rejected_contains(
        &bob_client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            identity_commitment(&unauthorized_witness),
            POLICY_HASH,
            vec![ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "unauthorized registrar binding a victim source account",
        "Failed to find [`Permission`] by id",
    );

    submit_and_wait(
        &client,
        Grant::account_permission(
            Permission::from(CanManageZkAceIdentityForAccount {
                account: ALICE_ID.clone(),
                asset: asset_definition_id.clone(),
            }),
            delegate_id.clone(),
        ),
        "grant delegated ZK-ACE manage permission for Alice",
    )?;
    let delegated_witness = witness(0x3d);
    let delegated_identity = identity_commitment(&delegated_witness);
    submit_and_wait(
        &delegate_client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            delegated_identity,
            POLICY_HASH,
            vec![ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "delegated ZK-ACE identity registration for Alice",
    )?;
    let delegated_rotated_witness = witness(0x3e);
    let delegated_rotated_identity = identity_commitment(&delegated_rotated_witness);
    assert_rejected_contains(
        &delegate_client,
        RotateZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            delegated_identity,
            delegated_rotated_identity,
            POLICY_HASH,
            vec![ALICE_ID.clone(), BOB_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "delegated ZK-ACE rotation without authority for every source account",
        "Failed to find [`Permission`] by id",
    );
    submit_and_wait(
        &client,
        Grant::account_permission(
            Permission::from(CanManageZkAceIdentityForAccount {
                account: BOB_ID.clone(),
                asset: asset_definition_id.clone(),
            }),
            delegate_id.clone(),
        ),
        "grant delegated ZK-ACE manage permission for Bob",
    )?;
    submit_and_wait(
        &delegate_client,
        RotateZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            delegated_identity,
            delegated_rotated_identity,
            POLICY_HASH,
            vec![BOB_ID.clone(), ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "delegated ZK-ACE identity rotation for all allowlisted accounts",
    )?;
    submit_and_wait(
        &delegate_client,
        RevokeZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            delegated_rotated_identity,
            Some([0x3f; 32]),
        ),
        "delegated ZK-ACE identity revocation",
    )?;

    let empty_allowlist_witness = witness(0x36);
    assert_rejected_contains(
        &client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            identity_commitment(&empty_allowlist_witness),
            POLICY_HASH,
            Vec::new(),
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "empty ZK-ACE source-account allowlist",
        "allowed_accounts must be non-empty",
    );

    let duplicate_allowlist_witness = witness(0x37);
    assert_rejected_contains(
        &client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            identity_commitment(&duplicate_allowlist_witness),
            POLICY_HASH,
            vec![ALICE_ID.clone(), ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "duplicate ZK-ACE source-account allowlist",
        "allowed_accounts must not contain duplicates",
    );

    let oversized_allowlist_witness = witness(0x38);
    assert_rejected_contains(
        &client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            identity_commitment(&oversized_allowlist_witness),
            POLICY_HASH,
            (0..=iroha::data_model::zk::ZK_ACE_MAX_ALLOWED_ACCOUNTS)
                .map(|_| ALICE_ID.clone())
                .collect(),
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "oversized ZK-ACE source-account allowlist",
        "allowed_accounts exceeds maximum",
    );

    let (missing_allowed_account, _) = gen_account_in("wonderland");
    let missing_account_allowlist_witness = witness(0x39);
    assert_rejected_contains(
        &client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            identity_commitment(&missing_account_allowlist_witness),
            POLICY_HASH,
            vec![missing_allowed_account],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "missing-account ZK-ACE source-account allowlist",
        "does not exist",
    );

    let inactive_witness = witness(0x32);
    assert_rejected_contains(
        &client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            identity_commitment(&inactive_witness),
            POLICY_HASH,
            vec![ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            inactive_vk_id,
        ),
        "inactive verifier key for identity registration",
        "verifying key is not active",
    );

    let wrong_circuit_witness = witness(0x33);
    assert_rejected_contains(
        &client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            identity_commitment(&wrong_circuit_witness),
            POLICY_HASH,
            vec![ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            wrong_circuit_vk_id,
        ),
        "wrong-circuit verifier key for identity registration",
        "not bound to zk_ace_pq_authorization_v0",
    );

    let wrong_schema_witness = witness(0x35);
    assert_rejected_contains(
        &client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            identity_commitment(&wrong_schema_witness),
            POLICY_HASH,
            vec![ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            wrong_schema_vk_id,
        ),
        "wrong-schema verifier key for identity registration",
        "public input schema hash mismatch",
    );

    let old_witness = witness(0x11);
    let old_identity = identity_commitment(&old_witness);
    submit_and_wait(
        &client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            old_identity,
            POLICY_HASH,
            vec![ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "register ZK-ACE identity commitment",
    )?;

    let unauthorized_rotation_witness = witness(0x3a);
    assert_rejected_contains(
        &bob_client,
        RotateZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            old_identity,
            identity_commitment(&unauthorized_rotation_witness),
            POLICY_HASH,
            vec![ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "unauthorized rotation binding a victim source account",
        "Failed to find [`Permission`] by id",
    );

    assert_rejected_contains(
        &bob_client,
        RevokeZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            old_identity,
            Some([0x3b; 32]),
        ),
        "unauthorized revocation of victim ZK-ACE identity",
        "Failed to find [`Permission`] by id",
    );

    let invalid_rotation_witness = witness(0x3c);
    assert_rejected_contains(
        &client,
        RotateZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            old_identity,
            identity_commitment(&invalid_rotation_witness),
            POLICY_HASH,
            Vec::new(),
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "empty allowlist on ZK-ACE rotation",
        "allowed_accounts must be non-empty",
    );

    let wrong_reference_transfer = authorized_transfer(
        ALICE_ID.clone(),
        BOB_ID.clone(),
        asset_definition_id.clone(),
        2,
        chain_id.clone(),
        old_witness,
        VerifyingKeyId::new(
            ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
            "unbound_zk_ace_pq_authorization_v0",
        ),
        vk_commitment,
    )?;
    assert_rejected_contains(
        &client,
        wrong_reference_transfer,
        "proof with wrong verifier key reference",
        "verifying key reference mismatch",
    );

    let mut altered_amount_transfer = authorized_transfer(
        ALICE_ID.clone(),
        BOB_ID.clone(),
        asset_definition_id.clone(),
        3,
        chain_id.clone(),
        old_witness,
        vk_id.clone(),
        vk_commitment,
    )?;
    altered_amount_transfer.amount += 1;
    assert_rejected_contains(
        &client,
        altered_amount_transfer,
        "proof replayed against altered amount",
        "tx_digest does not match transfer fields",
    );

    let mut altered_recipient_transfer = authorized_transfer(
        ALICE_ID.clone(),
        BOB_ID.clone(),
        asset_definition_id.clone(),
        4,
        chain_id.clone(),
        old_witness,
        vk_id.clone(),
        vk_commitment,
    )?;
    altered_recipient_transfer.to = ALICE_ID.clone();
    assert_rejected_contains(
        &client,
        altered_recipient_transfer,
        "proof replayed against altered recipient",
        "tx_digest does not match transfer fields",
    );

    let withdrawn_witness = witness(0x34);
    let withdrawn_identity = identity_commitment(&withdrawn_witness);
    submit_and_wait(
        &client,
        RegisterZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            withdrawn_identity,
            POLICY_HASH,
            vec![ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            withdrawn_vk_id.clone(),
        ),
        "register identity bound to withdrawable verifier key",
    )?;
    let mut withdrawn_record =
        zk_ace_verifying_key_record_v1(5).wrap_err("build withdrawn ZK-ACE verifier record")?;
    withdrawn_record.status = ConfidentialStatus::Withdrawn;
    submit_and_wait(
        &client,
        verifying_keys::UpdateVerifyingKey {
            id: withdrawn_vk_id.clone(),
            record: withdrawn_record,
        },
        "withdraw ZK-ACE verifier key",
    )?;
    let withdrawn_verifier_transfer = authorized_transfer(
        ALICE_ID.clone(),
        BOB_ID.clone(),
        asset_definition_id.clone(),
        6,
        chain_id.clone(),
        withdrawn_witness,
        withdrawn_vk_id,
        withdrawn_vk_commitment,
    )?;
    assert_rejected_contains(
        &client,
        withdrawn_verifier_transfer,
        "transfer using withdrawn verifier key",
        "verifying key is not active",
    );

    let transfer = authorized_transfer(
        ALICE_ID.clone(),
        BOB_ID.clone(),
        asset_definition_id.clone(),
        11,
        chain_id.clone(),
        old_witness,
        vk_id.clone(),
        vk_commitment,
    )?;
    let replay = transfer.clone();
    submit_and_wait(&client, transfer, "submit protected transparent transfer")?;
    wait_for_balance(
        &client,
        &asset_definition_id,
        &ALICE_ID,
        89,
        "Alice after first transfer",
    )?;
    wait_for_balance(
        &client,
        &asset_definition_id,
        &BOB_ID,
        11,
        "Bob after first transfer",
    )?;
    assert_rejected_contains(
        &client,
        replay,
        "replay ZK-ACE transfer",
        "replay nullifier already consumed",
    );

    let non_allowlisted_source = authorized_transfer(
        BOB_ID.clone(),
        ALICE_ID.clone(),
        asset_definition_id.clone(),
        1,
        chain_id.clone(),
        old_witness,
        vk_id.clone(),
        vk_commitment,
    )?;
    assert_rejected_contains(
        &client,
        non_allowlisted_source,
        "non-allowlisted source account",
        "source account is not in the identity allowlist",
    );

    let new_witness = witness(0x21);
    let new_identity = identity_commitment(&new_witness);
    submit_and_wait(
        &client,
        RotateZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            old_identity,
            new_identity,
            POLICY_HASH,
            vec![ALICE_ID.clone()],
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            vk_id.clone(),
        ),
        "rotate ZK-ACE identity commitment",
    )?;

    let old_after_rotation = authorized_transfer(
        ALICE_ID.clone(),
        BOB_ID.clone(),
        asset_definition_id.clone(),
        1,
        chain_id.clone(),
        old_witness,
        vk_id.clone(),
        vk_commitment,
    )?;
    assert_rejected_contains(
        &client,
        old_after_rotation,
        "rotated-out ZK-ACE identity commitment",
        "rotated out",
    );

    let new_transfer = authorized_transfer(
        ALICE_ID.clone(),
        BOB_ID.clone(),
        asset_definition_id.clone(),
        5,
        chain_id.clone(),
        new_witness,
        vk_id.clone(),
        vk_commitment,
    )?;
    submit_and_wait(
        &client,
        new_transfer,
        "submit rotated ZK-ACE identity transfer",
    )?;
    wait_for_balance(
        &client,
        &asset_definition_id,
        &ALICE_ID,
        84,
        "Alice after rotated transfer",
    )?;
    wait_for_balance(
        &client,
        &asset_definition_id,
        &BOB_ID,
        16,
        "Bob after rotated transfer",
    )?;

    submit_and_wait(
        &client,
        RevokeZkAceIdentityCommitment::new(
            asset_definition_id.clone(),
            new_identity,
            Some([0x77; 32]),
        ),
        "revoke ZK-ACE identity commitment",
    )?;
    let after_revoke = authorized_transfer(
        ALICE_ID.clone(),
        BOB_ID.clone(),
        asset_definition_id,
        1,
        chain_id,
        new_witness,
        vk_id,
        vk_commitment,
    )?;
    assert_rejected_contains(
        &client,
        after_revoke,
        "revoked ZK-ACE identity commitment",
        "revoked",
    );

    Ok(())
}
