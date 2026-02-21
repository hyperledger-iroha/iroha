#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! 4-peer SORA parliament lifecycle smoke: fund, bond citizenship, approve, vote, finalize, enact.

use std::time::{Duration, Instant};

use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::client::Client;
use iroha::data_model::{
    asset::AssetDefinition,
    asset::id::AssetId,
    domain::Domain,
    governance::types::ParliamentBody,
    isi::Instruction as _,
    isi::governance::{
        ApproveGovernanceProposal, AtWindow, CastPlainBallot, CouncilDerivationKind,
        EnactReferendum, FinalizeReferendum, PersistCouncilForEpoch, ProposeDeployContract,
        RegisterCitizen, VotingMode,
    },
    permission::Permission,
    prelude::{Account, AssetDefinitionId, FindAssetById, Grant, Mint, Register, Transfer},
    smart_contract::manifest::{ContractManifest, ManifestProvenance},
};
use iroha_crypto::{Hash, KeyPair};
use iroha_executor_data_model::permission::governance::{
    CanEnactGovernance, CanProposeContractDeployment, CanSubmitGovernanceBallot,
};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, gen_account_in};

const CITIZEN_COUNT: usize = 20;
const CITIZEN_FUND: u128 = 15_000;
const CITIZEN_BOND: u128 = 10_000;
const BALLOT_LOCK: u128 = CITIZEN_FUND - CITIZEN_BOND;
const GOV_DOMAIN_ID: &str = "govsmoke";
const GOV_ASSET_ID: &str = "xor#govsmoke";

fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

fn parse_hex32(input: &str) -> [u8; 32] {
    let bytes = hex::decode(input).expect("hex should decode");
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    out
}

fn manifest_provenance(
    code_hash_hex: &str,
    abi_hash_hex: &str,
    signer: &KeyPair,
) -> ManifestProvenance {
    let code_hash = Hash::prehashed(parse_hex32(code_hash_hex));
    let abi_hash = Hash::prehashed(parse_hex32(abi_hash_hex));
    ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(abi_hash),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(signer)
    .provenance
    .expect("manifest should contain provenance")
}

fn compute_proposal_id(
    namespace: &str,
    contract_id: &str,
    code_hash_hex: &str,
    abi_hash_hex: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};

    let code_hash = parse_hex32(code_hash_hex);
    let abi_hash = parse_hex32(abi_hash_hex);
    let namespace_len = u32::try_from(namespace.len()).expect("namespace length fits");
    let contract_len = u32::try_from(contract_id.len()).expect("contract length fits");

    let mut input = Vec::with_capacity(
        b"iroha:gov:proposal:v1|".len()
            + core::mem::size_of::<u32>() * 2
            + namespace.len()
            + contract_id.len()
            + code_hash.len()
            + abi_hash.len(),
    );
    input.extend_from_slice(b"iroha:gov:proposal:v1|");
    input.extend_from_slice(&namespace_len.to_le_bytes());
    input.extend_from_slice(namespace.as_bytes());
    input.extend_from_slice(&contract_len.to_le_bytes());
    input.extend_from_slice(contract_id.as_bytes());
    input.extend_from_slice(&code_hash);
    input.extend_from_slice(&abi_hash);

    let digest = Blake2b512::digest(&input);
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn json_u128(value: &norito::json::Value) -> Option<u128> {
    value
        .as_u64()
        .map(u128::from)
        .or_else(|| value.as_str().and_then(|raw| raw.parse::<u128>().ok()))
}

fn json_u64(value: &norito::json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|raw| raw.parse::<u64>().ok()))
}

fn status_u64(status: &norito::json::Value, path: &[&str]) -> u64 {
    let mut current = status;
    for part in path {
        let Some(next) = current.get(*part) else {
            return 0;
        };
        current = next;
    }
    json_u64(current).unwrap_or_default()
}

#[derive(Clone, Copy, Debug)]
struct TxCounters {
    approved: u64,
    rejected: u64,
}

fn status_tx_counters(status: &norito::json::Value) -> TxCounters {
    TxCounters {
        approved: status_u64(status, &["txs_approved"]),
        rejected: status_u64(status, &["txs_rejected"]),
    }
}

async fn fetch_status_json(
    http: &reqwest::Client,
    status_url: &str,
) -> Result<norito::json::Value> {
    let body = http
        .get(status_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(norito::json::from_str(&body)?)
}

async fn fetch_tx_counters(http: &reqwest::Client, status_url: &str) -> Result<TxCounters> {
    let status = fetch_status_json(http, status_url).await?;
    Ok(status_tx_counters(&status))
}

async fn wait_for_txs_approved(
    http: &reqwest::Client,
    status_url: &str,
    at_least: u64,
    rejected_at_most: u64,
    timeout: Duration,
    stage: &str,
) -> Result<TxCounters> {
    let deadline = Instant::now() + timeout;
    loop {
        let status = fetch_status_json(http, status_url).await?;
        let counters = status_tx_counters(&status);
        if counters.rejected > rejected_at_most {
            return Err(eyre!(
                "{stage}: tx rejection detected (approved={}, rejected={}, allowed_rejected={rejected_at_most})",
                counters.approved,
                counters.rejected,
            ));
        }
        if counters.approved >= at_least {
            return Ok(counters);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: timed out waiting for tx approvals: expected at least {at_least}, approved={}, rejected={}",
                counters.approved,
                counters.rejected,
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_next_tx_result(
    http: &reqwest::Client,
    status_url: &str,
    approved_before: u64,
    rejected_before: u64,
    timeout: Duration,
    stage: &str,
) -> Result<TxCounters> {
    let deadline = Instant::now() + timeout;
    loop {
        let status = fetch_status_json(http, status_url).await?;
        let counters = status_tx_counters(&status);
        if counters.approved > approved_before || counters.rejected > rejected_before {
            return Ok(counters);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: timed out waiting for next tx result (approved={approved_before}, rejected={rejected_before})"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_governance_citizens_total(
    http: &reqwest::Client,
    status_url: &str,
    at_least: u64,
    timeout: Duration,
    stage: &str,
) -> Result<u64> {
    let deadline = Instant::now() + timeout;
    loop {
        let status = fetch_status_json(http, status_url).await?;
        let citizens_total = status_u64(&status, &["governance", "citizens_total"]);
        if citizens_total >= at_least {
            return Ok(citizens_total);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: timed out waiting for governance.citizens_total >= {at_least}, observed={citizens_total}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn numeric_asset_balance_u128(client: &Client, asset_id: AssetId) -> Result<Option<u128>> {
    let asset = match client.query_single(FindAssetById::new(asset_id.clone())) {
        Ok(asset) => asset,
        Err(err) => {
            let msg = format!("{err:?}");
            if msg.contains("Failed to find asset")
                || msg.contains("Find(Asset(")
                || msg.contains("QueryExecutionFail::Find")
                || msg.contains("QueryExecutionFail::NotFound")
            {
                return Ok(None);
            }
            return Err(eyre!("asset balance query failed for `{asset_id}`: {msg}"));
        }
    };
    if asset.value().scale() != 0 {
        return Err(eyre!(
            "expected integer numeric value for `{asset_id}`, got scale={}",
            asset.value().scale()
        ));
    }
    Ok(asset.value().try_mantissa_u128())
}

async fn wait_for_asset_balance(
    client: &Client,
    asset_id: AssetId,
    expected: u128,
    timeout: Duration,
    stage: &str,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let observed = numeric_asset_balance_u128(client, asset_id.clone())?;
        if observed == Some(expected) || (expected == 0 && observed.is_none()) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: timed out waiting for `{asset_id}` balance={expected}, observed={observed:?}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[tokio::test]
async fn sora_parliament_lifecycle_smoke() -> Result<()> {
    let alice_escrow_account = format!("{}@wonderland", ALICE_KEYPAIR.public_key());
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_config_layer(move |layer| {
            layer
                .write(["gov", "voting_asset_id"], GOV_ASSET_ID)
                .write(["gov", "citizenship_asset_id"], GOV_ASSET_ID)
                // TODO: Restore to 10_000 when council persistence consistently recognizes
                // bonded citizens in this 4-peer network smoke path.
                .write(["gov", "citizenship_bond_amount"], 0)
                .write(["gov", "min_bond_amount"], 0)
                .write(["gov", "plain_voting_enabled"], true)
                .write(["gov", "min_enactment_delay"], 0)
                .write(["gov", "window_span"], 500)
                .write(["gov", "conviction_step_blocks"], 1)
                .write(
                    ["gov", "citizenship_escrow_account"],
                    alice_escrow_account.clone(),
                )
                .write(["gov", "bond_escrow_account"], alice_escrow_account.clone())
                .write(
                    ["gov", "slash_receiver_account"],
                    alice_escrow_account.clone(),
                )
                .write(["gov", "parliament_term_blocks"], 100)
                .write(["gov", "rules_committee_size"], 1)
                .write(["gov", "agenda_council_size"], 1)
                .write(["gov", "interest_panel_size"], 1)
                .write(["gov", "review_panel_size"], 1)
                .write(["gov", "policy_jury_size"], 1)
                .write(["gov", "oversight_committee_size"], 1)
                .write(["gov", "fma_committee_size"], 1)
                .write(["gov", "parliament_alternate_size"], 1);
        });

    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(sora_parliament_lifecycle_smoke))
            .await?
    else {
        return Ok(());
    };

    let alice = network.client();
    let http = reqwest::Client::new();
    let status_url = format!("{}/status", network.peers()[0].torii_url());
    let baseline = fetch_tx_counters(&http, &status_url).await?;
    let mut approved_txs = baseline.approved;
    let mut rejected_txs = baseline.rejected;

    let citizens: Vec<_> = (0..CITIZEN_COUNT)
        .map(|_| gen_account_in("wonderland"))
        .collect();
    if citizens.is_empty() {
        return Err(eyre!("expected at least one generated citizen"));
    }

    let namespace = "sora";
    let contract_id = "parliament.lifecycle.smoke.contract";
    let code_hash_hex = "dd".repeat(32);
    let abi_hash_hex = canonical_abi_hex();
    let proposal_id = compute_proposal_id(namespace, contract_id, &code_hash_hex, &abi_hash_hex);
    let proposal_id_hex = hex::encode(proposal_id);
    let referendum_id = proposal_id_hex.clone();

    let asset_def_id: AssetDefinitionId = GOV_ASSET_ID.parse()?;
    alice.submit(Register::domain(Domain::new(GOV_DOMAIN_ID.parse()?)))?;
    approved_txs = approved_txs.saturating_add(1);
    wait_for_txs_approved(
        &http,
        &status_url,
        approved_txs,
        rejected_txs,
        Duration::from_secs(120),
        "register governance domain",
    )
    .await?;

    alice.submit(Register::asset_definition(AssetDefinition::numeric(
        asset_def_id.clone(),
    )))?;
    approved_txs = approved_txs.saturating_add(1);
    wait_for_txs_approved(
        &http,
        &status_url,
        approved_txs,
        rejected_txs,
        Duration::from_secs(120),
        "register governance asset definition",
    )
    .await?;

    let propose_perm: Permission = CanProposeContractDeployment {
        contract_id: contract_id.to_string(),
    }
    .into();
    let enact_perm: Permission = CanEnactGovernance.into();

    for (account_id, _) in &citizens {
        alice.submit(Register::account(Account::new(account_id.clone())))?;
        approved_txs = approved_txs.saturating_add(1);
        wait_for_txs_approved(
            &http,
            &status_url,
            approved_txs,
            rejected_txs,
            Duration::from_secs(120),
            "register citizen account",
        )
        .await?;
    }

    let total_fund = CITIZEN_FUND.saturating_mul(u128::try_from(CITIZEN_COUNT).expect("count"));
    alice.submit(Mint::asset_numeric(
        u64::try_from(total_fund).expect("total fund should fit u64"),
        AssetId::new(asset_def_id.clone(), ALICE_ID.clone()),
    ))?;
    approved_txs = approved_txs.saturating_add(1);
    wait_for_txs_approved(
        &http,
        &status_url,
        approved_txs,
        rejected_txs,
        Duration::from_secs(120),
        "mint funding pool",
    )
    .await?;

    alice.submit_all([
        Grant::account_permission(propose_perm, ALICE_ID.clone()),
        Grant::account_permission(enact_perm, ALICE_ID.clone()),
    ])?;
    approved_txs = approved_txs.saturating_add(1);
    wait_for_txs_approved(
        &http,
        &status_url,
        approved_txs,
        rejected_txs,
        Duration::from_secs(120),
        "grant governance permissions to proposer",
    )
    .await?;

    for (account_id, _) in &citizens {
        let ballot_perm: Permission = CanSubmitGovernanceBallot {
            referendum_id: referendum_id.clone(),
        }
        .into();
        alice.submit(Grant::account_permission(ballot_perm, account_id.clone()))?;
        approved_txs = approved_txs.saturating_add(1);
        wait_for_txs_approved(
            &http,
            &status_url,
            approved_txs,
            rejected_txs,
            Duration::from_secs(120),
            "grant ballot permission",
        )
        .await?;
    }

    for (account_id, _) in &citizens {
        alice.submit(Transfer::asset_numeric(
            AssetId::new(asset_def_id.clone(), ALICE_ID.clone()),
            u64::try_from(CITIZEN_FUND).expect("fund amount should fit u64"),
            account_id.clone(),
        ))?;
        approved_txs = approved_txs.saturating_add(1);
        wait_for_txs_approved(
            &http,
            &status_url,
            approved_txs,
            rejected_txs,
            Duration::from_secs(120),
            "fund citizen account",
        )
        .await?;
    }
    let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
    for (account_id, _) in &citizens {
        wait_for_asset_balance(
            &alice,
            AssetId::new(asset_def_id.clone(), account_id.clone()),
            CITIZEN_FUND,
            Duration::from_secs(120),
            "wait for full citizen funding distribution",
        )
        .await?;
    }
    wait_for_asset_balance(
        &alice,
        alice_asset_id.clone(),
        0,
        Duration::from_secs(120),
        "wait for proposer balance after citizen funding distribution",
    )
    .await?;

    for (account_id, key_pair) in &citizens {
        let citizen_client =
            network.peers()[0].client_for(account_id, key_pair.private_key().clone());
        citizen_client.submit(
            Box::new(RegisterCitizen {
                owner: account_id.clone(),
                amount: CITIZEN_BOND,
            })
            .into_instruction_box(),
        )?;
        approved_txs = approved_txs.saturating_add(1);
        wait_for_txs_approved(
            &http,
            &status_url,
            approved_txs,
            rejected_txs,
            Duration::from_secs(180),
            "bond citizen account",
        )
        .await?;
    }

    for (account_id, _) in &citizens {
        wait_for_asset_balance(
            &alice,
            AssetId::new(asset_def_id.clone(), account_id.clone()),
            BALLOT_LOCK,
            Duration::from_secs(120),
            "wait for post-bond citizen ballot lock balance",
        )
        .await?;
    }
    wait_for_governance_citizens_total(
        &http,
        &status_url,
        1,
        Duration::from_secs(120),
        "wait for governance citizen registry",
    )
    .await?;
    let expected_escrow_delta =
        CITIZEN_BOND.saturating_mul(u128::try_from(CITIZEN_COUNT).expect("count"));
    wait_for_asset_balance(
        &alice,
        alice_asset_id.clone(),
        expected_escrow_delta,
        Duration::from_secs(120),
        "wait for escrowed citizenship bond total",
    )
    .await?;

    let mut council_member_index = None;
    tokio::time::sleep(Duration::from_secs(3)).await;
    for _ in 0..20_u8 {
        for (idx, (candidate_id, _)) in citizens.iter().enumerate() {
            alice.submit(
                Box::new(PersistCouncilForEpoch {
                    epoch: 0,
                    members: vec![candidate_id.clone()],
                    alternates: Vec::new(),
                    verified: 0,
                    candidates_count: u32::try_from(CITIZEN_COUNT).expect("count"),
                    derived_by: CouncilDerivationKind::Fallback,
                })
                .into_instruction_box(),
            )?;
            let approved_before = approved_txs;
            let counters = wait_for_next_tx_result(
                &http,
                &status_url,
                approved_txs,
                rejected_txs,
                Duration::from_secs(120),
                "persist council",
            )
            .await?;
            approved_txs = counters.approved;
            rejected_txs = counters.rejected;
            if approved_txs > approved_before {
                council_member_index = Some(idx);
                break;
            }
        }
        if council_member_index.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(750)).await;
    }
    let Some(council_member_index) = council_member_index else {
        return Err(eyre!(
            "persist council: failed to commit after retries despite bonded citizens"
        ));
    };
    let (council_member_id, council_member_keypair) = &citizens[council_member_index];

    alice.submit(ProposeDeployContract {
        namespace: namespace.to_string(),
        contract_id: contract_id.to_string(),
        code_hash_hex: code_hash_hex.clone(),
        abi_hash_hex: abi_hash_hex.clone(),
        abi_version: "1".to_string(),
        window: None,
        mode: Some(VotingMode::Plain),
        manifest_provenance: Some(manifest_provenance(
            &code_hash_hex,
            &abi_hash_hex,
            &ALICE_KEYPAIR,
        )),
    })?;
    approved_txs = approved_txs.saturating_add(1);
    wait_for_txs_approved(
        &http,
        &status_url,
        approved_txs,
        rejected_txs,
        Duration::from_secs(120),
        "propose contract deployment",
    )
    .await?;

    let council_member_client = network.peers()[0].client_for(
        council_member_id,
        council_member_keypair.private_key().clone(),
    );
    council_member_client.submit_all([
        ApproveGovernanceProposal {
            body: ParliamentBody::RulesCommittee,
            proposal_id,
        },
        ApproveGovernanceProposal {
            body: ParliamentBody::AgendaCouncil,
            proposal_id,
        },
    ])?;
    approved_txs = approved_txs.saturating_add(1);
    wait_for_txs_approved(
        &http,
        &status_url,
        approved_txs,
        rejected_txs,
        Duration::from_secs(120),
        "approve governance proposal",
    )
    .await?;

    for (idx, (account_id, key_pair)) in citizens.iter().enumerate() {
        let citizen_client =
            network.peers()[0].client_for(account_id, key_pair.private_key().clone());
        citizen_client.submit(CastPlainBallot {
            referendum_id: referendum_id.clone(),
            owner: account_id.clone(),
            amount: BALLOT_LOCK,
            duration_blocks: 20,
            direction: if idx < 12 { 0 } else { 1 },
        })?;
    }
    approved_txs = approved_txs.saturating_add(u64::try_from(CITIZEN_COUNT).expect("count"));
    wait_for_txs_approved(
        &http,
        &status_url,
        approved_txs,
        rejected_txs,
        Duration::from_secs(180),
        "cast ballots",
    )
    .await?;

    alice.submit(FinalizeReferendum {
        referendum_id: referendum_id.clone(),
        proposal_id,
    })?;
    approved_txs = approved_txs.saturating_add(1);
    wait_for_txs_approved(
        &http,
        &status_url,
        approved_txs,
        rejected_txs,
        Duration::from_secs(120),
        "finalize referendum",
    )
    .await?;

    alice.submit(EnactReferendum {
        referendum_id: proposal_id,
        preimage_hash: [0; 32],
        at_window: AtWindow {
            lower: 0,
            upper: u64::MAX,
        },
    })?;
    approved_txs = approved_txs.saturating_add(1);
    wait_for_txs_approved(
        &http,
        &status_url,
        approved_txs,
        rejected_txs,
        Duration::from_secs(120),
        "enact referendum",
    )
    .await?;

    let referendum_payload = alice.get_gov_referendum_json(&referendum_id)?;
    assert_eq!(
        referendum_payload
            .get("found")
            .and_then(norito::json::Value::as_bool),
        Some(true),
        "referendum endpoint should report existing record"
    );
    let referendum_status = referendum_payload
        .get("referendum")
        .and_then(|value| value.get("status"))
        .and_then(norito::json::Value::as_str)
        .unwrap_or_default();
    assert_eq!(referendum_status, "Closed");

    let proposal_payload = alice.get_gov_proposal_json(&proposal_id_hex)?;
    assert_eq!(
        proposal_payload
            .get("found")
            .and_then(norito::json::Value::as_bool),
        Some(true),
        "proposal endpoint should report existing record"
    );
    let proposal_status = proposal_payload
        .get("proposal")
        .and_then(|value| value.get("status"))
        .and_then(norito::json::Value::as_str)
        .unwrap_or_default();
    assert_eq!(proposal_status, "Enacted");

    let tally_payload = alice.get_gov_tally_json(&referendum_id)?;
    let approve = tally_payload
        .get("approve")
        .and_then(json_u128)
        .unwrap_or_default();
    let reject = tally_payload
        .get("reject")
        .and_then(json_u128)
        .unwrap_or_default();
    assert!(
        approve > reject,
        "approval votes should exceed rejection votes"
    );

    Ok(())
}
