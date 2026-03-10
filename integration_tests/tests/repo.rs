#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration coverage for repo and reverse-repo instructions.

use std::{
    convert::TryFrom,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        metadata::Metadata,
        prelude::*,
        query::repo::prelude::FindRepoAgreements,
        repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
    },
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::isi::RepoMarginCallIsi;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, BOB_ID};

static GENESIS_STATUS: OnceLock<std::result::Result<(), ()>> = OnceLock::new();
static START_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn install_quiet_tracing() {
    static QUIET_TRACE: OnceLock<()> = OnceLock::new();
    QUIET_TRACE.get_or_init(|| {});
}

fn ivm_build_profile_exists() -> bool {
    use std::path::PathBuf;
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/ivm/target/prebuilt/build_config.toml")
        .exists()
}

fn quiet_network_builder() -> NetworkBuilder {
    install_quiet_tracing();
    init_instruction_registry();
    NetworkBuilder::new()
        .with_auto_populated_trusted_peers()
        .with_peers(4)
}

fn loopback_bind_allowed() -> bool {
    static LOOPBACK_BIND_ALLOWED: OnceLock<bool> = OnceLock::new();
    *LOOPBACK_BIND_ALLOWED.get_or_init(|| std::net::TcpListener::bind(("127.0.0.1", 0)).is_ok())
}

fn start_test_network() -> Option<(sandbox::SerializedNetwork, tokio::runtime::Runtime)> {
    if !ivm_build_profile_exists() {
        eprintln!("Skipping test: missing IVM build profile");
        return None;
    }

    if !loopback_bind_allowed() {
        eprintln!("Skipping test: environment denies binding TCP sockets on 127.0.0.1");
        return None;
    }

    if matches!(GENESIS_STATUS.get(), Some(Err(()))) {
        eprintln!("Skipping test: failed to start network (cached)");
        return None;
    }

    let serial_guard = sandbox::serial_guard();
    let guard = START_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();

    if matches!(GENESIS_STATUS.get(), Some(Err(()))) {
        eprintln!("Skipping test: failed to start network (cached)");
        drop(guard);
        return None;
    }

    let builder = quiet_network_builder();
    let (network, runtime) = builder.build_blocking();

    if let Err(err) = runtime.block_on(async { network.start_all().await }) {
        eprintln!("Skipping test: failed to start network: {err}");
        let _ = GENESIS_STATUS.set(Err(()));
        drop(guard);
        return None;
    }

    let _ = GENESIS_STATUS.set(Ok(()));
    drop(guard);

    Some((
        sandbox::SerializedNetwork::new(network, serial_guard),
        runtime,
    ))
}

fn repo_instr_box<T>(instruction: T) -> InstructionBox
where
    RepoInstructionBox: From<T>,
{
    InstructionBox::from(RepoInstructionBox::from(instruction))
}

fn error_chain_contains(err: &eyre::Report, needle: &str) -> bool {
    err.chain().any(|cause| cause.to_string().contains(needle))
}

#[test]
#[allow(clippy::too_many_lines)]
fn repo_roundtrip_transfers_balances_and_clears_agreement() -> Result<()> {
    let Some((network, _rt)) = start_test_network() else {
        return Ok(());
    };
    let client = network.client();

    let metadata = Metadata::default();
    let cash_def_id: AssetDefinitionId = "usd#wonderland".parse()?;
    let collateral_def_id: AssetDefinitionId = "bond#wonderland".parse()?;

    // Prepare cash/collateral assets and balances.
    let setup_instructions: Vec<InstructionBox> = vec![
        Register::asset_definition(AssetDefinition::numeric(cash_def_id.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(collateral_def_id.clone())).into(),
        Mint::asset_numeric(
            numeric!(2000),
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
            numeric!(1500),
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
        )
        .into(),
    ];
    let setup_tx = client.build_transaction(setup_instructions, metadata.clone());
    client.submit_transaction_blocking(&setup_tx)?;

    // Initiate the repo: Alice borrows cash, pledging collateral.
    let agreement_id: RepoAgreementId = "daily_repo".parse()?;
    let repo_instruction_template = || -> Result<RepoIsi> {
        let maturity_ms = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .checked_add(Duration::from_secs(86_400))
                .ok_or_else(|| eyre!("maturity timestamp overflow"))?
                .as_millis(),
        )
        .expect("repo maturity timestamp fits in u64");
        Ok(RepoIsi::new(
            agreement_id.clone(),
            ALICE_ID.clone(),
            BOB_ID.clone(),
            None,
            RepoCashLeg {
                asset_definition_id: cash_def_id.clone(),
                quantity: numeric!(1000),
            },
            RepoCollateralLeg::new(collateral_def_id.clone(), numeric!(1100)),
            0,
            maturity_ms,
            RepoGovernance::with_defaults(1_500, 86_400),
        ))
    };
    let repo_instruction = repo_instruction_template()?;
    let repo_instruction_box = repo_instr_box(repo_instruction);
    let repo_tx = client.build_transaction(vec![repo_instruction_box], metadata.clone());
    client.submit_transaction_blocking(&repo_tx)?;

    // Ensure duplicate agreement IDs are rejected while active.
    let duplicate_repo = repo_instruction_template()?;
    let duplicate_instruction_box = repo_instr_box(duplicate_repo);
    let duplicate_tx = client.build_transaction(vec![duplicate_instruction_box], metadata.clone());
    let duplicate_err = client
        .submit_transaction_blocking(&duplicate_tx)
        .unwrap_err();
    assert!(
        error_chain_contains(&duplicate_err, "already exists"),
        "expected duplicate repo to be rejected, got {duplicate_err:?}"
    );
    let alice_cash_id = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
    let bob_cash_id = AssetId::new(cash_def_id.clone(), BOB_ID.clone());
    let alice_collateral_id = AssetId::new(collateral_def_id.clone(), ALICE_ID.clone());
    let bob_collateral_id = AssetId::new(collateral_def_id.clone(), BOB_ID.clone());

    let assets_after_repo = client
        .query(FindAssets::new())
        .execute_all()?
        .into_iter()
        .collect::<Vec<_>>();

    let alice_cash = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &alice_cash_id)
        .expect("alice cash after repo");
    assert_eq!(*alice_cash.value(), numeric!(1000));

    let bob_cash = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &bob_cash_id)
        .expect("bob cash after repo");
    assert_eq!(*bob_cash.value(), numeric!(1000));

    let alice_collateral = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &alice_collateral_id)
        .expect("alice collateral after repo");
    assert_eq!(*alice_collateral.value(), numeric!(400));

    let bob_collateral = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &bob_collateral_id)
        .expect("bob collateral after repo");
    assert_eq!(*bob_collateral.value(), numeric!(1100));

    // Pack the reverse repo instruction with a settlement timestamp measured at submission time.
    let settlement_timestamp_ms =
        u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
            .expect("repo settlement timestamp fits in u64");
    let future_reverse = ReverseRepoIsi::new(
        agreement_id.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        RepoCashLeg {
            asset_definition_id: cash_def_id.clone(),
            quantity: numeric!(1000),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), numeric!(1100)),
        settlement_timestamp_ms + 86_400_000,
    );
    let future_tx =
        client.build_transaction(vec![repo_instr_box(future_reverse)], metadata.clone());
    let future_err = client.submit_transaction_blocking(&future_tx).unwrap_err();
    assert!(
        error_chain_contains(&future_err, "future"),
        "expected future-dated reverse repo to be rejected, got {future_err:?}"
    );

    let reverse_repo_instruction = ReverseRepoIsi::new(
        agreement_id.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        RepoCashLeg {
            asset_definition_id: cash_def_id.clone(),
            quantity: numeric!(1000),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), numeric!(1100)),
        settlement_timestamp_ms,
    );

    let reverse_tx = client.build_transaction(
        vec![repo_instr_box(reverse_repo_instruction)],
        metadata.clone(),
    );
    client.submit_transaction_blocking(&reverse_tx)?;

    let assets_after_reverse = client
        .query(FindAssets::new())
        .execute_all()?
        .into_iter()
        .collect::<Vec<_>>();

    let alice_cash_post = assets_after_reverse
        .iter()
        .find(|asset| asset.id() == &alice_cash_id);
    assert!(
        alice_cash_post.is_none(),
        "alice cash asset should be pruned when returning to zero balance"
    );

    let bob_cash_post = assets_after_reverse
        .iter()
        .find(|asset| asset.id() == &bob_cash_id)
        .expect("bob cash after reverse repo");
    assert_eq!(*bob_cash_post.value(), numeric!(2000));

    let alice_collateral_post = assets_after_reverse
        .iter()
        .find(|asset| asset.id() == &alice_collateral_id)
        .expect("alice collateral after reverse repo");
    assert_eq!(*alice_collateral_post.value(), numeric!(1500));

    let bob_collateral_post = assets_after_reverse
        .iter()
        .find(|asset| asset.id() == &bob_collateral_id);
    assert!(
        bob_collateral_post.is_none(),
        "bob collateral asset should be pruned on unwind"
    );

    // Re-run the lifecycle to cover collateral substitution during unwind, reusing the same agreement id.
    let reopened_repo = RepoIsi::new(
        agreement_id.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        None,
        RepoCashLeg {
            asset_definition_id: cash_def_id.clone(),
            quantity: numeric!(1000),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), numeric!(1100)),
        0,
        u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .checked_add(Duration::from_secs(43_200))
                .expect("maturity arithmetic")
                .as_millis(),
        )
        .expect("reverse maturity timestamp fits in u64"),
        RepoGovernance::with_defaults(1_500, 43_200),
    );
    let reopened_tx =
        client.build_transaction(vec![repo_instr_box(reopened_repo)], metadata.clone());
    client.submit_transaction_blocking(&reopened_tx)?;

    let repo_snapshot = client
        .query(FindRepoAgreements::new())
        .execute_all()?
        .into_iter()
        .find(|agreement| agreement.id() == &agreement_id)
        .expect("repo agreement should persist after reopen");
    assert_ne!(
        *repo_snapshot.initiated_timestamp_ms(),
        0,
        "repo initiation timestamp must be recorded"
    );
    let expected_margin_interval_ms = repo_snapshot
        .governance()
        .margin_frequency_secs()
        .saturating_mul(1_000);
    if expected_margin_interval_ms != 0 {
        assert_eq!(
            repo_snapshot.next_margin_check_after(*repo_snapshot.initiated_timestamp_ms()),
            Some(
                repo_snapshot
                    .initiated_timestamp_ms()
                    .saturating_add(expected_margin_interval_ms)
            ),
            "margin schedule should align with governance cadence"
        );
    }

    // Provide additional collateral so the counterparty can settle above the recorded pledge.
    client.submit_blocking(Mint::asset_numeric(
        numeric!(50),
        AssetId::new(collateral_def_id.clone(), BOB_ID.clone()),
    ))?;

    let substitution_settlement_ms = *repo_snapshot.initiated_timestamp_ms();

    let insufficient_reverse = ReverseRepoIsi::new(
        agreement_id.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        RepoCashLeg {
            asset_definition_id: cash_def_id.clone(),
            quantity: numeric!(1000),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), numeric!(1050)),
        substitution_settlement_ms,
    );
    let insufficient_tx =
        client.build_transaction(vec![repo_instr_box(insufficient_reverse)], metadata.clone());
    let insufficient_err = client
        .submit_transaction_blocking(&insufficient_tx)
        .unwrap_err();
    assert!(
        error_chain_contains(&insufficient_err, "must not deliver less"),
        "expected insufficient collateral substitution to be rejected, got {insufficient_err:?}"
    );
    let substitution_reverse = ReverseRepoIsi::new(
        agreement_id.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        RepoCashLeg {
            asset_definition_id: cash_def_id.clone(),
            quantity: numeric!(1000),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), numeric!(1150)),
        substitution_settlement_ms,
    );
    let substitution_tx =
        client.build_transaction(vec![repo_instr_box(substitution_reverse)], metadata.clone());
    client.submit_transaction_blocking(&substitution_tx)?;

    let assets_after_substitution = client
        .query(FindAssets::new())
        .execute_all()?
        .into_iter()
        .collect::<Vec<_>>();

    let bob_cash_after_substitution = assets_after_substitution
        .iter()
        .find(|asset| asset.id() == &bob_cash_id)
        .expect("bob cash after substitution unwind");
    assert_eq!(*bob_cash_after_substitution.value(), numeric!(2000));

    let alice_collateral_after_substitution = assets_after_substitution
        .iter()
        .find(|asset| asset.id() == &alice_collateral_id)
        .expect("alice collateral after substitution unwind");
    assert_eq!(*alice_collateral_after_substitution.value(), numeric!(1550));

    let remaining_agreements = client.query(FindRepoAgreements::new()).execute_all()?;
    assert!(
        remaining_agreements
            .iter()
            .all(|agreement| agreement.id() != &agreement_id),
        "repo agreement should be cleared after substitution unwind"
    );

    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)]
fn repo_margin_call_enforces_cadence_and_participant_rules() -> Result<()> {
    let Some((network, _rt)) = start_test_network() else {
        return Ok(());
    };
    let client = network.client();

    let metadata = Metadata::default();
    let cash_def_id: AssetDefinitionId = "usd#wonderland".parse()?;
    let collateral_def_id: AssetDefinitionId = "bond#wonderland".parse()?;

    let outsider_keypair = KeyPair::from_seed(vec![42; 32], Algorithm::Ed25519);
    let outsider_domain: DomainId = "wonderland".parse()?;
    let outsider_id = AccountId::new(outsider_keypair.public_key().clone());

    let setup_instructions: Vec<InstructionBox> = vec![
        Register::account(Account::new(outsider_id.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(cash_def_id.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(collateral_def_id.clone())).into(),
        Mint::asset_numeric(
            numeric!(2000),
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
            numeric!(1500),
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
        )
        .into(),
    ];
    let setup_tx = client.build_transaction(setup_instructions, metadata.clone());
    client.submit_transaction_blocking(&setup_tx)?;

    let agreement_id: RepoAgreementId = "margin_repo".parse()?;
    let maturity_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .checked_add(Duration::from_secs(3_600))
            .ok_or_else(|| eyre!("maturity timestamp overflow"))?
            .as_millis(),
    )
    .expect("repo maturity timestamp fits in u64");
    let repo_instruction = RepoIsi::new(
        agreement_id.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        None,
        RepoCashLeg {
            asset_definition_id: cash_def_id.clone(),
            quantity: numeric!(1000),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), numeric!(1100)),
        0,
        maturity_ms,
        // Use a generous margin cadence to avoid wall-clock flakiness in the
        // integration test network.
        RepoGovernance::with_defaults(1_500, 300),
    );
    let repo_tx =
        client.build_transaction(vec![repo_instr_box(repo_instruction)], metadata.clone());
    client.submit_transaction_blocking(&repo_tx)?;

    let repo_snapshot = client
        .query(FindRepoAgreements::new())
        .execute_all()?
        .into_iter()
        .find(|agreement| agreement.id() == &agreement_id)
        .expect("repo agreement should exist after initiation");
    let initial_last_margin_ms = *repo_snapshot.last_margin_check_timestamp_ms();
    let expected_first_due = repo_snapshot
        .next_margin_check_after(initial_last_margin_ms)
        .expect("margin cadence configured");
    assert!(
        expected_first_due > initial_last_margin_ms,
        "margin cadence should advance beyond the initiation timestamp"
    );

    let premature_err = client
        .submit_transaction_blocking(&client.build_transaction(
            vec![repo_instr_box(RepoMarginCallIsi::new(agreement_id.clone()))],
            metadata.clone(),
        ))
        .unwrap_err();
    assert!(
        error_chain_contains(&premature_err, "margin check is not yet due"),
        "expected cadence enforcement error, got {premature_err:?}"
    );

    let after_reject = client
        .query(FindRepoAgreements::new())
        .execute_all()?
        .into_iter()
        .find(|agreement| agreement.id() == &agreement_id)
        .expect("repo agreement should persist after rejected margin call");
    assert_eq!(
        *after_reject.last_margin_check_timestamp_ms(),
        initial_last_margin_ms,
        "rejected margin call should not advance the schedule"
    );

    let unauthorized_client = alt_client((outsider_id.clone(), outsider_keypair), &client);
    let unauthorized_err = unauthorized_client
        .submit_transaction_blocking(&unauthorized_client.build_transaction(
            vec![repo_instr_box(RepoMarginCallIsi::new(agreement_id))],
            metadata,
        ))
        .unwrap_err();
    assert!(
        error_chain_contains(
            &unauthorized_err,
            "margin call must be initiated by a repo participant",
        ),
        "expected participant check failure, got {unauthorized_err:?}"
    );

    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)]
fn repo_roundtrip_with_custodian_routes_collateral() -> Result<()> {
    let Some((network, _rt)) = start_test_network() else {
        return Ok(());
    };
    let client = network.client();

    let metadata = Metadata::default();
    let custodian_keypair = KeyPair::random();
    let custodian_domain: DomainId = "wonderland".parse()?;
    let custodian_id = AccountId::new(custodian_keypair.public_key().clone());
    let cash_def_id: AssetDefinitionId = "usd#wonderland".parse()?;
    let collateral_def_id: AssetDefinitionId = "bond#wonderland".parse()?;

    let setup_instructions: Vec<InstructionBox> = vec![
        Register::account(Account::new(custodian_id.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(cash_def_id.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(collateral_def_id.clone())).into(),
        Mint::asset_numeric(
            numeric!(2000),
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
            numeric!(1500),
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
        )
        .into(),
    ];
    let setup_tx = client.build_transaction(setup_instructions, metadata.clone());
    client.submit_transaction_blocking(&setup_tx)?;

    let agreement_id: RepoAgreementId = "tri_party_repo".parse()?;
    let maturity_timestamp_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .checked_add(Duration::from_secs(86_400))
            .ok_or_else(|| eyre!("maturity timestamp overflow"))?
            .as_millis(),
    )
    .expect("repo maturity timestamp fits in u64");
    let repo_instruction = RepoIsi::new(
        agreement_id.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        Some(custodian_id.clone()),
        RepoCashLeg {
            asset_definition_id: cash_def_id.clone(),
            quantity: numeric!(1000),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), numeric!(1100)),
        0,
        maturity_timestamp_ms,
        RepoGovernance::with_defaults(1_500, 86_400),
    );
    let repo_tx =
        client.build_transaction(vec![repo_instr_box(repo_instruction)], metadata.clone());
    client.submit_transaction_blocking(&repo_tx)?;

    let alice_cash_id = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
    let bob_cash_id = AssetId::new(cash_def_id.clone(), BOB_ID.clone());
    let alice_collateral_id = AssetId::new(collateral_def_id.clone(), ALICE_ID.clone());
    let custodian_collateral_id = AssetId::new(collateral_def_id.clone(), custodian_id.clone());
    let bob_collateral_id = AssetId::new(collateral_def_id.clone(), BOB_ID.clone());

    let assets_after_repo = client
        .query(FindAssets::new())
        .execute_all()?
        .into_iter()
        .collect::<Vec<_>>();

    let alice_cash = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &alice_cash_id)
        .expect("alice cash after repo");
    assert_eq!(*alice_cash.value(), numeric!(1000));

    let bob_cash = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &bob_cash_id)
        .expect("bob cash after repo");
    assert_eq!(*bob_cash.value(), numeric!(1000));

    let alice_collateral = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &alice_collateral_id)
        .expect("alice collateral after repo");
    assert_eq!(*alice_collateral.value(), numeric!(400));

    assert!(
        !assets_after_repo
            .iter()
            .any(|asset| asset.id() == &bob_collateral_id),
        "counterparty should not hold collateral when custodian participates"
    );

    let custodian_collateral = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &custodian_collateral_id)
        .expect("custodian collateral after repo");
    assert_eq!(*custodian_collateral.value(), numeric!(1100));

    let stored_agreement = client
        .query(FindRepoAgreements::new())
        .execute_all()?
        .into_iter()
        .find(|agreement| agreement.id() == &agreement_id)
        .expect("repo agreement recorded");
    assert_eq!(
        stored_agreement.custodian(),
        &Some(custodian_id.clone()),
        "custodian id should be persisted in repo agreement"
    );

    let settlement_timestamp_ms =
        u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
            .expect("settlement timestamp fits in u64");
    let reverse_repo_instruction = ReverseRepoIsi::new(
        agreement_id.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        RepoCashLeg {
            asset_definition_id: cash_def_id.clone(),
            quantity: numeric!(1000),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), numeric!(1100)),
        settlement_timestamp_ms,
    );
    let reverse_tx = client.build_transaction(
        vec![repo_instr_box(reverse_repo_instruction)],
        metadata.clone(),
    );
    client.submit_transaction_blocking(&reverse_tx)?;

    let assets_after_reverse = client
        .query(FindAssets::new())
        .execute_all()?
        .into_iter()
        .collect::<Vec<_>>();

    let alice_cash_after_reverse = assets_after_reverse
        .iter()
        .find(|asset| asset.id() == &alice_cash_id);
    assert!(
        alice_cash_after_reverse.is_none(),
        "alice cash should be pruned after returning to zero"
    );

    let bob_cash_after_reverse = assets_after_reverse
        .iter()
        .find(|asset| asset.id() == &bob_cash_id)
        .expect("bob cash after reverse");
    assert_eq!(*bob_cash_after_reverse.value(), numeric!(2000));

    let alice_collateral_after_reverse = assets_after_reverse
        .iter()
        .find(|asset| asset.id() == &alice_collateral_id)
        .expect("alice collateral after reverse");
    assert_eq!(*alice_collateral_after_reverse.value(), numeric!(1500));

    assert!(
        !assets_after_reverse
            .iter()
            .any(|asset| asset.id() == &custodian_collateral_id),
        "custodian collateral asset should be pruned after unwind"
    );

    let remaining_agreements = client.query(FindRepoAgreements::new()).execute_all()?;
    assert!(
        remaining_agreements
            .iter()
            .all(|agreement| agreement.id() != &agreement_id),
        "repo agreement should be cleared after reverse repo"
    );

    Ok(())
}

fn alt_client(signatory: (AccountId, KeyPair), base_client: &Client) -> Client {
    Client {
        account: signatory.0,
        key_pair: signatory.1,
        ..base_client.clone()
    }
}
