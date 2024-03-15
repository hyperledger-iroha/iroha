//! Code that should be generated by a procmacro in future.

#![allow(missing_docs)]

use std::{
    error::Error,
    num::{NonZeroU32, NonZeroUsize},
    path::PathBuf,
};

use eyre::{eyre, WrapErr};
use iroha_config_base::{
    Emitter, ErrorsCollection, ExtendsPaths, FromEnv, FromEnvDefaultFallback, FromEnvResult,
    HumanBytes, HumanDuration, Merge, MissingFieldError, ParseEnvResult, ReadEnv, UnwrapPartial,
    UnwrapPartialResult, UserField,
};
use iroha_crypto::{PrivateKey, PublicKey};
use iroha_data_model::{
    metadata::Limits as MetadataLimits,
    prelude::{ChainId, PeerId},
    transaction::TransactionLimits,
    LengthLimits, Level,
};
use iroha_primitives::addr::SocketAddr;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    kura::InitMode as KuraInitMode,
    logger::Format,
    parameters::{
        defaults::{self, chain_wide::*, network::*, queue::*, torii::*},
        user,
        user::{
            ChainWide, Genesis, Kura, KuraDebug, Logger, Network, Queue, Root, Snapshot, Sumeragi,
            SumeragiDebug, Telemetry, TelemetryDev, Torii,
        },
    },
    snapshot::Mode as SnapshotMode,
};

#[derive(Deserialize, Serialize, Debug, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct RootPartial {
    pub extends: Option<ExtendsPaths>,
    pub chain_id: UserField<ChainId>,
    pub public_key: UserField<PublicKey>,
    pub private_key: UserField<PrivateKey>,
    pub genesis: GenesisPartial,
    pub kura: KuraPartial,
    pub sumeragi: SumeragiPartial,
    pub network: NetworkPartial,
    pub logger: LoggerPartial,
    pub queue: QueuePartial,
    pub snapshot: SnapshotPartial,
    pub telemetry: TelemetryPartial,
    pub torii: ToriiPartial,
    pub chain_wide: ChainWidePartial,
}

impl RootPartial {
    /// Creates new empty user configuration
    pub fn new() -> Self {
        // TODO: generate this function with macro. For now, use default
        Self::default()
    }
}

impl UnwrapPartial for RootPartial {
    type Output = Root;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        let mut emitter = Emitter::new();

        macro_rules! nested {
            ($item:expr) => {
                match UnwrapPartial::unwrap_partial($item) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        emitter.emit_collection(error);
                        None
                    }
                }
            };
        }

        if self.chain_id.is_none() {
            emitter.emit_missing_field("chain_id");
        }
        if self.public_key.is_none() {
            emitter.emit_missing_field("public_key");
        }
        if self.private_key.is_none() {
            emitter.emit_missing_field("private_key");
        }

        let genesis = nested!(self.genesis);
        let kura = nested!(self.kura);
        let sumeragi = nested!(self.sumeragi);
        let network = nested!(self.network);
        let logger = nested!(self.logger);
        let queue = nested!(self.queue);
        let snapshot = nested!(self.snapshot);
        let telemetry = nested!(self.telemetry);
        let torii = nested!(self.torii);
        let chain_wide = nested!(self.chain_wide);

        emitter.finish()?;

        Ok(Root {
            chain_id: self.chain_id.get().unwrap(),
            public_key: self.public_key.get().unwrap(),
            private_key: self.private_key.get().unwrap(),
            genesis: genesis.unwrap(),
            kura: kura.unwrap(),
            sumeragi: sumeragi.unwrap(),
            telemetry: telemetry.unwrap(),
            logger: logger.unwrap(),
            queue: queue.unwrap(),
            snapshot: snapshot.unwrap(),
            torii: torii.unwrap(),
            network: network.unwrap(),
            chain_wide: chain_wide.unwrap(),
        })
    }
}

impl FromEnv for RootPartial {
    fn from_env<E: Error, R: ReadEnv<E>>(env: &R) -> FromEnvResult<Self> {
        let mut emitter = Emitter::new();

        let chain_id = env
            .read_env("CHAIN_ID")
            .map_err(|e| eyre!("{e}"))
            .wrap_err("failed to read CHAIN_ID field (iroha.chain_id param)")
            .map_or_else(
                |err| {
                    emitter.emit(err);
                    None
                },
                |maybe_value| maybe_value.map(ChainId::from),
            )
            .into();
        let public_key =
            ParseEnvResult::parse_simple(&mut emitter, env, "PUBLIC_KEY", "iroha.public_key")
                .into();
        let private_key =
            user::private_key_from_env(&mut emitter, env, "PRIVATE_KEY", "iroha.private_key")
                .into();

        let genesis = emitter.try_from_env(env);
        let kura = emitter.try_from_env(env);
        let sumeragi = emitter.try_from_env(env);
        let network = emitter.try_from_env(env);
        let logger = emitter.try_from_env(env);
        let queue = emitter.try_from_env(env);
        let snapshot = emitter.try_from_env(env);
        let telemetry = emitter.try_from_env(env);
        let torii = emitter.try_from_env(env);
        let chain_wide = emitter.try_from_env(env);

        emitter.finish()?;

        Ok(Self {
            extends: None,
            chain_id,
            public_key,
            private_key,
            genesis: genesis.unwrap(),
            kura: kura.unwrap(),
            sumeragi: sumeragi.unwrap(),
            network: network.unwrap(),
            logger: logger.unwrap(),
            queue: queue.unwrap(),
            snapshot: snapshot.unwrap(),
            telemetry: telemetry.unwrap(),
            torii: torii.unwrap(),
            chain_wide: chain_wide.unwrap(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct GenesisPartial {
    pub public_key: UserField<PublicKey>,
    pub private_key: UserField<PrivateKey>,
    pub file: UserField<PathBuf>,
}

impl UnwrapPartial for GenesisPartial {
    type Output = Genesis;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        let public_key = self
            .public_key
            .get()
            .ok_or_else(|| MissingFieldError::new("genesis.public_key"))?;

        let private_key = self.private_key.get();
        let file = self.file.get();

        Ok(Genesis {
            public_key,
            private_key,
            file,
        })
    }
}

impl FromEnv for GenesisPartial {
    fn from_env<E: Error, R: ReadEnv<E>>(env: &R) -> FromEnvResult<Self>
    where
        Self: Sized,
    {
        let mut emitter = Emitter::new();

        let public_key = ParseEnvResult::parse_simple(
            &mut emitter,
            env,
            "GENESIS_PUBLIC_KEY",
            "genesis.public_key",
        )
        .into();
        let private_key = user::private_key_from_env(
            &mut emitter,
            env,
            "GENESIS_PRIVATE_KEY",
            "genesis.private_key",
        )
        .into();
        let file =
            ParseEnvResult::parse_simple(&mut emitter, env, "GENESIS_FILE", "genesis.file").into();

        emitter.finish()?;

        Ok(Self {
            public_key,
            private_key,
            file,
        })
    }
}

/// `Kura` configuration.
#[derive(Clone, Deserialize, Serialize, Debug, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct KuraPartial {
    pub init_mode: UserField<KuraInitMode>,
    pub store_dir: UserField<PathBuf>,
    pub debug: KuraDebugPartial,
}

impl UnwrapPartial for KuraPartial {
    type Output = Kura;

    fn unwrap_partial(self) -> Result<Self::Output, ErrorsCollection<MissingFieldError>> {
        let mut emitter = Emitter::new();

        let init_mode = self.init_mode.unwrap_or_default();

        let store_dir = self
            .store_dir
            .get()
            .unwrap_or_else(|| PathBuf::from(defaults::kura::DEFAULT_STORE_DIR));

        let debug = UnwrapPartial::unwrap_partial(self.debug).map_or_else(
            |err| {
                emitter.emit_collection(err);
                None
            },
            Some,
        );

        emitter.finish()?;

        Ok(Kura {
            init_mode,
            store_dir,
            debug: debug.unwrap(),
        })
    }
}

#[derive(Clone, Deserialize, Serialize, Debug, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct KuraDebugPartial {
    output_new_blocks: UserField<bool>,
}

impl UnwrapPartial for KuraDebugPartial {
    type Output = KuraDebug;

    fn unwrap_partial(self) -> Result<Self::Output, ErrorsCollection<MissingFieldError>> {
        Ok(KuraDebug {
            output_new_blocks: self.output_new_blocks.unwrap_or(false),
        })
    }
}

impl FromEnv for KuraPartial {
    fn from_env<E: Error, R: ReadEnv<E>>(env: &R) -> FromEnvResult<Self>
    where
        Self: Sized,
    {
        let mut emitter = Emitter::new();

        let init_mode =
            ParseEnvResult::parse_simple(&mut emitter, env, "KURA_INIT_MODE", "kura.init_mode")
                .into();
        let store_dir =
            ParseEnvResult::parse_simple(&mut emitter, env, "KURA_STORE_DIR", "kura.store_dir")
                .into();
        let debug_output_new_blocks = ParseEnvResult::parse_simple(
            &mut emitter,
            env,
            "KURA_DEBUG_OUTPUT_NEW_BLOCKS",
            "kura.debug.output_new_blocks",
        )
        .into();

        emitter.finish()?;

        Ok(Self {
            init_mode,
            store_dir,
            debug: KuraDebugPartial {
                output_new_blocks: debug_output_new_blocks,
            },
        })
    }
}

#[derive(Deserialize, Serialize, Debug, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct SumeragiPartial {
    pub trusted_peers: UserField<Vec<PeerId>>,
    pub debug: SumeragiDebugPartial,
}

impl UnwrapPartial for SumeragiPartial {
    type Output = Sumeragi;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        let mut emitter = Emitter::new();

        let debug = self.debug.unwrap_partial().map_or_else(
            |err| {
                emitter.emit_collection(err);
                None
            },
            Some,
        );

        emitter.finish()?;

        Ok(Sumeragi {
            trusted_peers: self.trusted_peers.get(),
            debug: debug.unwrap(),
        })
    }
}

#[derive(Deserialize, Serialize, Debug, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct SumeragiDebugPartial {
    pub force_soft_fork: UserField<bool>,
}

impl UnwrapPartial for SumeragiDebugPartial {
    type Output = SumeragiDebug;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        Ok(SumeragiDebug {
            force_soft_fork: self.force_soft_fork.unwrap_or(false),
        })
    }
}

impl FromEnv for SumeragiPartial {
    fn from_env<E: Error, R: ReadEnv<E>>(env: &R) -> FromEnvResult<Self>
    where
        Self: Sized,
    {
        let mut emitter = Emitter::new();

        let trusted_peers = ParseEnvResult::parse_json(
            &mut emitter,
            env,
            "SUMERAGI_TRUSTED_PEERS",
            "sumeragi.trusted_peers",
        )
        .into();
        let debug = emitter.try_from_env(env);

        emitter.finish()?;

        Ok(Self {
            trusted_peers,
            debug: debug.unwrap(),
        })
    }
}

impl FromEnvDefaultFallback for SumeragiDebugPartial {}

#[derive(Deserialize, Serialize, Debug, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct NetworkPartial {
    pub address: UserField<SocketAddr>,
    pub block_gossip_max_size: UserField<NonZeroU32>,
    pub block_gossip_period: UserField<HumanDuration>,
    pub transaction_gossip_max_size: UserField<NonZeroU32>,
    pub transaction_gossip_period: UserField<HumanDuration>,
}

impl UnwrapPartial for NetworkPartial {
    type Output = Network;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        if self.address.is_none() {
            return Err(MissingFieldError::new("network.address").into());
        }

        Ok(Network {
            address: self.address.get().unwrap(),
            block_gossip_period: self
                .block_gossip_period
                .map(HumanDuration::get)
                .unwrap_or(DEFAULT_BLOCK_GOSSIP_PERIOD),
            transaction_gossip_period: self
                .transaction_gossip_period
                .map(HumanDuration::get)
                .unwrap_or(DEFAULT_TRANSACTION_GOSSIP_PERIOD),
            transaction_gossip_max_size: self
                .transaction_gossip_max_size
                .get()
                .unwrap_or(DEFAULT_MAX_TRANSACTIONS_PER_GOSSIP),
            block_gossip_max_size: self
                .block_gossip_max_size
                .get()
                .unwrap_or(DEFAULT_MAX_BLOCKS_PER_GOSSIP),
        })
    }
}

impl FromEnv for NetworkPartial {
    fn from_env<E: Error, R: ReadEnv<E>>(env: &R) -> FromEnvResult<Self>
    where
        Self: Sized,
    {
        let mut emitter = Emitter::new();

        // TODO: also parse `NETWORK_ADDRESS`?
        let address =
            ParseEnvResult::parse_simple(&mut emitter, env, "P2P_ADDRESS", "network.address")
                .into();

        emitter.finish()?;

        Ok(Self {
            address,
            ..Self::default()
        })
    }
}

#[derive(Deserialize, Serialize, Debug, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct QueuePartial {
    /// The upper limit of the number of transactions waiting in the queue.
    pub capacity: UserField<NonZeroUsize>,
    /// The upper limit of the number of transactions waiting in the queue for single user.
    /// Use this option to apply throttling.
    pub capacity_per_user: UserField<NonZeroUsize>,
    /// The transaction will be dropped after this time if it is still in the queue.
    pub transaction_time_to_live: UserField<HumanDuration>,
    /// The threshold to determine if a transaction has been tampered to have a future timestamp.
    pub future_threshold: UserField<HumanDuration>,
}

impl UnwrapPartial for QueuePartial {
    type Output = Queue;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        Ok(Queue {
            capacity: self.capacity.unwrap_or(DEFAULT_MAX_TRANSACTIONS_IN_QUEUE),
            capacity_per_user: self
                .capacity_per_user
                .unwrap_or(DEFAULT_MAX_TRANSACTIONS_IN_QUEUE),
            transaction_time_to_live: self
                .transaction_time_to_live
                .map_or(DEFAULT_TRANSACTION_TIME_TO_LIVE, HumanDuration::get),
            future_threshold: self
                .future_threshold
                .map_or(DEFAULT_FUTURE_THRESHOLD, HumanDuration::get),
        })
    }
}

impl FromEnvDefaultFallback for QueuePartial {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct LoggerPartial {
    pub level: UserField<Level>,
    pub format: UserField<Format>,
    pub tokio_console_address: UserField<SocketAddr>,
}

impl UnwrapPartial for LoggerPartial {
    type Output = Logger;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        Ok(Logger {
            level: self.level.unwrap_or_default(),
            format: self.format.unwrap_or_default(),
            tokio_console_address: self.tokio_console_address.get().unwrap_or_else(|| {
                defaults::logger::DEFAULT_TOKIO_CONSOLE_ADDR.clone()
            }),
        })
    }
}

impl FromEnv for LoggerPartial {
    fn from_env<E: Error, R: ReadEnv<E>>(env: &R) -> FromEnvResult<Self>
    where
        Self: Sized,
    {
        let mut emitter = Emitter::new();

        let level =
            ParseEnvResult::parse_simple(&mut emitter, env, "LOG_LEVEL", "logger.level").into();
        let format =
            ParseEnvResult::parse_simple(&mut emitter, env, "LOG_FORMAT", "logger.format").into();

        emitter.finish()?;

        #[allow(clippy::needless_update)] // triggers if tokio console addr is feature-gated
        Ok(Self {
            level,
            format,
            ..Self::default()
        })
    }
}

#[derive(Clone, Deserialize, Serialize, Debug, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct TelemetryPartial {
    pub name: UserField<String>,
    pub url: UserField<Url>,
    pub min_retry_period: UserField<HumanDuration>,
    pub max_retry_delay_exponent: UserField<u8>,
    pub dev: TelemetryDevPartial,
}

#[derive(Clone, Deserialize, Serialize, Debug, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct TelemetryDevPartial {
    pub out_file: UserField<PathBuf>,
}

impl UnwrapPartial for TelemetryDevPartial {
    type Output = TelemetryDev;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        Ok(TelemetryDev {
            out_file: self.out_file.get(),
        })
    }
}

impl UnwrapPartial for TelemetryPartial {
    type Output = Telemetry;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        let Self {
            name,
            url,
            max_retry_delay_exponent,
            min_retry_period,
            dev,
        } = self;

        Ok(Telemetry {
            name: name.get(),
            url: url.get(),
            max_retry_delay_exponent: max_retry_delay_exponent.get(),
            min_retry_period: min_retry_period.get().map(HumanDuration::get),
            dev: dev.unwrap_partial()?,
        })
    }
}

impl FromEnvDefaultFallback for TelemetryPartial {}

#[derive(Debug, Clone, Deserialize, Serialize, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct SnapshotPartial {
    pub mode: UserField<SnapshotMode>,
    pub create_every: UserField<HumanDuration>,
    pub store_dir: UserField<PathBuf>,
}

impl UnwrapPartial for SnapshotPartial {
    type Output = Snapshot;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        Ok(Snapshot {
            mode: self.mode.unwrap_or_default(),
            create_every: self
                .create_every
                .get()
                .map_or(defaults::snapshot::DEFAULT_CREATE_EVERY, HumanDuration::get),
            store_dir: self
                .store_dir
                .get()
                .unwrap_or_else(|| PathBuf::from(defaults::snapshot::DEFAULT_STORE_DIR)),
        })
    }
}

impl FromEnv for SnapshotPartial {
    fn from_env<E: Error, R: ReadEnv<E>>(env: &R) -> FromEnvResult<Self>
    where
        Self: Sized,
    {
        let mut emitter = Emitter::new();

        let mode =
            ParseEnvResult::parse_simple(&mut emitter, env, "SNAPSHOT_MODE", "snapshot.mode")
                .into();
        let store_dir = ParseEnvResult::parse_simple(
            &mut emitter,
            env,
            "SNAPSHOT_STORE_DIR",
            "snapshot.store_dir",
        )
        .into();

        emitter.finish()?;

        Ok(Self {
            mode,
            store_dir,
            ..Self::default()
        })
    }
}

#[derive(Deserialize, Serialize, Debug, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct ChainWidePartial {
    pub max_transactions_in_block: UserField<NonZeroU32>,
    pub block_time: UserField<HumanDuration>,
    pub commit_time: UserField<HumanDuration>,
    pub transaction_limits: UserField<TransactionLimits>,
    pub asset_metadata_limits: UserField<MetadataLimits>,
    pub asset_definition_metadata_limits: UserField<MetadataLimits>,
    pub account_metadata_limits: UserField<MetadataLimits>,
    pub domain_metadata_limits: UserField<MetadataLimits>,
    pub ident_length_limits: UserField<LengthLimits>,
    pub executor_fuel_limit: UserField<u64>,
    pub executor_max_memory: UserField<HumanBytes<u32>>,
    pub wasm_fuel_limit: UserField<u64>,
    pub wasm_max_memory: UserField<HumanBytes<u32>>,
}

impl UnwrapPartial for ChainWidePartial {
    type Output = ChainWide;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        Ok(ChainWide {
            max_transactions_in_block: self.max_transactions_in_block.unwrap_or(DEFAULT_MAX_TXS),
            block_time: self
                .block_time
                .map_or(DEFAULT_BLOCK_TIME, HumanDuration::get),
            commit_time: self
                .commit_time
                .map_or(DEFAULT_COMMIT_TIME, HumanDuration::get),
            transaction_limits: self
                .transaction_limits
                .unwrap_or(DEFAULT_TRANSACTION_LIMITS),
            asset_metadata_limits: self
                .asset_metadata_limits
                .unwrap_or(DEFAULT_METADATA_LIMITS),
            asset_definition_metadata_limits: self
                .asset_definition_metadata_limits
                .unwrap_or(DEFAULT_METADATA_LIMITS),
            account_metadata_limits: self
                .account_metadata_limits
                .unwrap_or(DEFAULT_METADATA_LIMITS),
            domain_metadata_limits: self
                .domain_metadata_limits
                .unwrap_or(DEFAULT_METADATA_LIMITS),
            ident_length_limits: self
                .ident_length_limits
                .unwrap_or(DEFAULT_IDENT_LENGTH_LIMITS),
            executor_fuel_limit: self.executor_fuel_limit.unwrap_or(DEFAULT_WASM_FUEL_LIMIT),
            executor_max_memory: self
                .executor_max_memory
                .unwrap_or(HumanBytes(DEFAULT_WASM_MAX_MEMORY_BYTES)),
            wasm_fuel_limit: self.wasm_fuel_limit.unwrap_or(DEFAULT_WASM_FUEL_LIMIT),
            wasm_max_memory: self
                .wasm_max_memory
                .unwrap_or(HumanBytes(DEFAULT_WASM_MAX_MEMORY_BYTES)),
        })
    }
}

impl FromEnvDefaultFallback for ChainWidePartial {}

#[derive(Debug, Clone, Deserialize, Serialize, Default, Merge)]
#[serde(deny_unknown_fields, default)]
pub struct ToriiPartial {
    pub address: UserField<SocketAddr>,
    pub max_content_len: UserField<HumanBytes<u64>>,
    pub query_idle_time: UserField<HumanDuration>,
}

impl UnwrapPartial for ToriiPartial {
    type Output = Torii;

    fn unwrap_partial(self) -> UnwrapPartialResult<Self::Output> {
        let mut emitter = Emitter::new();

        if self.address.is_none() {
            emitter.emit_missing_field("torii.address");
        }

        let max_content_len = self
            .max_content_len
            .get()
            .unwrap_or(HumanBytes(DEFAULT_MAX_CONTENT_LENGTH));

        let query_idle_time = self
            .query_idle_time
            .map(HumanDuration::get)
            .unwrap_or(DEFAULT_QUERY_IDLE_TIME);

        emitter.finish()?;

        Ok(Torii {
            address: self.address.get().unwrap(),
            max_content_len,
            query_idle_time,
        })
    }
}

impl FromEnv for ToriiPartial {
    fn from_env<E: Error, R: ReadEnv<E>>(env: &R) -> FromEnvResult<Self>
    where
        Self: Sized,
    {
        let mut emitter = Emitter::new();

        let address =
            ParseEnvResult::parse_simple(&mut emitter, env, "API_ADDRESS", "torii.address").into();

        emitter.finish()?;

        Ok(Self {
            address,
            ..Self::default()
        })
    }
}
