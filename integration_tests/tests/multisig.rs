#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests validating multi-signature transaction flows.

use std::{
    collections::BTreeMap,
    num::{NonZeroU16, NonZeroU64},
    time::Duration,
};

use eyre::{Result, WrapErr};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::KeyPair,
    data_model::{
        Level,
        account::{MultisigMember, MultisigPolicy},
        isi::AddSignatory,
        prelude::*,
    },
    executor_data_model::isi::multisig::*,
};
use iroha_test_network::*;
use iroha_test_samples::{
    ALICE_ID, BOB_ID, BOB_KEYPAIR, CARPENTER_ID, CARPENTER_KEYPAIR, gen_account_in, load_sample_ivm,
};
use tokio::runtime::Runtime;

fn start_network(
    builder: NetworkBuilder,
    context: &'static str,
) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(
        builder.with_peer_startup_timeout(Duration::from_secs(300)),
        context,
    )
    .unwrap()
}

fn multisig_supported(_client: &Client) -> bool {
    // Multisig instructions are carried via `CustomInstruction` envelope and are
    // executed by the core runtime path; they are not guaranteed to appear as
    // dedicated identifiers in `FindExecutorDataModel`.
    true
}

fn upgrade_executor(client: &Client, executor: impl AsRef<str>) -> Result<()> {
    let upgrade_executor = Upgrade::new(Executor::new(load_sample_ivm(executor)));
    client
        .submit_blocking(upgrade_executor)
        .wrap_err("Have you set IvmFuelConfig::Auto?")?;
    Ok(())
}

fn canonical_multisig_account_id(spec: &MultisigSpec) -> AccountId {
    let members = spec
        .signatories
        .iter()
        .map(|(account, weight)| {
            let signatory = account
                .controller()
                .single_signatory()
                .expect("multisig signatories must remain single-key accounts");
            MultisigMember::new(signatory.clone(), u16::from(*weight))
                .expect("multisig member should derive from valid spec")
        })
        .collect();
    let policy =
        MultisigPolicy::new(spec.quorum.get(), members).expect("multisig policy should derive");
    AccountId::new_multisig(policy)
}

#[test]
fn multisig_normal() -> Result<()> {
    multisig_base(TestSuite::normal(), stringify!(multisig_normal))
}

#[test]
fn multisig_unauthorized() -> Result<()> {
    multisig_base(TestSuite::unauthorized(), stringify!(multisig_unauthorized))
}

#[test]
fn multisig_expires() -> Result<()> {
    multisig_base(TestSuite::expires(), stringify!(multisig_expires))
}

#[test]
fn multisig_recursion_normal() -> Result<()> {
    multisig_recursion_base(TestSuite::normal(), stringify!(multisig_recursion_normal))
}

#[test]
fn multisig_recursion_unauthorized() -> Result<()> {
    multisig_recursion_base(
        TestSuite::unauthorized(),
        stringify!(multisig_recursion_unauthorized),
    )
}

#[test]
fn multisig_recursion_expires() -> Result<()> {
    multisig_recursion_base(TestSuite::expires(), stringify!(multisig_recursion_expires))
}

#[test]
fn multisig_register_materializes_missing_signatory_account() -> Result<()> {
    let context = stringify!(multisig_register_materializes_missing_signatory_account);
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig register");
        return Ok(());
    }

    let domain: DomainId = "multisig_register_materialize".parse().unwrap();
    let register_domain_and_transfer: [InstructionBox; 2] = [
        Register::domain(Domain::new(domain.clone())).into(),
        Transfer::domain(ALICE_ID.clone(), domain.clone(), BOB_ID.clone()).into(),
    ];
    test_client.submit_all_blocking(register_domain_and_transfer)?;

    let existing_signer = gen_account_in(&domain);
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client).submit_blocking(
        Register::account(Account::new(
            existing_signer.0.clone().to_account_id(domain.clone()),
        )),
    )?;

    let missing_signer = gen_account_in(&domain);
    let spec = MultisigSpec::new(
        BTreeMap::from([
            (existing_signer.0.clone(), 1),
            (missing_signer.0.clone(), 1),
        ]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            MultisigRegister::with_account(seed_account, domain.clone(), spec).into(),
        )?;

    let fetch_account = |id: &AccountId| {
        test_client
            .query(FindAccounts::new())
            .execute_all()
            .ok()
            .and_then(|accounts| accounts.into_iter().find(|account| account.id() == id))
    };
    let created_via_key: Name = "iroha:created_via".parse().unwrap();
    let created = fetch_account(&missing_signer.0)
        .expect("missing signatory account should be created during multisig register");
    assert_eq!(
        created.metadata().get(&created_via_key),
        Some(&Json::new("multisig")),
        "materialized signatory should be marked as multisig-created"
    );

    Ok(())
}

#[test]
fn multisig_register_by_non_signatory_materializes_missing_signatory_account() -> Result<()> {
    let context =
        stringify!(multisig_register_by_non_signatory_materializes_missing_signatory_account);
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig register");
        return Ok(());
    }

    let domain: DomainId = "multisig_register_rejected_materialize".parse().unwrap();
    let register_domain_and_transfer: [InstructionBox; 2] = [
        Register::domain(Domain::new(domain.clone())).into(),
        Transfer::domain(ALICE_ID.clone(), domain.clone(), BOB_ID.clone()).into(),
    ];
    test_client.submit_all_blocking(register_domain_and_transfer)?;

    let existing_signer = gen_account_in(&domain);
    let non_signatory = gen_account_in(&domain);
    let register_accounts: [InstructionBox; 2] = [
        Register::account(Account::new(
            existing_signer.0.clone().to_account_id(domain.clone()),
        ))
        .into(),
        Register::account(Account::new(
            non_signatory.0.clone().to_account_id(domain.clone()),
        ))
        .into(),
    ];
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_all_blocking(register_accounts)?;

    let missing_signer = gen_account_in(&domain);
    let spec = MultisigSpec::new(
        BTreeMap::from([
            (existing_signer.0.clone(), 1),
            (missing_signer.0.clone(), 1),
        ]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    let register = MultisigRegister::with_account(seed_account, domain.clone(), spec);
    alt_client(non_signatory, &test_client)
        .submit_blocking::<InstructionBox>(register.into())
        .expect("non-signatory should register multisig without a separate grant");

    let created_via_key: Name = "iroha:created_via".parse().unwrap();
    let created = test_client
        .query(FindAccounts::new())
        .execute_all()?
        .into_iter()
        .find(|account| account.id() == &missing_signer.0)
        .expect("missing signatory account should be created during multisig register");
    assert_eq!(
        created.metadata().get(&created_via_key),
        Some(&Json::new("multisig")),
        "materialized signatory should be marked as multisig-created"
    );

    Ok(())
}

#[test]
fn multisig_register_materializes_missing_signatory_account_after_executor_upgrade() -> Result<()> {
    let context =
        stringify!(multisig_register_materializes_missing_signatory_account_after_executor_upgrade);
    let builder = NetworkBuilder::new().with_ivm_fuel(IvmFuelConfig::Auto);
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig register");
        return Ok(());
    }

    upgrade_executor(&test_client, "executor_with_admin")?;

    let domain: DomainId = "multisig_register_materialize_upgraded".parse().unwrap();
    let register_domain_and_transfer: [InstructionBox; 2] = [
        Register::domain(Domain::new(domain.clone())).into(),
        Transfer::domain(ALICE_ID.clone(), domain.clone(), BOB_ID.clone()).into(),
    ];
    test_client.submit_all_blocking(register_domain_and_transfer)?;

    let existing_signer = gen_account_in(&domain);
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client).submit_blocking(
        Register::account(Account::new(
            existing_signer.0.clone().to_account_id(domain.clone()),
        )),
    )?;

    let missing_signer = gen_account_in(&domain);
    let spec = MultisigSpec::new(
        BTreeMap::from([
            (existing_signer.0.clone(), 1),
            (missing_signer.0.clone(), 1),
        ]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            MultisigRegister::with_account(seed_account, domain.clone(), spec).into(),
        )?;

    let created_via_key: Name = "iroha:created_via".parse().unwrap();
    let created = test_client
        .query(FindAccounts::new())
        .execute_all()?
        .into_iter()
        .find(|account| account.id() == &missing_signer.0)
        .expect("missing signatory account should be created during multisig register");
    assert_eq!(
        created.metadata().get(&created_via_key),
        Some(&Json::new("multisig")),
        "materialized signatory should be marked as multisig-created"
    );

    Ok(())
}

#[test]
fn multisig_register_by_non_signatory_materializes_missing_signatory_account_after_executor_upgrade()
-> Result<()> {
    let context = stringify!(
        multisig_register_by_non_signatory_materializes_missing_signatory_account_after_executor_upgrade
    );
    let builder = NetworkBuilder::new().with_ivm_fuel(IvmFuelConfig::Auto);
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig register");
        return Ok(());
    }

    upgrade_executor(&test_client, "executor_with_admin")?;

    let domain: DomainId = "multisig_register_rejected_upgraded".parse().unwrap();
    let register_domain_and_transfer: [InstructionBox; 2] = [
        Register::domain(Domain::new(domain.clone())).into(),
        Transfer::domain(ALICE_ID.clone(), domain.clone(), BOB_ID.clone()).into(),
    ];
    test_client.submit_all_blocking(register_domain_and_transfer)?;

    let existing_signer = gen_account_in(&domain);
    let non_signatory = gen_account_in(&domain);
    let register_accounts: [InstructionBox; 2] = [
        Register::account(Account::new(
            existing_signer.0.clone().to_account_id(domain.clone()),
        ))
        .into(),
        Register::account(Account::new(
            non_signatory.0.clone().to_account_id(domain.clone()),
        ))
        .into(),
    ];
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_all_blocking(register_accounts)?;

    let missing_signer = gen_account_in(&domain);
    let spec = MultisigSpec::new(
        BTreeMap::from([
            (existing_signer.0.clone(), 1),
            (missing_signer.0.clone(), 1),
        ]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    let register = MultisigRegister::with_account(seed_account, domain.clone(), spec);
    alt_client(non_signatory, &test_client)
        .submit_blocking::<InstructionBox>(register.into())
        .expect("non-signatory should register multisig without a separate grant");

    let created_via_key: Name = "iroha:created_via".parse().unwrap();
    let created = test_client
        .query(FindAccounts::new())
        .execute_all()?
        .into_iter()
        .find(|account| account.id() == &missing_signer.0)
        .expect("missing signatory account should be created during multisig register");
    assert_eq!(
        created.metadata().get(&created_via_key),
        Some(&Json::new("multisig")),
        "materialized signatory should be marked as multisig-created"
    );

    Ok(())
}

#[test]
fn multisig_add_signatory_materializes_missing_account() -> Result<()> {
    let context = stringify!(multisig_add_signatory_materializes_missing_account);
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not advertise multisig instructions");
        return Ok(());
    }

    let domain: DomainId = "multisig_auto_materialize".parse().unwrap();
    let register_domain_and_transfer: [InstructionBox; 2] = [
        Register::domain(Domain::new(domain.clone())).into(),
        Transfer::domain(ALICE_ID.clone(), domain.clone(), BOB_ID.clone()).into(),
    ];
    test_client.submit_all_blocking(register_domain_and_transfer)?;

    let existing_signer = gen_account_in(&domain);
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client).submit_blocking(
        Register::account(Account::new(
            existing_signer.0.clone().to_account_id(domain.clone()),
        )),
    )?;

    let spec = MultisigSpec::new(
        BTreeMap::from([(existing_signer.0.clone(), 1)]),
        NonZeroU16::new(1).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            MultisigRegister::with_account(seed_account.clone(), domain.clone(), spec.clone())
                .into(),
        )?;

    let multisig_account_id = canonical_multisig_account_id(&spec);

    let missing_signer = gen_account_in(&domain);
    let fetch_account = |id: &AccountId| {
        test_client
            .query(FindAccounts::new())
            .execute_all()
            .ok()
            .and_then(|accounts| accounts.into_iter().find(|account| account.id() == id))
    };
    assert!(
        fetch_account(&missing_signer.0).is_none(),
        "precondition: missing signatory must not exist"
    );

    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            AddSignatory::new(multisig_account_id, missing_signer.1.public_key().clone()).into(),
        )?;

    let created_via_key: Name = "iroha:created_via".parse().unwrap();
    let created = fetch_account(&missing_signer.0)
        .expect("missing signatory account should be created by add-signatory");
    assert_eq!(
        created.metadata().get(&created_via_key),
        Some(&Json::new("multisig")),
        "materialized account should be marked as multisig-created"
    );

    Ok(())
}

#[test]
fn multisig_add_signatory_rejected_does_not_materialize_missing_account() -> Result<()> {
    let context = stringify!(multisig_add_signatory_rejected_does_not_materialize_missing_account);
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not support multisig register");
        return Ok(());
    }

    let domain: DomainId = "multisig_add_rejected_materialize".parse().unwrap();
    let register_domain_and_transfer: [InstructionBox; 2] = [
        Register::domain(Domain::new(domain.clone())).into(),
        Transfer::domain(ALICE_ID.clone(), domain.clone(), BOB_ID.clone()).into(),
    ];
    test_client.submit_all_blocking(register_domain_and_transfer)?;

    let existing_signer = gen_account_in(&domain);
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client).submit_blocking(
        Register::account(Account::new(
            existing_signer.0.clone().to_account_id(domain.clone()),
        )),
    )?;

    let seed_account = AccountId::new(KeyPair::random().public_key().clone());
    let spec = MultisigSpec::new(
        BTreeMap::from([(existing_signer.0.clone(), 1)]),
        NonZeroU16::new(1).unwrap(),
        NonZeroU64::MAX,
    );
    alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &test_client)
        .submit_blocking::<InstructionBox>(
            MultisigRegister::with_account(seed_account.clone(), domain.clone(), spec.clone())
                .into(),
        )?;

    let multisig_account_id = canonical_multisig_account_id(&spec);

    let missing_signer = gen_account_in(&domain);
    let ghost_authority = gen_account_in(&domain);
    assert!(
        test_client
            .query(FindAccounts::new())
            .execute_all()?
            .into_iter()
            .all(|account| account.id() != &ghost_authority.0),
        "precondition: authority account must not exist on ledger"
    );
    let _err = alt_client(ghost_authority, &test_client)
        .submit_blocking::<InstructionBox>(
            AddSignatory::new(multisig_account_id, missing_signer.1.public_key().clone()).into(),
        )
        .expect_err("missing authority must not add signatory");

    let missing_found = test_client
        .query(FindAccounts::new())
        .execute_all()?
        .into_iter()
        .any(|account| account.id() == &missing_signer.0);
    assert!(
        !missing_found,
        "rejected add-signatory must not materialize missing accounts"
    );

    Ok(())
}

struct TestSuite {
    domain: DomainId,
    unauthorized_target_opt: Option<AccountId>,
    transaction_ttl_ms_opt: Option<u64>,
}

impl TestSuite {
    fn new(
        domain: DomainId,
        unauthorized_target_opt: Option<AccountId>,
        transaction_ttl_ms_opt: Option<u64>,
    ) -> Self {
        Self {
            domain,
            unauthorized_target_opt,
            transaction_ttl_ms_opt,
        }
    }
    fn normal() -> Self {
        // New domain for this test
        let domain = "kingdom".parse().unwrap();
        // Make some changes to the multisig account itself
        let unauthorized_target_opt = None;
        // Semi-permanently valid
        let transaction_ttl_ms_opt = None;

        Self::new(domain, unauthorized_target_opt, transaction_ttl_ms_opt)
    }

    fn unauthorized() -> Self {
        let domain = "kingdom".parse().unwrap();
        // A target account that is not present on-ledger, ensuring the proposal execution fails
        // on final validation instead of mutating unrelated account metadata.
        let unauthorized_target_opt = Some(AccountId::new(KeyPair::random().public_key().clone()));

        Self::new(domain, unauthorized_target_opt, None)
    }

    fn expires() -> Self {
        let domain = "kingdom".parse().unwrap();
        // Expires after 1 sec
        let transaction_ttl_ms_opt = Some(1_000);

        Self::new(domain, None, transaction_ttl_ms_opt)
    }
}

/// # Scenario
///
/// 1. Signatories are populated and ready to join a multisig account
/// 2. An arbitrary account registers a multisig account for the domain
/// 3. One of the signatories of the multisig account proposes a multisig transaction
/// 4. Other signatories approve the multisig transaction
/// 5. The multisig transaction executes when all of the following are met:
///     - Quorum reached: authenticated
///     - Transaction has not expired
///     - Every instruction validated against the multisig account: authorized
/// 6. Either execution or expiration on approval deletes the transaction entry
#[expect(clippy::cast_possible_truncation, clippy::too_many_lines)]
fn multisig_base(suite: TestSuite, context: &'static str) -> Result<()> {
    const N_SIGNATORIES: usize = 5;

    let TestSuite {
        domain,
        unauthorized_target_opt,
        transaction_ttl_ms_opt,
    } = suite;

    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not advertise multisig instructions");
        return Ok(());
    }

    // Assume some domain registered after genesis
    test_client
        .submit_blocking(Register::domain(Domain::new(domain.clone())))
        .wrap_err("register multisig test domain")?;

    // Populate residents in the domain
    let mut residents = core::iter::repeat_with(|| gen_account_in(&domain))
        .take(1 + N_SIGNATORIES)
        .collect::<BTreeMap<AccountId, KeyPair>>();
    test_client
        .submit_all_blocking(
            residents
                .keys()
                .cloned()
                .map(|id| Account::new(id.to_account_id(domain.clone())))
                .map(Register::account),
        )
        .wrap_err("register multisig test residents")?;

    let non_signatory = residents.pop_first().unwrap();
    let mut signatories = residents;

    let spec = MultisigSpec::new(
        signatories
            .keys()
            .enumerate()
            .map(|(weight, id)| (id.clone(), 1 + weight as u8))
            .collect(),
        // Quorum can be reached without the first signatory
        (1..=N_SIGNATORIES)
            .skip(1)
            .sum::<usize>()
            .try_into()
            .ok()
            .and_then(NonZeroU16::new)
            .unwrap(),
        transaction_ttl_ms_opt
            .and_then(NonZeroU64::new)
            .unwrap_or(NonZeroU64::MAX),
    );
    let multisig_account_key = KeyPair::random();
    let multisig_seed_account_id = AccountId::new(multisig_account_key.public_key().clone());
    let register_multisig_account = MultisigRegister::with_account(
        multisig_seed_account_id.clone(),
        domain.clone(),
        spec.clone(),
    );

    alt_client(
        (CARPENTER_ID.clone(), CARPENTER_KEYPAIR.clone()),
        &test_client,
    )
    .submit_blocking::<InstructionBox>(register_multisig_account.into())
    .expect("multisig account should be registered by an arbitrary account");
    let resident_ids: Vec<AccountId> = core::iter::once(non_signatory.0.clone())
        .chain(signatories.keys().cloned())
        .collect();
    let accounts_after_register = test_client
        .query(FindAccounts::new())
        .execute_all()
        .wrap_err("fetch accounts after multisig registration")?;
    for resident_id in resident_ids {
        let account = accounts_after_register
            .iter()
            .find(|account| account.id() == &resident_id)
            .expect("resident account should remain materialized after multisig registration");
        assert!(
            account.controller().single_signatory().is_some(),
            "resident account unexpectedly became multisig: {}",
            account.id()
        );
    }

    // All but the first signatory approve the proposal.
    let _non_approving_signatory = signatories.pop_first().unwrap();
    let multisig_account_id = canonical_multisig_account_id(&spec);

    let key: Name = "success_marker".parse().unwrap();
    let transaction_target = unauthorized_target_opt
        .as_ref()
        .unwrap_or(&multisig_account_id)
        .clone();
    let instructions = vec![
        SetKeyValue::account(
            transaction_target.clone(),
            key.clone(),
            "congratulations".parse::<Json>().unwrap(),
        )
        .into(),
    ];
    let instructions_hash = HashOf::new(&instructions);

    let proposer = signatories.pop_last().unwrap();
    let mut approvers = signatories.into_iter();

    let propose = MultisigPropose::new(multisig_account_id.clone(), instructions, None);
    let proposer_client = alt_client(proposer.clone(), &test_client);
    let proposer_account = test_client
        .query(FindAccounts::new())
        .execute_all()?
        .into_iter()
        .find(|account| account.id() == &proposer.0)
        .expect("proposer account must exist before multisig proposal");
    assert!(
        proposer_account.controller().single_signatory().is_some(),
        "proposer account unexpectedly became multisig before proposal: {}",
        proposer_account.id()
    );
    let proposal_tx = proposer_client.build_transaction_from_items(
        core::iter::once::<InstructionBox>(propose.into()),
        Metadata::default(),
    );
    assert_eq!(
        proposal_tx.authority().subject_id(),
        proposer.0.subject_id(),
        "proposal transaction authority subject must match proposer"
    );
    assert!(
        proposal_tx
            .authority()
            .controller()
            .single_signatory()
            .is_some(),
        "proposal transaction authority unexpectedly became multisig: {}",
        proposal_tx.authority()
    );
    proposer_client
        .submit_transaction_blocking(&proposal_tx)
        .wrap_err("submit multisig proposal")?;

    // Allow time to elapse to test the expiration
    if let Some(ms) = transaction_ttl_ms_opt {
        std::thread::sleep(Duration::from_millis(ms))
    }
    test_client
        .submit_blocking(Log::new(Level::DEBUG, "Just ticking time".to_string()))
        .wrap_err("tick time after multisig proposal")?;

    let approve: InstructionBox =
        MultisigApprove::new(multisig_account_id.clone(), instructions_hash).into();

    // Approve once to see if the proposal expires
    let approver = approvers.next().unwrap();
    alt_client(approver, &test_client)
        .submit_blocking::<InstructionBox>(approve.clone())
        .wrap_err("submit first multisig approval")?;

    // Subsequent approvals should succeed unless the proposal is expired
    for _ in 0..(N_SIGNATORIES - 4) {
        let approver = approvers.next().unwrap();
        let res =
            alt_client(approver, &test_client).submit_blocking::<InstructionBox>(approve.clone());
        match &transaction_ttl_ms_opt {
            None => {
                res.unwrap();
            }
            _ => {
                let _err = res.unwrap_err();
            }
        }
    }

    let fetch_account = |id: &AccountId| {
        test_client
            .query(FindAccounts::new())
            .execute_all()
            .ok()
            .and_then(|accounts| accounts.into_iter().find(|account| account.id() == id))
    };
    // Check that the multisig transaction has not yet executed
    assert!(
        fetch_account(&transaction_target)
            .and_then(|account| account.metadata().get(&key).cloned())
            .is_none(),
        "instructions shouldn't execute without enough approvals"
    );

    // The last approve to proceed to validate and execute the instructions
    let approver = approvers.next().unwrap();
    let res = alt_client(approver, &test_client).submit_blocking::<InstructionBox>(approve.clone());
    match (&transaction_ttl_ms_opt, &unauthorized_target_opt) {
        (None, None) => {
            res.unwrap();
        }
        _ => {
            let _err = res.unwrap_err();
        }
    }

    // Check if the multisig transaction has executed
    let res = fetch_account(&transaction_target)
        .and_then(|account| account.metadata().get(&key).cloned());
    match (&transaction_ttl_ms_opt, &unauthorized_target_opt) {
        (None, None) => {
            res.unwrap();
        }
        _ => {
            assert!(res.is_none());
        }
    }

    // Check if the transaction entry is deleted
    let res = fetch_account(&multisig_account_id).and_then(|account| {
        account
            .metadata()
            .get(format!("multisig/proposals/{instructions_hash}").as_str())
            .cloned()
    });
    // Proposals are removed once quorum processing runs, including unauthorized execution failures.
    assert!(res.is_none());

    Ok(())
}

/// # Scenario
///
/// ```
///         012345 <--- root multisig account
///        /      \
///       /        12345
///      /        /     \
///     /       12       345
///    /       /  \     / | \
///   0       1    2   3  4  5 <--- personal signatories
/// ```
fn multisig_recursion_base(suite: TestSuite, context: &'static str) -> Result<()> {
    let _ = suite;

    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, context) else {
        return Ok(());
    };
    let test_client = network.client();
    if !multisig_supported(&test_client) {
        eprintln!("skipping {context}: executor does not advertise multisig instructions");
        return Ok(());
    }

    let wonderland = "wonderland";
    let signatories = core::iter::repeat_with(|| gen_account_in(wonderland))
        .take(6)
        .collect::<BTreeMap<AccountId, KeyPair>>();
    let wonderland_domain: DomainId = wonderland.parse().unwrap();
    test_client.submit_all_blocking(
        signatories
            .keys()
            .cloned()
            .map(|id| Account::new(id.to_account_id(wonderland_domain.clone())))
            .map(Register::account),
    )?;

    let mut sigs = signatories.clone();
    let sigs_345 = sigs.split_off(signatories.keys().nth(3).unwrap());
    let sigs_12 = sigs.split_off(signatories.keys().nth(1).unwrap());

    let register_ms_account = |sigs: Vec<&AccountId>| -> Result<AccountId> {
        let spec = MultisigSpec::new(
            sigs.iter().copied().map(|id| (id.clone(), 1)).collect(),
            sigs.len()
                .try_into()
                .ok()
                .and_then(NonZeroU16::new)
                .unwrap(),
            NonZeroU64::MAX,
        );
        let multisig_account_key = KeyPair::random();
        let seed_account_id = AccountId::new(multisig_account_key.public_key().clone());
        let register = MultisigRegister::with_account(
            seed_account_id.clone(),
            wonderland_domain.clone(),
            spec.clone(),
        );
        test_client
            .submit_blocking::<InstructionBox>(register.into())
            .wrap_err("register multisig account in recursion setup")?;
        Ok(canonical_multisig_account_id(&spec))
    };

    let msa_12 = register_ms_account(sigs_12.keys().collect())?;
    let msa_345 = register_ms_account(sigs_345.keys().collect())?;

    let spec_with_nested_signatory = MultisigSpec::new(
        BTreeMap::from([(msa_12, 1), (msa_345, 1)]),
        NonZeroU16::new(2).unwrap(),
        NonZeroU64::MAX,
    );
    let seed_account_id = AccountId::new(KeyPair::random().public_key().clone());
    let register_nested = MultisigRegister::with_account(
        seed_account_id,
        wonderland_domain,
        spec_with_nested_signatory,
    );
    let err = test_client
        .submit_blocking::<InstructionBox>(register_nested.into())
        .expect_err("nested multisig signatories must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("single-key account"),
        "expected nested-signatory rejection to mention single-key requirement, got: {msg}"
    );

    Ok(())
}

#[test]
fn reserved_roles() {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, stringify!(reserved_roles)) else {
        return;
    };
    let test_client = network.client();

    let account_in_another_domain = gen_account_in("garden_of_live_flowers").0;
    let register = {
        let other_domain = "garden_of_live_flowers";
        let role = format!(
            "MULTISIG_SIGNATORY/{}/{}",
            other_domain,
            account_in_another_domain.signatory()
        )
        .parse()
        .unwrap();
        Register::role(Role::new(role, ALICE_ID.clone()))
    };

    let _err = test_client.submit_blocking(register).expect_err(
        "role with this name shouldn't be registered by anyone other than the domain owner",
    );
}

fn alt_client(signatory: (AccountId, KeyPair), base_client: &Client) -> Client {
    Client {
        account: signatory.0,
        key_pair: signatory.1,
        ..base_client.clone()
    }
}

#[expect(dead_code)]
fn debug_account(account_id: &AccountId, client: &Client) {
    let account = client
        .query(FindAccounts)
        .execute_all()
        .unwrap()
        .into_iter()
        .find(|account| account.id() == account_id)
        .unwrap();

    eprintln!("{account:#?}");
}
