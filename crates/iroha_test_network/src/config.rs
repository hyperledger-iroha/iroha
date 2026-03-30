//! Sample configuration builders

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use color_eyre::{Report, eyre::eyre};
use iroha_config::base::toml::WriteExt;
use iroha_config::parameters::actual::{Crypto as ActualCrypto, Nexus as ActualNexus};
use iroha_core::{
    block::ValidBlock,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
    sumeragi::network_topology::Topology as CoreTopology,
    telemetry::StateTelemetry,
};
use iroha_crypto::{KeyPair, SignatureOf};
use iroha_data_model::{
    ChainId, Registrable as _,
    account::{Account, AccountId},
    asset::{AssetDefinitionId, id::AssetId},
    consensus::HsmBinding,
    da::commitment::DaProofPolicyBundle,
    domain::{Domain, DomainId},
    isi::{Grant, InstructionBox, Mint, SetParameter, register::RegisterPeerWithPop},
    metadata::Metadata,
    name::Name,
    parameter::{
        Parameter,
        custom::CustomParameter,
        system::{confidential_metadata, consensus_metadata},
    },
    peer::PeerId,
    permission::Permission,
    prelude::{HashOf, Transfer},
    transaction::signed::TransactionResultInner,
    trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
};
use iroha_executor_data_model::permission::{
    account::CanRegisterAccount,
    asset::{CanMintAssetWithDefinition, CanTransferAssetWithDefinition},
    domain::{CanRegisterDomain, CanUnregisterDomain},
    executor::CanUpgradeExecutor,
    parameter::CanSetParameters,
    peer::CanManagePeers,
    role::CanManageRoles,
    trigger::CanRegisterTrigger,
};
use iroha_genesis::{GenesisBlock, GenesisBuilder, GenesisTopologyEntry, ManifestCrypto};
use iroha_primitives::{json::Json, numeric::NumericSpec, time::TimeSource, unique_vec::UniqueVec};
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, BOB_KEYPAIR, CARPENTER_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
};
#[cfg(test)]
use norito::json::Value;
use toml::Table;
use tracing::warn;

use crate::init_instruction_registry;

pub fn chain_id() -> ChainId {
    ChainId::from("00000000-0000-0000-0000-000000000000")
}

#[cfg(test)]
fn sanitize_strings(value: &mut Value) {
    match value {
        Value::String(s) => {
            if s.chars().any(char::is_whitespace) {
                *s = s.split_whitespace().collect::<Vec<_>>().join("_");
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(sanitize_strings),
        Value::Object(map) => map.values_mut().for_each(sanitize_strings),
        _ => {}
    }
}

fn sanitize_account_id(id: &AccountId) -> AccountId {
    let raw = id.to_string();
    if !raw.chars().any(char::is_whitespace) {
        // Avoid reparsing I105 addresses unless we actually sanitize.
        return id.clone();
    }
    let sanitized = sanitize_account_id_str(&raw);
    AccountId::parse_encoded(&sanitized)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("sanitized AccountId should parse")
}

fn sanitize_account_id_str(s: &str) -> String {
    if s.chars().any(char::is_whitespace) {
        s.split_whitespace().collect::<Vec<_>>().join("_")
    } else {
        s.to_string()
    }
}

pub fn base_iroha_config() -> Table {
    Table::new()
        .write("chain", chain_id().to_string())
        .write(
            ["genesis", "public_key"],
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().to_string(),
        )
        // Enable extended telemetry in test networks so `/metrics` snapshots are available.
        .write("telemetry_enabled", true)
        .write("telemetry_profile", "extended")
        // There is no need in persistence in tests.
        .write(["snapshot", "mode"], "disabled")
        .write(["kura", "store_dir"], "./storage")
        // Default to broadcasting blocks to the entire test topology so small networks
        // do not stall waiting for block sync retries when some peers miss a gossip hop.
        .write(["network", "block_gossip_size"], 256)
        .write(["confidential", "enabled"], true)
        .write(["logger", "level"], "INFO")
        .write(["logger", "format"], "pretty")
        // Keep debug RBC toggles explicitly present so config deserialization never fails when
        // layers only override a subset (e.g., adversarial shuffle/duplicate/drop settings).
        .write(["sumeragi", "debug", "rbc", "shuffle_chunks"], false)
        .write(["sumeragi", "debug", "rbc", "duplicate_inits"], false)
        .write(["sumeragi", "debug", "rbc", "corrupt_witness_ack"], false)
        .write(
            ["sumeragi", "debug", "rbc", "corrupt_ready_signature"],
            false,
        )
        .write(["sumeragi", "debug", "rbc", "drop_validator_mask"], 0i64)
        .write(["sumeragi", "debug", "rbc", "equivocate_chunk_mask"], 0i64)
        .write(
            ["sumeragi", "debug", "rbc", "equivocate_validator_mask"],
            0i64,
        )
        .write(["sumeragi", "debug", "rbc", "conflicting_ready_mask"], 0i64)
        .write(["sumeragi", "debug", "rbc", "partial_chunk_mask"], 0i64)
}

#[must_use]
pub(crate) fn manifest_crypto_from_actual(crypto: &ActualCrypto) -> ManifestCrypto {
    ManifestCrypto {
        sm_openssl_preview: crypto.enable_sm_openssl_preview,
        sm_intrinsics: crypto.sm_intrinsics.as_str().to_owned(),
        default_hash: crypto.default_hash.clone(),
        allowed_signing: crypto.allowed_signing.clone(),
        sm2_distid_default: crypto.sm2_distid_default.clone(),
        allowed_curve_ids: crypto.allowed_curve_ids.clone(),
    }
}

pub fn genesis(
    extra_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
) -> GenesisBlock {
    genesis_with_keypair(
        extra_transactions,
        topology,
        topology_entries,
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
    )
}

/// Build the default genesis using a custom signing key pair.
pub fn genesis_with_keypair(
    extra_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
) -> GenesisBlock {
    // Always construct a deterministic, minimal built-in genesis tailored for tests.
    // This avoids surprises from `defaults/genesis.json` contents and keeps the
    // first transaction shape predictable (e.g., single Upgrade when a sample
    // executor is available).
    init_instruction_registry();
    build_minimal_genesis(
        extra_transactions,
        topology,
        topology_entries,
        genesis_key_pair,
    )
}

/// Build the default genesis using a custom signing key pair and post-topology instructions.
#[allow(dead_code)]
pub fn genesis_with_keypair_and_post_topology(
    extra_transactions: Vec<Vec<InstructionBox>>,
    post_topology_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
) -> GenesisBlock {
    genesis_with_keypair_and_post_topology_with_policies(
        extra_transactions,
        post_topology_transactions,
        topology,
        topology_entries,
        genesis_key_pair,
        chain_id(),
        None,
        None,
        None,
        None,
    )
}

pub(crate) fn genesis_with_keypair_and_post_topology_with_policies(
    extra_transactions: Vec<Vec<InstructionBox>>,
    post_topology_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
    chain_id: ChainId,
    genesis_crypto: Option<ManifestCrypto>,
    da_proof_policies: Option<DaProofPolicyBundle>,
    _nexus_config: Option<ActualNexus>,
    consensus_handshake_meta: Option<Parameter>,
) -> GenesisBlock {
    init_instruction_registry();
    build_minimal_genesis_with_post_topology(
        extra_transactions,
        post_topology_transactions,
        topology,
        topology_entries,
        genesis_key_pair,
        chain_id,
        genesis_crypto,
        da_proof_policies,
        _nexus_config,
        consensus_handshake_meta,
    )
}

fn strip_handshake_metadata_transactions(transactions: &mut [Vec<InstructionBox>]) {
    for instruction_batch in transactions {
        instruction_batch.retain(|instruction| {
            !instruction
                .as_any()
                .downcast_ref::<SetParameter>()
                .is_some_and(|set_param| {
                    matches!(
                        set_param.inner(),
                        Parameter::Custom(custom)
                            if custom.id() == &consensus_metadata::handshake_meta_id()
                    )
                })
        });
    }
}

fn build_minimal_genesis(
    extra_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
) -> GenesisBlock {
    build_minimal_genesis_with_post_topology(
        extra_transactions,
        Vec::new(),
        topology,
        topology_entries,
        genesis_key_pair,
        chain_id(),
        None,
        None,
        None,
        None,
    )
}

fn build_minimal_genesis_with_post_topology(
    extra_transactions: Vec<Vec<InstructionBox>>,
    post_topology_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
    chain_id: ChainId,
    genesis_crypto: Option<ManifestCrypto>,
    da_proof_policies: Option<DaProofPolicyBundle>,
    nexus_config: Option<ActualNexus>,
    consensus_handshake_meta: Option<Parameter>,
) -> GenesisBlock {
    let mut extra_transactions = extra_transactions;
    let mut post_topology_transactions = post_topology_transactions;

    strip_handshake_metadata_transactions(&mut extra_transactions);
    strip_handshake_metadata_transactions(&mut post_topology_transactions);

    let (mut block, genesis_account, topology_vec, genesis_key_pair) =
        build_minimal_genesis_unexecuted_with_post_topology(
            extra_transactions,
            post_topology_transactions,
            topology,
            topology_entries,
            genesis_key_pair,
            chain_id,
            genesis_crypto,
            da_proof_policies,
            nexus_config.clone(),
            consensus_handshake_meta,
        );
    ensure_genesis_results(
        &mut block,
        &genesis_account,
        &topology_vec,
        &genesis_key_pair,
        nexus_config.as_ref(),
    );
    block
}

#[allow(dead_code)]
fn build_minimal_genesis_unexecuted(
    extra_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
) -> (GenesisBlock, AccountId, Vec<PeerId>, KeyPair) {
    build_minimal_genesis_unexecuted_with_post_topology(
        extra_transactions,
        Vec::new(),
        topology,
        topology_entries,
        genesis_key_pair,
        chain_id(),
        None,
        None,
        None,
        None,
    )
}

fn build_minimal_genesis_unexecuted_with_post_topology(
    extra_transactions: Vec<Vec<InstructionBox>>,
    post_topology_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
    chain_id: ChainId,
    genesis_crypto: Option<ManifestCrypto>,
    da_proof_policies: Option<DaProofPolicyBundle>,
    _nexus_config: Option<ActualNexus>,
    consensus_handshake_meta: Option<Parameter>,
) -> (GenesisBlock, AccountId, Vec<PeerId>, KeyPair) {
    fn try_default_executor_path() -> Option<PathBuf> {
        if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")
            .ok()
            .as_deref()
            != Some("1")
        {
            return None;
        }
        let sample = iroha_test_samples::sample_ivm_path("default_executor");
        match std::fs::metadata(&sample) {
            Ok(meta) if meta.len() > 0 => Some(sample),
            _ => None,
        }
    }

    fn default_ivm_dir() -> PathBuf {
        iroha_test_samples::sample_ivm_path("default_executor")
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
    let chain = chain_id.clone();
    let genesis_account = AccountId::new(genesis_key_pair.public_key().clone());
    let genesis_id = sanitize_account_id(&genesis_account);
    let alice_id = sanitize_account_id(&ALICE_ID);
    let ivm_dir = default_ivm_dir();

    let mut builder = if let Some(executor_path) = try_default_executor_path() {
        GenesisBuilder::new(chain.clone(), executor_path, ivm_dir.clone())
    } else {
        GenesisBuilder::new_without_executor(chain.clone(), ivm_dir.clone())
    };
    if let Some(crypto) = genesis_crypto {
        builder = builder.with_crypto(crypto);
    }
    if let Some(policies) = da_proof_policies {
        builder = builder.with_da_proof_policies(policies);
    }

    let wonderland_name: Name = "wonderland".parse().expect("wonderland domain");
    let rose_name: Name = "rose".parse().expect("rose asset name");
    let camomile_name: Name = "camomile".parse().expect("camomile asset name");
    let garden_name: Name = "garden_of_live_flowers"
        .parse()
        .expect("garden_of_live_flowers domain");
    let aid_name: Name = "aid".parse().expect("aid domain");
    let cabbage_name: Name = "cabbage".parse().expect("cabbage asset name");
    let alice_metadata = Metadata::default();

    builder = builder
        .domain(wonderland_name.clone())
        .account_with_metadata(ALICE_KEYPAIR.public_key().clone(), alice_metadata)
        .account(BOB_KEYPAIR.public_key().clone())
        .asset(rose_name, NumericSpec::default())
        .asset(camomile_name, NumericSpec::default())
        .finish_domain()
        .domain(garden_name)
        .account(CARPENTER_KEYPAIR.public_key().clone())
        .asset(cabbage_name, NumericSpec::default())
        .finish_domain()
        .domain(aid_name)
        .finish_domain();

    let wonderland_domain: DomainId = "wonderland".parse().expect("wonderland domain id");
    let garden_domain: DomainId = "garden_of_live_flowers"
        .parse()
        .expect("garden_of_live_flowers domain id");
    let aid_domain: DomainId = "aid".parse().expect("aid domain id");
    let rose_definition_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "rose".parse().unwrap(),
    );
    let camomile_definition_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "camomile".parse().unwrap(),
    );
    let cabbage_definition_id: AssetDefinitionId = AssetDefinitionId::new(
        "garden_of_live_flowers".parse().unwrap(),
        "cabbage".parse().unwrap(),
    );
    let rose_asset_id = AssetId::new(rose_definition_id.clone(), alice_id.clone());
    let cabbage_asset_id = AssetId::new(cabbage_definition_id.clone(), alice_id.clone());

    builder = builder.append_instruction(Transfer::domain(
        genesis_id.clone(),
        wonderland_domain.clone(),
        alice_id.clone(),
    ));
    builder = builder.append_instruction(Transfer::domain(
        genesis_id.clone(),
        aid_domain.clone(),
        alice_id.clone(),
    ));
    builder = builder.append_instruction(Mint::asset_numeric(13u32, rose_asset_id));
    builder = builder.append_instruction(Mint::asset_numeric(44u32, cabbage_asset_id));

    builder = builder.next_transaction();

    let test_domain_id: DomainId = "domain".parse().expect("domain id");
    let and_domain_id: DomainId = "and".parse().expect("and domain id");
    let xor_asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "xor".parse().unwrap(),
    );
    let may_and_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "and".parse().unwrap(),
        "MAY".parse().unwrap(),
    );

    let grant_instructions = [
        InstructionBox::from(Grant::account_permission(
            CanRegisterDomain,
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanRegisterAccount {
                domain: test_domain_id.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanRegisterAccount {
                domain: wonderland_domain.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanRegisterAccount {
                domain: garden_domain.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanRegisterAccount {
                domain: aid_domain.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanRegisterAccount {
                domain: and_domain_id.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanMintAssetWithDefinition {
                asset_definition: xor_asset_def.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanTransferAssetWithDefinition {
                asset_definition: xor_asset_def.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanMintAssetWithDefinition {
                asset_definition: rose_definition_id.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanMintAssetWithDefinition {
                asset_definition: camomile_definition_id.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanMintAssetWithDefinition {
                asset_definition: cabbage_definition_id.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanMintAssetWithDefinition {
                asset_definition: may_and_def,
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(CanManagePeers, alice_id.clone())),
        InstructionBox::from(Grant::account_permission(CanManageRoles, alice_id.clone())),
        InstructionBox::from(Grant::account_permission(
            CanUnregisterDomain {
                domain: wonderland_domain.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanUpgradeExecutor,
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanSetParameters,
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanSetParameters,
            genesis_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            Permission::new("CanManageSoracloud".into(), Json::new(())),
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanRegisterTrigger {
                authority: alice_id.clone(),
            },
            alice_id.clone(),
        )),
    ];

    for grant in grant_instructions {
        builder = builder.append_instruction(grant);
    }

    for tx_instr in extra_transactions.into_iter() {
        if tx_instr.is_empty() {
            continue;
        }
        builder = builder.next_transaction();
        for instruction in tx_instr {
            builder = builder.append_instruction(instruction);
        }
    }

    let topology_vec: Vec<PeerId> = topology.iter().cloned().collect();
    if !topology_vec.is_empty() {
        let mut pop_map: BTreeMap<iroha_crypto::PublicKey, Vec<u8>> = topology_entries
            .iter()
            .map(|entry| {
                let pop = entry
                    .pop_bytes()
                    .unwrap_or_else(|err| {
                        panic!(
                            "invalid pop_hex for topology peer {}: {err}",
                            entry.peer.public_key()
                        )
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "missing pop_hex for topology peer {}",
                            entry.peer.public_key()
                        )
                    });
                (entry.peer.public_key().clone(), pop)
            })
            .collect();

        builder = builder
            .next_transaction()
            .set_topology(topology_entries.clone());

        for peer_id in &topology_vec {
            let pop_bytes = pop_map
                .remove(peer_id.public_key())
                .unwrap_or_else(|| panic!("missing BLS PoP for peer {}", peer_id.public_key()));
            // Bind consensus keys to a softkey provider so genesis passes the HSM policy gate.
            let hsm_binding = HsmBinding {
                provider: "softkey".to_owned(),
                key_label: peer_id.public_key().to_string(),
                slot: None,
            };
            let register =
                RegisterPeerWithPop::new(peer_id.clone(), pop_bytes).with_hsm(hsm_binding);
            let instruction =
                <RegisterPeerWithPop as iroha_data_model::isi::Instruction>::into_instruction_box(
                    Box::new(register),
                );
            builder = builder.append_instruction(instruction);
        }

        if let Some((dangling_pk, _)) = pop_map.into_iter().next() {
            panic!("topology entry present for peer {dangling_pk} that is absent from topology");
        }
    }

    for tx_instr in post_topology_transactions.into_iter() {
        if tx_instr.is_empty() {
            continue;
        }
        builder = builder.next_transaction();
        for instruction in tx_instr {
            builder = builder.append_instruction(instruction);
        }
    }

    let conf_param = Parameter::Custom(CustomParameter::new(
        confidential_metadata::registry_root_id(),
        Json::new(norito::json!({ "vk_set_hash": null })),
    ));
    builder = builder.append_parameter(conf_param);
    if let Some(handshake_meta) = consensus_handshake_meta {
        builder =
            builder.append_instruction(InstructionBox::from(SetParameter::new(handshake_meta)));
    }

    let block = builder
        .build_and_sign(&genesis_key_pair)
        .expect("build minimal genesis");
    (block, genesis_account, topology_vec, genesis_key_pair)
}

pub(crate) fn ensure_genesis_results(
    block: &mut GenesisBlock,
    genesis_account: &AccountId,
    topology: &[PeerId],
    genesis_key_pair: &KeyPair,
    nexus_config: Option<&ActualNexus>,
) {
    let tx_count = block.0.transactions_vec().len();
    let result_count = block.0.results().count();
    let missing_results = tx_count > 0 && tx_count != result_count;
    let signature_is_canonical = genesis_signature_is_canonical(&block.0, genesis_key_pair);
    if !missing_results && signature_is_canonical {
        return;
    }

    // Preserve already computed execution results while restoring the canonical genesis signature.
    if !missing_results {
        block.0 = rebuild_block_with_results(&block.0, genesis_key_pair);
        return;
    }

    match populate_genesis_results(
        block,
        genesis_account,
        topology,
        genesis_key_pair,
        nexus_config,
    ) {
        Ok(new_block) => block.0 = new_block,
        Err(err) => {
            warn!(
                ?err,
                "Failed to pre-execute genesis block; falling back to synthetic success results"
            );
            block.0 = build_placeholder_block(&block.0, genesis_key_pair);
        }
    }
}

fn genesis_signature_is_canonical(
    block: &iroha_data_model::block::SignedBlock,
    genesis_key_pair: &KeyPair,
) -> bool {
    let mut signatures = block.signatures();
    let Some(signature) = signatures.next() else {
        return false;
    };
    if signatures.next().is_some() {
        return false;
    }
    signature
        .signature()
        .verify_hash(genesis_key_pair.public_key(), block.hash())
        .is_ok()
}

fn populate_genesis_results(
    block: &GenesisBlock,
    genesis_account: &AccountId,
    topology: &[PeerId],
    genesis_key_pair: &KeyPair,
    nexus_config: Option<&ActualNexus>,
) -> Result<iroha_data_model::block::SignedBlock, Report> {
    if topology.is_empty() {
        return Err(eyre!("genesis topology is empty"));
    }

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let effective_genesis_account = block
        .0
        .transactions_vec()
        .first()
        .map(|tx| tx.authority().clone())
        .unwrap_or_else(|| genesis_account.clone());
    let genesis_domain =
        Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&effective_genesis_account);
    let genesis_account_entry =
        Account::new(effective_genesis_account.clone()).build(&effective_genesis_account);
    let mut world = World::with([genesis_domain], [genesis_account_entry], []);
    iroha_core::sns::seed_genesis_alias_bootstrap(&mut world, &block.0);
    let mut state = State::with_telemetry(world, kura, query_handle, StateTelemetry::default());
    apply_preexec_nexus_overrides(
        &mut state,
        genesis_key_pair,
        nexus_config,
        block.0.da_proof_policies(),
    )?;
    let core_topology = CoreTopology::new(topology.to_vec());
    let chain = block
        .0
        .transactions_vec()
        .first()
        .map(|tx| tx.chain().clone())
        .unwrap_or_else(chain_id);

    let mut voting_block = None;
    let time_source = TimeSource::new_system();
    let validation = ValidBlock::validate_keep_voting_block(
        block.0.clone(),
        &core_topology,
        &chain,
        &effective_genesis_account,
        &time_source,
        &state,
        &mut voting_block,
        false,
    )
    .unpack(|_| {});
    let (valid_block, state_block) = match validation {
        Ok(validated) => validated,
        Err((rejected_block, err)) => {
            let first_tx_error =
                rejected_block
                    .results()
                    .enumerate()
                    .find_map(|(index, result)| {
                        result
                            .as_ref()
                            .err()
                            .map(|tx_err| format!("tx#{index}: {tx_err}; details: {tx_err:?}"))
                    });
            let mut report = Report::new(err);
            if let Some(first_tx_error) = first_tx_error {
                report = report.wrap_err(format!(
                    "genesis pre-execution produced rejected transaction result ({first_tx_error})"
                ));
            } else if rejected_block.has_results() {
                report = report.wrap_err(
                    "genesis pre-execution produced invalid results without a concrete transaction \
                     rejection reason"
                        .to_owned(),
                );
            } else {
                report = report.wrap_err(
                    "genesis pre-execution failed before transaction results were recorded"
                        .to_owned(),
                );
            }
            return Err(report);
        }
    };
    drop(state_block);
    let signed_block: iroha_data_model::block::SignedBlock = valid_block.into();
    Ok(rebuild_block_with_results(&signed_block, genesis_key_pair))
}

fn apply_preexec_nexus_overrides(
    state: &mut State,
    _genesis_key_pair: &KeyPair,
    nexus_config: Option<&ActualNexus>,
    block_policies: Option<&DaProofPolicyBundle>,
) -> Result<(), Report> {
    // Use a single-domain test account literal to avoid ambiguous subject->domain resolution.
    let gas_account = ALICE_ID.to_string();

    let mut nexus = nexus_config.cloned().unwrap_or_default();
    if let Some(policies) = block_policies
        && !policies.policies.is_empty()
    {
        let mut lanes = Vec::with_capacity(policies.policies.len());
        let mut dataspace_ids = BTreeSet::new();
        let mut max_lane = 0u32;
        for policy in &policies.policies {
            max_lane = max_lane.max(policy.lane_id.as_u32());
            dataspace_ids.insert(policy.dataspace_id);
            lanes.push(iroha_data_model::nexus::LaneConfig {
                id: policy.lane_id,
                dataspace_id: policy.dataspace_id,
                alias: policy.alias.clone(),
                proof_scheme: policy.proof_scheme,
                ..iroha_data_model::nexus::LaneConfig::default()
            });
        }
        let lane_count = std::num::NonZeroU32::new(max_lane.saturating_add(1))
            .ok_or_else(|| eyre!("proof policies must include at least one lane"))?;
        let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(lane_count, lanes)
            .map_err(|err| Report::new(err).wrap_err("build lane catalog from proof policies"))?;

        let mut dataspace_entries = nexus.dataspace_catalog.entries().to_vec();
        let mut existing_ids: BTreeSet<_> =
            dataspace_entries.iter().map(|entry| entry.id).collect();
        let mut existing_aliases: BTreeSet<_> = dataspace_entries
            .iter()
            .map(|entry| entry.alias.clone())
            .collect();
        for dataspace_id in dataspace_ids {
            if existing_ids.insert(dataspace_id) {
                let base_alias = format!("policy-ds-{}", u64::from(dataspace_id));
                let mut alias = base_alias.clone();
                let mut idx = 1u32;
                while existing_aliases.contains(&alias) {
                    alias = format!("{base_alias}-{idx}");
                    idx = idx.saturating_add(1);
                }
                existing_aliases.insert(alias.clone());
                dataspace_entries.push(iroha_data_model::nexus::DataSpaceMetadata {
                    id: dataspace_id,
                    alias,
                    description: None,
                    fault_tolerance: 1,
                });
            }
        }
        let dataspace_catalog = iroha_data_model::nexus::DataSpaceCatalog::new(dataspace_entries)
            .map_err(|err| {
            Report::new(err).wrap_err("build dataspace catalog from proof policies")
        })?;

        nexus.lane_catalog = lane_catalog;
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        nexus.dataspace_catalog = dataspace_catalog;
    }
    nexus.staking.stake_escrow_account_id = gas_account.clone();
    nexus.staking.slash_sink_account_id = gas_account;

    state
        .set_nexus(nexus)
        .map_err(|err| Report::new(err).wrap_err("apply nexus config for genesis pre-execution"))?;
    Ok(())
}

fn build_placeholder_block(
    template: &iroha_data_model::block::SignedBlock,
    genesis_key_pair: &KeyPair,
) -> iroha_data_model::block::SignedBlock {
    let transactions = template.transactions_vec().clone();
    let hashes = transactions
        .iter()
        .map(|tx| tx.hash_as_entrypoint())
        .collect::<Vec<_>>();
    let results = hashes
        .iter()
        .map(|_| Ok(DataTriggerSequence::default()))
        .collect::<Vec<_>>();
    rebuild_block_from_parts(
        template,
        transactions,
        Vec::new(),
        hashes,
        results,
        genesis_key_pair,
    )
}

fn rebuild_block_with_results(
    template: &iroha_data_model::block::SignedBlock,
    genesis_key_pair: &KeyPair,
) -> iroha_data_model::block::SignedBlock {
    let transactions = template.transactions_vec().clone();
    let time_triggers = template.time_triggers().cloned().collect::<Vec<_>>();
    let hashes = template.entrypoint_hashes().collect::<Vec<_>>();
    let results = template
        .results()
        .map(|result| match result.as_ref() {
            Ok(seq) => Ok(seq.clone()),
            Err(err) => Err(err.clone()),
        })
        .collect::<Vec<_>>();

    rebuild_block_from_parts(
        template,
        transactions,
        time_triggers,
        hashes,
        results,
        genesis_key_pair,
    )
}

fn rebuild_block_from_parts(
    template: &iroha_data_model::block::SignedBlock,
    transactions: Vec<iroha_data_model::transaction::SignedTransaction>,
    time_triggers: Vec<TimeTriggerEntrypoint>,
    hashes: Vec<HashOf<iroha_data_model::transaction::TransactionEntrypoint>>,
    results: Vec<TransactionResultInner>,
    genesis_key_pair: &KeyPair,
) -> iroha_data_model::block::SignedBlock {
    let header = template.payload().header;
    let initial_signature = template.signatures().next().cloned().unwrap_or_else(|| {
        iroha_data_model::block::BlockSignature::new(
            0,
            SignatureOf::from_hash(genesis_key_pair.private_key(), header.hash()),
        )
    });
    let da_commitments = template.da_commitments().cloned();
    let da_proof_policies = template.da_proof_policies().cloned();
    let da_pin_intents = template.da_pin_intents().cloned();

    let signer_index = initial_signature.index();
    let mut working = iroha_data_model::block::SignedBlock::presigned(
        initial_signature,
        header,
        transactions.clone(),
    );
    working.set_da_commitments(da_commitments.clone());
    working.set_da_proof_policies(da_proof_policies.clone());
    working.set_da_pin_intents(da_pin_intents.clone());
    working.set_transaction_results(time_triggers.clone(), &hashes, results.clone());
    let signature = iroha_data_model::block::BlockSignature::new(
        signer_index,
        SignatureOf::from_hash(genesis_key_pair.private_key(), working.hash()),
    );

    let mut rebuilt = iroha_data_model::block::SignedBlock::presigned(
        signature,
        working.payload().header,
        transactions,
    );
    rebuilt.set_da_commitments(da_commitments);
    rebuilt.set_da_proof_policies(da_proof_policies);
    rebuilt.set_da_pin_intents(da_pin_intents);
    rebuilt.set_transaction_results(time_triggers, &hashes, results);
    rebuilt
}

#[cfg(test)]
mod tests {
    use iroha_core::state::StateReadOnly;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        Registrable,
        asset::AssetDefinition,
        domain::Domain,
        isi::{offline::RegisterOfflineAllowance, register::RegisterPeerWithPop},
        parameter::system::SumeragiParameters,
    };
    use norito::codec::Decode;

    use super::*;

    #[test]
    fn base_config_enables_confidential_verification() {
        let table = super::base_iroha_config();
        let confidential = table
            .get("confidential")
            .and_then(|value| value.as_table())
            .expect("confidential section present");
        assert_eq!(
            confidential
                .get("enabled")
                .and_then(|value| value.as_bool()),
            Some(true),
            "`confidential.enabled` must be true for validator peers"
        );
    }

    #[test]
    fn base_config_enables_extended_telemetry() {
        let table = super::base_iroha_config();
        assert_eq!(
            table
                .get("telemetry_enabled")
                .and_then(|value| value.as_bool()),
            Some(true),
            "telemetry must be enabled for test networks"
        );
        assert_eq!(
            table
                .get("telemetry_profile")
                .and_then(|value| value.as_str()),
            Some("extended"),
            "test networks should expose expensive telemetry metrics"
        );
    }

    #[test]
    fn base_config_includes_debug_rbc_defaults() {
        let table = super::base_iroha_config();
        let debug_rbc = table
            .get("sumeragi")
            .and_then(toml::Value::as_table)
            .and_then(|value| value.get("debug"))
            .and_then(toml::Value::as_table)
            .and_then(|value| value.get("rbc"))
            .and_then(toml::Value::as_table)
            .expect("sumeragi.debug.rbc section present");
        assert_eq!(
            debug_rbc
                .get("shuffle_chunks")
                .and_then(toml::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            debug_rbc
                .get("duplicate_inits")
                .and_then(toml::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            debug_rbc
                .get("corrupt_witness_ack")
                .and_then(toml::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            debug_rbc
                .get("corrupt_ready_signature")
                .and_then(toml::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            debug_rbc
                .get("drop_validator_mask")
                .and_then(toml::Value::as_integer),
            Some(0)
        );
        assert_eq!(
            debug_rbc
                .get("equivocate_chunk_mask")
                .and_then(toml::Value::as_integer),
            Some(0)
        );
        assert_eq!(
            debug_rbc
                .get("equivocate_validator_mask")
                .and_then(toml::Value::as_integer),
            Some(0)
        );
        assert_eq!(
            debug_rbc
                .get("conflicting_ready_mask")
                .and_then(toml::Value::as_integer),
            Some(0)
        );
        assert_eq!(
            debug_rbc
                .get("partial_chunk_mask")
                .and_then(toml::Value::as_integer),
            Some(0)
        );
    }

    #[test]
    fn builds_signed_genesis_block() {
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let block = genesis(Vec::new(), topology, vec![entry]);
        assert!(block.0.signatures().next().is_some());
        assert!(
            block.0.has_results(),
            "genesis block must carry execution results"
        );
        assert!(
            block.0.results().all(|result| result.as_ref().is_ok()),
            "genesis transactions should execute successfully"
        );
    }

    #[test]
    fn genesis_allows_wonderland_assets_from_genesis_authority() {
        use iroha_core::block::check_genesis_block;
        use iroha_data_model::{asset::AssetDefinition, isi::Register};

        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );

        let asset_definition_id: AssetDefinitionId = AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "genesis_extra".parse().unwrap(),
        );
        let instructions = vec![InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id).with_name("Genesis Extra".to_owned()),
        ))];

        let block = genesis(vec![instructions], topology, vec![entry]);
        let genesis_account = AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone());
        check_genesis_block(
            &block.0,
            &genesis_account,
            &ChainId::from("00000000-0000-0000-0000-000000000000"),
        )
        .expect("genesis authority should be permitted to seed wonderland assets");
    }

    #[test]
    fn genesis_executes_offline_allowance_instruction() {
        use std::{
            fs,
            path::{Path, PathBuf},
            str::FromStr,
        };

        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        use iroha_core::{
            block::ValidBlock,
            kura::Kura,
            query::store::LiveQueryStore,
            smartcontracts::ValidQuery,
            state::{State, World},
            sumeragi::network_topology::Topology as CoreTopology,
            telemetry::StateTelemetry,
        };
        use iroha_crypto::{Hash, Signature};
        use iroha_data_model::{
            account::Account,
            isi::InstructionBox,
            metadata::Metadata,
            offline::{
                OFFLINE_ASSET_ENABLED_METADATA_KEY, OfflineAllowanceCommitment,
                OfflineWalletCertificate, OfflineWalletPolicy,
            },
            query::{dsl::CompoundPredicate, offline::prelude::FindOfflineAllowances},
        };
        use iroha_primitives::json::Json;

        fn parse_optional_hash(
            value: Option<&norito::json::Value>,
            path: &Path,
            field: &'static str,
        ) -> Option<Hash> {
            match value {
                None => None,
                Some(value) if value.is_null() => None,
                Some(value) => {
                    let hex = value.as_str().unwrap_or_else(|| {
                        panic!(
                            "offline allowance fixture `{}` `{field}` must be hex string or null",
                            path.display()
                        )
                    });
                    Some(Hash::from_str(hex).unwrap_or_else(|err| {
                        panic!(
                            "offline allowance fixture `{}` `{field}`: {err}",
                            path.display()
                        )
                    }))
                }
            }
        }

        fn parse_offline_certificate_fixture(
            value: &norito::json::Value,
            path: &Path,
        ) -> OfflineWalletCertificate {
            fn decode_hex_bytes(hex_literal: &str) -> Result<Vec<u8>, String> {
                if hex_literal.len() % 2 != 0 {
                    return Err("hex input has odd length".to_owned());
                }
                let mut bytes = Vec::with_capacity(hex_literal.len() / 2);
                for index in (0..hex_literal.len()).step_by(2) {
                    let byte = u8::from_str_radix(&hex_literal[index..index + 2], 16)
                        .map_err(|err| format!("invalid hex at index {index}: {err}"))?;
                    bytes.push(byte);
                }
                Ok(bytes)
            }

            let object = value.as_object().unwrap_or_else(|| {
                panic!(
                    "offline allowance fixture `{}` certificate must be a JSON object",
                    path.display()
                )
            });

            let controller_literal = object
                .get("controller")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing certificate controller",
                        path.display()
                    )
                });
            let controller = AccountId::parse_encoded(controller_literal)
                .map(|parsed| parsed.into_account_id())
                .unwrap_or_else(|err| {
                    panic!("failed to parse controller `{controller_literal}`: {err}")
                });

            let operator_literal = object
                .get("operator")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing certificate operator",
                        path.display()
                    )
                });
            let operator = AccountId::parse_encoded(operator_literal)
                .map(|parsed| parsed.into_account_id())
                .unwrap_or_else(|err| {
                    panic!("failed to parse operator `{operator_literal}`: {err}")
                });

            let allowance = object
                .get("allowance")
                .and_then(norito::json::Value::as_object)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing allowance block",
                        path.display()
                    )
                });
            let asset_literal = allowance
                .get("asset")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing allowance asset",
                        path.display()
                    )
                });
            let asset = asset_literal.parse().unwrap_or_else(|err| {
                panic!("failed to parse allowance asset `{asset_literal}`: {err}")
            });
            let amount_literal = allowance
                .get("amount")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing allowance amount",
                        path.display()
                    )
                });
            let amount = amount_literal.parse().unwrap_or_else(|err| {
                panic!("failed to parse allowance amount `{amount_literal}`: {err}")
            });
            let commitment_hex = allowance
                .get("commitment_hex")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing allowance commitment_hex",
                        path.display()
                    )
                });
            let commitment = decode_hex_bytes(commitment_hex)
                .unwrap_or_else(|err| panic!("commitment_hex decode: {err}"));

            let spend_key_literal = object
                .get("spend_public_key")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing spend_public_key",
                        path.display()
                    )
                });
            let spend_public_key = spend_key_literal.parse().unwrap_or_else(|err| {
                panic!("failed to parse spend_public_key `{spend_key_literal}`: {err}")
            });

            let attestation_b64 = object
                .get("attestation_report_b64")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing attestation_report_b64",
                        path.display()
                    )
                });
            let attestation_report = BASE64
                .decode(attestation_b64)
                .unwrap_or_else(|err| panic!("attestation_report_b64 decode: {err}"));

            let issued_at_ms = object
                .get("issued_at_ms")
                .and_then(norito::json::Value::as_u64)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing issued_at_ms",
                        path.display()
                    )
                });
            let expires_at_ms = object
                .get("expires_at_ms")
                .and_then(norito::json::Value::as_u64)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing expires_at_ms",
                        path.display()
                    )
                });

            let policy = object
                .get("policy")
                .and_then(norito::json::Value::as_object)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing policy block",
                        path.display()
                    )
                });
            let policy_max_balance = policy
                .get("max_balance")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing policy max_balance",
                        path.display()
                    )
                });
            let policy_max_tx_value = policy
                .get("max_tx_value")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing policy max_tx_value",
                        path.display()
                    )
                });
            let policy_expires_at_ms = policy
                .get("expires_at_ms")
                .and_then(norito::json::Value::as_u64)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing policy expires_at_ms",
                        path.display()
                    )
                });

            let operator_signature_hex = object
                .get("operator_signature_hex")
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "offline allowance fixture `{}` missing operator_signature_hex",
                        path.display()
                    )
                });
            let operator_signature = Signature::from_hex(operator_signature_hex)
                .unwrap_or_else(|err| panic!("operator_signature_hex decode: {err}"));

            let metadata_value = object
                .get("metadata")
                .cloned()
                .unwrap_or_else(|| norito::json::Value::Object(Default::default()));
            let metadata: Metadata = norito::json::from_value(metadata_value)
                .unwrap_or_else(|err| panic!("metadata parse: {err}"));

            let verdict_id =
                parse_optional_hash(object.get("verdict_id_hex"), path, "verdict_id_hex");
            let attestation_nonce = parse_optional_hash(
                object.get("attestation_nonce_hex"),
                path,
                "attestation_nonce_hex",
            );
            let refresh_at_ms = object
                .get("refresh_at_ms")
                .and_then(norito::json::Value::as_u64);

            OfflineWalletCertificate {
                controller,
                operator,
                allowance: OfflineAllowanceCommitment {
                    asset,
                    amount,
                    commitment,
                },
                spend_public_key,
                attestation_report,
                issued_at_ms,
                expires_at_ms,
                policy: OfflineWalletPolicy {
                    max_balance: policy_max_balance
                        .parse()
                        .unwrap_or_else(|err| panic!("policy max_balance parse: {err}")),
                    max_tx_value: policy_max_tx_value
                        .parse()
                        .unwrap_or_else(|err| panic!("policy max_tx_value parse: {err}")),
                    expires_at_ms: policy_expires_at_ms,
                },
                operator_signature,
                metadata,
                verdict_id,
                attestation_nonce,
                refresh_at_ms,
            }
        }

        init_instruction_registry();
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/offline_allowance/android-demo/register_instruction.json");
        let raw = fs::read_to_string(&fixture_path)
            .expect("offline allowance fixture should be readable for genesis");
        let fixture: norito::json::Value =
            norito::json::from_str(&raw).expect("fixture should parse as JSON");
        let certificate_value = fixture
            .get("certificate")
            .cloned()
            .expect("fixture must contain `certificate` field");
        let certificate = parse_offline_certificate_fixture(&certificate_value, &fixture_path);
        let controller = certificate.controller.clone();
        let controller_asset_id = certificate.allowance.asset.clone();
        let controller_asset_amount = certificate.allowance.amount.clone();
        let asset_definition_id = controller_asset_id.definition().clone();
        let asset_scale = controller_asset_amount.scale();
        let instruction = InstructionBox::from(RegisterOfflineAllowance { certificate });

        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology: UniqueVec<PeerId> = [peer_id].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let topology_vec: Vec<PeerId> = topology.iter().cloned().collect();
        let block = genesis(vec![vec![instruction]], topology.clone(), vec![entry]);

        let genesis_account = AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone());
        let genesis_domain_id = iroha_genesis::GENESIS_DOMAIN_ID.clone();
        let genesis_account_entry = Account::new(genesis_account.clone()).build(&genesis_account);
        let controller_account_entry = Account::new(controller.clone()).build(&controller);
        let domains = vec![Domain::new(genesis_domain_id).build(&genesis_account)];
        let mut asset_definition =
            AssetDefinition::new(asset_definition_id, NumericSpec::fractional(asset_scale))
                .with_name("Offline Allowance Asset".to_owned())
                .build(&genesis_account);
        asset_definition.metadata_mut().insert(
            OFFLINE_ASSET_ENABLED_METADATA_KEY
                .parse()
                .expect("offline.enabled metadata key should parse"),
            Json::new(true),
        );
        let controller_asset_entry =
            iroha_data_model::asset::Asset::new(controller_asset_id, controller_asset_amount);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            domains,
            vec![genesis_account_entry, controller_account_entry],
            vec![asset_definition],
            vec![controller_asset_entry],
            [],
        );
        // Mirror the production pre-exec bootstrap so direct validation sees the same
        // genesis alias state as `populate_genesis_results`.
        iroha_core::sns::seed_genesis_alias_bootstrap(&mut world, &block.0);
        let state = State::with_telemetry(world, kura, query_handle, StateTelemetry::default());
        let chain = block
            .0
            .transactions_vec()
            .first()
            .map(|tx| tx.chain().clone())
            .unwrap_or_else(chain_id);
        let core_topology = CoreTopology::new(topology_vec.clone());
        let time_source = TimeSource::new_system();
        let mut voting_block = None;
        let validation = ValidBlock::validate_keep_voting_block(
            block.0.clone(),
            &core_topology,
            &chain,
            &genesis_account,
            &time_source,
            &state,
            &mut voting_block,
            false,
        )
        .unpack(|_| {});
        let (validated, mut state_block) =
            validation.expect("genesis block should validate successfully");
        let allowances_after_validation: Vec<_> =
            ValidQuery::execute(FindOfflineAllowances, CompoundPredicate::PASS, &state_block)
                .expect("query offline allowances from genesis state")
                .collect();
        assert_eq!(
            allowances_after_validation.len(),
            1,
            "offline allowance from genesis fixture must be registered during genesis validation"
        );

        let committed_block = validated.commit_unchecked().unpack(|_| {});
        let _ = state_block.apply_without_execution(&committed_block, topology_vec.clone());
        state_block
            .commit()
            .expect("genesis state should commit successfully");

        let view = state.view();
        let allowances_after_commit: Vec<_> =
            ValidQuery::execute(FindOfflineAllowances, CompoundPredicate::PASS, &view)
                .expect("query offline allowances after committing genesis")
                .collect();
        assert_eq!(
            allowances_after_commit.len(),
            1,
            "offline allowance must persist after committing genesis"
        );
    }

    #[test]
    fn ensure_genesis_results_populates_when_preexecution_succeeds() {
        init_instruction_registry();
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()]
            .into_iter()
            .collect::<iroha_primitives::unique_vec::UniqueVec<_>>();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let (mut block, genesis_account, topology_vec, genesis_key_pair) =
            super::build_minimal_genesis_unexecuted(
                Vec::new(),
                topology,
                vec![entry],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
            );
        assert_eq!(
            block.0.results().count(),
            0,
            "freshly built genesis block should lack transaction results"
        );
        super::ensure_genesis_results(
            &mut block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            None,
        );
        assert!(
            block.0.has_results(),
            "ensure_genesis_results must attach execution results"
        );
        assert!(
            block.0.results().all(|result| result.as_ref().is_ok()),
            "pre-executed genesis should yield successful outcomes"
        );
    }

    #[test]
    fn ensure_genesis_results_resigns_mutated_genesis_with_existing_results() {
        init_instruction_registry();
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()]
            .into_iter()
            .collect::<iroha_primitives::unique_vec::UniqueVec<_>>();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let (mut block, genesis_account, topology_vec, genesis_key_pair) =
            super::build_minimal_genesis_unexecuted(
                Vec::new(),
                topology,
                vec![entry],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
            );
        super::ensure_genesis_results(
            &mut block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            None,
        );
        assert!(
            block.0.has_results(),
            "precondition: genesis has execution results"
        );

        block.0.set_da_proof_policies(Some(
            iroha_data_model::da::commitment::DaProofPolicyBundle::new(Vec::new()),
        ));
        let stale_signature = block
            .0
            .signatures()
            .next()
            .expect("genesis signature present")
            .signature()
            .verify_hash(genesis_key_pair.public_key(), block.0.hash())
            .is_err();
        assert!(
            stale_signature,
            "mutating header should stale existing signature"
        );

        super::ensure_genesis_results(
            &mut block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            None,
        );

        let signatures: Vec<_> = block.0.signatures().collect();
        assert_eq!(
            signatures.len(),
            1,
            "genesis should keep a single canonical signature"
        );
        assert!(
            signatures[0]
                .signature()
                .verify_hash(genesis_key_pair.public_key(), block.0.hash())
                .is_ok(),
            "genesis signature must be refreshed after metadata mutation"
        );
    }

    #[test]
    fn populate_genesis_results_executes_without_fallback() {
        init_instruction_registry();
        assert!(
            !SumeragiParameters::default().key_require_hsm,
            "defaults no longer require HSM bindings; test peers rely on softkey bindings only when enabled explicitly"
        );
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()]
            .into_iter()
            .collect::<iroha_primitives::unique_vec::UniqueVec<_>>();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let (block, genesis_account, topology_vec, genesis_key_pair) =
            super::build_minimal_genesis_unexecuted(
                Vec::new(),
                topology,
                vec![entry],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
            );
        let executed = super::populate_genesis_results(
            &block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            None,
        )
        .expect("genesis pre-execution should succeed");
        assert!(
            executed.results().all(|result| result.as_ref().is_ok()),
            "pre-executed genesis should not carry errors when HSM bindings are optional"
        );
    }

    #[test]
    fn populate_genesis_results_accepts_block_proof_policies() {
        use std::num::NonZeroU32;

        use iroha_data_model::nexus::{LaneCatalog, LaneConfig, LaneId};

        init_instruction_registry();
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let lane0 = LaneConfig {
            id: LaneId::from_lane_index(0, lane_count).expect("lane 0 id"),
            alias: "alpha".to_string(),
            ..LaneConfig::default()
        };
        let lane1 = LaneConfig {
            id: LaneId::from_lane_index(1, lane_count).expect("lane 1 id"),
            alias: "beta".to_string(),
            ..LaneConfig::default()
        };
        let catalog =
            LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("lane catalog should validate");

        let nexus = ActualNexus {
            enabled: true,
            lane_catalog: catalog.clone(),
            lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&catalog),
            ..Default::default()
        };
        let policies = iroha_core::da::proof_policy_bundle(&nexus.lane_config);

        let (block, genesis_account, topology_vec, genesis_key_pair) =
            super::build_minimal_genesis_unexecuted_with_post_topology(
                Vec::new(),
                Vec::new(),
                topology,
                vec![entry],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
                super::chain_id(),
                None,
                Some(policies),
                None,
                None,
            );
        let executed = super::populate_genesis_results(
            &block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            None,
        )
        .expect("genesis pre-execution should accept proof-policy-derived catalogs");
        assert!(
            executed.results().all(|result| result.as_ref().is_ok()),
            "pre-executed genesis should succeed with custom lane config"
        );
    }

    #[test]
    fn populate_genesis_results_uses_supplied_nexus_config_for_custom_staking_genesis() {
        use std::num::NonZeroU32;

        use iroha_data_model::nexus::{DataSpaceId, LaneCatalog, LaneConfig, LaneId};
        use iroha_data_model::{
            isi::{
                Register,
                staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
            },
            prelude::Numeric,
        };

        init_instruction_registry();
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );

        let validator_id = AccountId::new(peer_id.public_key().clone());
        let nexus_domain: DomainId = "nexus".parse().expect("nexus domain");
        let stake_asset_id = AssetDefinitionId::new(
            nexus_domain.clone(),
            "multilane_stake".parse().expect("stake asset name"),
        );
        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let lane_zero = LaneConfig {
            id: LaneId::from_lane_index(0, lane_count).expect("lane 0 id"),
            alias: "nexus".to_owned(),
            ..LaneConfig::default()
        };
        let lane_one = LaneConfig {
            id: LaneId::from_lane_index(1, lane_count).expect("lane 1 id"),
            alias: "ds1".to_owned(),
            dataspace_id: DataSpaceId::new(7),
            ..LaneConfig::default()
        };
        let catalog = LaneCatalog::new(lane_count, vec![lane_zero, lane_one.clone()])
            .expect("lane catalog should validate");
        let nexus = ActualNexus {
            enabled: true,
            staking: iroha_config::parameters::actual::NexusStaking {
                stake_asset_id: stake_asset_id.to_string(),
                ..Default::default()
            },
            lane_catalog: catalog.clone(),
            lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&catalog),
            ..Default::default()
        };

        let post_topology_transactions = vec![vec![
            Register::domain(Domain::new(nexus_domain.clone())).into(),
            Register::account(Account::new(validator_id.clone())).into(),
            Register::asset_definition({
                let __asset_definition_id = stake_asset_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .into(),
            Mint::asset_numeric(
                10_u32,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
            RegisterPublicLaneValidator::new(
                lane_one.id,
                validator_id.clone(),
                peer_id.clone(),
                validator_id.clone(),
                Numeric::from(10_u32),
                Metadata::default(),
            )
            .into(),
            ActivatePublicLaneValidator::new(lane_one.id, validator_id.clone()).into(),
        ]];

        let (block, genesis_account, topology_vec, genesis_key_pair) =
            super::build_minimal_genesis_unexecuted_with_post_topology(
                Vec::new(),
                post_topology_transactions,
                topology,
                vec![entry],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
                super::chain_id(),
                None,
                None,
                Some(nexus.clone()),
                None,
            );

        let err = super::populate_genesis_results(
            &block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            None,
        )
        .expect_err("custom staking genesis should fail without the supplied nexus config");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("stake asset definition missing")
                || rendered.contains("nexus.staking.stake_asset_id")
                || rendered.contains("Find(AssetDefinition("),
            "unexpected pre-exec error without nexus config: {err:?}"
        );

        let executed = super::populate_genesis_results(
            &block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            Some(&nexus),
        )
        .expect("custom staking genesis should succeed with the resolved nexus config");
        assert!(
            executed.results().all(|result| result.as_ref().is_ok()),
            "pre-executed custom staking genesis should succeed when the builder threads the resolved nexus config"
        );
    }

    #[test]
    fn populate_genesis_results_leases_genesis_account_labels() {
        use iroha_data_model::{
            account::rekey::{AccountAlias, AccountAliasDomain},
            block::SignedBlock,
            isi::{InstructionBox, Register},
            nexus::DataSpaceId,
            transaction::TransactionBuilder,
        };

        init_instruction_registry();
        let chain_id = super::chain_id();
        let genesis_key_pair = SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone();
        let genesis_account = AccountId::new(genesis_key_pair.public_key().clone());
        let ivm_domain: DomainId = "ivm".parse().expect("ivm domain");
        let gas_label: Name = "gas".parse().expect("gas label");
        let gas_account =
            Account::new(AccountId::new(KeyPair::random().public_key().clone()).clone())
                .with_label(Some(AccountAlias::new(
                    gas_label,
                    Some(AccountAliasDomain::new(ivm_domain.name().clone())),
                    DataSpaceId::GLOBAL,
                )));
        let tx = TransactionBuilder::new(chain_id, genesis_account.clone())
            .with_instructions([
                InstructionBox::from(Register::domain(Domain::new(ivm_domain))),
                InstructionBox::from(Register::account(gas_account)),
            ])
            .sign(genesis_key_pair.private_key());
        let block = GenesisBlock(SignedBlock::genesis(
            vec![tx],
            genesis_key_pair.private_key(),
            None,
            None,
        ));

        let bls = KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
        let topology = vec![PeerId::new(bls.public_key().clone())];
        let executed = super::populate_genesis_results(
            &block,
            &genesis_account,
            &topology,
            &genesis_key_pair,
            None,
        )
        .expect("genesis pre-execution should lease aliases used by labeled genesis accounts");

        assert!(
            executed.results().all(|result| result.as_ref().is_ok()),
            "labeled genesis accounts should not fail SNS lease checks"
        );
    }

    #[test]
    fn preexec_overrides_recompute_lane_config_from_policies() {
        use std::num::NonZeroU32;

        use iroha_data_model::{
            da::commitment::{DaProofPolicy, DaProofPolicyBundle, DaProofScheme},
            nexus::{DataSpaceId, LaneId},
        };

        let genesis_key_pair = SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone();
        let genesis_account = AccountId::new(genesis_key_pair.public_key().clone());
        let genesis_account_entry = Account {
            id: genesis_account,
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = World::with(
            Vec::<iroha_data_model::domain::Domain>::new(),
            vec![genesis_account_entry],
            Vec::<iroha_data_model::asset::AssetDefinition>::new(),
        );
        let mut state = State::with_telemetry(world, kura, query_handle, StateTelemetry::default());

        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let policy0 = DaProofPolicy {
            lane_id: LaneId::from_lane_index(0, lane_count).expect("lane 0 id"),
            dataspace_id: DataSpaceId::GLOBAL,
            alias: "alpha".to_string(),
            proof_scheme: DaProofScheme::MerkleSha256,
        };
        let policy1 = DaProofPolicy {
            lane_id: LaneId::from_lane_index(1, lane_count).expect("lane 1 id"),
            dataspace_id: DataSpaceId::new(7),
            alias: "beta".to_string(),
            proof_scheme: DaProofScheme::KzgBls12_381,
        };
        let bundle = DaProofPolicyBundle::new(vec![policy0, policy1]);

        super::apply_preexec_nexus_overrides(&mut state, &genesis_key_pair, None, Some(&bundle))
            .expect("preexec should apply proof policy overrides");

        let view = state.view();
        let nexus = view.nexus();
        let expected =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        assert_eq!(
            nexus.lane_config.entries().len(),
            expected.entries().len(),
            "lane config must match updated lane catalog"
        );
        for (actual, expected) in nexus
            .lane_config
            .entries()
            .iter()
            .zip(expected.entries().iter())
        {
            assert_eq!(actual.lane_id, expected.lane_id);
            assert_eq!(actual.dataspace_id, expected.dataspace_id);
            assert_eq!(actual.proof_scheme, expected.proof_scheme);
            assert_eq!(actual.alias, expected.alias);
        }
    }

    #[test]
    fn ensure_genesis_results_falls_back_when_preexecution_fails() {
        init_instruction_registry();
        let empty_topology = iroha_primitives::unique_vec::UniqueVec::new();
        let (mut block, genesis_account, _, genesis_key_pair) =
            super::build_minimal_genesis_unexecuted(
                Vec::new(),
                empty_topology,
                Vec::new(),
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
            );
        assert_eq!(
            block.0.results().count(),
            0,
            "freshly built genesis block should lack transaction results"
        );
        super::ensure_genesis_results(&mut block, &genesis_account, &[], &genesis_key_pair, None);
        assert!(
            block.0.has_results(),
            "fallback path must still populate synthetic results"
        );
        let tx_count = block.0.transactions_vec().len();
        assert_eq!(
            block.0.results().count(),
            tx_count,
            "each genesis transaction needs a matching synthetic result"
        );
        assert!(
            block.0.results().all(|result| result.as_ref().is_ok()),
            "synthetic fallback results should be successes"
        );
    }

    #[test]
    fn genesis_registers_peers_with_pop() {
        use iroha_data_model::transaction::Executable;

        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let block = genesis(Vec::new(), topology, vec![entry]);
        let mut register_pop = 0;
        let mut hsm_bound = 0;
        for tx in block.0.transactions_vec() {
            match tx.instructions() {
                Executable::Instructions(isi) => {
                    for instr in isi {
                        if let Some(isi) = instr.as_any().downcast_ref::<RegisterPeerWithPop>() {
                            register_pop += 1;
                            if isi.hsm.is_some() {
                                hsm_bound += 1;
                            }
                        }
                    }
                }
                Executable::Ivm(_) => {}
                Executable::IvmProved(_) => {}
            }
        }
        assert_eq!(
            register_pop, 1,
            "exactly one RegisterPeerWithPop instruction expected"
        );
        assert_eq!(
            hsm_bound, register_pop,
            "consensus peers in genesis must carry softkey HSM bindings"
        );
    }

    #[test]
    fn genesis_with_crypto_override_embeds_manifest_metadata() {
        use iroha_data_model::{
            isi::SetParameter,
            parameter::{Parameter, system::crypto_metadata},
            transaction::Executable,
        };

        fn embedded_manifest_crypto(block: &GenesisBlock) -> ManifestCrypto {
            for tx in block.0.transactions_vec() {
                let Executable::Instructions(instrs) = tx.instructions() else {
                    continue;
                };
                for instr in instrs {
                    let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>() else {
                        continue;
                    };
                    let Parameter::Custom(custom) = set_param.inner() else {
                        continue;
                    };
                    if custom.id() == &crypto_metadata::manifest_meta_id() {
                        return custom
                            .payload()
                            .try_into_any()
                            .expect("decode embedded crypto manifest");
                    }
                }
            }
            panic!("crypto manifest metadata parameter not found in genesis");
        }

        let allowed_signing = vec![
            Algorithm::Ed25519,
            Algorithm::Secp256k1,
            Algorithm::BlsNormal,
        ];
        let expected =
            super::manifest_crypto_from_actual(&iroha_config::parameters::actual::Crypto {
                allowed_curve_ids:
                    iroha_config::parameters::defaults::crypto::derive_curve_ids_from_algorithms(
                        &allowed_signing,
                    ),
                allowed_signing,
                ..Default::default()
            });

        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = [PeerId::new(bls.public_key().clone())]
            .into_iter()
            .collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let block = super::genesis_with_keypair_and_post_topology_with_policies(
            Vec::new(),
            Vec::new(),
            topology,
            vec![entry],
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
            super::chain_id(),
            Some(expected.clone()),
            None,
            None,
            None,
        );

        assert_eq!(embedded_manifest_crypto(&block), expected);
    }

    #[test]
    fn minimal_genesis_contains_fixture_accounts() {
        use iroha_data_model::{Identifiable, isi::RegisterBox, transaction::Executable};

        init_instruction_registry();

        fn assert_registers_fixture_accounts(
            topology: iroha_primitives::unique_vec::UniqueVec<PeerId>,
            pops: Vec<GenesisTopologyEntry>,
        ) {
            let (block, _, _, _) = build_minimal_genesis_unexecuted(
                Vec::new(),
                topology,
                pops,
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
            );
            let mut saw_alice = false;
            let mut saw_carpenter = false;
            for tx in block.0.transactions_vec() {
                if let Executable::Instructions(instrs) = tx.instructions() {
                    for instr in instrs {
                        if let Some(RegisterBox::Account(isi)) =
                            instr.as_any().downcast_ref::<RegisterBox>()
                            && isi.object().id() == &*ALICE_ID
                        {
                            saw_alice = true;
                            continue;
                        }
                        if let Some(RegisterBox::Account(isi)) =
                            instr.as_any().downcast_ref::<RegisterBox>()
                            && isi.object().id().signatory() == CARPENTER_KEYPAIR.public_key()
                        {
                            saw_carpenter = true;
                            continue;
                        }
                    }
                }
            }
            assert!(saw_alice, "minimal genesis should register ALICE_ID");
            assert!(
                saw_carpenter,
                "minimal genesis should register a fixture account in garden_of_live_flowers"
            );
        }

        let empty_topology = iroha_primitives::unique_vec::UniqueVec::new();
        assert_registers_fixture_accounts(empty_topology, Vec::new());

        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        assert_registers_fixture_accounts(topology, vec![entry]);
    }

    #[test]
    fn genesis_grants_alice_soracloud_management_permission() {
        use iroha_data_model::{isi::GrantBox, transaction::Executable};

        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let block = genesis(Vec::new(), topology, vec![entry]);
        let mut saw_permission = false;
        for tx in block.0.transactions_vec() {
            let Executable::Instructions(instrs) = tx.instructions() else {
                continue;
            };
            for instr in instrs {
                let Some(GrantBox::Permission(grant)) = instr.as_any().downcast_ref::<GrantBox>()
                else {
                    continue;
                };
                if grant.destination == *ALICE_ID && grant.object.name() == "CanManageSoracloud" {
                    saw_permission = true;
                    break;
                }
            }
            if saw_permission {
                break;
            }
        }
        assert!(
            saw_permission,
            "default test-network genesis should grant ALICE_ID CanManageSoracloud"
        );
    }

    #[test]
    fn sanitize_strings_removes_whitespace() {
        let mut v = norito::json!({"name": "foo bar"});
        super::sanitize_strings(&mut v);
        assert_eq!(v["name"], norito::json!("foo_bar"));
    }

    #[test]
    fn sanitize_account_id_strips_whitespace() {
        let raw = "foo bar@baz qux";
        let sanitized = super::sanitize_account_id_str(raw);
        assert_eq!(sanitized, "foo_bar@baz_qux");
    }

    #[test]
    fn genesis_contains_upgrade_instruction() {
        use iroha_data_model::{isi::Upgrade, transaction::Executable};

        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let block = genesis(Vec::new(), topology, vec![entry]);
        let first_tx = block.0.transactions_vec().first().unwrap();
        let Executable::Instructions(isi) = first_tx.instructions() else {
            panic!("expected instructions in first transaction");
        };
        let sample = iroha_test_samples::sample_ivm_path("default_executor");
        let has_sample = std::fs::metadata(&sample)
            .map(|m| m.len() > 0)
            .unwrap_or(false);
        if has_sample {
            assert_eq!(isi.len(), 1);
            assert!(isi[0].as_any().downcast_ref::<Upgrade>().is_some());
        } else {
            // When no sample executor is available, we skip the upgrade in genesis.
            // Ensure we still have some instructions (e.g., domain/account bootstrap).
            assert!(!isi.is_empty());
        }
    }

    #[test]
    fn genesis_includes_confidential_digest() {
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let block = genesis(Vec::new(), topology, vec![entry]);
        assert!(
            block.0.header().confidential_features().is_some(),
            "genesis block must advertise confidential feature digest"
        );
    }

    #[test]
    fn genesis_topology_entry_norito_roundtrip() {
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let pop =
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation");
        let peer_id = PeerId::new(bls.public_key().clone());
        let entry = GenesisTopologyEntry::new(peer_id.clone(), pop.clone());
        let encoded = norito::codec::encode_adaptive(&entry);
        let decoded = GenesisTopologyEntry::decode(&mut encoded.as_slice())
            .expect("decode GenesisTopologyEntry");
        assert_eq!(decoded.peer, peer_id);
        assert_eq!(decoded.pop_hex, entry.pop_hex);
    }
}
