//! Sample configuration builders
use crate::init_instruction_registry;
use color_eyre::{Report, eyre::eyre};
use iroha_config::base::toml::WriteExt;
use iroha_config::parameters::{
    actual::{
        Crypto as ActualCrypto, Nexus as ActualNexus, Pipeline as ActualPipeline,
        Root as ActualRoot, Zk as ActualZk,
    },
    defaults,
};
use iroha_core::{
    block::ValidBlock,
    compliance::LaneComplianceEngine,
    governance::manifest::LaneManifestRegistry,
    query::store::LiveQueryStore,
    state::{State, World},
    sumeragi::network_topology::Topology as CoreTopology,
};
use iroha_crypto::{Hash, KeyPair, MerkleTree, SignatureOf};
use iroha_data_model::{
    ChainId, Registrable as _,
    account::{Account, AccountId},
    asset::{AssetDefinitionId, definition::AssetDefinition, id::AssetId},
    block::consensus_v2::{ConsensusMode as WireConsensusMode, SumeragiV2GenesisContextParameters},
    da::commitment::DaProofPolicyBundle,
    domain::{Domain, DomainId},
    hijiri::HijiriParametersV1,
    isi::{
        Grant, InstructionBox, Mint, SetParameter,
        kagemusha_v1::{
            KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterTemplateV1,
            KagemushaMintFinalityGenesisParametersV1,
        },
        register::{Register, RegisterPeerWithPop},
    },
    metadata::Metadata,
    name::Name,
    parameter::{
        Parameter,
        custom::CustomParameter,
        system::{
            ConsensusHandshakeMetadata, SumeragiConsensusMode, confidential_metadata,
            consensus_metadata,
        },
    },
    peer::PeerId,
    permission::Permission,
    prelude::{HashOf, Transfer},
    transaction::{Executable, signed::TransactionResultInner},
    trigger::TimeTriggerEntrypoint,
};
use iroha_executor_data_model::permission::{
    account::CanRegisterAccount,
    asset::{CanMintAssetWithDefinition, CanTransferAssetWithDefinition},
    domain::CanUnregisterDomain,
    executor::CanUpgradeExecutor,
    governance::CanManageParliament,
    parameter::{CanSetHijiriParameters, CanSetParameters},
    peer::CanManagePeers,
    query::CanReadAllLedgerData,
    role::CanManageRoles,
    trigger::CanRegisterTrigger,
};
use iroha_genesis::{GenesisBlock, GenesisBuilder, GenesisTopologyEntry, ManifestCrypto};
use iroha_primitives::{json::Json, numeric::NumericSpec, time::TimeSource, unique_vec::UniqueVec};
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, CARPENTER_ID, CARPENTER_KEYPAIR,
    SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
};
#[cfg(test)]
use norito::json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
};
use toml::Table;
/// Exact policy commitments derived by isolated genesis pre-execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StagedGenesisPolicyHashes {
    /// Nexus/AMX execution context committed by the signed genesis carrier.
    pub nexus_amx: Hash,
    /// Complete process-local execution policy committed by the signed genesis carrier.
    pub execution_policy: Hash,
}
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
    AccountId::parse_encoded(&sanitized).expect("sanitized AccountId should parse")
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
        .write("telemetry_profile", "extended")
        // There is no need in persistence in tests.
        .write(["snapshot", "mode"], "disabled")
        .write(["kura", "store_dir"], "./storage")
        .write(
            ["kura", "lane_history_retention"],
            i64::try_from(defaults::kura::LANE_HISTORY_RETENTION.get())
                .expect("Kura lane-history retention default fits a TOML integer"),
        )
        // Default to broadcasting blocks to the entire test topology so small networks
        // do not stall waiting for block sync retries when some peers miss a gossip hop.
        .write(["network", "block_gossip_size"], 256)
        .write(["confidential", "enabled"], true)
        .write(["logger", "level"], "INFO")
        .write(["logger", "format"], "pretty")
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
    // This avoids treating `defaults/genesis.template.json` as a runtime manifest and keeps the
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
        None,
        None,
        Some(iroha_core::state::default_genesis_confidential_policy_hash()),
    )
}

/// Build and sign the default genesis with post-topology instructions without
/// pre-executing its transactions.
///
/// This is the internal half of the custom-`NetworkBuilder` path. The builder
/// must pre-execute the returned block under its fully merged runtime
/// configuration before any peer starts; direct node startup must never use
/// this block as prepared genesis.
pub(crate) fn genesis_unexecuted_with_keypair_and_post_topology(
    extra_transactions: Vec<Vec<InstructionBox>>,
    post_topology_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
) -> GenesisBlock {
    init_instruction_registry();
    build_minimal_genesis_unexecuted_with_post_topology(
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
        None,
        None,
        Some(iroha_core::state::default_genesis_confidential_policy_hash()),
    )
    .0
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
    zk_config: Option<ActualZk>,
    consensus_handshake_meta: Option<Parameter>,
    consensus_mode_override: Option<SumeragiConsensusMode>,
    confidential_policy_hash: Option<[u8; 32]>,
) -> GenesisBlock {
    genesis_with_keypair_and_post_topology_with_policies_and_staged_hash(
        extra_transactions,
        post_topology_transactions,
        topology,
        topology_entries,
        genesis_key_pair,
        chain_id,
        genesis_crypto,
        da_proof_policies,
        None,
        _nexus_config,
        zk_config,
        None,
        consensus_handshake_meta,
        consensus_mode_override,
        confidential_policy_hash,
    )
    .0
}
pub(crate) fn genesis_with_keypair_and_post_topology_with_policies_and_staged_hash(
    extra_transactions: Vec<Vec<InstructionBox>>,
    post_topology_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
    chain_id: ChainId,
    genesis_crypto: Option<ManifestCrypto>,
    da_proof_policies: Option<DaProofPolicyBundle>,
    pipeline_config: Option<ActualPipeline>,
    nexus_config: Option<ActualNexus>,
    zk_config: Option<ActualZk>,
    runtime_config: Option<ActualRoot>,
    consensus_handshake_meta: Option<Parameter>,
    consensus_mode_override: Option<SumeragiConsensusMode>,
    confidential_policy_hash: Option<[u8; 32]>,
) -> (GenesisBlock, StagedGenesisPolicyHashes) {
    init_instruction_registry();
    build_minimal_genesis_with_post_topology_and_staged_hash(
        extra_transactions,
        post_topology_transactions,
        topology,
        topology_entries,
        genesis_key_pair,
        chain_id,
        genesis_crypto,
        da_proof_policies,
        pipeline_config,
        nexus_config,
        zk_config,
        runtime_config,
        consensus_handshake_meta,
        consensus_mode_override,
        confidential_policy_hash,
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
fn decode_consensus_handshake_metadata(
    parameter: &Parameter,
) -> Result<ConsensusHandshakeMetadata, Report> {
    let Parameter::Custom(custom) = parameter else {
        return Err(eyre!(
            "consensus handshake metadata must be encoded as a custom parameter"
        ));
    };
    if custom.id() != &consensus_metadata::handshake_meta_id() {
        return Err(eyre!(
            "consensus handshake metadata has unexpected parameter id `{}`",
            custom.id()
        ));
    }
    let metadata: ConsensusHandshakeMetadata = norito::json::from_str(custom.payload().get())
        .map_err(|error| eyre!("failed to decode consensus handshake metadata: {error}"))?;
    metadata
        .validate()
        .map_err(|error| eyre!("invalid consensus handshake metadata: {error}"))?;
    Ok(metadata)
}

fn test_kagemusha_mint_finality_genesis_parameters(
    topology: &UniqueVec<PeerId>,
) -> KagemushaMintFinalityGenesisParametersV1 {
    let mut voters = topology.iter().cloned().collect::<Vec<_>>();
    voters.sort();
    let validators = voters
        .into_iter()
        .enumerate()
        .map(|(index, validator)| {
            let seed_byte = 0xA0_u8.wrapping_add(
                u8::try_from(index).expect("test-network validator index fits in one byte"),
            );
            iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &[seed_byte; 32],
                0,
                validator,
            )
            .expect("derive independent test-only paired-Pasta validator keys")
        })
        .collect();
    let epoch_roster = KagemushaMintFinalityEpochRosterTemplateV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        epoch: 0,
        validators,
    };
    let parameters = KagemushaMintFinalityGenesisParametersV1 {
        epoch_roster,
        next_epoch_roster: None,
    };
    parameters
        .validate()
        .expect("test-network topology must form a canonical mint-finality template");
    parameters
}
fn signed_genesis_consensus_mode(block: &GenesisBlock) -> Result<WireConsensusMode, Report> {
    let mut metadata_entries = Vec::new();
    for transaction in block.0.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(eyre!(
                "signed genesis consensus metadata must be carried by instruction batches"
            ));
        };
        for set_parameter in instructions
            .iter()
            .filter_map(|instruction| instruction.as_any().downcast_ref::<SetParameter>())
        {
            if matches!(
                set_parameter.inner(),
                Parameter::Custom(custom)
                    if custom.id() == &consensus_metadata::handshake_meta_id()
            ) {
                metadata_entries.push(decode_consensus_handshake_metadata(set_parameter.inner())?);
            }
        }
    }
    let [metadata] = metadata_entries.as_slice() else {
        return Err(eyre!(
            "signed genesis requires exactly one consensus handshake metadata entry, found {}",
            metadata_entries.len()
        ));
    };
    Ok(match metadata.mode {
        SumeragiConsensusMode::Permissioned => WireConsensusMode::Permissioned,
        SumeragiConsensusMode::Npos => WireConsensusMode::Npos,
    })
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
        None,
        None,
        Some(iroha_core::state::default_genesis_confidential_policy_hash()),
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
    zk_config: Option<ActualZk>,
    consensus_handshake_meta: Option<Parameter>,
    consensus_mode_override: Option<SumeragiConsensusMode>,
    confidential_policy_hash: Option<[u8; 32]>,
) -> GenesisBlock {
    build_minimal_genesis_with_post_topology_and_staged_hash(
        extra_transactions,
        post_topology_transactions,
        topology,
        topology_entries,
        genesis_key_pair,
        chain_id,
        genesis_crypto,
        da_proof_policies,
        None,
        nexus_config,
        zk_config,
        None,
        consensus_handshake_meta,
        consensus_mode_override,
        confidential_policy_hash,
    )
    .0
}
fn build_minimal_genesis_with_post_topology_and_staged_hash(
    extra_transactions: Vec<Vec<InstructionBox>>,
    post_topology_transactions: Vec<Vec<InstructionBox>>,
    topology: UniqueVec<PeerId>,
    topology_entries: Vec<GenesisTopologyEntry>,
    genesis_key_pair: KeyPair,
    chain_id: ChainId,
    genesis_crypto: Option<ManifestCrypto>,
    da_proof_policies: Option<DaProofPolicyBundle>,
    pipeline_config: Option<ActualPipeline>,
    nexus_config: Option<ActualNexus>,
    zk_config: Option<ActualZk>,
    runtime_config: Option<ActualRoot>,
    consensus_handshake_meta: Option<Parameter>,
    consensus_mode_override: Option<SumeragiConsensusMode>,
    confidential_policy_hash: Option<[u8; 32]>,
) -> (GenesisBlock, StagedGenesisPolicyHashes) {
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
            zk_config.clone(),
            consensus_handshake_meta,
            consensus_mode_override,
            confidential_policy_hash,
        );
    let (signed_block, staged_hash) = preexecute_genesis_with_runtime_config(
        &block,
        &genesis_account,
        &topology_vec,
        &genesis_key_pair,
        pipeline_config.as_ref(),
        nexus_config.as_ref(),
        zk_config.as_ref(),
        runtime_config.as_ref(),
    )
    .expect("minimal genesis must pre-execute without synthetic results");
    block.0 = signed_block;
    (block, staged_hash)
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
        None,
        None,
        Some(iroha_core::state::default_genesis_confidential_policy_hash()),
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
    nexus_config: Option<ActualNexus>,
    _zk_config: Option<ActualZk>,
    consensus_handshake_meta: Option<Parameter>,
    consensus_mode_override: Option<SumeragiConsensusMode>,
    confidential_policy_hash: Option<[u8; 32]>,
) -> (GenesisBlock, AccountId, Vec<PeerId>, KeyPair) {
    fn append_external_genesis_transaction(
        mut builder: GenesisBuilder,
        instructions: Vec<InstructionBox>,
        vk_registry_instructions: &mut Vec<InstructionBox>,
    ) -> GenesisBuilder {
        if instructions.is_empty() {
            return builder;
        }

        vk_registry_instructions.extend(instructions.iter().cloned());
        let mut transaction_instructions = Vec::with_capacity(instructions.len());
        for instruction in instructions {
            if let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>() {
                builder = builder.append_parameter(set_parameter.inner().clone());
            } else {
                transaction_instructions.push(instruction);
            }
        }
        if transaction_instructions.is_empty() {
            return builder;
        }

        builder = builder.next_transaction();
        for instruction in transaction_instructions {
            builder = builder.append_instruction(instruction);
        }
        builder
    }

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
    let da_proof_policies = da_proof_policies.or_else(|| {
        nexus_config
            .as_ref()
            .map(|nexus| iroha_core::da::active_proof_policy_bundle_at_height(nexus, 1))
    });
    let chain = chain_id.clone();
    let genesis_account = AccountId::new(genesis_key_pair.public_key().clone());
    let genesis_id = sanitize_account_id(&genesis_account);
    let alice_id = sanitize_account_id(&ALICE_ID);
    let bob_id = sanitize_account_id(&BOB_ID);
    let carpenter_id = sanitize_account_id(&CARPENTER_ID);
    let ivm_dir = default_ivm_dir();
    let mut builder = if let Some(executor_path) = try_default_executor_path() {
        GenesisBuilder::new(chain.clone(), executor_path, ivm_dir.clone())
    } else {
        GenesisBuilder::new_without_executor(chain.clone(), ivm_dir.clone())
    };
    let consensus_handshake_metadata = consensus_handshake_meta
        .as_ref()
        .map(decode_consensus_handshake_metadata)
        .transpose()
        .expect("test-network consensus handshake metadata must be canonical");
    if let (Some(metadata), Some(mode_override)) = (
        consensus_handshake_metadata.as_ref(),
        consensus_mode_override,
    ) {
        assert_eq!(
            metadata.mode, mode_override,
            "consensus mode override must agree with signed handshake metadata"
        );
    }
    let consensus_mode = consensus_handshake_metadata.as_ref().map_or_else(
        || consensus_mode_override.unwrap_or(SumeragiConsensusMode::Permissioned),
        |metadata| metadata.mode,
    );
    let (block_cadence_ms, sumeragi_v2, kagemusha_mint_finality) = consensus_handshake_metadata
        .map_or_else(
            || {
                (
                    None,
                    SumeragiV2GenesisContextParameters::recommended(),
                    test_kagemusha_mint_finality_genesis_parameters(&topology),
                )
            },
            |metadata| {
                (
                    Some(metadata.block_cadence_ms),
                    metadata.sumeragi_v2,
                    metadata.kagemusha_mint_finality,
                )
            },
        );
    builder = builder
        .with_sumeragi_v2_context_parameters(sumeragi_v2)
        .with_kagemusha_mint_finality_genesis_parameters(kagemusha_mint_finality);
    if let Some(block_cadence_ms) = block_cadence_ms {
        builder = builder.with_block_cadence_ms(block_cadence_ms);
    }
    if let Some(crypto) = genesis_crypto {
        builder = builder.with_crypto(crypto);
    }
    if let Some(policies) = &da_proof_policies {
        builder = builder.with_da_proof_policies(policies.clone());
    }
    let wonderland_name: Name = "wonderland".parse().expect("wonderland domain");
    let rose_name: Name = "rose".parse().expect("rose asset name");
    let camomile_name: Name = "camomile".parse().expect("camomile asset name");
    let garden_name: Name = "garden_of_live_flowers"
        .parse()
        .expect("garden_of_live_flowers domain");
    let cabbage_name: Name = "cabbage".parse().expect("cabbage asset name");
    let test_domain_name: Name = "domain".parse().expect("test domain");
    let and_domain_name: Name = "and".parse().expect("and domain");
    let xor_name: Name = "xor".parse().expect("xor asset name");
    let may_name: Name = "MAY".parse().expect("MAY asset name");
    let alice_metadata = Metadata::default();
    let universal_dataspace: Name = "universal".parse().expect("universal dataspace");
    let wonderland_domain =
        DomainId::try_new(&wonderland_name, &universal_dataspace).expect("wonderland domain id");
    let garden_domain =
        DomainId::try_new(&garden_name, &universal_dataspace).expect("garden domain id");
    let test_domain_id =
        DomainId::try_new(&test_domain_name, &universal_dataspace).expect("test domain id");
    let and_domain_id =
        DomainId::try_new(&and_domain_name, &universal_dataspace).expect("and domain id");
    builder = builder
        .domain(wonderland_domain.clone())
        .account_with_metadata(ALICE_KEYPAIR.public_key().clone(), alice_metadata)
        .account(BOB_KEYPAIR.public_key().clone())
        .asset(rose_name, NumericSpec::default())
        .asset(camomile_name, NumericSpec::default())
        .finish_domain()
        .domain(garden_domain.clone())
        .account(CARPENTER_KEYPAIR.public_key().clone())
        .asset(cabbage_name, NumericSpec::default())
        .finish_domain()
        .domain(test_domain_id.clone())
        .asset(xor_name, NumericSpec::default())
        .finish_domain()
        .domain(and_domain_id.clone())
        .asset(may_name, NumericSpec::default())
        .finish_domain();
    let wonderland_domain =
        DomainId::parse_fully_qualified("wonderland.universal").expect("wonderland domain id");
    let garden_domain = DomainId::parse_fully_qualified("garden_of_live_flowers.universal")
        .expect("garden_of_live_flowers domain id");
    let rose_definition_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            wonderland_domain.clone(),
            "rose".parse().unwrap(),
        );
    let camomile_definition_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            wonderland_domain.clone(),
            "camomile".parse().unwrap(),
        );
    let cabbage_definition_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        garden_domain.clone(),
        "cabbage".parse().unwrap(),
    );
    let rose_asset_id = AssetId::new(rose_definition_id.clone(), alice_id.clone());
    let cabbage_asset_id = AssetId::new(cabbage_definition_id.clone(), alice_id.clone());
    builder = builder.append_instruction(Transfer::domain(
        genesis_id.clone(),
        wonderland_domain.clone(),
        alice_id.clone(),
    ));
    builder = builder.append_instruction(Mint::asset_quantity(13u32, rose_asset_id));
    builder = builder.append_instruction(Mint::asset_quantity(44u32, cabbage_asset_id));
    builder = builder.next_transaction();
    let xor_asset_def: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            test_domain_id.clone(),
            "xor".parse().unwrap(),
        );
    let may_and_def: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            and_domain_id.clone(),
            "MAY".parse().unwrap(),
        );
    let grant_instructions = [
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
            CanSetHijiriParameters,
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanReadAllLedgerData,
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanSetParameters,
            genesis_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanSetHijiriParameters,
            genesis_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            Permission::new("CanManageSoracloud".into(), Json::new(())),
            alice_id.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            CanManageParliament,
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
    let agent_wallet_asset_definition =
        AssetDefinitionId::parse_address_literal("61CtjvNd9T3THAR65GsMVHr82Bjc")
            .expect("soracloud agent wallet asset definition id");
    let hf_shared_lease_asset_definition =
        AssetDefinitionId::parse_address_literal("5PeSrQmLNwwKtruJvDZrbrm9RuMw")
            .expect("soracloud HF shared lease asset definition id");
    // Topology entries only carry the peer BLS identity. Runtime accounts for validator
    // processes are seeded later from the peer streaming identities in `NetworkBuilder`.
    let soracloud_bootstrap_accounts =
        BTreeSet::from([alice_id.clone(), bob_id.clone(), carpenter_id.clone()]);
    builder = builder.next_transaction();
    builder = builder.append_instruction(Register::asset_definition(AssetDefinition::numeric(
        agent_wallet_asset_definition.clone(),
        "soracloud_agent_wallet".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )));
    builder = builder.append_instruction(Register::asset_definition(AssetDefinition::numeric(
        hf_shared_lease_asset_definition.clone(),
        "soracloud_hf_lease".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )));
    for account_id in soracloud_bootstrap_accounts {
        builder = builder.append_instruction(Mint::asset_quantity(
            500_000_u32,
            AssetId::new(agent_wallet_asset_definition.clone(), account_id.clone()),
        ));
        builder = builder.append_instruction(Mint::asset_quantity(
            500_000_u32,
            AssetId::new(hf_shared_lease_asset_definition.clone(), account_id),
        ));
    }
    let mut vk_registry_instructions = Vec::new();
    for tx_instr in extra_transactions {
        builder =
            append_external_genesis_transaction(builder, tx_instr, &mut vk_registry_instructions);
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
        // Expand the topology into proof-bearing peer registrations here instead of
        // `GenesisBuilder::set_topology`, which emits plain registrations.
        builder = builder.next_transaction();
        for peer_id in &topology_vec {
            let pop_bytes = pop_map
                .remove(peer_id.public_key())
                .unwrap_or_else(|| panic!("missing BLS PoP for peer {}", peer_id.public_key()));
            let register = RegisterPeerWithPop::new(peer_id.clone(), pop_bytes);
            let instruction = InstructionBox::from(register);
            builder = builder.append_instruction(instruction);
        }
        if let Some((dangling_pk, _)) = pop_map.into_iter().next() {
            panic!("topology entry present for peer {dangling_pk} that is absent from topology");
        }
    }
    for tx_instr in post_topology_transactions {
        builder =
            append_external_genesis_transaction(builder, tx_instr, &mut vk_registry_instructions);
    }
    let vk_set_hash = iroha_genesis::compute_genesis_vk_set_hash(vk_registry_instructions.iter())
        .expect("compute genesis verifying key set hash");
    let vk_set_hash_field = vk_set_hash.map_or(norito::json::Value::Null, |hash| {
        norito::json::Value::String(format_hash_hex(hash))
    });
    let mut confidential_root = norito::json::Map::new();
    confidential_root.insert("vk_set_hash".to_owned(), vk_set_hash_field);
    let conf_param = Parameter::Custom(CustomParameter::new(
        confidential_metadata::registry_root_id(),
        Json::new(norito::json::Value::Object(confidential_root)),
    ));
    builder = builder.append_parameter(Parameter::Custom(
        HijiriParametersV1::first_release_genesis().into_custom_parameter(),
    ));
    builder = builder.append_parameter(conf_param);
    let raw_genesis = builder
        .build_raw()
        .expect("build canonical test-network genesis manifest")
        .with_consensus_mode(consensus_mode);
    let block = raw_genesis
        .build_and_sign_with_da_proof_policies_and_confidential_policy_hash(
            &genesis_key_pair,
            da_proof_policies,
            confidential_policy_hash,
        )
        .expect("build minimal genesis");
    (block, genesis_account, topology_vec, genesis_key_pair)
}
fn format_hash_hex(hash: [u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(66);
    encoded.push_str("0x");
    for byte in hash {
        write!(&mut encoded, "{byte:02x}").expect("write to String");
    }
    encoded
}
#[cfg(test)]
pub(crate) fn ensure_genesis_results(
    block: &mut GenesisBlock,
    genesis_account: &AccountId,
    topology: &[PeerId],
    genesis_key_pair: &KeyPair,
    nexus_config: Option<&ActualNexus>,
    zk_config: Option<&ActualZk>,
) {
    ensure_genesis_results_with_runtime_config(
        block,
        genesis_account,
        topology,
        genesis_key_pair,
        None,
        nexus_config,
        zk_config,
        None,
    );
}
pub(crate) fn ensure_genesis_results_with_runtime_config(
    block: &mut GenesisBlock,
    genesis_account: &AccountId,
    topology: &[PeerId],
    genesis_key_pair: &KeyPair,
    pipeline_config: Option<&ActualPipeline>,
    nexus_config: Option<&ActualNexus>,
    zk_config: Option<&ActualZk>,
    runtime_config: Option<&ActualRoot>,
) {
    let has_results = block.0.has_results();
    let results_are_canonical = genesis_results_are_canonical(&block.0);
    assert!(
        !has_results || results_are_canonical,
        "provided genesis execution results must be complete and canonical"
    );
    let signature_is_canonical = genesis_signature_is_canonical(&block.0, genesis_key_pair);
    if results_are_canonical && signature_is_canonical {
        return;
    }
    // Preserve already computed execution results while restoring the canonical genesis signature.
    if has_results {
        block.0 = rebuild_block_with_results(&block.0, genesis_key_pair);
        return;
    }
    let (executed, _) = preexecute_genesis_with_runtime_config(
        block,
        genesis_account,
        topology,
        genesis_key_pair,
        pipeline_config,
        nexus_config,
        zk_config,
        runtime_config,
    )
    .unwrap_or_else(|error| panic!("genesis pre-execution must succeed: {error:#}"));
    block.0 = executed;
}
fn genesis_results_are_canonical(block: &iroha_data_model::block::SignedBlock) -> bool {
    if !block.has_results() {
        return false;
    }
    let entrypoint_count = block.entrypoint_hashes().len();
    let result_count = block.results().len();
    if result_count != entrypoint_count || block.results().any(|result| result.as_ref().is_err()) {
        return false;
    }
    let Ok(minimum_committed_fragments) = u64::try_from(result_count) else {
        return false;
    };
    let Some(actual_committed_fragments) = block.committed_fragment_count() else {
        return false;
    };
    if actual_committed_fragments < minimum_committed_fragments
        || block.validate_entrypoint_merkle_cache().is_err()
        || block.validate_result_merkle_cache().is_err()
    {
        return false;
    }
    let expected_result_root = block.result_hashes().collect::<MerkleTree<_>>().root();
    block.header().result_merkle_root() == expected_result_root
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
#[cfg(test)]
fn populate_genesis_results(
    block: &GenesisBlock,
    genesis_account: &AccountId,
    topology: &[PeerId],
    genesis_key_pair: &KeyPair,
    nexus_config: Option<&ActualNexus>,
    zk_config: Option<&ActualZk>,
) -> Result<iroha_data_model::block::SignedBlock, Report> {
    preexecute_genesis_with_runtime_config(
        block,
        genesis_account,
        topology,
        genesis_key_pair,
        None,
        nexus_config,
        zk_config,
        None,
    )
    .map(|(block, _)| block)
}
pub(crate) fn staged_genesis_policy_hashes(
    block: &GenesisBlock,
    genesis_account: &AccountId,
    topology: &[PeerId],
    genesis_key_pair: &KeyPair,
    pipeline_config: Option<&ActualPipeline>,
    nexus_config: Option<&ActualNexus>,
    zk_config: Option<&ActualZk>,
    runtime_config: Option<&ActualRoot>,
) -> Result<StagedGenesisPolicyHashes, Report> {
    preexecute_genesis_with_runtime_config(
        block,
        genesis_account,
        topology,
        genesis_key_pair,
        pipeline_config,
        nexus_config,
        zk_config,
        runtime_config,
    )
    .map(|(_, hashes)| hashes)
}
pub(crate) fn preexecute_genesis_with_runtime_config(
    block: &GenesisBlock,
    genesis_account: &AccountId,
    topology: &[PeerId],
    genesis_key_pair: &KeyPair,
    pipeline_config: Option<&ActualPipeline>,
    nexus_config: Option<&ActualNexus>,
    zk_config: Option<&ActualZk>,
    runtime_config: Option<&ActualRoot>,
) -> Result<
    (
        iroha_data_model::block::SignedBlock,
        StagedGenesisPolicyHashes,
    ),
    Report,
> {
    if topology.is_empty() {
        return Err(eyre!("genesis topology is empty"));
    }
    let effective_nexus = runtime_config.map(|config| &config.nexus).or(nexus_config);
    let nexus = resolve_preexec_nexus_config(effective_nexus, block.0.da_proof_policies())?;
    let query_handle = LiveQueryStore::start_test();
    let effective_genesis_account = block
        .0
        .external_transactions()
        .next()
        .map(|tx| tx.authority().clone())
        .unwrap_or_else(|| genesis_account.clone());
    let genesis_domain =
        Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&effective_genesis_account);
    let genesis_account_entry =
        Account::new(effective_genesis_account.clone()).build(&effective_genesis_account);
    let mut world = World::with([genesis_domain], [genesis_account_entry], []);
    iroha_core::sns::seed_genesis_alias_bootstrap(&mut world, &block.0, &nexus.dataspace_catalog);
    let mut state = State::new_with_pre_genesis_nexus_for_testing(world, nexus, query_handle);
    if let Some(pipeline_config) = runtime_config
        .map(|config| &config.pipeline)
        .or(pipeline_config)
    {
        state.set_pipeline(pipeline_config.clone());
    }
    if let Some(config) = runtime_config {
        state.set_crypto(config.crypto.clone());
        state.set_oracle(config.oracle.clone());
        state.set_fraud_monitoring(config.fraud_monitoring.clone());
        state.set_gov(config.gov.clone());
        state.content = config.content.clone();
        state.set_settlement(config.settlement.clone());
    }
    if let Some(zk_config) = runtime_config.map(|config| &config.zk).or(zk_config) {
        state.set_zk(zk_config.clone()).map_err(Report::from)?;
    }
    install_preexec_lane_manifests(&state, runtime_config)?;
    let core_topology = CoreTopology::new(topology.to_vec());
    let mut voting_block = None;
    let time_source = TimeSource::new_system();
    let consensus_mode = signed_genesis_consensus_mode(block)?;
    let validation = ValidBlock::validate_signed_genesis_keep_voting_block(
        block.0.clone(),
        &core_topology,
        &effective_genesis_account,
        &time_source,
        &state,
        &mut voting_block,
        consensus_mode,
    )
    .unpack(|_| {});
    let (valid_block, state_block) = match validation {
        Ok(validated) => validated,
        Err((rejected_block, err)) => {
            let first_tx_error = rejected_block
                .has_results()
                .then(|| {
                    rejected_block
                        .results()
                        .enumerate()
                        .find_map(|(index, result)| {
                            result
                                .as_ref()
                                .err()
                                .map(|tx_err| format!("tx#{index}: {tx_err}; details: {tx_err:?}"))
                        })
                })
                .flatten();
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
    let staged_hashes = StagedGenesisPolicyHashes {
        nexus_amx: iroha_core::sumeragi::staged_genesis_nexus_amx_context_hash(&state_block),
        execution_policy: iroha_core::sumeragi::staged_genesis_execution_policy_hash(&state_block)
            .map_err(Report::from)?,
    };
    drop(state_block);
    let signed_block: iroha_data_model::block::SignedBlock = valid_block.into();
    Ok((
        rebuild_block_with_results(&signed_block, genesis_key_pair),
        staged_hashes,
    ))
}
fn resolve_preexec_nexus_config(
    nexus_config: Option<&ActualNexus>,
    block_policies: Option<&DaProofPolicyBundle>,
) -> Result<ActualNexus, Report> {
    let has_authoritative_nexus = nexus_config.is_some();
    let mut nexus = nexus_config.cloned().unwrap_or_default();
    if let Some(policies) = block_policies
        && !policies.policies.is_empty()
        && !has_authoritative_nexus
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
    if !has_authoritative_nexus {
        // Direct fixture calls have no resolved runtime config. Keep their
        // account literals unambiguous, but never rewrite an authoritative Nexus
        // snapshot because the signed v2 commitment must match peer startup.
        let gas_account = ALICE_ID.to_string();
        nexus.staking.stake_escrow_account_id = gas_account.clone();
        nexus.staking.slash_sink_account_id = gas_account;
    }
    Ok(nexus)
}
fn install_preexec_lane_manifests(
    state: &State,
    runtime_config: Option<&ActualRoot>,
) -> Result<(), Report> {
    let nexus = state.nexus_snapshot();
    let lane_compliance = match runtime_config {
        Some(config) if config.nexus.compliance.enabled => {
            let policy_dir = config.nexus.compliance.policy_dir.as_ref().ok_or_else(|| {
                eyre!("lane compliance is enabled but no policy_dir is configured")
            })?;
            let engine = LaneComplianceEngine::from_directory(
                policy_dir,
                config.nexus.compliance.audit_only,
            )
            .map_err(|error| {
                eyre!("load lane compliance policies for genesis pre-execution: {error}")
            })?;
            engine
                .validate_active_catalog(&nexus.lane_catalog)
                .map_err(|error| {
                    eyre!("validate lane compliance policies for genesis pre-execution: {error}")
                })?;
            Some(Arc::new(engine))
        }
        _ => None,
    };
    state.install_lane_compliance_engine(lane_compliance);
    let lane_manifests =
        LaneManifestRegistry::from_config(&nexus.lane_catalog, &nexus.governance, &nexus.registry);
    lane_manifests
        .validate_active_coverage_for_catalog(&nexus.lane_catalog)
        .map_err(|error| {
            eyre!("validate lane manifest registry for genesis pre-execution: {error}")
        })?;
    state.install_lane_manifests(&Arc::new(lane_manifests));
    Ok(())
}
fn rebuild_block_with_results(
    template: &iroha_data_model::block::SignedBlock,
    genesis_key_pair: &KeyPair,
) -> iroha_data_model::block::SignedBlock {
    let transactions = template
        .external_transactions()
        .cloned()
        .collect::<Vec<_>>();
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
            SignatureOf::try_from_hash(genesis_key_pair.private_key(), header.hash())
                .expect("sign genesis placeholder header"),
        )
    });
    let da_commitments = template.da_commitments().cloned();
    let da_proof_policies = template.da_proof_policies().cloned();
    let da_pin_intents = template.da_pin_intents().cloned();
    let committed_fragment_count = template.committed_fragment_count();
    let signer_index = initial_signature.index();
    let mut working = iroha_data_model::block::SignedBlock::presigned(
        initial_signature,
        header,
        transactions.clone(),
    );
    working.set_da_commitments(da_commitments.clone());
    working.set_da_proof_policies(da_proof_policies.clone());
    working.set_da_pin_intents(da_pin_intents.clone());
    working
        .set_transaction_results(time_triggers.clone(), &hashes, results.clone())
        .expect("genesis result hashes should match payload");
    if let Some(count) = committed_fragment_count {
        working.set_committed_fragment_count(count);
    }
    let signature = iroha_data_model::block::BlockSignature::new(
        signer_index,
        SignatureOf::try_from_hash(genesis_key_pair.private_key(), working.hash())
            .expect("sign rebuilt genesis header"),
    );
    let mut rebuilt = iroha_data_model::block::SignedBlock::presigned(
        signature,
        working.payload().header,
        transactions,
    );
    rebuilt.set_da_commitments(da_commitments);
    rebuilt.set_da_proof_policies(da_proof_policies);
    rebuilt.set_da_pin_intents(da_pin_intents);
    rebuilt
        .set_transaction_results(time_triggers, &hashes, results)
        .expect("genesis result hashes should match payload");
    if let Some(count) = committed_fragment_count {
        rebuilt.set_committed_fragment_count(count);
    }
    rebuilt
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::state::StateReadOnly;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{asset::AssetDefinition, domain::Domain};
    use norito::codec::Decode;
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
                .get("telemetry_profile")
                .and_then(|value| value.as_str()),
            Some("extended"),
            "test networks should expose expensive telemetry metrics"
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
    fn minimal_genesis_seeds_neutral_first_release_hijiri_parameters() {
        init_instruction_registry();
        let (block, _, _, _) = build_minimal_genesis_unexecuted(
            Vec::new(),
            UniqueVec::new(),
            Vec::new(),
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
        );
        let mut hijiri_parameters = Vec::new();
        for transaction in block.0.external_transactions() {
            let Executable::Instructions(instructions) = transaction.instructions() else {
                continue;
            };
            for instruction in instructions {
                let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>()
                else {
                    continue;
                };
                let Parameter::Custom(custom) = set_parameter.inner() else {
                    continue;
                };
                if custom.id() != &HijiriParametersV1::parameter_id() {
                    continue;
                }
                hijiri_parameters.push(
                    HijiriParametersV1::from_custom_parameter(custom)
                        .expect("decode test-network genesis Hijiri parameters")
                        .expect("test-network genesis must preserve the reserved Hijiri identity"),
                );
            }
        }
        assert_eq!(
            hijiri_parameters,
            vec![HijiriParametersV1::first_release_genesis()],
            "test-network genesis must seed exactly one neutral first-release Hijiri snapshot"
        );
    }
    #[test]
    fn parameter_only_addition_does_not_create_empty_genesis_transaction() {
        init_instruction_registry();
        let parameter = Parameter::Block(
            iroha_data_model::parameter::system::BlockParameter::MaxTransactions(
                std::num::NonZeroU64::new(17).expect("non-zero test transaction limit"),
            ),
        );
        let (baseline, _, _, _) = build_minimal_genesis_unexecuted(
            Vec::new(),
            UniqueVec::new(),
            Vec::new(),
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
        );
        let (with_parameter, _, _, _) = build_minimal_genesis_unexecuted(
            vec![vec![InstructionBox::from(SetParameter::new(
                parameter.clone(),
            ))]],
            UniqueVec::new(),
            Vec::new(),
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
        );

        assert_eq!(
            with_parameter.0.external_transactions().count(),
            baseline.0.external_transactions().count(),
            "routing a parameter into the authoritative snapshot must not leave an empty transaction"
        );
        assert!(with_parameter.0.external_transactions().any(|transaction| {
            let Executable::Instructions(instructions) = transaction.instructions() else {
                return false;
            };
            instructions.iter().any(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<SetParameter>()
                    .is_some_and(|set_parameter| set_parameter.inner() == &parameter)
            })
        }));
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
        let asset_definition_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "genesis_extra".parse().unwrap(),
        );
        let instructions = vec![InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric(
                asset_definition_id,
                "Genesis Extra".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            ),
        ))];
        let block = genesis(vec![instructions], topology, vec![entry]);
        let genesis_account = AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone());
        check_genesis_block(&block.0, &genesis_account)
            .expect("genesis authority should be permitted to seed wonderland assets");
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
        assert!(
            block.0.is_resultless_proposal(),
            "freshly built genesis must be an explicitly resultless proposal"
        );
        super::ensure_genesis_results(
            &mut block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            None,
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
    fn rebuild_block_with_results_preserves_committed_fragment_count() {
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
            None,
        );
        let minimum_count =
            u64::try_from(block.0.results().count()).expect("genesis result count fits u64");
        let preserved_count = block
            .0
            .committed_fragment_count()
            .expect("executed genesis carries its committed fragment count");
        assert!(
            preserved_count >= minimum_count,
            "execution-derived committed count must cover every result row"
        );
        let rebuilt = super::rebuild_block_with_results(&block.0, &genesis_key_pair);
        assert_eq!(
            rebuilt.committed_fragment_count(),
            Some(preserved_count),
            "re-signing genesis must preserve the execution-derived committed fragment count"
        );
        assert!(
            super::genesis_signature_is_canonical(&rebuilt, &genesis_key_pair),
            "preserved committed fragment count must be included before the canonical signature"
        );
        let mut extra_internal_fragments = rebuilt;
        let augmented_count = preserved_count
            .checked_add(1)
            .expect("genesis fixture committed count has room for an internal fragment");
        extra_internal_fragments.set_committed_fragment_count(augmented_count);
        assert!(
            super::genesis_results_are_canonical(&extra_internal_fragments),
            "deterministic internal fragments may increase the committed count beyond the result count"
        );
    }
    #[test]
    #[should_panic(expected = "provided genesis execution results must be complete and canonical")]
    fn ensure_genesis_results_rejects_noncanonical_fragment_count() {
        init_instruction_registry();
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()]
            .into_iter()
            .collect::<iroha_primitives::unique_vec::UniqueVec<_>>();
        let entry = GenesisTopologyEntry::new(
            peer_id,
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
            None,
        );
        let expected_count =
            u64::try_from(block.0.results().len()).expect("genesis result count fits u64");
        assert_ne!(
            expected_count, 0,
            "fixture must execute at least one entrypoint"
        );
        block.0.set_committed_fragment_count(0);
        super::ensure_genesis_results(
            &mut block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            None,
            None,
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
            None,
        )
        .expect("genesis pre-execution should succeed");
        assert!(
            executed.results().all(|result| result.as_ref().is_ok()),
            "pre-executed genesis should not carry errors for valid proof-bearing peers"
        );
    }
    #[test]
    fn populate_genesis_results_accepts_block_proof_policies() {
        use iroha_data_model::nexus::{LaneCatalog, LaneConfig, LaneId};
        use std::num::NonZeroU32;
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
                None,
                None,
                Some(iroha_core::state::default_genesis_confidential_policy_hash()),
            );
        let executed = super::populate_genesis_results(
            &block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            None,
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
        use iroha_data_model::nexus::{
            DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog, LaneConfig, LaneId,
        };
        use iroha_data_model::{
            isi::{
                Register,
                staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
            },
            prelude::Quantity,
        };
        use std::num::NonZeroU32;
        init_instruction_registry();
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let validator_key = KeyPair::random();
        let validator_id = AccountId::new(validator_key.public_key().clone());
        let nexus_domain: DomainId = DomainId::try_new("nexus", "universal").expect("nexus domain");
        let stake_asset_id = AssetDefinitionId::derive_from_components(
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
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: lane_one.dataspace_id,
                alias: lane_one.alias.clone(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog should validate");
        let nexus = ActualNexus {
            staking: iroha_config::parameters::actual::NexusStaking {
                stake_asset_id: stake_asset_id.to_string(),
                ..Default::default()
            },
            lane_catalog: catalog.clone(),
            lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&catalog),
            dataspace_catalog,
            ..Default::default()
        };
        let post_topology_transactions = vec![vec![
            Register::domain(Domain::new(nexus_domain.clone())).into(),
            Register::account(Account::new(validator_id.clone())).into(),
            Register::asset_definition({
                let __asset_definition_id = stake_asset_id.clone();
                AssetDefinition::numeric(
                    __asset_definition_id.clone(),
                    "multilane_stake".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
            })
            .into(),
            Mint::asset_quantity(
                10_u32,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
            RegisterPublicLaneValidator::new(
                lane_one.id,
                validator_id.clone(),
                peer_id.clone(),
                validator_id.clone(),
                Quantity::from(10_u32),
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
                None,
                None,
                Some(iroha_core::state::default_genesis_confidential_policy_hash()),
            );
        let err = super::populate_genesis_results(
            &block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            None,
            None,
        )
        .expect_err("custom staking genesis should fail without the supplied nexus config");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("stake asset definition missing")
                || rendered.contains("nexus.staking.stake_asset_id")
                || rendered.contains("Find(AssetDefinition(")
                || rendered.contains(
                    "register_public_lane_validator rejected: lane 1 is not active at block height 1"
                ),
            "unexpected pre-exec error without nexus config: {err:?}"
        );
        let executed = super::populate_genesis_results(
            &block,
            &genesis_account,
            &topology_vec,
            &genesis_key_pair,
            Some(&nexus),
            None,
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
            isi::{InstructionBox, Register},
            nexus::DataSpaceId,
        };
        init_instruction_registry();
        let genesis_key_pair = SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone();
        let ivm_domain: DomainId = DomainId::try_new("ivm", "universal").expect("ivm domain");
        let gas_label: Name = "gas".parse().expect("gas label");
        let gas_account = Account::new(AccountId::new(KeyPair::random().public_key().clone()))
            .with_label(Some(AccountAlias::new(
                gas_label,
                Some(AccountAliasDomain::new(ivm_domain.name().clone())),
                DataSpaceId::UNIVERSAL,
            )));
        let extra_transactions = vec![vec![
            InstructionBox::from(Register::domain(Domain::new(ivm_domain))),
            InstructionBox::from(Register::account(gas_account)),
        ]];
        let bls = KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            peer_id,
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let (block, genesis_account, topology, genesis_key_pair) =
            super::build_minimal_genesis_unexecuted(
                extra_transactions,
                topology,
                vec![entry],
                genesis_key_pair,
            );
        assert_eq!(
            super::signed_genesis_consensus_mode(&block)
                .expect("canonical minimal genesis must contain exactly one handshake entry"),
            WireConsensusMode::Permissioned
        );
        let executed = super::populate_genesis_results(
            &block,
            &genesis_account,
            &topology,
            &genesis_key_pair,
            None,
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
        use iroha_data_model::{
            da::commitment::{DaProofPolicy, DaProofPolicyBundle, DaProofScheme},
            nexus::{DataSpaceId, LaneId},
        };
        use std::num::NonZeroU32;
        let genesis_account = AccountId::new(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone());
        let genesis_account_entry = Account {
            id: genesis_account,
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };
        let query_handle = LiveQueryStore::start_test();
        let world = World::with(
            Vec::<iroha_data_model::domain::Domain>::new(),
            vec![genesis_account_entry],
            Vec::<iroha_data_model::asset::AssetDefinition>::new(),
        );
        let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
        let policy0 = DaProofPolicy {
            lane_id: LaneId::from_lane_index(0, lane_count).expect("lane 0 id"),
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "alpha".to_string(),
            proof_scheme: DaProofScheme::MerkleSha256,
        };
        let policy1 = DaProofPolicy {
            lane_id: LaneId::from_lane_index(1, lane_count).expect("lane 1 id"),
            dataspace_id: DataSpaceId::new(7),
            alias: "beta".to_string(),
            proof_scheme: DaProofScheme::MerkleSha256,
        };
        let bundle = DaProofPolicyBundle::new(vec![policy0, policy1]);
        let nexus = super::resolve_preexec_nexus_config(None, Some(&bundle))
            .expect("preexec should resolve proof policy overrides");
        let state = State::new_with_pre_genesis_nexus_for_testing(world, nexus, query_handle);
        super::install_preexec_lane_manifests(&state, None)
            .expect("preexec should install proof-policy-derived lane manifests");
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
        let manifests = state.lane_manifests.read();
        for lane in nexus.lane_catalog.lanes() {
            assert!(
                manifests.status(lane.id).is_some(),
                "pre-execution manifest registry must be derived from lane {}",
                lane.id.as_u32()
            );
        }
    }
    #[test]
    #[should_panic(expected = "genesis pre-execution must succeed")]
    fn ensure_genesis_results_fails_closed_when_preexecution_fails() {
        init_instruction_registry();
        let empty_topology = iroha_primitives::unique_vec::UniqueVec::new();
        let (mut block, genesis_account, _, genesis_key_pair) =
            super::build_minimal_genesis_unexecuted(
                Vec::new(),
                empty_topology,
                Vec::new(),
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
            );
        assert!(
            block.0.is_resultless_proposal(),
            "freshly built genesis must be an explicitly resultless proposal"
        );
        super::ensure_genesis_results(
            &mut block,
            &genesis_account,
            &[],
            &genesis_key_pair,
            None,
            None,
        );
    }
    #[test]
    fn genesis_registers_peers_with_pop() {
        use iroha_data_model::{isi::RegisterBox, transaction::Executable};
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let block = genesis(Vec::new(), topology, vec![entry]);
        let mut register_pop = 0;
        for tx in block.0.external_transactions() {
            match tx.instructions() {
                Executable::Instructions(isi) => {
                    for instr in isi {
                        if let Some(RegisterBox::Peer(isi)) =
                            instr.as_any().downcast_ref::<RegisterBox>()
                        {
                            register_pop += 1;
                        }
                    }
                }
                Executable::ContractCall(_) => {}
                Executable::Ivm(_) => {}
                Executable::IvmProved(_) => {}
                Executable::Batch(_) => {}
            }
        }
        assert_eq!(
            register_pop, 1,
            "exactly one RegisterPeerWithPop instruction expected"
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
            for tx in block.0.external_transactions() {
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
            None,
            None,
            Some(iroha_core::state::default_genesis_confidential_policy_hash()),
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
            for tx in block.0.external_transactions() {
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
                            && isi.object().id().expect_single_signatory()
                                == CARPENTER_KEYPAIR.public_key()
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
    fn genesis_grants_alice_bootstrap_management_permissions() {
        use iroha_data_model::{isi::GrantBox, transaction::Executable};
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let block = genesis(Vec::new(), topology, vec![entry]);
        let alice_id = sanitize_account_id(&ALICE_ID);
        let genesis_id = sanitize_account_id(&AccountId::new(
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
        ));
        let mut saw_soracloud_permission = false;
        let mut saw_parliament_permission = false;
        let mut saw_read_all_permission = false;
        let mut saw_alice_hijiri_permission = false;
        let mut saw_genesis_hijiri_permission = false;
        for tx in block.0.external_transactions() {
            let Executable::Instructions(instrs) = tx.instructions() else {
                continue;
            };
            for instr in instrs {
                let Some(GrantBox::Permission(grant)) = instr.as_any().downcast_ref::<GrantBox>()
                else {
                    continue;
                };
                if grant.destination == alice_id && grant.object.name() == "CanManageSoracloud" {
                    saw_soracloud_permission = true;
                }
                if grant.destination == alice_id && grant.object.name() == "CanManageParliament" {
                    saw_parliament_permission = true;
                }
                if grant.destination == alice_id && grant.object.name() == "CanReadAllLedgerData" {
                    saw_read_all_permission = true;
                }
                if grant.object.name() == "CanSetHijiriParameters" {
                    saw_alice_hijiri_permission |= grant.destination == alice_id;
                    saw_genesis_hijiri_permission |= grant.destination == genesis_id;
                }
            }
            if saw_soracloud_permission
                && saw_parliament_permission
                && saw_read_all_permission
                && saw_alice_hijiri_permission
                && saw_genesis_hijiri_permission
            {
                break;
            }
        }
        assert!(
            saw_soracloud_permission,
            "default test-network genesis should grant ALICE_ID CanManageSoracloud"
        );
        assert!(
            saw_parliament_permission,
            "default test-network genesis should grant ALICE_ID CanManageParliament"
        );
        assert!(
            saw_read_all_permission,
            "default test-network genesis should grant ALICE_ID CanReadAllLedgerData"
        );
        assert!(
            saw_alice_hijiri_permission,
            "default test-network genesis should grant ALICE_ID CanSetHijiriParameters"
        );
        assert!(
            saw_genesis_hijiri_permission,
            "default test-network genesis should grant the genesis authority CanSetHijiriParameters"
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
        let first_tx = block.0.external_transactions().next().unwrap();
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
    fn genesis_confidential_digest_tracks_registered_verifying_keys() {
        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(bls.public_key().clone());
        let topology = [peer_id.clone()].into_iter().collect();
        let entry = GenesisTopologyEntry::new(
            PeerId::new(bls.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
        );
        let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "offline-test");
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new(
            1,
            "offline-test",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pallas",
            [0xAA; 32],
            [0xBB; 32],
        );
        record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        record.gas_schedule_id = Some("halo2_default".into());
        let register = InstructionBox::from(
            iroha_data_model::isi::verifying_keys::RegisterVerifyingKey { id: vk_id, record },
        );
        let expected = iroha_genesis::compute_genesis_vk_set_hash([&register])
            .expect("compute verifier set hash")
            .expect("active verifier registry hash");
        let (block, _, _, _) = super::build_minimal_genesis_unexecuted_with_post_topology(
            Vec::new(),
            vec![vec![register]],
            topology,
            vec![entry],
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
            super::chain_id(),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(iroha_core::state::default_genesis_confidential_policy_hash()),
        );
        let declared_hash = block
            .0
            .external_transactions()
            .find_map(|tx| {
                use iroha_data_model::{isi::SetParameter, transaction::Executable};
                let Executable::Instructions(instrs) = tx.instructions() else {
                    return None;
                };
                instrs.iter().find_map(|instr| {
                    let set_parameter = instr.as_any().downcast_ref::<SetParameter>()?;
                    let Parameter::Custom(custom) = set_parameter.inner() else {
                        return None;
                    };
                    if custom.id() != &confidential_metadata::registry_root_id() {
                        return None;
                    }
                    let value: norito::json::Value = custom
                        .payload()
                        .try_into_any_norito()
                        .expect("decode confidential registry root");
                    let Some(norito::json::Value::String(hash)) = value.get("vk_set_hash") else {
                        return None;
                    };
                    Some(hash.clone())
                })
            })
            .expect("confidential registry root parameter");
        assert_eq!(declared_hash, format_hash_hex(expected));
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
