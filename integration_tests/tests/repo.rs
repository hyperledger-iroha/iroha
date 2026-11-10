//! Integration coverage for repo and reverse-repo instructions.
use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    collections::{Collection, FilterExpr, ListQuery, Page, field},
    data_model::{
        prelude::*,
        repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
    },
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::isi::RepoMarginCallIsi;
use iroha_executor_data_model::permission::settlement::CanExecuteSettlement;
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, BOB_ID, BOB_KEYPAIR};
use std::{
    collections::BTreeSet,
    convert::TryFrom,
    sync::OnceLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
fn install_quiet_tracing() {
    static QUIET_TRACE: OnceLock<()> = OnceLock::new();
    QUIET_TRACE.get_or_init(|| {});
}
fn ivm_build_profile_exists() -> bool {
    iroha_test_samples::ivm_build_profile_path().is_file()
}
fn quiet_network_builder() -> NetworkBuilder {
    install_quiet_tracing();
    init_instruction_registry();
    NetworkBuilder::new()
        .with_auto_populated_trusted_peers()
        .with_peers(4)
}
fn start_test_network(
    context: &'static str,
) -> Result<Option<(sandbox::SerializedNetwork, tokio::runtime::Runtime)>> {
    eyre::ensure!(ivm_build_profile_exists(), "missing IVM build profile");
    sandbox::start_network_blocking_or_skip(quiet_network_builder(), context)
}
fn repo_outsider_key_pair() -> KeyPair {
    KeyPair::try_from_seed(vec![42; 32], Algorithm::Ed25519).expect("fixture repo outsider key")
}
#[test]
fn repo_outsider_fixture_uses_checked_seed_derivation() {
    let key_pair = repo_outsider_key_pair();
    let expected = KeyPair::try_from_seed(vec![42; 32], Algorithm::Ed25519)
        .expect("fixture repo outsider key");
    assert_eq!(key_pair.public_key(), expected.public_key());
}
fn repo_instr_box<T>(instruction: T) -> InstructionBox
where
    RepoInstructionBox: From<T>,
{
    InstructionBox::from(RepoInstructionBox::from(instruction))
}
fn grant_repo_consents(
    base_client: &Client,
    instruction: &RepoIsi,
    collateral_holder: (&AccountId, &KeyPair),
) -> Result<()> {
    let settlement_id = instruction.settlement_id();
    let cash_consent = Permission::from(CanExecuteSettlement {
        debited_asset: AssetId::new(
            instruction.cash_leg().asset_definition_id().clone(),
            instruction.counterparty().clone(),
        ),
        settlement_id: settlement_id.clone(),
        intent_hash: instruction.initiation_intent_hash(),
    });
    let maturity_consent = Permission::from(CanExecuteSettlement {
        debited_asset: AssetId::new(
            instruction.collateral_leg().asset_definition_id().clone(),
            collateral_holder.0.clone(),
        ),
        settlement_id,
        intent_hash: instruction.maturity_intent_hash(),
    });
    let bob_client = alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), base_client);
    bob_client.submit(
        Grant::account_permission(cash_consent, instruction.initiator().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let holder_client = if collateral_holder.0 == &*BOB_ID {
        bob_client
    } else {
        alt_client(
            (collateral_holder.0.clone(), collateral_holder.1.clone()),
            base_client,
        )
    };
    holder_client.submit(
        Grant::account_permission(maturity_consent, instruction.initiator().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    Ok(())
}
fn quoted_transaction(
    client: &Client,
    instructions: impl IntoIterator<Item = InstructionBox>,
    metadata: Metadata,
) -> Result<SignedTransaction> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
        instructions,
        FeePaymentIntent::authority(Vec::new(), None),
        metadata,
    ))?;
    let quote = client
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })?;
    eyre::ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "repo fee quote changed payer or gas bound"
    );
    payload.fee_payment = quote.intent;
    account.sign_transaction(payload).map_err(Into::into)
}
fn fund_runtime_signer(client: &Client, signer: AccountId) -> Result<()> {
    let fee_asset: AssetDefinitionId =
        iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?;
    client.submit(
        Transfer::asset_quantity(AssetId::new(fee_asset, ALICE_ID.clone()), 100u32, signer),
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    Ok(())
}
#[derive(norito::JsonSerialize, norito::JsonDeserialize)]
struct AssetProjection {
    id: AssetId,
    definition_id: AssetDefinitionId,
    account_id: AccountId,
    value: Quantity,
}
fn coherent_assets_from_page(
    page: Page<norito::json::Value>,
    expected: &BTreeSet<AssetId>,
) -> Result<Vec<Asset>> {
    eyre::ensure!(
        page.next_cursor.is_none(),
        "asset snapshot must fit one coherent page"
    );
    eyre::ensure!(
        page.items.len() <= expected.len(),
        "asset snapshot returned excess rows"
    );
    let mut seen = BTreeSet::new();
    page.items
        .into_iter()
        .map(|value| {
            let row: AssetProjection = norito::json::from_value(value)?;
            eyre::ensure!(
                expected.contains(&row.id),
                "asset snapshot returned an unrequested identity"
            );
            eyre::ensure!(
                row.id.definition() == &row.definition_id && row.id.account() == &row.account_id,
                "asset snapshot identity disagrees with its definition or account"
            );
            eyre::ensure!(
                seen.insert(row.id.clone()),
                "asset snapshot repeated an identity"
            );
            Ok(Asset::new(row.id, row.value))
        })
        .collect()
}
fn coherent_assets(
    client: &Client,
    expected: &[(&AssetId, Option<Quantity>)],
) -> Result<Vec<Asset>> {
    let ids = expected
        .iter()
        .map(|(id, _)| (*id).clone())
        .collect::<BTreeSet<_>>();
    eyre::ensure!(
        ids.len() == expected.len() && !ids.is_empty() && ids.len() < 64,
        "asset snapshot requires one to 63 distinct identities"
    );
    let filter = FilterExpr::Or(
        ids.iter()
            .map(|id| field("id").eq(id.to_string()))
            .collect(),
    );
    // Explorer applies the predicate after its bounded native scan. Include the
    // fixture's genesis and fee assets in that scan so all selected balances are
    // observed together; reject any continuation instead of combining snapshots.
    let page = client.list_page(
        &Collection::ExplorerAssets,
        &ListQuery::new().filter(filter).limit(64),
    )?;
    coherent_assets_from_page(page, &ids)
}
#[test]
fn coherent_asset_snapshot_rejects_incomplete_or_inconsistent_pages() -> Result<()> {
    let definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "usd".parse()?,
    );
    let id = AssetId::new(definition.clone(), ALICE_ID.clone());
    let other = AssetId::new(definition.clone(), BOB_ID.clone());
    let expected = BTreeSet::from([id.clone(), other.clone()]);
    let row = norito::json::to_value(&AssetProjection {
        id: id.clone(),
        definition_id: definition.clone(),
        account_id: ALICE_ID.clone(),
        value: Quantity::from(1000u32),
    })?;
    let page = |items, next_cursor| Page {
        items,
        next_cursor,
        total: None,
    };
    let assets = coherent_assets_from_page(page(vec![row.clone()], None), &expected)?;
    assert_eq!(asset_value(&assets, &id), Some(Quantity::from(1000u32)));
    assert_eq!(asset_value(&assets, &other), None);
    assert!(coherent_assets_from_page(page(Vec::new(), None), &expected)?.is_empty());
    assert!(
        coherent_assets_from_page(page(vec![row.clone()], Some("more".into())), &expected).is_err()
    );
    assert!(
        coherent_assets_from_page(page(vec![row.clone(), row.clone()], None), &expected).is_err()
    );
    assert!(
        coherent_assets_from_page(page(vec![row.clone()], None), &BTreeSet::from([other])).is_err()
    );
    let different_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "bond".parse()?,
    );
    for (field, replacement) in [
        (
            "definition_id",
            norito::json::to_value(&different_definition)?,
        ),
        ("account_id", norito::json::to_value(&*BOB_ID)?),
        ("value", norito::json::Value::from("-1")),
        ("value", norito::json::Value::from("invalid quantity")),
    ] {
        let mut malformed = row.clone();
        let norito::json::Value::Object(fields) = &mut malformed else {
            panic!("asset projection must be an object");
        };
        fields.insert(field.into(), replacement);
        assert!(
            coherent_assets_from_page(page(vec![malformed], None), &expected).is_err(),
            "snapshot accepted inconsistent {field}"
        );
    }
    Ok(())
}
// The native agreement collection projects the ledger fields below. It does
// not expose collateral metadata; do not fabricate a complete RepoAgreement.
#[derive(norito::JsonDeserialize)]
struct RepoCollateralProjection {
    asset_definition_id: AssetDefinitionId,
    quantity: Quantity,
}
#[derive(norito::JsonDeserialize)]
struct RepoAgreementProjection {
    id: RepoAgreementId,
    initiator: AccountId,
    counterparty: AccountId,
    custodian: Option<AccountId>,
    cash_leg: RepoCashLeg,
    cash_source: AssetId,
    collateral_leg: RepoCollateralProjection,
    collateral_custody_asset: AssetId,
    rate_bps: u16,
    maturity_timestamp_ms: u64,
    initiated_timestamp_ms: u64,
    last_margin_check_timestamp_ms: u64,
    governance: RepoGovernance,
    settlement_timestamp_ms: Option<u64>,
    status: String,
}
fn repo_agreement_from_page(
    page: Page<norito::json::Value>,
    id: &RepoAgreementId,
) -> Result<Option<RepoAgreementProjection>> {
    eyre::ensure!(
        page.next_cursor.is_none() && page.items.len() <= 1,
        "exact repo agreement query returned excess rows or continuation"
    );
    let Some(row) = page.items.into_iter().next() else {
        return Ok(None);
    };
    let agreement: RepoAgreementProjection = norito::json::from_value(row)?;
    eyre::ensure!(
        &agreement.id == id,
        "repo agreement query returned a different identity"
    );
    eyre::ensure!(
        agreement.initiator == *ALICE_ID && agreement.counterparty == *BOB_ID,
        "repo agreement participants differ from the fixture"
    );
    eyre::ensure!(
        agreement.cash_source.definition() == agreement.cash_leg.asset_definition_id()
            && agreement.cash_source.account() == &agreement.counterparty,
        "repo cash source disagrees with its leg or counterparty"
    );
    eyre::ensure!(
        agreement.collateral_custody_asset.definition()
            == &agreement.collateral_leg.asset_definition_id
            && agreement.collateral_custody_asset.account()
                == agreement
                    .custodian
                    .as_ref()
                    .unwrap_or(&agreement.counterparty),
        "repo collateral custody disagrees with its leg or holder"
    );
    eyre::ensure!(
        *agreement.cash_leg.quantity() == Quantity::from(1000u32)
            && agreement.collateral_leg.quantity == Quantity::from(1100u32)
            && agreement.rate_bps == 0,
        "repo agreement quantities or rate differ from the fixture"
    );
    eyre::ensure!(
        agreement.initiated_timestamp_ms < agreement.maturity_timestamp_ms
            && agreement.last_margin_check_timestamp_ms >= agreement.initiated_timestamp_ms,
        "repo agreement timestamps are inconsistent"
    );
    let expected_status = if agreement.settlement_timestamp_ms.is_some() {
        "settled"
    } else {
        "active"
    };
    eyre::ensure!(
        agreement.status == expected_status,
        "repo agreement status differs from settlement timestamp"
    );
    Ok(Some(agreement))
}
#[test]
fn exact_repo_projection_rejects_excess_or_inconsistent_rows() -> Result<()> {
    let id: RepoAgreementId = "snapshot_repo".parse()?;
    let domain = DomainId::try_new("wonderland", "universal")?;
    let cash = AssetDefinitionId::derive_from_components(domain.clone(), "usd".parse()?);
    let collateral = AssetDefinitionId::derive_from_components(domain, "bond".parse()?);
    let row = norito::json!({
        "id": (id.to_string()),
        "initiator": (ALICE_ID.to_string()),
        "counterparty": (BOB_ID.to_string()),
        "custodian": null,
        "cash_leg": {"asset_definition_id": (cash.to_string()), "quantity": "1000"},
        "cash_source": (AssetId::new(cash, BOB_ID.clone()).to_string()),
        "collateral_leg": {"asset_definition_id": (collateral.to_string()), "quantity": "1100"},
        "collateral_custody_asset": (AssetId::new(collateral, BOB_ID.clone()).to_string()),
        "rate_bps": 0,
        "maturity_timestamp_ms": 2000,
        "initiated_timestamp_ms": 1000,
        "last_margin_check_timestamp_ms": 1000,
        "governance": {"haircut_bps": 1500, "margin_frequency_secs": 300, "allow_early_repayment": false},
        "settlement_timestamp_ms": null,
        "status": "active"
    });
    let page = |items, next_cursor| Page {
        items,
        next_cursor,
        total: None,
    };
    assert!(repo_agreement_from_page(page(Vec::new(), None), &id)?.is_none());
    let agreement = repo_agreement_from_page(page(vec![row.clone()], None), &id)?
        .expect("complete agreement projection");
    assert_eq!(agreement.id, id);
    assert_eq!(agreement.governance.margin_frequency_secs, 300);
    assert!(repo_agreement_from_page(page(vec![row.clone()], Some("more".into())), &id).is_err());
    assert!(repo_agreement_from_page(page(vec![row.clone(), row.clone()], None), &id).is_err());
    for (field, replacement) in [
        ("id", norito::json::Value::from("different_repo")),
        ("initiator", norito::json::Value::from(BOB_ID.to_string())),
        ("status", norito::json::Value::from("settled")),
        ("maturity_timestamp_ms", norito::json::Value::from(999u64)),
    ] {
        let mut malformed = row.clone();
        let norito::json::Value::Object(fields) = &mut malformed else {
            panic!("repo projection must be an object");
        };
        fields.insert(field.into(), replacement);
        assert!(
            repo_agreement_from_page(page(vec![malformed], None), &id).is_err(),
            "agreement query accepted inconsistent {field}"
        );
    }
    Ok(())
}
fn error_chain_contains(err: &eyre::Report, needle: &str) -> bool {
    err.chain().any(|cause| cause.to_string().contains(needle))
}
fn asset_value(assets: &[Asset], asset_id: &AssetId) -> Option<Quantity> {
    assets
        .iter()
        .find(|asset| asset.id() == asset_id)
        .map(|asset| asset.value().clone())
}
fn wait_for_assets(
    client: &Client,
    expected: &[(&AssetId, Option<Quantity>)],
    context: &str,
) -> Result<Vec<Asset>> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);
    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "assets were not queried".to_owned();
    while Instant::now() < deadline {
        match coherent_assets(client, expected) {
            Ok(assets) => {
                let all_match = expected.iter().all(|(asset_id, expected_value)| {
                    asset_value(&assets, asset_id).as_ref() == expected_value.as_ref()
                });
                last_observed = expected
                    .iter()
                    .map(|(asset_id, _)| format!("{asset_id}={:?}", asset_value(&assets, asset_id)))
                    .collect::<Vec<_>>()
                    .join(", ");
                if all_match {
                    return Ok(assets);
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "timed out waiting for assets after {context}; last_observed={last_observed}"
    ))
}
fn wait_for_repo_agreement(
    client: &Client,
    agreement_id: &RepoAgreementId,
    context: &str,
) -> Result<RepoAgreementProjection> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);
    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "repo agreements were not queried".to_owned();
    while Instant::now() < deadline {
        let query = ListQuery::new()
            .filter(field("id").eq(agreement_id.to_string()))
            .limit(2);
        match client
            .list_page(&Collection::RepoAgreements, &query)
            .map_err(Into::<eyre::Report>::into)
            .and_then(|page| repo_agreement_from_page(page, agreement_id))
        {
            Ok(Some(agreement)) => return Ok(agreement),
            Ok(None) => "agreement not present".clone_into(&mut last_observed),
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "timed out waiting for repo agreement after {context}; id={agreement_id}; last_observed={last_observed}"
    ))
}
#[test]
#[allow(clippy::too_many_lines)]
fn repo_roundtrip_transfers_balances_and_seals_agreement() -> Result<()> {
    let Some((network, _rt)) = start_test_network(stringify!(
        repo_roundtrip_transfers_balances_and_seals_agreement
    ))?
    else {
        return Ok(());
    };
    let client = network.client();
    let metadata = Metadata::default();
    let cash_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "usd".parse()?,
    );
    let collateral_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "bond".parse()?,
    );
    // Prepare cash/collateral assets and balances.
    let setup_instructions: Vec<InstructionBox> = vec![
        Register::asset_definition({
            let __asset_definition_id = cash_def_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(DomainId::try_new("wonderland", "universal")?),
            )
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = collateral_def_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "bond".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(DomainId::try_new("wonderland", "universal")?),
            )
        })
        .into(),
        Mint::asset_quantity(
            Quantity::from(2000_u64),
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            Quantity::from(1500_u64),
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
        )
        .into(),
    ];
    let setup_tx = quoted_transaction(&client, setup_instructions, metadata.clone())?;
    client.submit_transaction_and_wait(&setup_tx)?;
    // Initiate the repo: Alice borrows cash, pledging collateral.
    let agreement_id: RepoAgreementId = "daily_repo".parse()?;
    let repo_instruction_template = || -> Result<RepoIsi> {
        // Both participant consents and the early-repayment rejection must commit
        // before this absolute maturity; those are separate consensus blocks.
        let maturity_ms = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .checked_add(Duration::from_secs(60))
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
                quantity: Quantity::from(1000_u64),
            },
            RepoCollateralLeg::new(collateral_def_id.clone(), Quantity::from(1100_u64)),
            0,
            maturity_ms,
            RepoGovernance::new(1_500, 86_400, false),
        ))
    };
    let repo_instruction = repo_instruction_template()?;
    grant_repo_consents(&client, &repo_instruction, (&BOB_ID, &BOB_KEYPAIR))?;
    let repo_instruction_box = repo_instr_box(repo_instruction);
    let repo_tx = quoted_transaction(&client, vec![repo_instruction_box], metadata.clone())?;
    client.submit_transaction_and_wait(&repo_tx)?;
    // Ensure duplicate agreement IDs are rejected while active.
    let duplicate_repo = repo_instruction_template()?;
    let duplicate_instruction_box = repo_instr_box(duplicate_repo);
    let duplicate_tx =
        quoted_transaction(&client, vec![duplicate_instruction_box], metadata.clone())?;
    let duplicate_err = client
        .submit_transaction_and_wait(&duplicate_tx)
        .unwrap_err();
    assert!(
        error_chain_contains(&duplicate_err, "already exists"),
        "expected duplicate repo to be rejected, got {duplicate_err:?}"
    );
    let alice_cash_id = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
    let bob_cash_id = AssetId::new(cash_def_id.clone(), BOB_ID.clone());
    let alice_collateral_id = AssetId::new(collateral_def_id.clone(), ALICE_ID.clone());
    let bob_collateral_id = AssetId::new(collateral_def_id.clone(), BOB_ID.clone());
    let assets_after_repo = wait_for_assets(
        &client,
        &[
            (&alice_cash_id, Some(Quantity::from(1000_u64))),
            (&bob_cash_id, Some(Quantity::from(1000_u64))),
            (&alice_collateral_id, Some(Quantity::from(400_u64))),
            (&bob_collateral_id, Some(Quantity::from(1100_u64))),
        ],
        "repo initiation",
    )?;
    let alice_cash = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &alice_cash_id)
        .expect("alice cash after repo");
    assert_eq!(*alice_cash.value(), Quantity::from(1000_u64));
    let bob_cash = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &bob_cash_id)
        .expect("bob cash after repo");
    assert_eq!(*bob_cash.value(), Quantity::from(1000_u64));
    let alice_collateral = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &alice_collateral_id)
        .expect("alice collateral after repo");
    assert_eq!(*alice_collateral.value(), Quantity::from(400_u64));
    let bob_collateral = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &bob_collateral_id)
        .expect("bob collateral after repo");
    assert_eq!(*bob_collateral.value(), Quantity::from(1100_u64));
    let repo_snapshot = wait_for_repo_agreement(&client, &agreement_id, "repo maturity lookup")?;
    let future_reverse = ReverseRepoIsi::new(agreement_id.clone());
    let future_tx = quoted_transaction(
        &client,
        vec![repo_instr_box(future_reverse)],
        metadata.clone(),
    )?;
    let future_err = client.submit_transaction_and_wait(&future_tx).unwrap_err();
    assert!(
        error_chain_contains(&future_err, "before its recorded maturity"),
        "expected pre-maturity repo settlement to be rejected, got {future_err:?}"
    );
    let now_ms = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
        .expect("current timestamp fits in u64");
    if repo_snapshot.maturity_timestamp_ms >= now_ms {
        std::thread::sleep(Duration::from_millis(
            repo_snapshot
                .maturity_timestamp_ms
                .saturating_sub(now_ms)
                .saturating_add(500),
        ));
    }
    let reverse_repo_instruction = ReverseRepoIsi::new(agreement_id.clone());
    let counterparty_client = alt_client((BOB_ID.clone(), BOB_KEYPAIR.clone()), &client);
    let reverse_tx = quoted_transaction(
        &counterparty_client,
        vec![repo_instr_box(reverse_repo_instruction)],
        metadata.clone(),
    )?;
    counterparty_client.submit_transaction_and_wait(&reverse_tx)?;
    let assets_after_reverse = wait_for_assets(
        &client,
        &[
            (&alice_cash_id, None),
            (&bob_cash_id, Some(Quantity::from(2000_u64))),
            (&alice_collateral_id, Some(Quantity::from(1500_u64))),
            (&bob_collateral_id, None),
        ],
        "reverse repo",
    )?;
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
    assert_eq!(*bob_cash_post.value(), Quantity::from(2000_u64));
    let alice_collateral_post = assets_after_reverse
        .iter()
        .find(|asset| asset.id() == &alice_collateral_id)
        .expect("alice collateral after reverse repo");
    assert_eq!(*alice_collateral_post.value(), Quantity::from(1500_u64));
    let bob_collateral_post = assets_after_reverse
        .iter()
        .find(|asset| asset.id() == &bob_collateral_id);
    assert!(
        bob_collateral_post.is_none(),
        "bob collateral asset should be pruned on unwind"
    );
    // A settled identifier remains on-chain and cannot be reused.
    let reopened_repo = RepoIsi::new(
        agreement_id.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        None,
        RepoCashLeg {
            asset_definition_id: cash_def_id.clone(),
            quantity: Quantity::from(1000_u64),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), Quantity::from(1100_u64)),
        0,
        u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .checked_add(Duration::from_secs(60))
                .expect("maturity arithmetic")
                .as_millis(),
        )
        .expect("reverse maturity timestamp fits in u64"),
        RepoGovernance::new(1_500, 3, false),
    );
    grant_repo_consents(&client, &reopened_repo, (&BOB_ID, &BOB_KEYPAIR))?;
    let reopened_tx = quoted_transaction(
        &client,
        vec![repo_instr_box(reopened_repo)],
        metadata.clone(),
    )?;
    let reopened_err = client
        .submit_transaction_and_wait(&reopened_tx)
        .expect_err("settled repo identifier must remain consumed");
    assert!(
        error_chain_contains(&reopened_err, "already exists"),
        "expected one-shot identifier rejection, got {reopened_err:?}"
    );
    let settled = wait_for_repo_agreement(&client, &agreement_id, "settled tombstone")?;
    assert_eq!(
        &settled.settlement_timestamp_ms,
        &Some(settled.maturity_timestamp_ms)
    );
    Ok(())
}
#[test]
#[allow(clippy::too_many_lines)]
fn repo_margin_call_enforces_cadence_and_participant_rules() -> Result<()> {
    let Some((network, _rt)) = start_test_network(stringify!(
        repo_margin_call_enforces_cadence_and_participant_rules
    ))?
    else {
        return Ok(());
    };
    let client = network.client();
    let metadata = Metadata::default();
    let cash_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "usd".parse()?,
    );
    let collateral_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "bond".parse()?,
    );
    let outsider_keypair = repo_outsider_key_pair();
    let outsider_id = AccountId::new(outsider_keypair.public_key().clone());
    let setup_instructions: Vec<InstructionBox> = vec![
        Register::account(Account::new(outsider_id.clone())).into(),
        Register::asset_definition({
            let __asset_definition_id = cash_def_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(DomainId::try_new("wonderland", "universal")?),
            )
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = collateral_def_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "bond".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(DomainId::try_new("wonderland", "universal")?),
            )
        })
        .into(),
        Mint::asset_quantity(
            Quantity::from(2000_u64),
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            Quantity::from(1500_u64),
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
        )
        .into(),
    ];
    let setup_tx = quoted_transaction(&client, setup_instructions, metadata.clone())?;
    client.submit_transaction_and_wait(&setup_tx)?;
    fund_runtime_signer(&client, outsider_id.clone())?;
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
            quantity: Quantity::from(1000_u64),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), Quantity::from(1100_u64)),
        0,
        maturity_ms,
        // Use a generous margin cadence to avoid wall-clock flakiness in the
        // integration test network.
        RepoGovernance::new(1_500, 300, false),
    );
    grant_repo_consents(&client, &repo_instruction, (&BOB_ID, &BOB_KEYPAIR))?;
    let repo_tx = quoted_transaction(
        &client,
        vec![repo_instr_box(repo_instruction)],
        metadata.clone(),
    )?;
    client.submit_transaction_and_wait(&repo_tx)?;
    let repo_snapshot = wait_for_repo_agreement(&client, &agreement_id, "margin repo initiation")?;
    let initial_last_margin_ms = repo_snapshot.last_margin_check_timestamp_ms;
    let expected_first_due = initial_last_margin_ms
        .checked_add(
            repo_snapshot
                .governance
                .margin_frequency_millis()
                .expect("margin cadence configured"),
        )
        .expect("margin cadence timestamp fits u64");
    assert!(
        expected_first_due > initial_last_margin_ms,
        "margin cadence should advance beyond the initiation timestamp"
    );
    let premature_err = client
        .submit_transaction_and_wait(&quoted_transaction(
            &client,
            vec![repo_instr_box(RepoMarginCallIsi::new(agreement_id.clone()))],
            metadata.clone(),
        )?)
        .unwrap_err();
    assert!(
        error_chain_contains(&premature_err, "margin check is not yet due"),
        "expected cadence enforcement error, got {premature_err:?}"
    );
    let after_reject = wait_for_repo_agreement(&client, &agreement_id, "rejected margin call")?;
    assert_eq!(
        after_reject.last_margin_check_timestamp_ms, initial_last_margin_ms,
        "rejected margin call should not advance the schedule"
    );
    let unauthorized_client = alt_client((outsider_id.clone(), outsider_keypair), &client);
    let unauthorized_err = unauthorized_client
        .submit_transaction_and_wait(&quoted_transaction(
            &unauthorized_client,
            vec![repo_instr_box(RepoMarginCallIsi::new(agreement_id))],
            metadata,
        )?)
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
    let Some((network, _rt)) =
        start_test_network(stringify!(repo_roundtrip_with_custodian_routes_collateral))?
    else {
        return Ok(());
    };
    let client = network.client();
    let metadata = Metadata::default();
    let custodian_keypair = KeyPair::random();
    let custodian_id = AccountId::new(custodian_keypair.public_key().clone());
    let cash_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "usd".parse()?,
    );
    let collateral_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "bond".parse()?,
    );
    let setup_instructions: Vec<InstructionBox> = vec![
        Register::account(Account::new(custodian_id.clone())).into(),
        Register::asset_definition({
            let __asset_definition_id = cash_def_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(DomainId::try_new("wonderland", "universal")?),
            )
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = collateral_def_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "bond".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(DomainId::try_new("wonderland", "universal")?),
            )
        })
        .into(),
        Mint::asset_quantity(
            Quantity::from(2000_u64),
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            Quantity::from(1500_u64),
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
        )
        .into(),
    ];
    let setup_tx = quoted_transaction(&client, setup_instructions, metadata.clone())?;
    client.submit_transaction_and_wait(&setup_tx)?;
    fund_runtime_signer(&client, custodian_id.clone())?;
    let agreement_id: RepoAgreementId = "tri_party_repo".parse()?;
    // Reserve time for both separately signed consents before repo initiation.
    let maturity_timestamp_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .checked_add(Duration::from_secs(60))
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
            quantity: Quantity::from(1000_u64),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), Quantity::from(1100_u64)),
        0,
        maturity_timestamp_ms,
        RepoGovernance::new(1_500, 3, false),
    );
    grant_repo_consents(
        &client,
        &repo_instruction,
        (&custodian_id, &custodian_keypair),
    )?;
    let repo_tx = quoted_transaction(
        &client,
        vec![repo_instr_box(repo_instruction)],
        metadata.clone(),
    )?;
    client.submit_transaction_and_wait(&repo_tx)?;
    let alice_cash_id = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
    let bob_cash_id = AssetId::new(cash_def_id.clone(), BOB_ID.clone());
    let alice_collateral_id = AssetId::new(collateral_def_id.clone(), ALICE_ID.clone());
    let custodian_collateral_id = AssetId::new(collateral_def_id.clone(), custodian_id.clone());
    let bob_collateral_id = AssetId::new(collateral_def_id.clone(), BOB_ID.clone());
    let assets_after_repo = wait_for_assets(
        &client,
        &[
            (&alice_cash_id, Some(Quantity::from(1000_u64))),
            (&bob_cash_id, Some(Quantity::from(1000_u64))),
            (&alice_collateral_id, Some(Quantity::from(400_u64))),
            (&bob_collateral_id, None),
            (&custodian_collateral_id, Some(Quantity::from(1100_u64))),
        ],
        "tri-party repo initiation",
    )?;
    let alice_cash = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &alice_cash_id)
        .expect("alice cash after repo");
    assert_eq!(*alice_cash.value(), Quantity::from(1000_u64));
    let bob_cash = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &bob_cash_id)
        .expect("bob cash after repo");
    assert_eq!(*bob_cash.value(), Quantity::from(1000_u64));
    let alice_collateral = assets_after_repo
        .iter()
        .find(|asset| asset.id() == &alice_collateral_id)
        .expect("alice collateral after repo");
    assert_eq!(*alice_collateral.value(), Quantity::from(400_u64));
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
    assert_eq!(*custodian_collateral.value(), Quantity::from(1100_u64));
    let stored_agreement =
        wait_for_repo_agreement(&client, &agreement_id, "tri-party repo initiation")?;
    assert_eq!(
        &stored_agreement.custodian,
        &Some(custodian_id.clone()),
        "custodian id should be persisted in repo agreement"
    );
    let now_ms = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
        .expect("current timestamp fits in u64");
    if stored_agreement.maturity_timestamp_ms >= now_ms {
        std::thread::sleep(Duration::from_millis(
            stored_agreement
                .maturity_timestamp_ms
                .saturating_sub(now_ms)
                .saturating_add(500),
        ));
    }
    let reverse_repo_instruction = ReverseRepoIsi::new(agreement_id.clone());
    let custodian_client = alt_client((custodian_id.clone(), custodian_keypair), &client);
    let reverse_tx = quoted_transaction(
        &custodian_client,
        vec![repo_instr_box(reverse_repo_instruction)],
        metadata.clone(),
    )?;
    custodian_client.submit_transaction_and_wait(&reverse_tx)?;
    let assets_after_reverse = wait_for_assets(
        &client,
        &[
            (&alice_cash_id, None),
            (&bob_cash_id, Some(Quantity::from(2000_u64))),
            (&alice_collateral_id, Some(Quantity::from(1500_u64))),
            (&custodian_collateral_id, None),
        ],
        "tri-party reverse repo",
    )?;
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
    assert_eq!(*bob_cash_after_reverse.value(), Quantity::from(2000_u64));
    let alice_collateral_after_reverse = assets_after_reverse
        .iter()
        .find(|asset| asset.id() == &alice_collateral_id)
        .expect("alice collateral after reverse");
    assert_eq!(
        *alice_collateral_after_reverse.value(),
        Quantity::from(1500_u64)
    );
    assert!(
        !assets_after_reverse
            .iter()
            .any(|asset| asset.id() == &custodian_collateral_id),
        "custodian collateral asset should be pruned after unwind"
    );
    let settled = wait_for_repo_agreement(&client, &agreement_id, "tri-party reverse repo")?;
    assert_eq!(
        &settled.settlement_timestamp_ms,
        &Some(settled.maturity_timestamp_ms)
    );
    Ok(())
}
fn alt_client(signatory: (AccountId, KeyPair), base_client: &Client) -> Client {
    integration_tests::sync::rebind_blocking_client(base_client, |client| {
        client.account = signatory.0;
        client.key_pair = signatory.1;
    })
}
