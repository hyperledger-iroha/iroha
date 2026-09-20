//! Assemble the sole facts frame from retained originals and actual staged genesis authority.
//!
//! This consuming semantic owner performs no publication. Its filesystem caller retains every
//! original descriptor; this owner retains the real Core completion and all four genesis owners.
//! Collector summaries, claimed rosters and observations never construct authority.

use super::*;
use crate::kura::scaling_evidence::export::{
    SuppliedHeightEvidence, VerifiedExport, export_from_kura,
    launcher::journal::{self, JournalExpectations, JournalObservation, JournalVariant},
};
use iroha_config::base::toml::TomlSource;
use iroha_config::parameters::actual;
use iroha_core::{
    kura::{CanonicalKuraEvidenceComplete, CanonicalKuraEvidenceLimits},
    queue::evaluate_policy_plan_with_nexus_and_world_at_block_height,
    state::FinalizedNativeContextV1,
    sumeragi::GenesisMergeAuthority,
};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    block::consensus_v2::{ConsensusMode, HeightContext},
    isi::RegisterBox,
    nexus::{LaneStorageProfile, LaneVisibility},
    parameter::system::SumeragiConsensusMode,
};
use iroha_genesis::RawGenesisTransaction;
use iroha_model_base::{chain::ChainId, peer::PeerId};
use std::{
    cell::Cell,
    path::{Component, Path},
    time::Duration,
};

/// One exact original, with its independent raw identity and admission reservation.
#[derive(Clone, Copy)]
pub(in crate::kura::scaling_evidence::export) struct OriginalFact<'a> {
    pub(in crate::kura::scaling_evidence::export) path: &'a Path,
    pub(in crate::kura::scaling_evidence::export) bytes: &'a [u8],
    pub(in crate::kura::scaling_evidence::export) expected_raw_sha256: [u8; 32],
    pub(in crate::kura::scaling_evidence::export) max_bytes: u64,
}
/// Fixed role order of all ten originals; Kura retains its five separate source files.
pub(in crate::kura::scaling_evidence::export) struct FactsOriginals<'a> {
    pub(in crate::kura::scaling_evidence::export) manifest: OriginalFact<'a>,
    pub(in crate::kura::scaling_evidence::export) signed_genesis: OriginalFact<'a>,
    pub(in crate::kura::scaling_evidence::export) peer_configs: [OriginalFact<'a>; 4],
    pub(in crate::kura::scaling_evidence::export) context: OriginalFact<'a>,
    pub(in crate::kura::scaling_evidence::export) journal: OriginalFact<'a>,
    pub(in crate::kura::scaling_evidence::export) finality: OriginalFact<'a>,
    pub(in crate::kura::scaling_evidence::export) queries: OriginalFact<'a>,
}
/// Independent public launch identities. Neither proofs nor child statistics supply them.
pub(in crate::kura::scaling_evidence::export) struct GenesisExpectations {
    pub(in crate::kura::scaling_evidence::export) chain_id: ChainId,
    pub(in crate::kura::scaling_evidence::export) network_id: NetworkId,
    pub(in crate::kura::scaling_evidence::export) chain_discriminant: u16,
    pub(in crate::kura::scaling_evidence::export) genesis_public_key: PublicKey,
    pub(in crate::kura::scaling_evidence::export) validators: [PublicKey; 4],
}
/// Independent byte reservations. Decoder accounting is separate from file reservations.
#[derive(Clone, Copy)]
pub(in crate::kura::scaling_evidence::export) struct FactsAssemblyCaps {
    pub(in crate::kura::scaling_evidence::export) input_bytes: u64,
    pub(in crate::kura::scaling_evidence::export) facts_bytes: u64,
    pub(in crate::kura::scaling_evidence::export) total_bytes: u64,
    pub(in crate::kura::scaling_evidence::export) decode_bytes: u64,
}
/// Move-only assembled bytes accompanied by the actual completed verifier and staged authorities.
///
/// There is no encoded authority, arbitrary-byte constructor or successful-prefix accessor.
pub(in crate::kura::scaling_evidence::export) struct AssembledFacts {
    bytes: Vec<u8>,
    verified: VerifiedExport,
    _authorities: [StagedFactsAuthority; 4],
    poisoned: Cell<bool>,
}
// Only authenticated staging constructs these owners. Each compact ordered projection binds
// the exact original signed bytes to the sole route resolved from that authority's staged world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RequestRoute {
    signed_sha256: [u8; 32],
    route: RoutingDecision,
}
struct DecodedRequest {
    identity: RequestRoute,
    signed: SignedTransaction,
}
struct StagedFactsAuthority {
    genesis: GenesisMergeAuthority,
    routes: Vec<RequestRoute>,
}
struct GenesisAdmission {
    validators: Vec<PeerId>,
    lane_count: usize,
}
impl AssembledFacts {
    pub(in crate::kura::scaling_evidence::export) fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    fn completion(&self) -> Result<&CanonicalKuraEvidenceComplete> {
        self.verified
            .disk_completion
            .as_ref()
            .ok_or_else(|| eyre!("facts lack retained Core completion"))
    }
    pub(in crate::kura::scaling_evidence::export) fn recheck_sources(&self) -> Result<()> {
        ensure!(
            !self.poisoned.replace(true),
            "assembled facts owner is poisoned"
        );
        self.completion()?.recheck_sources()?;
        self.poisoned.set(false);
        Ok(())
    }
    pub(in crate::kura::scaling_evidence::export) fn ensure_publication_ancestry(
        &self,
        ancestry: &[(u64, u64)],
    ) -> Result<()> {
        ensure!(
            !self.poisoned.replace(true),
            "assembled facts owner is poisoned"
        );
        self.completion()?.ensure_publication_ancestry(ancestry)?;
        self.poisoned.set(false);
        Ok(())
    }
}
impl FactsOriginals<'_> {
    fn ordered(&self) -> [OriginalFact<'_>; 10] {
        [
            self.manifest,
            self.signed_genesis,
            self.peer_configs[0],
            self.peer_configs[1],
            self.peer_configs[2],
            self.peer_configs[3],
            self.context,
            self.journal,
            self.finality,
            self.queries,
        ]
    }
    fn admit(&self, caps: FactsAssemblyCaps) -> Result<()> {
        ensure!(
            [caps.input_bytes, caps.facts_bytes, caps.total_bytes]
                .iter()
                .all(|n| (1..=MAX_PROOF_BYTES).contains(n))
                && (1..=MAX_PROOF_BYTES * 2).contains(&caps.decode_bytes),
            "invalid facts byte reservations"
        );
        ensure!(
            caps.input_bytes
                .checked_add(caps.facts_bytes)
                .is_some_and(|n| n <= caps.total_bytes),
            "facts aggregate reservation exceeded"
        );
        ensure!(
            self.context.max_bytes <= 8 * 1024 * 1024,
            "facts context reservation exceeds 8 MiB"
        );
        let mut paths = BTreeSet::new();
        let mut reserved = 0u64;
        // Every role and reservation is checked before hashing or parsing any original.
        for original in self.ordered() {
            ensure!(
                original.path.is_absolute()
                    && original
                        .path
                        .components()
                        .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
                    && paths.insert(original.path),
                "facts original paths must be distinct and normalized absolute names"
            );
            ensure!(
                (1..=MAX_PROOF_BYTES).contains(&original.max_bytes)
                    && !original.bytes.is_empty()
                    && u64::try_from(original.bytes.len())? <= original.max_bytes,
                "facts original byte cap exceeded"
            );
            reserved = reserved
                .checked_add(original.max_bytes)
                .ok_or_else(|| eyre!("facts original reservation overflow"))?;
        }
        ensure!(
            reserved <= caps.input_bytes,
            "facts original reservations exceed input cap"
        );
        for original in self.ordered() {
            ensure!(
                iroha_crypto::sha256(original.bytes) == original.expected_raw_sha256,
                "facts original raw digest mismatch"
            );
        }
        Ok(())
    }
}

/// Authenticate actual fixed-genesis authority and the complete original stopped transcript.
#[expect(
    clippy::too_many_arguments,
    reason = "independent typed launch inputs retain separate owners"
)]
pub(in crate::kura::scaling_evidence::export) fn assemble(
    originals: FactsOriginals<'_>,
    genesis: GenesisExpectations,
    journal: JournalExpectations,
    verification: VerificationLimits,
    block_store: &Path,
    merge_log: &Path,
    reader: CanonicalKuraEvidenceLimits,
    caps: FactsAssemblyCaps,
) -> Result<AssembledFacts> {
    let _discriminant = ChainDiscriminantGuard::enter(genesis.chain_discriminant);
    journal::admit_expectations(&journal)?;
    admit_work(verification, reader)?;
    originals.admit(caps)?;
    ensure!(
        journal.network_id == genesis.network_id
            && reader.first_height == 1
            && reader.last_height >= 2
            && reader.last_height < u64::MAX
            && reader.last_height <= verification.heights,
        "facts independent network or complete interval mismatch"
    );
    let standard = norito::canonical_decode_limits(usize::try_from(caps.input_bytes)?);
    let limits = norito::DecodeLimits::new(
        standard.max_sequence_elements(),
        standard.max_field_bytes(),
        standard.max_total_elements(),
        usize::try_from(caps.decode_bytes)?,
        128,
    );
    // This is one cumulative caller-thread budget for context, vectors, signed journal frames,
    // copy admission and verifier decoders. Core's separate genesis staging thread has its own
    // allocations; these reservations do not claim a process-wide heap or RSS ceiling.
    norito::with_decode_limits_scope(limits, || {
        let manifest = parse_manifest(originals.manifest)?;
        let configs = originals.peer_configs.map(parse_config);
        let [a, b, c, d] = configs;
        let configs = [a?, b?, c?, d?];
        let admission = admit_genesis(&manifest, &configs, &genesis, &journal)?;
        let (scheduled, observations, _) = journal::read_original_journal(
            originals.journal.bytes,
            originals.journal.expected_raw_sha256,
            originals.journal.max_bytes,
            journal,
        )?
        .into_parts();
        // Perform one additional shared routing decode after journal validation on the caller's
        // cumulative budget. The four workers borrow these exact immutable originals; reserve
        // all retained route slots before starting any worker.
        let requests = decode_requests(&scheduled, verification.requests)?;
        let authorities = authenticate_genesis(
            &manifest,
            originals.signed_genesis.bytes,
            &configs,
            &genesis,
            admission,
            &requests,
        )?;
        for authority in &authorities {
            check_projected_routes(&authority.routes, &scheduled)?;
        }
        drop(requests);
        let context: HeightContext = canonical(originals.context.bytes)?;
        ensure!(
            &context == authorities[0].genesis.context(),
            "original context differs from actual staged genesis"
        );
        let catalog_bytes = catalog_copy_bytes(
            authorities[0].genesis.active_lanes().len(),
            authorities[0].genesis.lane_authority_catalog(),
        )?;
        ensure!(
            u64::try_from(catalog_bytes)? <= caps.decode_bytes,
            "facts catalog copy exceeds work allocation"
        );
        norito::core::reserve_decode_allocation(catalog_bytes)?;
        let plan = TrustedRunPlan {
            network_id: genesis.network_id,
            first_context: context.id(),
            first_height: 1,
            last_height: reader.last_height,
            nexus_amx_context_hash: context.nexus_amx_context_hash,
            execution_policy_hash: context.execution_policy_hash,
            active_lanes: authorities[0]
                .genesis
                .active_lanes()
                .iter()
                .map(|lane| NativeWorkloadLane {
                    lane_id: lane.lane_id,
                    dataspace_id: lane.dataspace_id,
                    incarnation: lane.incarnation,
                    activation_height: lane.activation_height,
                })
                .collect(),
            lane_authorities: authorities[0].genesis.lane_authority_catalog().clone(),
            scheduled,
        };
        let finality: Vec<FinalizedNativeContextV1> = canonical(originals.finality.bytes)?;
        let queries: Vec<CommittedTransaction> = canonical(originals.queries.bytes)?;
        let heights = group_supplied(finality, queries, reader.last_height, verification)?;
        let bindings = super::derive_bindings(&heights, &plan, verification)?;
        let (verify_plan, supplied) = verification_copy(&plan, &heights, caps)?;
        let verified = export_from_kura(
            verify_plan,
            verification,
            block_store,
            merge_log,
            reader,
            &bindings,
            supplied,
        )?;
        let complete = verified
            .disk_completion
            .as_ref()
            .ok_or_else(|| eyre!("facts verification lacks Core completion"))?;
        ensure!(
            complete.committed_height() == reader.last_height,
            "facts interval is shorter than the actual stopped durable tip"
        );
        join_observations(&plan, &observations, &verified)?;
        complete.recheck_sources()?;
        let bytes = super::encode_assembled(plan, verification, heights, caps.facts_bytes)?;
        complete.recheck_sources()?;
        Ok(AssembledFacts {
            bytes,
            verified,
            _authorities: authorities,
            poisoned: Cell::new(false),
        })
    })
}

fn request_slot_bytes(count: usize, max_requests: usize) -> Result<usize> {
    ensure!(
        count > 0 && count <= max_requests && max_requests <= MAX_REQUESTS,
        "facts original requests exceed independent work bound"
    );
    count
        .checked_mul(std::mem::size_of::<DecodedRequest>())
        .and_then(|bytes| {
            count
                .checked_mul(4)?
                .checked_mul(std::mem::size_of::<RequestRoute>())?
                .checked_add(bytes)
        })
        .and_then(|bytes| bytes.checked_add(4 * std::mem::size_of::<Vec<RequestRoute>>()))
        .ok_or_else(|| eyre!("facts staged request reservation overflow"))
}
fn decode_requests(
    scheduled: &[ScheduledRequest],
    max_requests: usize,
) -> Result<Vec<DecodedRequest>> {
    norito::core::reserve_decode_allocation(request_slot_bytes(scheduled.len(), max_requests)?)?;
    let mut requests = Vec::new();
    requests.try_reserve_exact(scheduled.len())?;
    for request in scheduled {
        requests.push(DecodedRequest {
            identity: RequestRoute {
                signed_sha256: iroha_crypto::sha256(&request.signed_transaction),
                route: request.route,
            },
            signed: canonical(&request.signed_transaction)?,
        });
    }
    Ok(requests)
}
fn check_route(actual: RoutingPlan, expected: RoutingDecision) -> Result<()> {
    ensure!(
        actual == RoutingPlan::single(expected),
        "original signed request does not have the exact staged configured route"
    );
    Ok(())
}
fn project_request_routes(
    genesis: &iroha_genesis::GenesisBlock,
    staged: &iroha_core::state::StateBlock<'_>,
    requests: &[DecodedRequest],
) -> Result<Vec<RequestRoute>> {
    let header = genesis.0.header();
    let ledger_time_ms = u64::try_from(header.creation_time().as_millis())?;
    let mut routes = Vec::new();
    // These four vectors were charged together before staging; no decoding happens on this thread.
    routes.try_reserve_exact(requests.len())?;
    for request in requests {
        let route = evaluate_policy_plan_with_nexus_and_world_at_block_height(
            &staged.nexus,
            request.signed.payload(),
            staged.world(),
            ledger_time_ms,
            header.height().get(),
        )?;
        check_route(route, request.identity.route)?;
        routes.push(request.identity);
    }
    Ok(routes)
}
fn check_projected_routes(routes: &[RequestRoute], scheduled: &[ScheduledRequest]) -> Result<()> {
    ensure!(
        routes.len() == scheduled.len(),
        "facts staged route count differs from journal"
    );
    for (route, request) in routes.iter().zip(scheduled) {
        ensure!(
            route.signed_sha256 == iroha_crypto::sha256(&request.signed_transaction)
                && route.route == request.route,
            "facts staged route differs from exact original request order or identity"
        );
    }
    Ok(())
}

// Direct callers must reject invalid finite work before parsing or spawning genesis staging.
// Core repeats its complete reader validation when it admits the retained source descriptors.
pub(in crate::kura::scaling_evidence::export) fn admit_work(
    limits: VerificationLimits,
    reader: CanonicalKuraEvidenceLimits,
) -> Result<()> {
    ensure!(
        (1..=MAX_PROOF_BYTES).contains(&limits.admitted_proof_bytes)
            && limits.input_bytes > 0
            && limits.output_bytes > 0
            && limits
                .input_bytes
                .checked_add(limits.output_bytes)
                .is_some_and(|n| n <= limits.admitted_proof_bytes)
            && (1..=65_536).contains(&limits.heights)
            && (1..=MAX_REQUESTS).contains(&limits.requests)
            && (1..=MAX_REQUESTS).contains(&limits.leaves_per_carrier),
        "facts invalid proof work reservations"
    );
    ensure!(
        reader.first_height == 1
            && reader.last_height >= 2
            && reader.last_height <= reader.max_committed_blocks
            && reader.max_committed_blocks <= 1_000_000
            && reader.last_height <= limits.heights
            && reader.max_store_data_bytes > 0
            && reader.max_store_data_bytes <= limits.input_bytes
            && reader.max_carrier_bytes > 0
            && reader.max_carrier_bytes <= MAX_CARRIER_BYTES
            && reader.max_merge_log_bytes <= limits.input_bytes
            && reader.max_merge_log_bytes <= MAX_PROOF_BYTES
            && reader.max_merge_frames <= reader.max_committed_blocks
            && reader.max_output_bytes > 0
            && reader.max_output_bytes <= limits.input_bytes
            && reader.max_decode_allocation_bytes > 0
            && u64::try_from(reader.max_decode_allocation_bytes)?
                <= limits.admitted_proof_bytes * 2,
        "facts invalid complete reader work reservations"
    );
    Ok(())
}

// The exact raw tables/fields below invoke external reads during actual config parsing. Reject
// their presence even when disabled or empty, before handing the sensitive source to that parser.
const FORBIDDEN_CONFIG: &[&[&str]] = &[
    &["extends"],
    &["private_key_file"],
    &["soranet_transport_private_key_file"],
    &["genesis", "expected_hash_file"],
    &["streaming", "identity_private_key_file"],
    &["network", "soranet_vpn", "operator_private_key_file"],
    &["torii", "account_onboarding"],
    &["torii", "faucet"],
    &[
        "torii",
        "kagemusha_v1_commands",
        "redemption_private_key_file",
    ],
    &["streaming", "codec"],
];
fn raw_field<'a>(table: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Value> {
    let (first, rest) = path.split_first()?;
    let mut value = table.get(*first)?;
    for part in rest {
        value = value.as_table()?.get(*part)?;
    }
    Some(value)
}
fn parse_config(original: OriginalFact<'_>) -> Result<actual::Root> {
    let text = std::str::from_utf8(original.bytes)
        .map_err(|_| eyre!("retained peer config is not UTF-8"))?;
    let mut table = crate::secret_toml::Table::new(crate::secret_toml::parse_table(
        text,
        "retained facts peer config",
    )?);
    for path in FORBIDDEN_CONFIG {
        ensure!(
            raw_field(&table, path).is_none(),
            "fixed facts config contains a forbidden external-read field or table"
        );
    }
    // Require the actual final public hash, without the signing-only placeholder substitution.
    ensure!(
        raw_field(&table, &["genesis", "expected_hash"])
            .and_then(toml::Value::as_str)
            .is_some(),
        "fixed facts config requires an inline final genesis identity"
    );
    let source = TomlSource::new_sensitive(
        original.path.to_path_buf(),
        std::mem::take(&mut *table),
        crate::secret_toml::zeroize_table,
    );
    let config = actual::Root::from_toml_source(source)
        .map_err(|_| eyre!("retained facts peer config is invalid"))?;
    ensure!(
        config.nexus.registry.manifest_directory.is_none()
            && config.nexus.registry.cache_directory.is_none()
            && !config.nexus.compliance.enabled
            && config.nexus.compliance.policy_dir.is_none(),
        "fixed facts genesis staging cannot read registry or compliance artifacts"
    );
    Ok(config)
}
fn parse_manifest(original: OriginalFact<'_>) -> Result<RawGenesisTransaction> {
    // This bounded inspection precedes the real strict manifest decoder and any staging loader.
    norito::json::preflight_slice(
        original.bytes,
        norito::json::JsonPreflightLimits::new(
            original.bytes.len(),
            1_000_000,
            original.bytes.len(),
            original.bytes.len(),
            original.bytes.len(),
            65_536,
            1_000_000,
            1_000_000,
            1_000_000,
            128,
        ),
    )?;
    let value: norito::json::Value = norito::json::from_slice(original.bytes)?;
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("facts manifest must be an object"))?;
    ensure!(
        object
            .get("executor")
            .is_none_or(norito::json::Value::is_null),
        "fixed facts manifest cannot load an executor file"
    );
    let transactions = object
        .get("transactions")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("facts manifest lacks transactions"))?;
    for tx in transactions {
        let tx = tx
            .as_object()
            .ok_or_else(|| eyre!("facts manifest transaction is not an object"))?;
        ensure!(
            tx.get("ivm_triggers")
                .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty)),
            "fixed facts manifest cannot load trigger files"
        );
    }
    drop(value);
    RawGenesisTransaction::from_json_slice_at_path(original.bytes, original.path)
}

fn admit_genesis(
    manifest: &RawGenesisTransaction,
    configs: &[actual::Root; 4],
    expected: &GenesisExpectations,
    journal: &JournalExpectations,
) -> Result<GenesisAdmission> {
    ensure!(
        manifest.chain_id() == &expected.chain_id
            && manifest.chain_discriminant() == expected.chain_discriminant
            && manifest.consensus_mode() == SumeragiConsensusMode::Npos,
        "facts manifest launch identity or NPoS mismatch"
    );
    let lane_count = match journal.variant {
        JournalVariant::OneLane => 1,
        JournalVariant::FourLane => 4,
    };
    ensure!(
        (4..=64).contains(&journal.accounts.len()) && journal.accounts.len().is_multiple_of(4),
        "facts account pool requires complete groups of four"
    );
    let mut validators = expected
        .validators
        .iter()
        .cloned()
        .map(PeerId::new)
        .collect::<Vec<_>>();
    validators.sort();
    ensure!(
        validators.windows(2).all(|w| w[0] < w[1]),
        "facts validators must be four distinct original identities"
    );
    let cadence = Duration::from_millis(
        manifest
            .effective_parameters()?
            .sumeragi()
            .block_cadence_ms()
            .get(),
    );
    let shared = configs[0]
        .sumeragi
        .v2_config(cadence, ConsensusMode::Npos)?;
    let registered: BTreeSet<_> = manifest
        .instructions()
        .filter_map(|instruction| {
            let RegisterBox::Account(register) =
                instruction.as_any().downcast_ref::<RegisterBox>()?
            else {
                return None;
            };
            Some(register.object.id.clone())
        })
        .collect();
    for (index, account) in journal.accounts.iter().enumerate() {
        ensure!(
            registered.contains(&account.authority)
                && account.route
                    == RoutingDecision::new(
                        LaneId::new(u32::try_from(index % lane_count)?),
                        DataSpaceId::UNIVERSAL
                    ),
            "facts account is not registered with its independent fixed route"
        );
    }
    // Check every effective config before staging any peer. Node-local storage/resource
    // equivalence remains the launch owner's responsibility; this binds Sumeragi and staged policy.
    for (index, config) in configs.iter().enumerate() {
        ensure!(
            config.common.chain == expected.chain_id
                && *config.common.chain_discriminant.value() == expected.chain_discriminant
                && config.genesis.public_key == expected.genesis_public_key
                && NetworkId::from_genesis_hash(config.genesis.expected_hash)
                    == expected.network_id
                && config.common.key_pair.public_key() == &expected.validators[index],
            "facts peer launch identity mismatch"
        );
        ensure!(
            config.sumeragi.role == actual::NodeRole::Validator
                && config.sumeragi.v2_config(cadence, ConsensusMode::Npos)? == shared,
            "facts peers do not share the exact Sumeragi v2 configuration"
        );
        let nexus = &config.nexus;
        ensure!(
            !nexus.autoscale.enabled
                && nexus.lane_catalog == nexus.configured_lane_catalog
                && nexus.lane_catalog.lanes().len() == lane_count
                && nexus.dataspace_catalog.entries().len() == 1,
            "facts require a fixed one/four-lane catalog"
        );
        let ds = &nexus.dataspace_catalog.entries()[0];
        ensure!(
            ds.id == DataSpaceId::UNIVERSAL && ds.alias == "universal" && ds.fault_tolerance == 1,
            "facts require universal dataspace with fault tolerance one"
        );
        for (slot, lane) in nexus.lane_catalog.lanes().iter().enumerate() {
            ensure!(
                lane.id == LaneId::new(u32::try_from(slot)?)
                    && lane.dataspace_id == DataSpaceId::UNIVERSAL
                    && lane.visibility == LaneVisibility::Public
                    && lane.storage == LaneStorageProfile::FullReplica,
                "facts lane geometry mismatch"
            );
        }
        let policy = &nexus.routing_policy;
        ensure!(
            policy.default_lane == LaneId::SINGLE
                && policy.default_dataspace == DataSpaceId::UNIVERSAL
                && policy.rules.len() == journal.accounts.len(),
            "facts require the exact account-only routing policy"
        );
        for (rule, account) in policy.rules.iter().zip(&journal.accounts) {
            ensure!(
                rule.lane == account.route.lane_id
                    && rule.dataspace == Some(account.route.dataspace_id)
                    && rule.matcher.account.as_deref()
                        == Some(account.authority.to_string().as_str())
                    && rule.matcher.instruction.is_none()
                    && rule.matcher.description.is_none(),
                "facts routing rule differs from independent account order"
            );
        }
    }
    Ok(GenesisAdmission {
        validators,
        lane_count,
    })
}
fn authenticate_genesis(
    manifest: &RawGenesisTransaction,
    signed: &[u8],
    configs: &[actual::Root; 4],
    expected: &GenesisExpectations,
    admission: GenesisAdmission,
    requests: &[DecodedRequest],
) -> Result<[StagedFactsAuthority; 4]> {
    let GenesisAdmission {
        validators,
        lane_count,
    } = admission;
    let mut authorities = Vec::with_capacity(4);
    for config in configs {
        let (authority, routes) = crate::genesis::staged_signed_genesis_with_projection(
            manifest,
            signed,
            config,
            |genesis, staged| project_request_routes(genesis, staged, requests),
        )?;
        ensure!(
            authority.context().network_id == expected.network_id
                && authority.context().height == 1
                && authority.context().mode == ConsensusMode::Npos
                && authority
                    .context()
                    .roster
                    .iter()
                    .map(|v| v.validator.clone())
                    .collect::<Vec<_>>()
                    == validators
                && authority.proofs_of_possession().len() == 4
                && authority.active_lanes().len() == lane_count,
            "facts staged genesis roster or geometry mismatch"
        );
        for (slot, binding) in authority.active_lanes().iter().enumerate() {
            ensure!(
                binding.lane_id == LaneId::new(u32::try_from(slot)?)
                    && binding.dataspace_id == DataSpaceId::UNIVERSAL
                    && binding.activation_height == 1
                    && authority
                        .lane_authority_catalog()
                        .roster_for_lane(slot)
                        .is_ok_and(|roster| roster.validators == validators),
                "facts staged lane does not retain the exact four-validator authority"
            );
        }
        if let Some(first) = authorities.first() {
            let first: &StagedFactsAuthority = first;
            ensure!(
                routes == first.routes,
                "facts staged peer request routes differ"
            );
            let first = &first.genesis;
            ensure!(
                authority.context() == first.context()
                    && authority.proofs_of_possession() == first.proofs_of_possession()
                    && authority.catalog_hash() == first.catalog_hash()
                    && authority.active_lanes() == first.active_lanes()
                    && authority.lane_authority_catalog() == first.lane_authority_catalog(),
                "facts staged peer projections differ"
            );
        }
        authorities.push(StagedFactsAuthority {
            genesis: authority,
            routes,
        });
    }
    authorities
        .try_into()
        .map_err(|_| eyre!("facts require four staged genesis authorities"))
}

fn bounded_frame<T: norito::NoritoSerialize>(value: &T, cap: usize) -> Result<Vec<u8>> {
    let count = norito::canonical_frame_len(value)?;
    ensure!(
        count > 0 && count <= cap,
        "facts canonical element exceeds cap"
    );
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = norito::core::to_bytes_bounded(value, cap)?;
    ensure!(
        bytes.len() == count,
        "facts canonical element count changed"
    );
    Ok(bytes)
}
fn group_supplied(
    finality: Vec<FinalizedNativeContextV1>,
    queries: Vec<CommittedTransaction>,
    last: u64,
    limits: VerificationLimits,
) -> Result<Vec<SuppliedEvidenceHeightV1>> {
    ensure!(
        u64::try_from(finality.len())? == last
            && last <= limits.heights
            && queries.len() <= limits.requests,
        "facts vectors do not cover the independently required interval or work bound"
    );
    let mut query = queries.into_iter();
    let mut rows = Vec::new();
    let slots = finality
        .len()
        .checked_mul(std::mem::size_of::<SuppliedEvidenceHeightV1>())
        .and_then(|n| n.checked_add(query.len().checked_mul(std::mem::size_of::<Vec<u8>>())?))
        .ok_or_else(|| eyre!("facts supplied slot reservation overflow"))?;
    let mut charged_bytes = charged(0, slots, limits.input_bytes)?;
    rows.try_reserve_exact(finality.len())?;
    for (index, original) in finality.into_iter().enumerate() {
        let FinalizedNativeContextV1 {
            finality: proof,
            contexts,
        } = original;
        let height = u64::try_from(index)?
            .checked_add(1)
            .ok_or_else(|| eyre!("facts height overflow"))?;
        ensure!(
            proof.block_header.height().get() == height,
            "facts finality vector is not contiguous in original order"
        );
        let mut row = SuppliedEvidenceHeightV1 {
            height,
            finality: bounded_frame(
                &proof,
                MAX_FINALITY_BYTES.min(usize::try_from(limits.input_bytes - charged_bytes)?),
            )?,
            contexts: Vec::new(),
            queries: Vec::new(),
        };
        charged_bytes = charged(charged_bytes, row.finality.len(), limits.input_bytes)?;
        row.contexts = bounded_frame(
            &contexts,
            MAX_FINALITY_BYTES.min(usize::try_from(limits.input_bytes - charged_bytes)?),
        )?;
        charged_bytes = charged(charged_bytes, row.contexts.len(), limits.input_bytes)?;
        let carrier_hash = proof.block_header.hash();
        let cohort = query
            .as_slice()
            .iter()
            .take_while(|q| q.block_hash == carrier_hash)
            .count();
        ensure!(
            cohort <= limits.leaves_per_carrier,
            "facts query cohort exceeds leaf reservation"
        );
        row.queries.try_reserve_exact(cohort)?;
        let mut previous_output = None;
        while query
            .as_slice()
            .first()
            .is_some_and(|q| q.block_hash == carrier_hash)
        {
            let q = query
                .next()
                .ok_or_else(|| eyre!("facts query iterator changed"))?;
            let output_index = q.output_proof.leaf_index();
            ensure!(
                row.queries.len() < limits.leaves_per_carrier
                    && usize::try_from(q.entrypoint_proof.leaf_index())? == row.queries.len()
                    && previous_output.is_none_or(|previous| previous < output_index),
                "facts query vector is not in full contiguous leaf order"
            );
            previous_output = Some(output_index);
            let bytes = bounded_frame(
                &q,
                MAX_TRANSACTION_BYTES.min(usize::try_from(limits.input_bytes - charged_bytes)?),
            )?;
            charged_bytes = charged(charged_bytes, bytes.len(), limits.input_bytes)?;
            row.queries.push(bytes);
        }
        rows.push(row);
    }
    ensure!(
        query.next().is_none(),
        "facts query vector has unmatched or reordered carrier rows"
    );
    Ok(rows)
}
fn catalog_copy_bytes(active_count: usize, catalog: &MergeLaneAuthorityCatalogV1) -> Result<usize> {
    // Validated fixed geometry has at most four BLS rosters with four keys apiece. The 1 KiB
    // key allowance covers each owned public identity; this is a conservative work reservation.
    active_count
        .checked_mul(std::mem::size_of::<NativeWorkloadLane>())
        .and_then(|n| {
            n.checked_add(
                catalog
                    .lane_roster_indices
                    .len()
                    .checked_mul(std::mem::size_of::<u16>())?,
            )
        })
        .and_then(|n| {
            n.checked_add(catalog.rosters.len().checked_mul(
                std::mem::size_of::<iroha_data_model::merge::MergeLaneCommitteeRosterV1>()
                    + 4 * 1024,
            )?)
        })
        .ok_or_else(|| eyre!("facts catalog copy reservation overflow"))
}
fn verification_copy(
    plan: &TrustedRunPlan,
    heights: &[SuppliedEvidenceHeightV1],
    caps: FactsAssemblyCaps,
) -> Result<(TrustedRunPlan, Vec<SuppliedHeightEvidence>)> {
    // Charge the complete additional ordinary-data copy to the same cumulative work
    // allowance before cloning. The output facts cap remains solely a canonical byte bound.
    let mut bytes = charged(
        0,
        catalog_copy_bytes(plan.active_lanes.len(), &plan.lane_authorities)?,
        caps.decode_bytes,
    )?;
    for request in &plan.scheduled {
        for length in [
            std::mem::size_of::<ScheduledRequest>(),
            request.logical_id.len(),
            request.signed_transaction.len(),
        ] {
            bytes = charged(bytes, length, caps.decode_bytes)?;
        }
    }
    for height in heights {
        bytes = charged(
            bytes,
            std::mem::size_of::<SuppliedHeightEvidence>(),
            caps.decode_bytes,
        )?;
        bytes = charged(bytes, height.finality.len(), caps.decode_bytes)?;
        bytes = charged(bytes, height.contexts.len(), caps.decode_bytes)?;
        for query in &height.queries {
            bytes = charged(bytes, std::mem::size_of::<Vec<u8>>(), caps.decode_bytes)?;
            bytes = charged(bytes, query.len(), caps.decode_bytes)?;
        }
    }
    norito::core::reserve_decode_allocation(usize::try_from(bytes)?)?;
    let mut scheduled = Vec::new();
    scheduled.try_reserve_exact(plan.scheduled.len())?;
    for r in &plan.scheduled {
        scheduled.push(ScheduledRequest {
            logical_id: r.logical_id.clone(),
            phase: r.phase,
            signed_transaction: r.signed_transaction.clone(),
            route: r.route,
        });
    }
    let mut supplied = Vec::new();
    supplied.try_reserve_exact(heights.len())?;
    for h in heights {
        supplied.push(SuppliedHeightEvidence {
            height: h.height,
            finality: h.finality.clone(),
            contexts: h.contexts.clone(),
            queries: h.queries.clone(),
        });
    }
    Ok((
        TrustedRunPlan {
            network_id: plan.network_id,
            first_context: plan.first_context,
            first_height: plan.first_height,
            last_height: plan.last_height,
            nexus_amx_context_hash: plan.nexus_amx_context_hash,
            execution_policy_hash: plan.execution_policy_hash,
            active_lanes: plan.active_lanes.clone(),
            lane_authorities: plan.lane_authorities.clone(),
            scheduled,
        },
        supplied,
    ))
}
fn join_observations(
    plan: &TrustedRunPlan,
    observations: &[JournalObservation],
    verified: &VerifiedExport,
) -> Result<()> {
    ensure!(
        observations.len() == plan.scheduled.len() && verified.rows().len() == observations.len(),
        "facts journal and authenticated schedule counts differ"
    );
    let mut phase = None;
    let mut sequence = 0u64;
    for ((request, observation), row) in
        plan.scheduled.iter().zip(observations).zip(verified.rows())
    {
        sequence = if phase == Some(request.phase) {
            sequence + 1
        } else {
            1
        };
        phase = Some(request.phase);
        let signed: SignedTransaction = canonical(&request.signed_transaction)?;
        ensure!(
            u64::try_from(observation.sequence)? == sequence
                && row.sequence == sequence
                && row.request.logical_id == request.logical_id
                && row.request.phase == request.phase
                && row.request.authority == *signed.authority()
                && row.request.entrypoint_hash == signed.hash_as_entrypoint()
                && row.request.carrier_height == observation.block_height
                && row.request.lane_id == request.route.lane_id
                && row.request.dataspace_id == request.route.dataspace_id,
            "facts journal observation differs from the authenticated original request"
        );
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::kura::scaling_evidence::export) mod tests;
