//! Sample configuration builders

use std::{collections::BTreeMap, path::PathBuf};

use color_eyre::{Report, eyre::eyre};
use iroha_config::base::toml::WriteExt;
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
    ChainId,
    account::{Account, AccountId},
    asset::{AssetDefinitionId, id::AssetId},
    consensus::HsmBinding,
    domain::DomainId,
    isi::{Grant, InstructionBox, Mint, register::RegisterPeerWithPop},
    metadata::Metadata,
    name::Name,
    parameter::{Parameter, custom::CustomParameter, system::confidential_metadata},
    peer::PeerId,
    prelude::{HashOf, Transfer},
    transaction::signed::TransactionResultInner,
    trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
};
use iroha_executor_data_model::permission::{
    account::CanRegisterAccount,
    asset::{CanMintAssetWithDefinition, CanTransferAssetWithDefinition},
    asset_definition::CanRegisterAssetDefinition,
    domain::{CanRegisterDomain, CanUnregisterDomain},
    executor::CanUpgradeExecutor,
    parameter::CanSetParameters,
    peer::CanManagePeers,
    role::CanManageRoles,
    trigger::CanRegisterTrigger,
};
use iroha_genesis::{GenesisBlock, GenesisBuilder, GenesisTopologyEntry};
use iroha_primitives::{json::Json, numeric::NumericSpec, time::TimeSource, unique_vec::UniqueVec};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
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
    sanitize_account_id_str(&id.to_string())
        .parse()
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

fn build_minimal_genesis(
    extra_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
) -> GenesisBlock {
    let (mut block, genesis_account, topology_vec, genesis_key_pair) =
        build_minimal_genesis_unexecuted(
            extra_transactions,
            topology,
            topology_entries,
            genesis_key_pair,
        );
    ensure_genesis_results(
        &mut block,
        &genesis_account,
        &topology_vec,
        &genesis_key_pair,
    );
    block
}

fn build_minimal_genesis_unexecuted(
    extra_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
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

    let chain = chain_id();
    let genesis_account = AccountId::new(
        iroha_genesis::GENESIS_DOMAIN_ID.clone(),
        genesis_key_pair.public_key().clone(),
    );
    let genesis_id = sanitize_account_id(&genesis_account);
    let alice_id = sanitize_account_id(&ALICE_ID);
    let ivm_dir = default_ivm_dir();

    let mut builder = if let Some(executor_path) = try_default_executor_path() {
        GenesisBuilder::new(chain.clone(), executor_path, ivm_dir.clone())
    } else {
        GenesisBuilder::new_without_executor(chain.clone(), ivm_dir.clone())
    };

    let wonderland_name: Name = "wonderland".parse().expect("wonderland domain");
    let rose_name: Name = "rose".parse().expect("rose asset name");
    let camomile_name: Name = "camomile".parse().expect("camomile asset name");
    let alice_metadata = Metadata::default();

    builder = builder
        .domain(wonderland_name.clone())
        .account_with_metadata(ALICE_KEYPAIR.public_key().clone(), alice_metadata)
        .account(BOB_KEYPAIR.public_key().clone())
        .asset(rose_name, NumericSpec::default())
        .asset(camomile_name, NumericSpec::default())
        .finish_domain();

    let wonderland_domain: DomainId = "wonderland".parse().expect("wonderland domain id");
    let rose_definition_id: AssetDefinitionId = "rose#wonderland".parse().expect("rose def");
    let camomile_definition_id: AssetDefinitionId =
        "camomile#wonderland".parse().expect("camomile def");
    let rose_asset_id = AssetId::new(rose_definition_id.clone(), alice_id.clone());

    builder = builder.append_instruction(Transfer::domain(
        genesis_id.clone(),
        wonderland_domain.clone(),
        alice_id.clone(),
    ));
    builder = builder.append_instruction(Mint::asset_numeric(13u32, rose_asset_id));

    builder = builder.next_transaction();

    let test_domain_id: DomainId = "domain".parse().expect("domain id");
    let and_domain_id: DomainId = "and".parse().expect("and domain id");
    let xor_asset_def: AssetDefinitionId = "xor#domain".parse().expect("xor asset def");
    let may_and_def: AssetDefinitionId = "MAY#and".parse().expect("MAY asset def");

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
            CanRegisterAssetDefinition {
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
            CanRegisterAssetDefinition {
                domain: wonderland_domain.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanRegisterAssetDefinition {
                domain: wonderland_domain.clone(),
            },
            genesis_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanRegisterAccount {
                domain: and_domain_id.clone(),
            },
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanRegisterAssetDefinition {
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

    let conf_param = Parameter::Custom(CustomParameter::new(
        confidential_metadata::registry_root_id(),
        Json::new(norito::json!({ "vk_set_hash": null })),
    ));
    builder = builder.append_parameter(conf_param);

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
) {
    let tx_count = block.0.transactions_vec().len();
    let result_count = block.0.results().count();
    if tx_count == 0 || tx_count == result_count {
        return;
    }

    match populate_genesis_results(block, genesis_account, topology, genesis_key_pair) {
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

fn populate_genesis_results(
    block: &GenesisBlock,
    genesis_account: &AccountId,
    topology: &[PeerId],
    genesis_key_pair: &KeyPair,
) -> Result<iroha_data_model::block::SignedBlock, Report> {
    if topology.is_empty() {
        return Err(eyre!("genesis topology is empty"));
    }

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let genesis_account_entry = Account {
        id: genesis_account.clone(),
        metadata: Metadata::default(),
        label: None,
        uaid: None,
    };
    let world = World::with(
        Vec::<iroha_data_model::domain::Domain>::new(),
        vec![genesis_account_entry],
        Vec::<iroha_data_model::asset::AssetDefinition>::new(),
    );
    let state = State::with_telemetry(world, kura, query_handle, StateTelemetry::default());
    let core_topology = CoreTopology::new(topology.to_vec());
    let chain = block
        .0
        .transactions_vec()
        .first()
        .map(|tx| tx.chain().clone())
        .unwrap_or_else(chain_id);

    let mut voting_block = None;
    let time_source = TimeSource::new_system();
    let effective_genesis_account = block
        .0
        .transactions_vec()
        .first()
        .map(|tx| tx.authority().clone())
        .unwrap_or_else(|| genesis_account.clone());
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
    let (valid_block, state_block) = validation.map_err(|(_, err)| Report::new(err))?;
    drop(state_block);
    let signed_block: iroha_data_model::block::SignedBlock = valid_block.into();
    Ok(rebuild_block_with_results(&signed_block, genesis_key_pair))
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

    let signer_index = initial_signature.index();
    let mut working = iroha_data_model::block::SignedBlock::presigned(
        initial_signature,
        header,
        transactions.clone(),
    );
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
    rebuilt.set_transaction_results(time_triggers, &hashes, results);
    rebuilt
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
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

        let asset_definition_id: AssetDefinitionId = "genesis_extra#wonderland"
            .parse()
            .expect("asset definition id");
        let instructions = vec![InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id),
        ))];

        let block = genesis(vec![instructions], topology, vec![entry]);
        let genesis_account = AccountId::new(
            iroha_genesis::GENESIS_DOMAIN_ID.clone(),
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
        );
        check_genesis_block(
            &block.0,
            &genesis_account,
            &ChainId::from("00000000-0000-0000-0000-000000000000"),
        )
        .expect("genesis authority should be permitted to seed wonderland assets");
    }

    #[test]
    fn genesis_executes_offline_allowance_instruction() {
        use std::{fs, path::PathBuf};

        use iroha_core::{
            block::ValidBlock,
            kura::Kura,
            query::store::LiveQueryStore,
            smartcontracts::ValidQuery,
            state::{State, World},
            sumeragi::network_topology::Topology as CoreTopology,
            telemetry::StateTelemetry,
        };
        use iroha_data_model::{
            account::Account,
            isi::InstructionBox,
            metadata::Metadata,
            offline::OfflineWalletCertificate,
            query::{dsl::CompoundPredicate, offline::prelude::FindOfflineAllowances},
        };

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
        let certificate: OfflineWalletCertificate =
            norito::json::from_value(certificate_value).expect("fixture certificate should decode");
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

        let genesis_account = AccountId::new(
            iroha_genesis::GENESIS_DOMAIN_ID.clone(),
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
        );
        let genesis_account_entry = Account {
            id: genesis_account.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
        };
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = World::with(
            Vec::<iroha_data_model::domain::Domain>::new(),
            vec![genesis_account_entry],
            Vec::<iroha_data_model::asset::AssetDefinition>::new(),
        );
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
        )
        .expect("genesis pre-execution should succeed");
        assert!(
            executed.results().all(|result| result.as_ref().is_ok()),
            "pre-executed genesis should not carry errors when HSM bindings are optional"
        );
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
        super::ensure_genesis_results(&mut block, &genesis_account, &[], &genesis_key_pair);
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
    fn minimal_genesis_contains_wonderland_account() {
        use iroha_data_model::{Identifiable, isi::RegisterBox, transaction::Executable};

        init_instruction_registry();

        fn assert_registers_alice(
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
            for tx in block.0.transactions_vec() {
                if let Executable::Instructions(instrs) = tx.instructions() {
                    for instr in instrs {
                        if let Some(RegisterBox::Account(isi)) =
                            instr.as_any().downcast_ref::<RegisterBox>()
                            && isi.object().id() == &*ALICE_ID
                        {
                            saw_alice = true;
                            break;
                        }
                    }
                }
            }
            assert!(
                saw_alice,
                "minimal genesis should register alice@wonderland"
            );
        }

        let empty_topology = iroha_primitives::unique_vec::UniqueVec::new();
        assert_registers_alice(empty_topology, Vec::new());

        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        assert_registers_alice(topology, vec![entry]);
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
