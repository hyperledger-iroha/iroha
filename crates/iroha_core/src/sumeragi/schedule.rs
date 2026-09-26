//! The lag-2 height-configuration schedule of the global Sumeragi instance (`specs/sumeragi.md`
//! §10.1), the genesis committee, the validation of the on-chain Sumeragi chain parameters and
//! the mapping to the core's [`Committee`], [`ChainParams`] and [`HeightConfig`].
//!
//! **Rule.** Heights `g + 1` and `g + 2` use the configuration of the genesis state; every later
//! height `h + 2` uses what the state after block `h` schedules. After executing block `h` the
//! executor calls [`advance`]: it rotates the World field `consensus_schedule` to the window
//! `(h, h + 1, h + 2)`, deriving the new entry `h + 2` from the post-state of `h`
//! ([`next_config`]), and `R_h` commits that entry (`commitment.rs`). A restart therefore fills
//! `Init.configs` (`t`, `t + 1`, `t + 2`) from state alone ([`ConsensusSchedule::init_configs`]).
//!
//! **Committee.** The committee scheduled for height `x` is every registered peer whose global
//! validator consensus key (`RegisterPeerWithPop`, role `Validator`) is live at `x`, in the core's
//! canonical key order. A `RegisterPeerWithPop` or `Unregister<Peer>` executed in block `h` thus
//! takes effect at `h + 2` (or at the key's later activation height). Plain lane-committee peers
//! (role `Committee`) are never members. Both consensus modes use this rule until NPoS elections
//! schedule committees (goal S8); every member has one vote and any `n ≥ 1` is accepted.
//!
//! **Chain parameters.** `ChainParams_{h+2}` are the on-chain `SumeragiParameters` of the state
//! after `h` (`block_cadence_ms` is `block_time`). `SetParameter` validates a change with
//! [`iroha_sumeragi::pacemaker::validate_chain`] against the fixed chain-wide transport limit
//! [`CHAIN_TRANSPORT_FRAME_LIMIT`] ([`validate_parameter_change`]); the demotion window `W` is a
//! genesis constant and is rejected after genesis.
//!
//! Every function here is deterministic: it reads committed state only (never node-local
//! configuration or clocks).

use std::collections::BTreeMap;

use iroha_crypto::Algorithm;
use iroha_data_model::{
    consensus::ConsensusKeyRole,
    isi::RegisterBox,
    parameter::system::{SumeragiParameter, SumeragiParameters},
    transaction::Executable,
};
use iroha_genesis::GenesisBlock;
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{
    api::ConfigError,
    pacemaker::{FRAME_OVERHEAD, validate_chain},
    types::{ChainParams, Committee, CommitteeError, HeightConfig, PublicKey},
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
};
use thiserror::Error;

use crate::state::{StateBlock, WorldReadOnly, live_consensus_key_pop_for_peer_with_role};

/// The schedule lag: the state after `h` schedules height `h + LAG` (§10.1).
pub const LAG: u64 = 2;

/// Largest consensus frame every node's transport accepts, the chain-wide bound that on-chain
/// chain parameters are validated against (§9.4, O10): 16 MiB of payload plus the core's frame
/// overhead. It is a protocol constant, not node configuration, so validation is deterministic;
/// it equals the driver's default frame limit.
pub const CHAIN_TRANSPORT_FRAME_LIMIT: u64 = 16 * 1024 * 1024 + FRAME_OVERHEAD as u64;

/// Chain parameters of one height as stored in World and committed in `R` (§10.1, §12.4): the
/// Norito and JSON form of the core's [`ChainParams`].
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::sumeragi::schedule::ChainParamsRecord")]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct ChainParamsRecord {
    /// Target block time in milliseconds (`block_cadence_ms`).
    pub block_time_ms: u64,
    /// Heartbeat interval of an idle chain in milliseconds.
    pub idle_block_interval_ms: u64,
    /// Execution budget `E_max` in milliseconds.
    pub exec_budget_ms: u64,
    /// Apply budget `A_max` in milliseconds.
    pub apply_budget_ms: u64,
    /// Largest block payload in bytes.
    pub max_block_bytes: u32,
    /// Fresh blocks from this view on are `EMPTY`.
    pub empty_after_views: u64,
    /// Epoch length in heights.
    pub epoch_length_blocks: u64,
}

impl ChainParamsRecord {
    /// The chain parameters the on-chain Sumeragi parameters define.
    #[must_use]
    pub fn from_parameters(params: &SumeragiParameters) -> Self {
        Self {
            block_time_ms: params.block_cadence_ms.get(),
            idle_block_interval_ms: params.idle_block_interval_ms.get(),
            exec_budget_ms: params.exec_budget_ms.get(),
            apply_budget_ms: params.apply_budget_ms.get(),
            max_block_bytes: params.max_block_bytes.get(),
            empty_after_views: params.empty_after_views.get(),
            epoch_length_blocks: params.epoch_length_blocks.get(),
        }
    }

    /// The core's chain parameters.
    #[must_use]
    pub fn to_core(&self) -> ChainParams {
        ChainParams {
            block_time: self.block_time_ms,
            idle_block_interval: self.idle_block_interval_ms,
            e_max: self.exec_budget_ms,
            a_max: self.apply_budget_ms,
            max_block_bytes: self.max_block_bytes,
            empty_after_views: self.empty_after_views,
            epoch_length: self.epoch_length_blocks,
        }
    }

    /// The record of the core's chain parameters.
    #[must_use]
    pub fn from_core(params: &ChainParams) -> Self {
        Self {
            block_time_ms: params.block_time,
            idle_block_interval_ms: params.idle_block_interval,
            exec_budget_ms: params.e_max,
            apply_budget_ms: params.a_max,
            max_block_bytes: params.max_block_bytes,
            empty_after_views: params.empty_after_views,
            epoch_length_blocks: params.epoch_length,
        }
    }

    /// §9.4 validation against [`CHAIN_TRANSPORT_FRAME_LIMIT`].
    ///
    /// # Errors
    /// The first violated rule.
    pub fn validate(&self) -> Result<(), ConfigError> {
        validate_chain(&self.to_core(), CHAIN_TRANSPORT_FRAME_LIMIT)
    }
}

/// The configuration of one height as stored in World: the committee (peers in the core's
/// canonical key order) and the chain parameters.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::sumeragi::schedule::ScheduledConfig")]
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct ScheduledConfig {
    /// The height this configuration applies to.
    pub height: u64,
    /// `C_height`: distinct BLS-normal validator peers in canonical order.
    pub committee: Vec<PeerId>,
    /// Chain parameters of `height`.
    pub params: ChainParamsRecord,
}

impl ScheduledConfig {
    /// The core's height configuration.
    ///
    /// # Errors
    /// A malformed committee (empty, too large, duplicate or non-BLS-normal keys) or chain
    /// parameters that fail §9.4 validation.
    pub fn height_config(&self) -> Result<HeightConfig, ScheduleError> {
        let keys = self
            .committee
            .iter()
            .map(consensus_key)
            .collect::<Result<Vec<_>, _>>()?;
        let committee = Committee::new(keys).map_err(ScheduleError::Committee)?;
        self.params.validate().map_err(ScheduleError::Params)?;
        Ok(HeightConfig {
            committee,
            params: self.params.to_core(),
        })
    }
}

/// The World-stored lag-2 schedule: the configurations of `t`, `t + 1` and `t + 2`, where `t`
/// is the height of the latest block whose execution advanced it. Empty before genesis.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::sumeragi::schedule::ConsensusSchedule")]
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct ConsensusSchedule {
    /// Three consecutive heights in ascending order, or none.
    entries: Vec<ScheduledConfig>,
}

impl ConsensusSchedule {
    /// The window `(g, g + 1, g + 2)` after the genesis block.
    #[must_use]
    pub fn genesis(genesis_height: u64, configs: [ScheduledConfig; 3]) -> Self {
        let entries = configs
            .into_iter()
            .zip(0..)
            .map(|(config, offset)| ScheduledConfig {
                height: genesis_height.saturating_add(offset),
                ..config
            })
            .collect();
        Self { entries }
    }

    /// The entries, oldest first.
    #[must_use]
    pub fn entries(&self) -> &[ScheduledConfig] {
        &self.entries
    }

    /// `t`: the height of the latest block that advanced the schedule (`None` before genesis).
    #[must_use]
    pub fn tip(&self) -> Option<u64> {
        self.entries.first().map(|entry| entry.height)
    }

    /// The configuration scheduled for `height`, if the window holds it.
    #[must_use]
    pub fn get(&self, height: u64) -> Option<&ScheduledConfig> {
        self.entries.iter().find(|entry| entry.height == height)
    }

    /// Whether the schedule is empty or three consecutive heights.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.entries.is_empty()
            || (self.entries.len() == 3
                && self
                    .entries
                    .windows(2)
                    .all(|pair| pair[0].height.checked_add(1) == Some(pair[1].height)))
    }

    /// The window after applying block `height` whose post-state scheduled `next` for
    /// `height + 2`: `(height, height + 1, height + 2)`. Re-advancing to the current tip replaces
    /// its last entry (re-derivation from the same state gives the same entry).
    ///
    /// # Errors
    /// The schedule is malformed or does not end at `height + 1` (or `height + 2`).
    pub fn advanced(&self, height: u64, next: ScheduledConfig) -> Result<Self, ScheduleError> {
        let target = height
            .checked_add(LAG)
            .ok_or(ScheduleError::HeightOverflow)?;
        if !self.is_well_formed() {
            return Err(ScheduleError::Malformed);
        }
        let next = ScheduledConfig {
            height: target,
            ..next
        };
        let mut entries = self.entries.clone();
        match self.entries.last().map(|entry| entry.height) {
            Some(last) if last.checked_add(1) == Some(target) => {
                entries.remove(0);
                entries.push(next);
            }
            Some(last) if last == target => {
                if let Some(slot) = entries.last_mut() {
                    *slot = next;
                }
            }
            last => {
                return Err(ScheduleError::NotConsecutive {
                    height,
                    last_scheduled: last,
                });
            }
        }
        Ok(Self { entries })
    }

    /// `Init.configs` (§12.1): the configurations of `t` (unless `t` is the genesis height),
    /// `t + 1` and `t + 2`.
    ///
    /// # Errors
    /// The schedule is empty or malformed, or an entry does not map to a core configuration.
    pub fn init_configs(
        &self,
        genesis_height: u64,
    ) -> Result<Vec<(u64, HeightConfig)>, ScheduleError> {
        if self.entries.is_empty() || !self.is_well_formed() {
            return Err(ScheduleError::Malformed);
        }
        self.entries
            .iter()
            .filter(|entry| entry.height != genesis_height)
            .map(|entry| Ok((entry.height, entry.height_config()?)))
            .collect()
    }
}

/// Why a schedule operation failed. Every variant is a deterministic function of committed
/// state: a block whose execution cannot advance the schedule is invalid on every node.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ScheduleError {
    /// A committee key is not a BLS-normal public key the core accepts.
    #[error("committee member {0} is not a BLS-normal consensus key")]
    NotBlsNormal(String),
    /// The committee cannot be formed (empty, too large, duplicate or malformed keys).
    #[error("invalid committee: {0}")]
    Committee(CommitteeError),
    /// The chain parameters fail §9.4 validation.
    #[error("invalid chain parameters: {0}")]
    Params(ConfigError),
    /// The stored schedule is not empty or three consecutive heights.
    #[error("the stored consensus schedule is malformed")]
    Malformed,
    /// The stored schedule does not end right before the height being scheduled.
    #[error("block {height} cannot advance a schedule ending at {last_scheduled:?}")]
    NotConsecutive {
        /// Height of the executed block.
        height: u64,
        /// Last height the stored schedule covers.
        last_scheduled: Option<u64>,
    },
    /// A block below the genesis height.
    #[error("block {height} is below the genesis height {genesis_height}")]
    BeforeGenesis {
        /// Height of the executed block.
        height: u64,
        /// Genesis height.
        genesis_height: u64,
    },
    /// A demotion-window change after genesis.
    #[error("the Sumeragi demotion window is a genesis constant")]
    GenesisOnly,
    /// A height does not fit `u64`.
    #[error("height overflow")]
    HeightOverflow,
}

/// The core consensus key of a validator peer: the raw 48-byte BLS-normal public key (§3.1).
///
/// # Errors
/// [`ScheduleError::NotBlsNormal`] for another algorithm or a malformed key.
pub fn consensus_key(peer: &PeerId) -> Result<PublicKey, ScheduleError> {
    let not_bls = || ScheduleError::NotBlsNormal(peer.to_string());
    let (algorithm, payload) = peer.public_key().try_to_bytes().map_err(|_| not_bls())?;
    if algorithm != Algorithm::BlsNormal {
        return Err(not_bls());
    }
    PublicKey::new(payload.to_vec()).map_err(|_| not_bls())
}

/// The peer of a core consensus key (inverse of [`consensus_key`]).
///
/// # Errors
/// [`ScheduleError::NotBlsNormal`] if the bytes are not a BLS-normal public key.
pub fn peer_of(key: &PublicKey) -> Result<PeerId, ScheduleError> {
    iroha_crypto::PublicKey::from_bytes(Algorithm::BlsNormal, key.as_bytes())
        .map(PeerId::new)
        .map_err(|_| ScheduleError::NotBlsNormal(format!("{key:?}")))
}

/// Peers in the core's canonical committee order, deduplicated.
///
/// # Errors
/// A peer without a BLS-normal key.
pub fn canonical_committee(
    peers: impl IntoIterator<Item = PeerId>,
) -> Result<Vec<PeerId>, ScheduleError> {
    let ordered = peers
        .into_iter()
        .map(|peer| Ok((consensus_key(&peer)?, peer)))
        .collect::<Result<BTreeMap<_, _>, ScheduleError>>()?;
    Ok(ordered.into_values().collect())
}

/// The committee the state `world` schedules for height `height`: every registered peer whose
/// global validator consensus key is live at `height`, in canonical order (see the module
/// documentation).
///
/// # Errors
/// A live validator whose key is not BLS-normal.
pub fn scheduled_committee(
    world: &impl WorldReadOnly,
    height: u64,
) -> Result<Vec<PeerId>, ScheduleError> {
    canonical_committee(
        world
            .peers()
            .iter()
            .filter(|peer| {
                live_consensus_key_pop_for_peer_with_role(
                    world,
                    peer,
                    height,
                    ConsensusKeyRole::Validator,
                )
                .is_some()
            })
            .cloned(),
    )
}

/// The configuration the state `world` schedules for height `height`, checked to map to a core
/// [`HeightConfig`] (non-empty committee of at most `MAX_COMMITTEE_SIZE` BLS-normal keys, valid
/// chain parameters).
///
/// # Errors
/// The derived configuration is not a valid core configuration.
pub fn next_config(
    world: &impl WorldReadOnly,
    height: u64,
) -> Result<ScheduledConfig, ScheduleError> {
    let config = ScheduledConfig {
        height,
        committee: scheduled_committee(world, height)?,
        params: ChainParamsRecord::from_parameters(world.parameters().sumeragi()),
    };
    config.height_config()?;
    Ok(config)
}

/// Advance the World schedule after executing `block` (height `h`, still in its overlay): at the
/// genesis height install `(g, g + 1, g + 2)` from the genesis state, otherwise rotate the
/// window to `(h, h + 1, h + 2)`. Returns the configuration of `h + 2`, which `R_h` commits.
///
/// # Errors
/// The block is below genesis, the stored schedule does not end at `h + 1`, or the state
/// schedules an invalid configuration (the block is then deterministically invalid).
pub fn advance(
    block: &mut StateBlock<'_>,
    genesis_height: u64,
) -> Result<ScheduledConfig, ScheduleError> {
    let height = block._curr_block.height().get();
    if height < genesis_height {
        return Err(ScheduleError::BeforeGenesis {
            height,
            genesis_height,
        });
    }
    let target = height
        .checked_add(LAG)
        .ok_or(ScheduleError::HeightOverflow)?;
    let next = next_config(&block.world, target)?;
    let schedule = if height == genesis_height {
        ConsensusSchedule::genesis(
            genesis_height,
            [
                next_config(&block.world, genesis_height)?,
                next_config(&block.world, genesis_height.saturating_add(1))?,
                next.clone(),
            ],
        )
    } else {
        block
            .world
            .consensus_schedule
            .get()
            .advanced(height, next.clone())?
    };
    *block.world.consensus_schedule.get_mut() = schedule;
    Ok(next)
}

/// Check a `SetParameter` of a Sumeragi parameter against the current parameters: the demotion
/// window only in the genesis block; after genesis, a chain parameter change must leave chain
/// parameters that pass [`ChainParamsRecord::validate`]. (In genesis the parameters are checked
/// together when the genesis schedule is installed, so their order does not matter.) The change
/// takes effect at `h + 2` through the schedule.
///
/// # Errors
/// [`ScheduleError::GenesisOnly`] or [`ScheduleError::Params`].
pub fn validate_parameter_change(
    current: &SumeragiParameters,
    change: &SumeragiParameter,
    is_genesis: bool,
) -> Result<(), ScheduleError> {
    if is_genesis {
        return Ok(());
    }
    if change.is_genesis_only() {
        return Err(ScheduleError::GenesisOnly);
    }
    if matches!(change, SumeragiParameter::MaxClockDriftMs(_)) {
        return Ok(());
    }
    let mut candidate = current.clone();
    match *change {
        SumeragiParameter::IdleBlockIntervalMs(value) => candidate.idle_block_interval_ms = value,
        SumeragiParameter::ExecBudgetMs(value) => candidate.exec_budget_ms = value,
        SumeragiParameter::ApplyBudgetMs(value) => candidate.apply_budget_ms = value,
        SumeragiParameter::MaxBlockBytes(value) => candidate.max_block_bytes = value,
        SumeragiParameter::EmptyAfterViews(value) => candidate.empty_after_views = value,
        SumeragiParameter::EpochLengthBlocks(value) => candidate.epoch_length_blocks = value,
        SumeragiParameter::MaxClockDriftMs(_) | SumeragiParameter::DemotionWindow(_) => {}
    }
    ChainParamsRecord::from_parameters(&candidate)
        .validate()
        .map_err(ScheduleError::Params)
}

/// Why a genesis block defines no committee.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum GenesisCommitteeError {
    /// A genesis transaction is not an instruction list.
    #[error("genesis transactions must be instruction lists")]
    UnsupportedExecutable,
    /// A registered validator key is not BLS-normal.
    #[error("genesis validator {0} is not a BLS-normal key")]
    NonBlsValidator(String),
    /// A registered validator's proof of possession does not verify.
    #[error("genesis validator {0} has an invalid proof of possession")]
    InvalidProofOfPossession(String),
    /// A validator is registered twice.
    #[error("genesis registers validator {0} more than once")]
    DuplicateValidator(String),
    /// The validators do not form a core committee (e.g. none).
    #[error("genesis validators do not form a committee: {0}")]
    Committee(CommitteeError),
}

/// The validators signed into genesis: every `RegisterPeerWithPop` of the genesis body with its
/// exact proof of possession, which is verified here (`specs/sumeragi.md` §10.3: a key enters
/// the schedule only with a valid PoP). Lane-committee registrations are not validators.
///
/// # Errors
/// A non-instruction transaction, a non-BLS-normal key, an invalid PoP or a duplicate.
pub fn genesis_validators(
    genesis: &GenesisBlock,
) -> Result<BTreeMap<PeerId, Vec<u8>>, GenesisCommitteeError> {
    let mut validators = BTreeMap::new();
    for transaction in genesis.0.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(GenesisCommitteeError::UnsupportedExecutable);
        };
        for register in instructions.iter().filter_map(|instruction| {
            match instruction.as_any().downcast_ref::<RegisterBox>()? {
                RegisterBox::Peer(register) => Some(register),
                _ => None,
            }
        }) {
            let name = || register.peer.to_string();
            if register.peer.public_key().try_algorithm() != Ok(Algorithm::BlsNormal) {
                return Err(GenesisCommitteeError::NonBlsValidator(name()));
            }
            iroha_crypto::bls_normal_pop_verify(register.peer.public_key(), &register.pop)
                .map_err(|_| GenesisCommitteeError::InvalidProofOfPossession(name()))?;
            if validators
                .insert(register.peer.clone(), register.pop.clone())
                .is_some()
            {
                return Err(GenesisCommitteeError::DuplicateValidator(name()));
            }
        }
    }
    Ok(validators)
}

/// The genesis committee `C_{g+1} = C_{g+2}` as the core sees it.
///
/// # Errors
/// See [`genesis_validators`]; also an empty or oversized committee.
pub fn genesis_committee(genesis: &GenesisBlock) -> Result<Committee, GenesisCommitteeError> {
    let keys = genesis_validators(genesis)?
        .into_keys()
        .map(|peer| {
            consensus_key(&peer)
                .map_err(|_| GenesisCommitteeError::NonBlsValidator(peer.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Committee::new(keys).map_err(GenesisCommitteeError::Committee)
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroU64};

    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::AccountId,
        block::{BlockHeader, SignedBlock},
        consensus::{ConsensusKeyRecord, ConsensusKeyStatus},
        isi::{InstructionBox, Log, register::RegisterPeerWithPop},
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use iroha_logger::Level;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World, derive_validator_key_id},
    };

    fn bls() -> KeyPair {
        KeyPair::random_with_algorithm(Algorithm::BlsNormal)
    }

    fn peer(pair: &KeyPair) -> PeerId {
        PeerId::new(pair.public_key().clone())
    }

    fn params() -> ChainParamsRecord {
        ChainParamsRecord::from_core(&ChainParams::default())
    }

    fn config(height: u64, committee: Vec<PeerId>) -> ScheduledConfig {
        ScheduledConfig {
            height,
            committee: canonical_committee(committee).expect("bls peers"),
            params: params(),
        }
    }

    fn header(height: u64) -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero"),
            None,
            None,
            height,
            0,
        )
    }

    fn register(world: &mut crate::state::WorldBlock<'_>, peer: &PeerId, activation: u64) {
        let _ = world.peers.get_mut().push(peer.clone());
        let id = derive_validator_key_id(peer.public_key());
        world.consensus_keys.insert(
            id.clone(),
            ConsensusKeyRecord {
                id: id.clone(),
                public_key: peer.public_key().clone(),
                pop: Some(vec![1]),
                activation_height: activation,
                expiry_height: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            },
        );
        world
            .consensus_keys_by_pk
            .insert(peer.public_key().to_string(), vec![id]);
    }

    fn state() -> State {
        State::new(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    #[test]
    fn chain_params_record_round_trips_the_core_and_on_chain_forms() {
        let core = ChainParams::default();
        let record = ChainParamsRecord::from_core(&core);
        assert_eq!(record.to_core(), core);
        assert_eq!(
            ChainParamsRecord::from_parameters(&SumeragiParameters::default()),
            record
        );
        record.validate().expect("defaults are valid");
        let bytes = norito::encode_canonical(&record).expect("encode");
        assert_eq!(
            norito::decode_canonical::<ChainParamsRecord>(&bytes).expect("decode"),
            record
        );
        let json = norito::json::to_json(&record).expect("json");
        assert_eq!(
            norito::json::from_str::<ChainParamsRecord>(&json).expect("parse"),
            record
        );
    }

    #[test]
    fn chain_params_validation_uses_the_chain_transport_limit() {
        let mut record = params();
        record.max_block_bytes =
            u32::try_from(CHAIN_TRANSPORT_FRAME_LIMIT - u64::from(FRAME_OVERHEAD)).expect("fits");
        record.validate().expect("exactly at the limit");
        record.max_block_bytes += 1;
        assert_eq!(
            record.validate(),
            Err(ConfigError::MaxBlockBytesAboveTransport)
        );
        let mut record = params();
        record.block_time_ms = record.idle_block_interval_ms + 1;
        assert_eq!(record.validate(), Err(ConfigError::BlockTimeAboveIdle));
        assert_eq!(
            CHAIN_TRANSPORT_FRAME_LIMIT,
            crate::sumeragi::driver::DriverConfig::default().frame_limit
        );
    }

    #[test]
    fn consensus_key_maps_bls_peers_both_ways_and_rejects_others() {
        let pair = bls();
        let key = consensus_key(&peer(&pair)).expect("bls");
        assert_eq!(key.as_bytes().len(), 48);
        assert_eq!(peer_of(&key).expect("inverse"), peer(&pair));
        let ed = KeyPair::random();
        assert!(matches!(
            consensus_key(&peer(&ed)),
            Err(ScheduleError::NotBlsNormal(_))
        ));
        assert!(peer_of(&PublicKey::new(vec![7; 48]).expect("len")).is_err());
    }

    #[test]
    fn canonical_committee_sorts_by_core_key_and_dedups() {
        let pairs = [bls(), bls(), bls()];
        let peers: Vec<_> = pairs.iter().map(peer).collect();
        let committee = canonical_committee(peers.iter().rev().cloned().chain([peers[0].clone()]))
            .expect("bls");
        assert_eq!(committee.len(), 3);
        let keys: Vec<_> = committee
            .iter()
            .map(|p| consensus_key(p).unwrap())
            .collect();
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn scheduled_config_maps_to_the_core_and_rejects_bad_configs() {
        let pairs = [bls(), bls()];
        let entry = config(5, pairs.iter().map(peer).collect());
        let height_config = entry.height_config().expect("valid");
        assert_eq!(height_config.committee.n(), 2);
        assert_eq!(height_config.params, ChainParams::default());
        let members: Vec<_> = height_config
            .committee
            .members()
            .iter()
            .map(|k| peer_of(k).unwrap())
            .collect();
        assert_eq!(members, entry.committee);
        let empty = config(5, Vec::new());
        assert_eq!(
            empty.height_config(),
            Err(ScheduleError::Committee(CommitteeError::Empty))
        );
        let mut bad = entry;
        bad.params.empty_after_views = 0;
        assert!(matches!(bad.height_config(), Err(ScheduleError::Params(_))));
    }

    #[test]
    fn schedule_genesis_advance_and_init_configs() {
        let a = peer(&bls());
        let b = peer(&bls());
        let genesis = ConsensusSchedule::genesis(
            1,
            [
                config(0, vec![a.clone()]),
                config(0, vec![a.clone()]),
                config(0, vec![a.clone()]),
            ],
        );
        assert!(genesis.is_well_formed());
        assert_eq!(genesis.tip(), Some(1));
        assert_eq!(
            genesis
                .entries()
                .iter()
                .map(|e| e.height)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        // Init at the genesis tip leaves out the genesis height itself.
        let init = genesis.init_configs(1).expect("configs");
        assert_eq!(init.iter().map(|(h, _)| *h).collect::<Vec<_>>(), [2, 3]);
        // Block 2 schedules height 4; block 3 height 5.
        let after_2 = genesis
            .advanced(2, config(0, vec![a.clone(), b.clone()]))
            .expect("advance");
        assert_eq!(after_2.tip(), Some(2));
        assert_eq!(after_2.get(4).expect("h+2").committee.len(), 2);
        assert_eq!(after_2.get(3), genesis.get(3));
        let init = after_2.init_configs(1).expect("configs");
        assert_eq!(init.iter().map(|(h, _)| *h).collect::<Vec<_>>(), [2, 3, 4]);
        // Re-advancing to the same tip re-derives the last entry only.
        let again = after_2
            .advanced(2, config(0, vec![b.clone()]))
            .expect("same tip");
        assert_eq!(again.get(3), after_2.get(3));
        assert_eq!(again.get(4).expect("h+2").committee, vec![b.clone()]);
        // A gap or a stale height is refused.
        assert!(matches!(
            after_2.advanced(4, config(0, vec![a.clone()])),
            Err(ScheduleError::NotConsecutive { .. })
        ));
        assert!(matches!(
            ConsensusSchedule::default().advanced(2, config(0, vec![a.clone()])),
            Err(ScheduleError::NotConsecutive {
                last_scheduled: None,
                ..
            })
        ));
        assert_eq!(
            ConsensusSchedule::default().init_configs(1),
            Err(ScheduleError::Malformed)
        );
        assert!(ConsensusSchedule::default().is_well_formed());
    }

    #[test]
    fn schedule_codec_and_json_round_trip() {
        let a = peer(&bls());
        let schedule = ConsensusSchedule::genesis(
            1,
            [
                config(0, vec![a.clone()]),
                config(0, vec![a.clone()]),
                config(0, vec![a]),
            ],
        );
        let bytes = norito::encode_canonical(&schedule).expect("encode");
        assert_eq!(
            norito::decode_canonical::<ConsensusSchedule>(&bytes).expect("decode"),
            schedule
        );
        let json = norito::json::to_json(&schedule).expect("json");
        assert_eq!(
            norito::json::from_str::<ConsensusSchedule>(&json).expect("parse"),
            schedule
        );
    }

    #[test]
    fn world_schedules_live_validators_at_the_target_height() {
        let state = state();
        let (early, late, observer) = (peer(&bls()), peer(&bls()), peer(&bls()));
        let mut block = state.block(header(1));
        register(&mut block.world, &early, 1);
        register(&mut block.world, &late, 4);
        // A peer without a validator key (e.g. a lane-committee peer) is never a member.
        let _ = block.world.peers.get_mut().push(observer.clone());
        assert_eq!(
            scheduled_committee(&block.world, 3).expect("bls"),
            vec![early.clone()]
        );
        let at_4 = scheduled_committee(&block.world, 4).expect("bls");
        assert_eq!(
            at_4,
            canonical_committee([early.clone(), late.clone()]).unwrap()
        );
        let next = next_config(&block.world, 4).expect("valid");
        assert_eq!(next.committee, at_4);
        assert_eq!(
            next.params,
            ChainParamsRecord::from_parameters(&SumeragiParameters::default())
        );
        // Nobody is live before the first activation: no committee.
        assert_eq!(
            next_config(&block.world, 0),
            Err(ScheduleError::Committee(CommitteeError::Empty))
        );
    }

    #[test]
    fn advance_installs_the_genesis_window_then_rotates_with_lag_two() {
        let state = state();
        let (a, b) = (peer(&bls()), peer(&bls()));
        let mut genesis = state.block(header(1));
        register(&mut genesis.world, &a, 1);
        let scheduled = advance(&mut genesis, 1).expect("genesis schedule");
        assert_eq!(scheduled.height, 3);
        let schedule = genesis.world.consensus_schedule.get().clone();
        assert_eq!(
            schedule
                .entries()
                .iter()
                .map(|e| (e.height, e.committee.clone()))
                .collect::<Vec<_>>(),
            vec![
                (1, vec![a.clone()]),
                (2, vec![a.clone()]),
                (3, vec![a.clone()])
            ]
        );
        genesis
            .commit_world_overlay_for_testing()
            .expect("commit genesis overlay");
        // The committed World carries the schedule (a restart reads `Init.configs` from it).
        assert_eq!(state.view().world().consensus_schedule(), &schedule);
        // Block 2 registers b (live from 3): it joins at 4 = 2 + LAG, not earlier.
        let mut block = state.block(header(2));
        register(&mut block.world, &b, 3);
        let scheduled = advance(&mut block, 1).expect("advance");
        assert_eq!(scheduled.height, 4);
        assert_eq!(
            scheduled.committee,
            canonical_committee([a.clone(), b.clone()]).unwrap()
        );
        let schedule = block.world.consensus_schedule.get().clone();
        assert_eq!(schedule.tip(), Some(2));
        assert_eq!(schedule.get(3).expect("kept").committee, vec![a.clone()]);
        // Advancing again from the same overlay re-derives the same entry.
        assert_eq!(advance(&mut block, 1).expect("idempotent"), scheduled);
        // A block below genesis cannot advance.
        assert!(matches!(
            advance(&mut block, 3),
            Err(ScheduleError::BeforeGenesis { .. })
        ));
    }

    #[test]
    fn parameter_changes_validate_after_genesis_only() {
        let current = SumeragiParameters::default();
        let window = SumeragiParameter::DemotionWindow(NonZeroU64::new(64).unwrap());
        validate_parameter_change(&current, &window, true).expect("genesis");
        assert_eq!(
            validate_parameter_change(&current, &window, false),
            Err(ScheduleError::GenesisOnly)
        );
        let too_fast_idle = SumeragiParameter::IdleBlockIntervalMs(NonZeroU64::new(999).unwrap());
        assert_eq!(
            validate_parameter_change(&current, &too_fast_idle, false),
            Err(ScheduleError::Params(ConfigError::BlockTimeAboveIdle))
        );
        // In genesis the combination is checked when the schedule is installed.
        validate_parameter_change(&current, &too_fast_idle, true).expect("genesis defers");
        let huge = SumeragiParameter::MaxBlockBytes(NonZeroU32::new(u32::MAX).unwrap());
        assert!(matches!(
            validate_parameter_change(&current, &huge, false),
            Err(ScheduleError::Params(
                ConfigError::MaxBlockBytesAboveTransport
            ))
        ));
        for ok in [
            SumeragiParameter::MaxClockDriftMs(5),
            SumeragiParameter::IdleBlockIntervalMs(NonZeroU64::new(6_000).unwrap()),
            SumeragiParameter::ExecBudgetMs(NonZeroU64::new(3_000).unwrap()),
            SumeragiParameter::ApplyBudgetMs(NonZeroU64::new(500).unwrap()),
            SumeragiParameter::MaxBlockBytes(NonZeroU32::new(1 << 20).unwrap()),
            SumeragiParameter::EmptyAfterViews(NonZeroU64::new(3).unwrap()),
            SumeragiParameter::EpochLengthBlocks(NonZeroU64::new(7_200).unwrap()),
        ] {
            validate_parameter_change(&current, &ok, false).expect("valid change");
        }
    }

    fn genesis_with(instructions: Vec<InstructionBox>) -> GenesisBlock {
        let genesis_key = KeyPair::random();
        let account = AccountId::new(genesis_key.public_key().clone());
        let transaction =
            TransactionBuilder::new_genesis(account, FeePaymentIntent::authority(Vec::new(), None))
                .with_instructions(instructions)
                .sign(genesis_key.private_key());
        GenesisBlock(SignedBlock::genesis(
            vec![transaction],
            genesis_key.private_key(),
            None,
            None,
        ))
    }

    fn register_instruction(pair: &KeyPair) -> InstructionBox {
        let pop = iroha_crypto::bls_normal_pop_prove(pair.private_key()).expect("pop");
        InstructionBox::from(RegisterBox::Peer(RegisterPeerWithPop::new(peer(pair), pop)))
    }

    #[test]
    fn genesis_committee_reads_the_signed_validator_registrations() {
        let pairs = [bls(), bls(), bls()];
        let mut instructions: Vec<_> = pairs.iter().map(register_instruction).collect();
        instructions.push(InstructionBox::from(Log::new(Level::INFO, "x".to_owned())));
        let genesis = genesis_with(instructions);
        let validators = genesis_validators(&genesis).expect("validators");
        assert_eq!(validators.len(), 3);
        let committee = genesis_committee(&genesis).expect("committee");
        assert_eq!(committee.n(), 3);
        for pair in &pairs {
            assert!(committee.contains(&consensus_key(&peer(pair)).unwrap()));
        }
    }

    #[test]
    fn genesis_committee_rejects_bad_registrations() {
        let pair = bls();
        let duplicate = genesis_with(vec![
            register_instruction(&pair),
            register_instruction(&pair),
        ]);
        assert!(matches!(
            genesis_validators(&duplicate),
            Err(GenesisCommitteeError::DuplicateValidator(_))
        ));
        let other = bls();
        let bad_pop = iroha_crypto::bls_normal_pop_prove(other.private_key()).expect("pop");
        let forged = genesis_with(vec![InstructionBox::from(RegisterBox::Peer(
            RegisterPeerWithPop::new(peer(&pair), bad_pop),
        ))]);
        assert!(matches!(
            genesis_validators(&forged),
            Err(GenesisCommitteeError::InvalidProofOfPossession(_))
        ));
        let empty = genesis_with(vec![InstructionBox::from(Log::new(
            Level::INFO,
            "x".to_owned(),
        ))]);
        assert_eq!(
            genesis_committee(&empty),
            Err(GenesisCommitteeError::Committee(CommitteeError::Empty))
        );
    }
}
