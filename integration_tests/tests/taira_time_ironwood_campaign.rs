//! Four-peer rehearsal of the one-time Taira Time and Ironwood activation.
//!
//! A network's identity is its genesis header hash, so a local network can never pass the release's
//! deployment check. This runs against a campaign daemon that differs from the release only in that
//! check (`specs/runtime/taira_time_ironwood_v1.md`), named by `TEST_NETWORK_BIN_IROHAD`. Four peers
//! start at the baseline, take the real certified repair, then the activation through the real
//! signing tool, the validators' own attestations and the block start at its effective height. One
//! peer sits out the boundary and catches up. No peer is configured differently from another.

use std::{
    fs,
    num::NonZeroU64,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::{
    binary_resolver::iroha_program,
    process::{output_with_timeout, process_timeout},
    sandbox,
};
use iroha::{
    blocking::Client,
    client::TransactionWaitOptions,
    data_model::{account::Account, isi::Register, prelude::*},
};
use iroha_core::{
    runtime::{release_catalog, sorafs_repair, taira_activation},
    smartcontracts::isi::sorafs_provider_admission::test_fixture::{key, raw, sign_envelope},
    sumeragi::{certified_chain::CertifiedPrefix, schedule::canonical_committee},
};
use iroha_crypto::{ExposedPrivateKey, KeyPair, Signature};
use iroha_data_model::{
    account::address::chain_discriminant,
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetDefinitionId},
    isi::{
        InstructionBox, decode_instruction_from_pair, privacy::BootstrapPrivacyIronwoodPoolV1,
        sorafs_repair::ApplySorafsRepairV1,
    },
    privacy::{PrivacyIronwoodPoolBootstrapV1, PrivacyPoolIdV1},
    runtime::{
        lifecycle::{ChangeSetRecordV1, ChangeSetStatusV1},
        sorafs_repair::{SORAFS_REPAIR_VERSION_V1, SorafsRepairCertificateV1, SorafsRepairPlanV1},
    },
    sorafs::{
        bootstrap::{SorafsBootstrapGrantV1, SorafsBootstrapPayloadV1, SorafsBootstrapProviderV1},
        capacity::ProviderId,
        provider_admission::{
            PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1, ProviderAdmissionCouncilPolicyV1,
        },
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanBindSorafsAlias, CanCompleteSorafsReplicationOrder, CanSetSorafsReservePolicy,
    CanUpsertSorafsProviderCredit,
};
use iroha_model_base::{domain::DomainId, peer::PeerId};
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer};
use iroha_test_samples::ALICE_ID;
use norito::json::{self, Value};
use sorafs_manifest::ProviderAdmissionEnvelopeV1;

/// Holds the network open after the run, so application scenarios can point at it.
const HOLD_ENV: &str = "TAIRA_ACTIVATION_CAMPAIGN_HOLD_SECS";
/// Taira's selection as read from its authenticated runtime route and replayed State.
const REPAIRED_SELECTION: &str = "f4f73c1261d087963bdfdd045b09551e9dbefbc889b9e735b62f72d9e49e07bb";
const ENVELOPE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
));

fn fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(vec![], NonZeroU64::new(1_000_000))
}

fn unix_ms() -> Result<u64> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

fn account(seed: u8) -> AccountId {
    AccountId::new(key(seed).public_key().clone())
}

fn asset_definition() -> Result<AssetDefinitionId> {
    Ok(AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "ironwood_campaign".parse()?,
    ))
}

fn pool(seed: u8) -> Result<PrivacyIronwoodPoolBootstrapV1> {
    Ok(PrivacyIronwoodPoolBootstrapV1::new(
        PrivacyPoolIdV1::new([seed; 32]),
        asset_definition()?,
        AssetBalanceScope::Global,
        account(7),
    )?)
}

fn tip(client: &Client) -> Result<u64> {
    Ok(client.client().get_sumeragi_status()?.committed_height)
}

fn tick(client: &Client) -> Result<()> {
    client.submit(
        Log::new(
            Level::INFO,
            format!("campaign tick {:?}", SystemTime::now()),
        ),
        fee(),
    )?;
    Ok(())
}

/// Submit and require state-resolved success; an error names the rejection.
fn applied(client: &Client, instruction: InstructionBox) -> Result<()> {
    let hash = client.submit(instruction, fee())?;
    client.wait_for_transaction_applied(hash, TransactionWaitOptions::default())?;
    Ok(())
}

/// Submit and require a state-resolved refusal that carries `reason`.
fn refused(client: &Client, instruction: InstructionBox, reason: &str) -> Result<()> {
    let outcome = client.submit(instruction, fee()).and_then(|hash| {
        client.wait_for_transaction_applied(hash, TransactionWaitOptions::default())
    });
    let Err(error) = outcome else {
        return Err(eyre!("expected a refusal naming `{reason}`"));
    };
    let error = format!("{error:?}");
    ensure!(
        error.contains(reason),
        "refused for another reason: {error}"
    );
    Ok(())
}

fn wait_until(what: &str, limit: Duration, mut done: impl FnMut() -> Result<bool>) -> Result<()> {
    let deadline = Instant::now() + limit;
    loop {
        if done()? {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn upgrades(peer: &NetworkPeer) -> Result<Value> {
    peer.client()
        .client()
        .get_runtime_upgrades_json()
        .wrap_err("read the runtime upgrades route")
}

fn selection_id(peer: &NetworkPeer) -> Result<String> {
    upgrades(peer)?["selection_id_hex"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| eyre!("the route names no selection"))
}

fn record(peer: &NetworkPeer, id: &str) -> Result<Option<ChangeSetRecordV1>> {
    let route = upgrades(peer)?;
    let items = route["items"]
        .as_array()
        .ok_or_else(|| eyre!("the route lists no change sets"))?;
    items
        .iter()
        .find(|item| item["id_hex"].as_str() == Some(id))
        .map(|item| Ok(json::from_json(&json::to_json(&item["record"])?)?))
        .transpose()
}

fn committee(network: &Network) -> Result<Vec<PeerId>> {
    Ok(canonical_committee(
        network.validators().iter().map(NetworkPeer::id),
    )?)
}

fn signer<'a>(network: &'a Network, seat: &PeerId) -> Result<&'a KeyPair> {
    network
        .validators()
        .iter()
        .find(|peer| &peer.id() == seat)
        .and_then(NetworkPeer::bls_key_pair)
        .ok_or_else(|| eyre!("no test key for seat {seat}"))
}

/// The authenticated native parent of the next block, as the real repair tooling reads it.
fn certified_tip(network: &Network, height: u64) -> Result<([u8; 32], [u8; 32])> {
    let client = network.peers()[0].client();
    let genesis = network.genesis().0;
    let root_scope = iroha_genesis::signed_genesis_consensus_metadata(&genesis)?
        .sumeragi_context
        .root_scope;
    let budget = iroha_core::state::AllocationBudget::new(32 * 1024 * 1024);
    let mut prefix = None;
    let mut target = None;
    for at in 1..=height {
        let proof = client
            .client()
            .get_sumeragi_finality_proof(NonZeroU64::new(at).unwrap(), root_scope)?;
        proof.decode_checked(root_scope.into())?;
        let block = iroha_data_model::block::decode_framed_signed_block(&proof.block_wire)?;
        let block = iroha_data_model::block::SharedSignedBlock::try_new(block, &budget)
            .map_err(|(_, error)| error)?;
        if at == 1 {
            prefix = Some(CertifiedPrefix::new(
                &network.chain_id(),
                network.network_id(),
                block,
            )?);
        } else {
            target = Some(prefix.as_mut().unwrap().push(block)?.into_parts().0);
        }
    }
    let target = target.ok_or_else(|| eyre!("no certified successor of genesis"))?;
    let committed = target.committed();
    Ok((committed.core_hash().0, committed.result().0))
}

fn repair_payload(network: &Network, now_secs: u64) -> Result<SorafsBootstrapPayloadV1> {
    let council = key(0x45);
    let policy = ProviderAdmissionCouncilPolicyV1 {
        network_id: *network.network_id().as_bytes(),
        policy_id: [0xC1; 32],
        version: PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        trusted_signers: vec![raw(&council)],
        signature_threshold: 1,
        paused: false,
    };
    let envelope = |index: u8| -> Result<ProviderAdmissionEnvelopeV1> {
        let mut envelope: ProviderAdmissionEnvelopeV1 = norito::decode_from_bytes(ENVELOPE)?;
        envelope.admission_revision = 1;
        envelope.expected_current_event_digest = None;
        envelope.issued_at = now_secs - 60;
        envelope.retention_epoch = now_secs + 3600;
        envelope.notes = None;
        envelope.proposal.provider_id = [0x11 + index; 32];
        envelope.advert_body.provider_id = envelope.proposal.provider_id;
        sign_envelope(&mut envelope, &policy, &council);
        Ok(envelope)
    };
    let mut grants = vec![
        SorafsBootstrapGrantV1 {
            account: account(3),
            permission: CanSetSorafsReservePolicy.into(),
        },
        SorafsBootstrapGrantV1 {
            account: account(3),
            permission: CanUpsertSorafsProviderCredit.into(),
        },
        SorafsBootstrapGrantV1 {
            account: account(4),
            permission: CanBindSorafsAlias.into(),
        },
        SorafsBootstrapGrantV1 {
            account: account(2),
            permission: CanCompleteSorafsReplicationOrder {
                provider_id: ProviderId::new(envelope(0)?.proposal.provider_id),
            }
            .into(),
        },
    ];
    grants.sort();
    Ok(SorafsBootstrapPayloadV1 {
        council: norito::encode_canonical(&policy)?,
        providers: (0..3)
            .map(|index| {
                Ok(SorafsBootstrapProviderV1 {
                    owner: account(1),
                    envelope: norito::encode_canonical(&envelope(index)?)?,
                })
            })
            .collect::<Result<_>>()?,
        grants,
        compliance_operators: vec![account(5)],
    })
}

/// Take the real certified repair at the next block, standing where Taira stands.
fn apply_repair(network: &Network, client: &Client) -> Result<()> {
    let parent = tip(client)?;
    let (core, result) = certified_tip(network, parent)?;
    let catalog = release_catalog()?;
    let baseline_definitions: std::collections::BTreeMap<_, _> = catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect();
    let now_ms = unix_ms()?;
    let roster = committee(network)?;
    let plan = SorafsRepairPlanV1 {
        version: SORAFS_REPAIR_VERSION_V1,
        chain_id: network.chain_id(),
        network_id: network.network_id(),
        operation_id: [0x32; 32],
        parent_height: parent,
        parent_block_hash: core,
        parent_result_hash: result,
        change_set: catalog.sorafs_repair_activation(
            &catalog.baseline,
            &baseline_definitions,
            network.network_id(),
            parent + 1,
            [0x31; 32],
        )?,
        committee: roster
            .iter()
            .map(|peer| peer.public_key().clone())
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| eyre!("four validators"))?,
        expires_at_unix_ms: now_ms + 3_600_000,
        authorization_digest: sorafs_repair::AUTHORIZATION_DIGEST,
        release_digest: [0x33; 32],
        source_closure_digest: [0x34; 32],
        payload: repair_payload(network, now_ms / 1000)?,
    };
    let preimage = plan.signing_bytes()?;
    let signatures = roster
        .iter()
        .map(|seat| {
            Ok(Signature::new(
                signer(network, seat)?.private_key(),
                &preimage,
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| eyre!("four signatures"))?;
    let certificate = SorafsRepairCertificateV1 { plan, signatures };
    client.submit(ApplySorafsRepairV1 { certificate }, fee())?;
    wait_until("the repair on every peer", Duration::from_secs(120), || {
        for peer in network.peers() {
            if selection_id(peer)? != REPAIRED_SELECTION {
                return Ok(false);
            }
        }
        Ok(true)
    })
}

/// One local file with a pin, as the operator hands it to the signing tool.
fn pinned(directory: &Path, name: &str, bytes: &[u8]) -> Result<(PathBuf, String)> {
    let path = directory.join(name);
    fs::write(&path, bytes)?;
    Ok((path, blake3::hash(bytes).to_hex().to_string()))
}

/// Run one `iroha taira time-ironwood` operation.
fn signing_tool(operation: &str, arguments: &[String]) -> Result<String> {
    let mut command = iroha_program()?.command()?;
    command
        .args(["taira", "time-ironwood", "--account-chain-discriminant"])
        .arg(chain_discriminant().to_string())
        .arg(operation)
        .args(arguments);
    let output = output_with_timeout(command, process_timeout())?;
    ensure!(
        output.status.success(),
        "{operation} failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(String::from_utf8(output.stdout)?)
}

fn printed(output: &str, name: &str) -> Result<String> {
    output
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}=")).map(str::to_owned))
        .ok_or_else(|| eyre!("the tool printed no {name}"))
}

struct Activation {
    effective_height: u64,
    change_set: String,
    successor: String,
}

/// Draft, sign and submit the activation through the tool real signing uses.
fn schedule_activation(network: &Network, client: &Client) -> Result<Activation> {
    let directory = tempfile::tempdir()?;
    let directory = directory.path();
    let route = upgrades(&network.peers()[0])?;
    let roster = committee(network)?;
    let listed: Vec<String> = route["committee"]
        .as_array()
        .ok_or_else(|| eyre!("the route lists no committee"))?
        .iter()
        .filter_map(|key| key.as_str().map(str::to_owned))
        .collect();
    ensure!(
        listed
            == roster
                .iter()
                .map(|peer| peer.public_key().to_string())
                .collect::<Vec<_>>(),
        "the route's committee order differs from the native order"
    );
    let effective_height = tip(client)? + 12;
    let (selection, selection_pin) = pinned(
        directory,
        "selection.json",
        &json::to_vec(&route["selection"])?,
    )?;
    let (roster_path, roster_pin) = pinned(directory, "roster.json", &json::to_vec(&roster)?)?;
    let (pool_path, pool_pin) = pinned(directory, "pool.json", &json::to_vec(&pool(0x41)?)?)?;
    let change_set_path = directory.join("change-set.json");
    let qualification = hex::encode([0x42; 32]);
    let network_id = network.network_id().to_string();
    let drafted = signing_tool(
        "draft-change-set",
        &[
            "--network-id".into(),
            network_id.clone(),
            "--effective-height".into(),
            effective_height.to_string(),
            "--selection-json".into(),
            selection.display().to_string(),
            "--selection-blake3-hex".into(),
            selection_pin,
            "--qualification-hex".into(),
            qualification,
            "--output".into(),
            change_set_path.display().to_string(),
        ],
    )?;
    let context = vec![
        "--chain-id".to_owned(),
        network.chain_id().to_string(),
        "--network-id".into(),
        network_id,
        "--operation-id-hex".into(),
        hex::encode([0x43; 32]),
        "--notice-blocks".into(),
        "1".into(),
        "--notice-ms".into(),
        "0".into(),
        "--notice-approval-cushion-ms".into(),
        "0".into(),
        "--expires-at-unix-ms".into(),
        (unix_ms()? + 3_600_000).to_string(),
        "--authorization-digest-hex".into(),
        hex::encode(taira_activation::AUTHORIZATION_DIGEST),
        "--release-digest-hex".into(),
        hex::encode([0x44; 32]),
        "--source-closure-digest-hex".into(),
        hex::encode([0x45; 32]),
        "--change-set-json".into(),
        change_set_path.display().to_string(),
        "--change-set-blake3-hex".into(),
        printed(&drafted, "change_set_blake3_hex")?,
        "--pool-json".into(),
        pool_path.display().to_string(),
        "--pool-blake3-hex".into(),
        pool_pin,
        "--roster".into(),
        roster_path.display().to_string(),
        "--roster-blake3-hex".into(),
        roster_pin,
    ];
    let draft_path = directory.join("draft.to");
    let prepared = signing_tool(
        "prepare",
        &[
            context.as_slice(),
            &["--output".into(), draft_path.display().to_string()],
        ]
        .concat(),
    )?;
    let reviewed = [
        context.as_slice(),
        &[
            "--draft".into(),
            draft_path.display().to_string(),
            "--draft-blake3-hex".into(),
            printed(&prepared, "draft_blake3_hex")?,
        ],
    ]
    .concat();
    let mut share_arguments = Vec::new();
    for (seat, peer) in roster.iter().enumerate() {
        let key_path = directory.join(format!("{seat}.key"));
        let secret = ExposedPrivateKey(signer(network, peer)?.private_key().clone())
            .try_to_multihash_string()?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        std::io::Write::write_all(&mut options.open(&key_path)?, secret.as_bytes())?;
        let share = directory.join(format!("{seat}.share"));
        signing_tool(
            "sign",
            &[
                reviewed.as_slice(),
                &[
                    "--signer-index".into(),
                    seat.to_string(),
                    "--private-key-file".into(),
                    key_path.display().to_string(),
                    "--output".into(),
                    share.display().to_string(),
                ],
            ]
            .concat(),
        )?;
        share_arguments.push("--signature".to_owned());
        share_arguments.push(share.display().to_string());
    }
    let instruction_path = directory.join("instruction.json");
    signing_tool(
        "assemble",
        &[
            reviewed.as_slice(),
            share_arguments.as_slice(),
            &["--output".into(), instruction_path.display().to_string()],
        ]
        .concat(),
    )?;
    let mut instructions: Vec<InstructionBox> =
        json::from_slice(&fs::read(&instruction_path)?).wrap_err("assembled instruction")?;
    ensure!(instructions.len() == 1, "exactly one instruction");
    let instruction = instructions.remove(0);
    let change_set = printed(&prepared, "change_set_id")?;
    applied(client, instruction).wrap_err("the signed schedule")?;
    let scheduled = record(&network.peers()[0], &change_set)?
        .ok_or_else(|| eyre!("the schedule recorded nothing"))?;
    ensure!(
        scheduled.status == ChangeSetStatusV1::Scheduled,
        "scheduling selects nothing"
    );
    ensure!(
        selection_id(&network.peers()[0])? == REPAIRED_SELECTION,
        "scheduling selects nothing"
    );
    Ok(Activation {
        effective_height,
        change_set,
        successor: scheduled.change_set.successor.to_string(),
    })
}

/// Each validator's own node decides support and signs; the operator only relays it.
fn attest_all(network: &Network, change_set: &str) -> Result<()> {
    for peer in network.peers() {
        let client = peer.client();
        let draft = client
            .client()
            .post_runtime_attest_upgrade_json(change_set)
            .wrap_err("the node drafts its attestation")?;
        let [drafted] = draft["tx_instructions"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
        else {
            return Err(eyre!("the node did not draft exactly one instruction"));
        };
        let wire_id = drafted["wire_id"]
            .as_str()
            .ok_or_else(|| eyre!("the draft has no wire ID"))?;
        let payload = hex::decode(
            drafted["payload_hex"]
                .as_str()
                .ok_or_else(|| eyre!("the draft has no payload"))?,
        )?;
        applied(&client, decode_instruction_from_pair(wire_id, &payload)?)
            .wrap_err("the attestation")?;
    }
    let attested = record(&network.peers()[0], change_set)?
        .ok_or_else(|| eyre!("the scheduled record disappeared"))?;
    ensure!(attested.attestations.len() == 4, "four attestations");
    Ok(())
}

/// Every peer's block at `height`, byte for byte, as the certified route serves it.
fn same_block_everywhere(network: &Network, height: u64) -> Result<()> {
    let genesis = network.genesis().0;
    let root_scope = iroha_genesis::signed_genesis_consensus_metadata(&genesis)?
        .sumeragi_context
        .root_scope;
    let mut seen = None;
    for peer in network.peers() {
        let proof = peer
            .client()
            .client()
            .get_sumeragi_finality_proof(NonZeroU64::new(height).unwrap(), root_scope)?;
        let block = proof.block_wire;
        if let Some(previous) = &seen {
            ensure!(previous == &block, "peers differ at height {height}");
        } else {
            seen = Some(block);
        }
    }
    Ok(())
}

fn lifecycle_route(peer: &NetworkPeer, runtime: &tokio::runtime::Runtime) -> Result<Value> {
    runtime.block_on(async {
        let response = reqwest::Client::new()
            .get(format!(
                "{}/v1/triggers/lifecycle-activation",
                peer.torii_url().trim_end_matches('/')
            ))
            .timeout(Duration::from_secs(60))
            .send()
            .await?;
        ensure!(response.status().is_success(), "{}", response.status());
        Ok(json::from_json(&response.text().await?)?)
    })
}

#[test]
#[ignore = "needs the campaign daemon named by TEST_NETWORK_BIN_IROHAD; see specs/runtime/taira_time_ironwood_v1.md"]
fn four_peers_take_the_repair_and_the_activation_with_one_peer_restarted_across_it() -> Result<()> {
    iroha_test_network::init_instruction_registry();
    let mut fees = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
    fees.fee_sink_account_id = ALICE_ID.clone();
    fees.base_fee = "0".parse()?;
    fees.per_byte_fee = "0".parse()?;
    fees.per_instruction_fee = "0".parse()?;
    fees.per_gas_unit_fee = "0".parse()?;
    let mut builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(1))
        .with_genesis_instruction(SetParameter::new(
            iroha_data_model::parameter::Parameter::Custom(fees.into_custom_parameter()?),
        ));
    for seed in 1..=5 {
        builder = builder.with_genesis_instruction(Register::account(Account::new(account(seed))));
    }
    let (network, rt) = sandbox::start_network_blocking_or_skip(builder, "taira_time_ironwood")?
        .ok_or_else(|| eyre!("the campaign requires real localhost sockets"))?;
    rt.block_on(network.ensure_blocks(1))?;
    let client = network.peers()[0].client();

    // Stand where Taira stands: a certified parent above genesis, then the real repair.
    applied(
        &client,
        Register::asset_definition(AssetDefinition::numeric(
            asset_definition()?,
            "Ironwood campaign".to_owned(),
            AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal")?),
        ))
        .into(),
    )
    .wrap_err("the campaign asset")?;
    apply_repair(&network, &client)?;
    let repair_height = tip(&client)?;
    same_block_everywhere(&network, repair_height)?;
    eprintln!("campaign: repaired at height {repair_height}");

    // The signed schedule selects nothing; every validator's own node attests.
    let activation = schedule_activation(&network, &client)?;
    attest_all(&network, &activation.change_set)?;
    eprintln!(
        "campaign: change set {} scheduled and attested for height {}",
        activation.change_set, activation.effective_height,
    );

    // One peer sits out the boundary; three carry the chain to the height.
    let restarted = &network.peers()[3];
    rt.block_on(restarted.shutdown());
    wait_until(
        "the effective height",
        Duration::from_secs(180),
        || -> Result<bool> {
            if tip(&client)? >= activation.effective_height {
                return Ok(true);
            }
            tick(&client)?;
            Ok(false)
        },
    )?;
    for peer in network.peers().iter().take(3) {
        ensure!(
            selection_id(peer)? == activation.successor,
            "every running peer selects the successor at the block start"
        );
        let activated =
            record(peer, &activation.change_set)?.ok_or_else(|| eyre!("the record disappeared"))?;
        ensure!(
            activated.status == ChangeSetStatusV1::Activated(activation.effective_height),
            "activation is recorded at its height"
        );
        let policy = lifecycle_route(peer, &rt)?;
        ensure!(
            policy["active"].as_bool() == Some(true),
            "lifecycle capture is active"
        );
        ensure!(
            policy["v2_activation_height"].as_u64() == Some(activation.effective_height),
            "the lineage names this activation"
        );
    }

    // The peer that sat out loads the old store, catches up and agrees.
    let layers: Vec<_> = network.config_layers_for_peer(restarted).collect();
    rt.block_on(restarted.start_checked(layers.iter(), None))?;
    // Its status reports the network's height before it has fetched and applied those blocks, so
    // only its own selection shows that it has crossed the boundary.
    wait_until(
        "the restarted peer to select the successor",
        Duration::from_secs(300),
        || Ok(selection_id(restarted).is_ok_and(|id| id == activation.successor)),
    )?;
    same_block_everywhere(&network, activation.effective_height)?;

    // The retained certificate authorizes exactly its one pool, once.
    refused(
        &client,
        BootstrapPrivacyIronwoodPoolV1::new(pool(0x51)?).into(),
        "CanEnactGovernance",
    )
    .wrap_err("any other pool still needs the governance permission")?;
    applied(&client, Register::account(Account::new(account(7))).into())
        .wrap_err("the reserve account")?;
    applied(
        &client,
        BootstrapPrivacyIronwoodPoolV1::new(pool(0x41)?).into(),
    )
    .wrap_err("the certified pool")?;
    refused(
        &client,
        BootstrapPrivacyIronwoodPoolV1::new(pool(0x41)?).into(),
        "already initialized",
    )
    .wrap_err("a repeat refuses")?;
    let final_height = tip(&client)?;
    wait_until(
        "the last block on every peer",
        Duration::from_secs(120),
        || {
            for peer in network.peers() {
                if tip(&peer.client())? < final_height {
                    return Ok(false);
                }
            }
            Ok(true)
        },
    )?;
    same_block_everywhere(&network, final_height)?;
    eprintln!(
        "campaign: passed, tip {final_height}, successor {}",
        activation.successor
    );

    if let Some(seconds) = std::env::var(HOLD_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
    {
        eprintln!(
            "campaign: holding for {seconds} s, Torii: {}",
            network.torii_urls().join(" ")
        );
        std::thread::sleep(Duration::from_secs(seconds));
    }
    Ok(())
}
