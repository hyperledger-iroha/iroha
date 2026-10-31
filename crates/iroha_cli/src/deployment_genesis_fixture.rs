//! Shared executed native genesis fixtures for data-space and Unix deployment tests.

use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::{AccountId, address::ChainDiscriminantGuard},
    block::{SignedBlock, consensus::SumeragiGenesisContextParameters},
    parameter::{Parameter, system::SumeragiConsensusMode},
};
use iroha_model_base::peer::PeerId;
use std::{num::NonZeroU64, path::PathBuf};

// These fixed test-network pins are shared by every consumer of this executed fixture.
const CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const CHAIN_DISCRIMINANT: u16 = 369;

pub(crate) fn key(seed: u8, algorithm: Algorithm) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], algorithm).expect("valid deterministic fixture seed")
}

#[derive(Clone)]
pub(crate) struct Fixture {
    pub(crate) block: SignedBlock,
    pub(crate) manifest: iroha_genesis::RawGenesisTransaction,
    pub(crate) genesis: KeyPair,
    pub(crate) canary: KeyPair,
    pub(crate) nexus_context: Vec<u8>,
}

pub(crate) fn fixture_staking_policy() -> iroha_data_model::block::consensus::PublicLaneStakingPolicy
{
    let fees = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
    iroha_genesis::default_staking_policy(
        fees.fee_asset_id,
        fees.fee_sink_account_id.clone(),
        fees.fee_sink_account_id,
    )
}

impl Fixture {
    pub(crate) fn new() -> Self {
        static FIXTURE: std::sync::OnceLock<Fixture> = std::sync::OnceLock::new();
        FIXTURE.get_or_init(Self::build).clone()
    }

    pub(crate) fn build() -> Self {
        Self::build_with_epoch(20)
    }

    /// The fixture genesis plus sixteen seated Parliament citizens (`specs/sccp.md` §4.14.5),
    /// executed with the fixture's fresh citizenship escrow that `peer0.toml` names.
    pub(crate) fn seated() -> Self {
        static FIXTURE: std::sync::OnceLock<Fixture> = std::sync::OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                Self::build_with_escrow(
                    20,
                    seated_citizen_instructions(),
                    Some(&crate::taira::parliament_seating::fixture_citizenship_escrow()),
                )
            })
            .clone()
    }

    pub(crate) fn build_with_epoch(epoch: u64) -> Self {
        Self::build_with_epoch_and_instructions(epoch, Vec::new())
    }

    pub(crate) fn build_with_epoch_and_instructions(
        epoch: u64,
        instructions: Vec<iroha_data_model::isi::InstructionBox>,
    ) -> Self {
        Self::build_with_escrow(epoch, instructions, None)
    }

    /// Build and execute the fixture genesis; `citizenship_escrow`, when given, replaces the
    /// default `gov.citizenship_escrow_account` the genesis executes with.
    pub(crate) fn build_with_escrow(
        epoch: u64,
        instructions: Vec<iroha_data_model::isi::InstructionBox>,
        citizenship_escrow: Option<&AccountId>,
    ) -> Self {
        let _profile = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
        let genesis = key(101, Algorithm::Ed25519);
        let canary = key(102, Algorithm::Ed25519);
        let topology = (110..114)
            .map(|seed| {
                let validator = key(seed, Algorithm::BlsNormal);
                iroha_genesis::GenesisTopologyEntry::new(
                    PeerId::new(validator.public_key().clone()),
                    iroha_crypto::bls_normal_pop_prove(validator.private_key()).unwrap(),
                )
            })
            .collect();
        let staking = fixture_staking_policy();
        let npos = iroha_data_model::parameter::system::SumeragiNposParameters {
            xor_asset_definition_id: staking.stake_asset_id.clone(),
            max_validators: 4,
            epoch_length_blocks: NonZeroU64::new(epoch).unwrap(),
            evidence_horizon_blocks: epoch,
            slashing_delay_blocks: 1,
            ..iroha_data_model::parameter::system::SumeragiNposParameters::default()
        };
        npos.validate().unwrap();
        let mut builder = iroha_genesis::GenesisBuilder::new_without_executor(
            CHAIN_ID.into(),
            ".",
            iroha_data_model::crypto_admission_policy::CryptoAdmissionPolicyV1::new(vec![
                iroha_crypto::Algorithm::Ed25519,
                iroha_crypto::Algorithm::Secp256k1,
                iroha_crypto::Algorithm::BlsNormal,
            ])
            .expect("explicit canonical genesis producer policy"),
        )
        .with_staking_policy(staking)
        .expect("explicit fixture staking policy")
        .with_sumeragi_context_parameters(SumeragiGenesisContextParameters::recommended())
        .set_topology(topology)
        .append_parameter(Parameter::Sumeragi(
            iroha_data_model::parameter::system::SumeragiParameter::EpochLengthBlocks(
                NonZeroU64::new(epoch).unwrap(),
            ),
        ))
        .append_parameter(Parameter::Custom(npos.into_custom_parameter()));
        if !instructions.is_empty() {
            // Genesis serializes topology registrations after a transaction's
            // explicit instructions. Staking must execute in the next batch,
            // after those peers and their consensus keys exist.
            builder = builder.next_transaction();
        }
        for instruction in instructions {
            builder = builder.append_instruction(instruction);
        }
        let manifest = builder
            .build_raw()
            .unwrap()
            .with_chain_discriminant(CHAIN_DISCRIMINANT)
            .with_consensus_mode(SumeragiConsensusMode::Npos)
            .with_consensus_meta()
            .expect("valid fixture consensus parameters");
        // Match Kagami's signing boundary: the unpublished draft may have unbound
        // policy commitments. Only the native validator's typed mismatch may supply
        // their actual values; the newly signed final fixture must pass unchanged.
        let (nexus_hash, execution_hash) =
            execute_fixture_genesis(&manifest, &genesis, citizenship_escrow).map_or_else(
                |derived_policies| derived_policies,
                |(_, nexus, execution, _)| (nexus, execution),
            );
        let mut context = manifest.sumeragi_context_parameters();
        context.nexus_amx_context_hash = nexus_hash.into();
        context.execution_policy_hash = execution_hash.into();
        let manifest = manifest
            .with_sumeragi_context_parameters(context)
            .with_consensus_meta()
            .expect("valid fixture consensus parameters");
        let (mut block, final_nexus_hash, final_execution_hash, nexus_context) =
            execute_fixture_genesis(&manifest, &genesis, citizenship_escrow)
                .expect("newly signed fixture must reproduce both exact native policies");
        assert_eq!(nexus_hash, final_nexus_hash);
        assert_eq!(execution_hash, final_execution_hash);
        block
            .replace_signatures(
                iroha_data_model::block::BlockSignatures::try_from_iter([
                    iroha_data_model::block::BlockSignature::new(
                        0,
                        iroha_crypto::SignatureOf::try_from_hash(
                            genesis.private_key(),
                            block.hash(),
                        )
                        .unwrap(),
                    ),
                ])
                .expect("at most 31 block signatures"),
            )
            .unwrap();
        iroha_genesis::validate_prepared_genesis_bundle(
            &block.encode_wire().unwrap(),
            &manifest,
            genesis.public_key(),
            block.hash(),
        )
        .unwrap();
        // The same production validator called by assembly must accept this fixture.
        iroha_core::release_identity::genesis_identity(
            &block.encode_wire().unwrap(),
            genesis.public_key(),
        )
        .unwrap();
        Self {
            block,
            manifest,
            genesis,
            canary,
            nexus_context,
        }
    }
}

/// Sixteen genesis citizens bonded at the canonical Taira bond (`specs/sccp.md` §4.14.5), under
/// the default citizenship asset and the fresh fixture escrow that the seated fixture genesis
/// executes with and its `peer0.toml` names.
fn seated_citizen_instructions() -> Vec<iroha_data_model::isi::InstructionBox> {
    use iroha_data_model::{
        asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId},
        domain::Domain,
        isi::Register,
    };
    use iroha_model_base::domain::DomainId;
    use iroha_primitives::numeric::{NumericSpec, Quantity};

    let _profile = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    let asset = AssetDefinitionId::parse_address_literal(
        &iroha_config::parameters::defaults::governance::citizenship_asset_id(),
    )
    .unwrap();
    let escrow = crate::taira::parliament_seating::fixture_citizenship_escrow();
    let citizens = (150_u8..166)
        .map(|seed| AccountId::new(key(seed, Algorithm::Ed25519).public_key().clone()))
        .collect::<Vec<_>>();
    let mut instructions: Vec<iroha_data_model::isi::InstructionBox> = vec![
        Register::domain(Domain::new(
            DomainId::parse_fully_qualified("sora.universal").unwrap(),
        ))
        .into(),
        Register::asset_definition(AssetDefinition::new(
            asset.clone(),
            "xor",
            NumericSpec::default(),
            AssetBalancePolicy::Global,
            None,
        ))
        .into(),
    ];
    instructions.extend(
        crate::taira::parliament_seating::citizen_genesis_instructions(
            &citizens,
            &asset,
            crate::taira::parliament_seating::SeatingProfile::canonical()
                .unwrap()
                .citizenship_bond(),
            &Quantity::from(1_000_u32),
            Some(&escrow),
            None,
        )
        .unwrap(),
    );
    instructions
}

// Exercise the same public Core validation/execution boundary as Kagami. A raw
// GenesisBuilder proposal is not release evidence: only actual execution may
// populate results and determine the two signed consensus context commitments.
type FixtureGenesisResult = std::result::Result<(SignedBlock, Hash, Hash, Vec<u8>), (Hash, Hash)>;

pub(crate) fn execute_fixture_genesis(
    manifest: &iroha_genesis::RawGenesisTransaction,
    key: &KeyPair,
    citizenship_escrow: Option<&AccountId>,
) -> FixtureGenesisResult {
    use iroha_config::{
        kura::InitMode,
        parameters::{actual, defaults},
    };
    use iroha_core::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::isi::Registrable as _,
        state::{State, World},
        sumeragi::network_topology::Topology,
    };
    use iroha_data_model::{account::Account, domain::Domain};
    // Sign the same computed native policy as Kagami; the data-model fallback
    // constant is not the configured Core policy used by this execution fixture.
    let provisional = manifest
        .clone()
        .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
            key,
            None,
            Some(iroha_core::state::default_genesis_confidential_policy_hash()),
            1_000,
        )
        .unwrap();
    let authority = AccountId::new(key.public_key().clone());
    // World and State must share the same original finite execution pool.
    let execution_budget = iroha_core::state::AllocationBudget::new(
        iroha_config::parameters::defaults::pipeline::IVM_EXECUTION_MAX_BYTES,
    );
    let world = World::try_with_execution_budget(
        [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [],
        &execution_budget,
    )
    .expect("initialize original World storage for native genesis fixture");
    let reprofile = |literal: &str| {
        iroha_data_model::account::address::AccountAddress::from_i105_for_discriminant(
            literal,
            Some(iroha_config::parameters::defaults::common::chain_discriminant()),
        )
        .unwrap()
        .to_i105_for_discriminant(CHAIN_DISCRIMINANT)
        .unwrap()
    };
    let nexus = iroha_config::parameters::actual::Nexus::default();
    // Monetary identities are typed original genesis parameters, independent of
    // the presentation discriminant used for node-local observer settings.

    let kura_config = actual::Kura {
        init_mode: InitMode::Strict,
        // The temporary constructor supplies and retains its isolated directory.
        store_dir: iroha_config::base::WithOrigin::inline(PathBuf::new()),
        max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
        native_context_archive_max_bytes:
            iroha_config::parameters::defaults::kura::NATIVE_CONTEXT_ARCHIVE_MAX_BYTES,
        history_checkpoint_cache_capacity:
            iroha_config::parameters::defaults::kura::HISTORY_CHECKPOINT_CACHE_CAPACITY,
        block_hash_history_bytes:
            iroha_config::parameters::defaults::kura::BLOCK_HASH_HISTORY_BYTES,
        transaction_history_bytes:
            iroha_config::parameters::defaults::kura::TRANSACTION_HISTORY_BYTES,
        membership_storage: iroha_config::parameters::defaults::kura::MEMBERSHIP_STORAGE_POLICY,
        fastpq_artifacts: defaults::kura::FASTPQ_ARTIFACT_POLICY,
        debug_output_new_blocks: false,
        fsync_mode: defaults::kura::FSYNC_MODE,
        fsync_interval: defaults::kura::FSYNC_INTERVAL,
    };
    let kura = Kura::new_temporary_with_configured_lane_catalog(
        &kura_config,
        &nexus.lane_config,
        &nexus.configured_lane_catalog,
    )
    .expect("initialize authenticated temporary Kura for native genesis");
    let mut state = State::try_new_with_chain_and_network_id_with_default_telemetry(
        execution_budget,
        world,
        kura,
        LiveQueryStore::start_test(),
        manifest.chain_id().clone(),
        NetworkId::from_genesis_hash(provisional.0.hash()),
    )
    .unwrap();
    // Match Kagami's configured startup: install the policy owner before any
    // runtime projection, then bind physical storage to this exact genesis network.
    let manifests = iroha_core::governance::manifest::LaneManifestRegistry::from_config(
        &nexus.lane_catalog,
        &nexus.governance,
        &nexus.registry,
    );
    manifests
        .validate_active_coverage_for_catalog(&nexus.lane_catalog)
        .unwrap();
    state.install_lane_manifests_for_testing(&std::sync::Arc::new(manifests));
    let mut pipeline = actual::Pipeline {
        workers: 1,
        ..actual::Pipeline::default()
    };
    pipeline.gas.tech_account_id = reprofile(&pipeline.gas.tech_account_id);
    state.set_pipeline(pipeline);
    state
        .prepare_configured_primary_geometry_anchor(&nexus.configured_lane_catalog)
        .expect("bind signed genesis network and configured primary geometry");
    state
        .restore_kura_lane_segments_before_startup_replay()
        .expect("restore authenticated primary before genesis execution");
    state
        .set_nexus_from_config(nexus)
        .expect("install the exact configured genesis Nexus baseline");
    state.set_crypto(actual::Crypto::default());
    if let Some(escrow) = citizenship_escrow {
        // Validators of a seated network name a fresh escrow, never the published default.
        state.set_gov(actual::Governance {
            citizenship_escrow_account: escrow.clone(),
            ..actual::Governance::default()
        });
    }
    let topology = Topology::new(
        iroha_core::sumeragi::startup::genesis_committee_peers(&provisional.0).unwrap(),
    );
    let root_scope = iroha_genesis::signed_genesis_consensus_metadata(&provisional.0)
        .unwrap()
        .sumeragi_context
        .root_scope;
    // Genesis timestamps advance one millisecond per transaction. Validate at
    // the signed block time so fixture execution cannot race the wall clock.
    let time_source =
        iroha_primitives::time::TimeSource::new_fixed(provisional.0.header().creation_time());
    let validation = ValidBlock::validate_signed_genesis(
        provisional.0,
        &topology,
        &authority,
        &time_source,
        &state,
        iroha_data_model::block::consensus::ConsensusMode::Npos,
        root_scope,
    )
    .unpack(|_| {});
    let (valid, staged) = match validation {
        Ok(executed) => executed,
        Err((block, error)) => {
            if let iroha_core::block::BlockValidationError::GenesisPolicyMismatch {
                actual_execution,
                actual_nexus,
                ..
            } = *error
            {
                return Err((actual_nexus, actual_execution));
            }
            let output_errors = block
                .failed_outputs()
                .map(|(index, reason)| format!("output[{index}]: {reason:?}"))
                .collect::<Vec<_>>();
            panic!(
                "native fixture genesis execution failed: {error:?}; {}",
                output_errors.join("; ")
            )
        }
    };
    let nexus_context = iroha_core::sumeragi::staged_genesis_nexus_amx_context_preimage(&staged);
    let nexus_hash = Hash::new(&nexus_context);
    let execution_hash =
        iroha_core::sumeragi::staged_genesis_execution_policy_hash(&staged).unwrap();
    drop(staged);
    Ok((valid.into(), nexus_hash, execution_hash, nexus_context))
}

pub(crate) fn deployment_genesis_fixture() -> (SignedBlock, KeyPair) {
    let fixture = Fixture::new();
    (fixture.block, fixture.genesis)
}

/// Reuse the exact executed genesis and independently checked native manifest binding.
pub(crate) fn deployment_validated_genesis_fixture() -> iroha_genesis::ValidatedGenesisBundle {
    let fixture = Fixture::new();
    iroha_genesis::validate_prepared_genesis_bundle(
        &fixture.block.encode_wire().unwrap(),
        &fixture.manifest,
        fixture.genesis.public_key(),
        fixture.block.hash(),
    )
    .unwrap()
}

/// Execute and sign explicit active lane bindings with authority keys distinct from peer keys.
pub(crate) fn deployment_lane_genesis_fixture() -> (SignedBlock, KeyPair) {
    static FIXTURE: std::sync::OnceLock<Fixture> = std::sync::OnceLock::new();
    let fixture = FIXTURE.get_or_init(|| {
        use iroha_data_model::{
            account::Account,
            asset::{AssetBalancePolicy, AssetDefinition, AssetId},
            domain::Domain,
            isi::{ActivatePublicLaneValidator, Mint, Register, RegisterPublicLaneValidator},
        };
        use iroha_model_base::{domain::DomainId, metadata::Metadata, topology::LaneId};
        use iroha_primitives::numeric::{NumericSpec, Quantity};

        let _profile = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
        let staking = fixture_staking_policy();
        let escrow = staking.stake_escrow_account_id;
        let stake_asset = staking.stake_asset_id;
        let mut instructions: Vec<iroha_data_model::isi::InstructionBox> = vec![
            Register::domain(Domain::new(
                DomainId::parse_fully_qualified("nexus.universal").unwrap(),
            ))
            .into(),
            Register::account(Account::new(escrow.clone())).into(),
            Register::asset_definition(AssetDefinition::new(
                stake_asset.clone(),
                "Fixture stake",
                NumericSpec::fractional(9),
                AssetBalancePolicy::Global,
                None,
            ))
            .into(),
        ];
        for seed in 110..114 {
            let validator = AccountId::new(key(seed + 20, Algorithm::Ed25519).public_key().clone());
            let peer = PeerId::new(key(seed, Algorithm::BlsNormal).public_key().clone());
            instructions.extend([
                Register::account(Account::new(validator.clone())).into(),
                Mint::asset_quantity(1_u64, AssetId::new(stake_asset.clone(), validator.clone()))
                    .into(),
                RegisterPublicLaneValidator::new(
                    LaneId::SINGLE,
                    validator.clone(),
                    peer,
                    validator.clone(),
                    Quantity::from(1_u64),
                    Metadata::default(),
                    iroha_data_model::nexus::PublicLaneMonetaryPlanV1::genesis_registration(
                        AssetId::new(stake_asset.clone(), validator.clone()),
                        AssetId::new(stake_asset.clone(), escrow.clone()),
                        Quantity::from(1_u64),
                    ),
                )
                .into(),
                ActivatePublicLaneValidator::new(LaneId::SINGLE, validator).into(),
            ]);
        }
        Fixture::build_with_epoch_and_instructions(20, instructions)
    });
    (fixture.block.clone(), fixture.genesis.clone())
}

#[test]
fn native_fixture_rejects_changed_signed_policies_after_draft_binding() {
    let _profile = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    let fixture = Fixture::new();
    let bound = fixture.manifest.sumeragi_context_parameters();
    let expected = (
        Hash::prehashed(bound.nexus_amx_context_hash),
        Hash::prehashed(bound.execution_policy_hash),
    );
    assert!(execute_fixture_genesis(&fixture.manifest, &fixture.genesis, None).is_ok());
    for change_execution in [true, false] {
        let mut changed = bound;
        let foreign = Hash::new(b"foreign signed fixture policy").into();
        if change_execution {
            changed.execution_policy_hash = foreign;
        } else {
            changed.nexus_amx_context_hash = foreign;
        }
        let manifest = fixture
            .manifest
            .clone()
            .with_sumeragi_context_parameters(changed)
            .with_consensus_meta()
            .expect("valid fixture consensus parameters");
        assert_eq!(
            execute_fixture_genesis(&manifest, &fixture.genesis, None).unwrap_err(),
            expected,
            "native execution must reject either changed commitment"
        );
    }
}
