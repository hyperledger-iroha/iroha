//! Actor responsible for configuration state and its dynamic updates.
//!
//! Currently the API exposed by [`KisoHandle`] works only with [`ConfigGetDTO`], because
//! no any part of Iroha is interested in the whole state. However, the API could be extended
//! in future.
//!
//! Updates mechanism is implemented via subscriptions to [`tokio::sync::watch`] channels. For now,
//! only `logger.level` field is dynamic, which might be tracked with [`KisoHandle::subscribe_on_logger_updates()`].

use std::{num::NonZeroU32, time::Duration};

use eyre::Result;
use hex;
use iroha_config::{
    base::WithOrigin,
    client_api::{
        ConfigGetDTO, ConfigUpdateDTO, Logger, NetworkAcl, ResumeHashDirective,
        SoranetHandshakePowUpdate, SoranetHandshakeUpdate, TransportUpdate,
    },
    parameters::actual::{
        ConfidentialGas as ActualConfidentialGas, Logger as LoggerConfig, NoritoRpcStage,
        Root as Config, SoranetHandshake as ActualSoranetHandshake, SoranetPow, SoranetPuzzle,
    },
};
use iroha_futures::supervisor::{Child, OnShutdown};
use tokio::sync::{mpsc, oneshot, watch};

const DEFAULT_CHANNEL_SIZE: usize = 32;

/// Handle to work with the actor.
///
/// The actor will shutdown when all its handles are dropped.
#[derive(Clone)]
pub struct KisoHandle {
    actor: mpsc::Sender<Message>,
}

impl KisoHandle {
    /// Spawn a new actor
    pub fn start(state: Config) -> (Self, Child) {
        let (actor_sender, actor_receiver) = mpsc::channel(DEFAULT_CHANNEL_SIZE);
        let initial_logger = state.logger.clone();
        let initial_acl = Actor::snapshot_network_acl(&state);
        let (logger_update, _) = watch::channel(initial_logger);
        let (network_acl_update, _) = watch::channel(initial_acl);
        let initial_handshake = state.network.soranet_handshake.clone();
        let (soranet_handshake_update, _) = watch::channel(initial_handshake);
        let initial_confidential_gas = state.confidential.gas;
        let (confidential_gas_update, _) = watch::channel(initial_confidential_gas);
        crate::gas::configure_confidential_gas(initial_confidential_gas.into());
        let mut actor = Actor {
            handle: actor_receiver,
            state,
            logger_update,
            network_acl_update,
            soranet_handshake_update,
            confidential_gas_update,
        };
        (
            Self {
                actor: actor_sender,
            },
            Child::new(
                tokio::spawn(async move { actor.run().await }),
                OnShutdown::Abort,
            ),
        )
    }

    /// Fetch the [`ConfigGetDTO`] from the actor's state.
    ///
    /// # Errors
    /// If communication with actor fails.
    pub async fn get_dto(&self) -> Result<ConfigGetDTO, Error> {
        let (tx, rx) = oneshot::channel();
        let msg = Message::GetDTO { respond_to: tx };
        let _ = self.actor.send(msg).await;
        let dto = rx.await?;
        Ok(dto)
    }

    /// Update the configuration state and notify subscribers.
    ///
    /// Works in a fire-and-forget way, i.e. completion of this task doesn't mean that updates are applied. However,
    /// subsequent call of [`Self::get_dto()`] will return an updated state.
    ///
    /// # Errors
    /// If communication with actor fails.
    pub async fn update_with_dto(&self, dto: ConfigUpdateDTO) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        let msg = Message::UpdateWithDTO {
            dto: Box::new(dto),
            respond_to: tx,
        };
        let _ = self.actor.send(msg).await;
        rx.await?
    }

    /// Subscribe on updates of `logger.level` parameter.
    ///
    /// # Errors
    /// If communication with actor fails.
    pub async fn subscribe_on_logger_updates(
        &self,
    ) -> Result<watch::Receiver<LoggerConfig>, Error> {
        let (tx, rx) = oneshot::channel();
        let msg = Message::SubscribeOnLogLevel { respond_to: tx };
        let _ = self.actor.send(msg).await;
        let receiver = rx.await?;
        Ok(receiver)
    }

    /// Subscribe on updates of network ACL settings.
    ///
    /// # Errors
    /// Returns an error if communication with the actor fails.
    pub async fn subscribe_on_network_acl_updates(
        &self,
    ) -> Result<watch::Receiver<NetworkAcl>, Error> {
        let (tx, rx) = oneshot::channel();
        let msg = Message::SubscribeOnNetworkAcl { respond_to: tx };
        let _ = self.actor.send(msg).await;
        let receiver = rx.await?;
        Ok(receiver)
    }

    /// Subscribe on updates of the `SoraNet` handshake configuration.
    ///
    /// # Errors
    /// Returns an error if communication with the actor fails.
    pub async fn subscribe_on_soranet_handshake_updates(
        &self,
    ) -> Result<watch::Receiver<ActualSoranetHandshake>, Error> {
        let (tx, rx) = oneshot::channel();
        let msg = Message::SubscribeOnSoranetHandshake { respond_to: tx };
        let _ = self.actor.send(msg).await;
        let receiver = rx.await?;
        Ok(receiver)
    }

    /// Subscribe on updates of the confidential gas schedule.
    ///
    /// # Errors
    /// Returns an error if communication with the actor fails.
    pub async fn subscribe_on_confidential_gas_updates(
        &self,
    ) -> Result<watch::Receiver<ActualConfidentialGas>, Error> {
        let (tx, rx) = oneshot::channel();
        let msg = Message::SubscribeOnConfidentialGas { respond_to: tx };
        let _ = self.actor.send(msg).await;
        let receiver = rx.await?;
        Ok(receiver)
    }

    /// Lightweight mock handle used in tests to avoid spinning up the full actor and watchers.
    ///
    /// The mock serves `get_dto` requests from the provided configuration snapshot and acknowledges
    /// updates/subscriptions with pre-seeded watch channels.
    pub fn mock(state: &Config) -> Self {
        let dto = ConfigGetDTO::from(state);
        let (actor_sender, mut actor_receiver) = mpsc::channel(DEFAULT_CHANNEL_SIZE);
        let logger = state.logger.clone();
        let network_acl = Actor::snapshot_network_acl(state);
        let soranet_handshake = state.network.soranet_handshake.clone();
        let confidential_gas = state.confidential.gas;
        tokio::spawn(async move {
            let (logger_tx, _) = watch::channel(logger);
            let (network_acl_tx, _) = watch::channel(network_acl);
            let (handshake_tx, _) = watch::channel(soranet_handshake);
            let (confidential_gas_tx, _) = watch::channel(confidential_gas);
            let mut dto_snapshot = dto;
            while let Some(msg) = actor_receiver.recv().await {
                match msg {
                    Message::GetDTO { respond_to } => {
                        let _ = respond_to.send(dto_snapshot.clone());
                    }
                    Message::UpdateWithDTO { dto, respond_to } => {
                        // Update the exposed Norito-RPC summary if provided; ignore the rest to
                        // keep the mock lightweight.
                        if let Some(transport) = dto.transport.as_ref() {
                            if let Some(norito_rpc) = transport.norito_rpc.as_ref() {
                                if let Some(enabled) = norito_rpc.enabled {
                                    dto_snapshot.transport.norito_rpc.enabled = enabled;
                                }
                                if let Some(stage) = norito_rpc.stage.as_ref() {
                                    dto_snapshot.transport.norito_rpc.stage.clone_from(stage);
                                }
                                if let Some(require_mtls) = norito_rpc.require_mtls {
                                    dto_snapshot.transport.norito_rpc.require_mtls = require_mtls;
                                }
                                if let Some(allowlist) = norito_rpc.allowed_clients.as_ref() {
                                    dto_snapshot.transport.norito_rpc.canary_allowlist_size =
                                        allowlist.len();
                                }
                            }
                        }
                        let _ = respond_to.send(Ok(()));
                    }
                    Message::SubscribeOnLogLevel { respond_to } => {
                        let _ = respond_to.send(logger_tx.subscribe());
                    }
                    Message::SubscribeOnNetworkAcl { respond_to } => {
                        let _ = respond_to.send(network_acl_tx.subscribe());
                    }
                    Message::SubscribeOnSoranetHandshake { respond_to } => {
                        let _ = respond_to.send(handshake_tx.subscribe());
                    }
                    Message::SubscribeOnConfidentialGas { respond_to } => {
                        let _ = respond_to.send(confidential_gas_tx.subscribe());
                    }
                }
            }
        });
        Self {
            actor: actor_sender,
        }
    }
}

enum Message {
    GetDTO {
        respond_to: oneshot::Sender<ConfigGetDTO>,
    },
    UpdateWithDTO {
        dto: Box<ConfigUpdateDTO>,
        respond_to: oneshot::Sender<Result<(), Error>>,
    },
    SubscribeOnLogLevel {
        respond_to: oneshot::Sender<watch::Receiver<LoggerConfig>>,
    },
    SubscribeOnNetworkAcl {
        respond_to: oneshot::Sender<watch::Receiver<NetworkAcl>>,
    },
    SubscribeOnSoranetHandshake {
        respond_to: oneshot::Sender<watch::Receiver<ActualSoranetHandshake>>,
    },
    SubscribeOnConfidentialGas {
        respond_to: oneshot::Sender<watch::Receiver<ActualConfidentialGas>>,
    },
}

/// Possible errors might occur while working with [`KisoHandle`]
#[derive(thiserror::Error, displaydoc::Display, Debug)]
pub enum Error {
    /// Failed to get actor's response
    Communication(#[from] oneshot::error::RecvError),
    /// Configuration validation failed: {0}
    Validation(String),
}

struct Actor {
    handle: mpsc::Receiver<Message>,
    state: Config,
    // Current implementation is somewhat not scalable in terms of code writing: for any
    // future dynamic parameter, it will require its own `subscribe_on_<field>` function in [`KisoHandle`],
    // new channel here, and new [`Message`] variant. If boilerplate expands, a more general solution will be
    // required. However, as of now a single manually written implementation seems optimal.
    logger_update: watch::Sender<LoggerConfig>,
    network_acl_update: watch::Sender<NetworkAcl>,
    soranet_handshake_update: watch::Sender<ActualSoranetHandshake>,
    confidential_gas_update: watch::Sender<ActualConfidentialGas>,
}

impl Actor {
    async fn run(&mut self) {
        while let Some(msg) = self.handle.recv().await {
            self.handle_message(msg)
        }
    }

    fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::GetDTO { respond_to } => {
                let dto = ConfigGetDTO::from(&self.state);
                let _ = respond_to.send(dto);
            }
            Message::UpdateWithDTO { dto, respond_to } => {
                let result = self.apply_config_update(*dto);
                let _ = respond_to.send(result);
            }
            Message::SubscribeOnLogLevel { respond_to } => {
                let _ = respond_to.send(self.logger_update.subscribe());
            }
            Message::SubscribeOnNetworkAcl { respond_to } => {
                let _ = respond_to.send(self.network_acl_update.subscribe());
            }
            Message::SubscribeOnSoranetHandshake { respond_to } => {
                let _ = respond_to.send(self.soranet_handshake_update.subscribe());
            }
            Message::SubscribeOnConfidentialGas { respond_to } => {
                let _ = respond_to.send(self.confidential_gas_update.subscribe());
            }
        }
    }

    fn apply_config_update(&mut self, dto: ConfigUpdateDTO) -> Result<(), Error> {
        let ConfigUpdateDTO {
            logger,
            network_acl,
            network,
            confidential_gas,
            soranet_handshake,
            transport,
            compute_pricing,
        } = dto;

        let Logger { level, filter } = logger;
        self.state.logger.level = level;
        self.state.logger.filter = filter;
        let _ = self.logger_update.send_replace(self.state.logger.clone());

        if let Some(acl) = network_acl {
            let iroha_config::client_api::NetworkAcl {
                allowlist_only,
                allow_keys,
                deny_keys,
                allow_cidrs,
                deny_cidrs,
            } = acl;

            if let Some(b) = allowlist_only {
                self.state.network.allowlist_only = b;
            }
            if let Some(keys) = allow_keys {
                self.state.network.allow_keys = keys;
            }
            if let Some(keys) = deny_keys {
                self.state.network.deny_keys = keys;
            }
            if let Some(cidrs) = allow_cidrs {
                self.state.network.allow_cidrs = cidrs;
            }
            if let Some(cidrs) = deny_cidrs {
                self.state.network.deny_cidrs = cidrs;
            }

            let snapshot = Self::snapshot_network_acl(&self.state);
            let _ = self.network_acl_update.send_replace(snapshot);
        }

        if let Some(network) = network {
            if let Some(value) = network.require_sm_handshake_match {
                self.state.network.require_sm_handshake_match = value;
            }
            if let Some(value) = network.require_sm_openssl_preview_match {
                self.state.network.require_sm_openssl_preview_match = value;
            }
            if let Some(profile) = network.lane_profile {
                self.state.network.lane_profile = profile;
                let limits = profile.derived_limits();
                self.state.network.max_incoming = limits.max_incoming;
                self.state.network.max_total_connections = limits.max_total_connections;
                self.state.network.low_priority_bytes_per_sec = limits.low_priority_bytes_per_sec;
                self.state.network.low_priority_rate_per_sec = limits.low_priority_rate_per_sec;
            }
        }

        if let Some(gas) = confidential_gas {
            self.state.confidential.gas.proof_base = gas.proof_base;
            self.state.confidential.gas.per_public_input = gas.per_public_input;
            self.state.confidential.gas.per_proof_byte = gas.per_proof_byte;
            self.state.confidential.gas.per_nullifier = gas.per_nullifier;
            self.state.confidential.gas.per_commitment = gas.per_commitment;
            crate::gas::configure_confidential_gas(self.state.confidential.gas.into());
            let _ = self
                .confidential_gas_update
                .send_replace(self.state.confidential.gas);
        }

        if let Some(handshake_update) = soranet_handshake {
            Self::apply_soranet_handshake_update(
                &mut self.state.network.soranet_handshake,
                handshake_update,
            )
            .map_err(Error::Validation)?;
            let _ = self
                .soranet_handshake_update
                .send_replace(self.state.network.soranet_handshake.clone());
        }

        if let Some(transport_update) = transport {
            Self::apply_transport_update(&mut self.state.torii.transport, transport_update)
                .map_err(Error::Validation)?;
        }

        if let Some(pricing_update) = compute_pricing {
            let mut compute = self.state.compute.clone();
            for (family, weights) in pricing_update.price_families {
                compute
                    .apply_price_update(&family, weights)
                    .map_err(|err| Error::Validation(err.to_string()))?;
            }
            if let Some(default_family) = pricing_update.default_price_family {
                if !compute.price_families.contains_key(&default_family) {
                    return Err(Error::Validation(format!(
                        "compute default_price_family `{default_family}` missing from price_families"
                    )));
                }
                compute.default_price_family = default_family;
            }
            self.state.compute = compute;
        }

        Ok(())
    }

    fn apply_soranet_handshake_update(
        handshake: &mut ActualSoranetHandshake,
        update: SoranetHandshakeUpdate,
    ) -> Result<(), String> {
        let decode_hex = |value: &str, field: &str| {
            hex::decode(value).map_err(|_| format!("invalid hex in {field}"))
        };

        if let Some(value) = update.descriptor_commit_hex {
            let bytes = decode_hex(&value, "descriptor_commit_hex")?;
            handshake.descriptor_commit = WithOrigin::inline(bytes);
        }
        if let Some(value) = update.client_capabilities_hex {
            let bytes = decode_hex(&value, "client_capabilities_hex")?;
            handshake.client_capabilities = WithOrigin::inline(bytes);
        }
        if let Some(value) = update.relay_capabilities_hex {
            let bytes = decode_hex(&value, "relay_capabilities_hex")?;
            handshake.relay_capabilities = WithOrigin::inline(bytes);
        }
        if let Some(value) = update.kem_id {
            handshake.kem_id = value;
        }
        if let Some(value) = update.sig_id {
            handshake.sig_id = value;
        }
        if let Some(resume) = update.resume_hash_hex {
            handshake.resume_hash = match resume {
                ResumeHashDirective::Set(hex_value) => {
                    let bytes = decode_hex(&hex_value, "resume_hash_hex")?;
                    Some(WithOrigin::inline(bytes))
                }
                ResumeHashDirective::Clear => None,
            };
        }
        if let Some(pow_update) = update.pow {
            Self::apply_pow_update(&mut handshake.pow, &pow_update);
        }

        Ok(())
    }

    fn apply_transport_update(
        transport: &mut iroha_config::parameters::actual::ToriiTransport,
        update: TransportUpdate,
    ) -> Result<(), String> {
        if let Some(norito_rpc_update) = update.norito_rpc {
            let cfg = &mut transport.norito_rpc;
            if let Some(value) = norito_rpc_update.enabled {
                cfg.enabled = value;
            }
            if let Some(value) = norito_rpc_update.require_mtls {
                cfg.require_mtls = value;
            }
            if let Some(clients) = norito_rpc_update.allowed_clients {
                cfg.allowed_clients = clients;
            }
            if let Some(stage_label) = norito_rpc_update.stage {
                let stage = NoritoRpcStage::parse(&stage_label).ok_or_else(|| {
                    format!(
                        "invalid transport.norito_rpc.stage `{stage_label}` (expected disabled|canary|ga)"
                    )
                })?;
                cfg.stage = stage;
            }
        }
        Ok(())
    }

    fn apply_pow_update(pow: &mut SoranetPow, update: &SoranetHandshakePowUpdate) {
        if let Some(required) = update.required {
            pow.required = required;
        }
        if let Some(difficulty) = update.difficulty {
            pow.difficulty = difficulty;
        }
        if let Some(secs) = update.max_future_skew_secs {
            pow.max_future_skew = Duration::from_secs(secs);
        }
        if let Some(secs) = update.min_ticket_ttl_secs {
            pow.min_ticket_ttl = Duration::from_secs(secs);
        }
        if let Some(secs) = update.ticket_ttl_secs {
            pow.ticket_ttl = Duration::from_secs(secs);
        }
        if let Some(puzzle_update) = &update.puzzle {
            if let Some(enabled) = puzzle_update.enabled {
                if !enabled {
                    pow.puzzle = None;
                } else if pow.puzzle.is_none() {
                    pow.puzzle = Some(default_puzzle_params());
                }
            }
            if let Some(puzzle) = &mut pow.puzzle {
                if let Some(memory) = puzzle_update.memory_kib {
                    puzzle.memory_kib = NonZeroU32::new(memory.max(1)).unwrap_or(puzzle.memory_kib);
                }
                if let Some(time_cost) = puzzle_update.time_cost {
                    puzzle.time_cost =
                        NonZeroU32::new(time_cost.max(1)).unwrap_or(puzzle.time_cost);
                }
                if let Some(lanes) = puzzle_update.lanes {
                    let clamped = lanes.clamp(1, 16);
                    puzzle.lanes = NonZeroU32::new(clamped).unwrap_or(puzzle.lanes);
                }
            } else if puzzle_update.enabled.unwrap_or(false) {
                pow.puzzle = Some(default_puzzle_params());
            }
        }
    }

    fn snapshot_network_acl(state: &Config) -> NetworkAcl {
        NetworkAcl {
            allowlist_only: Some(state.network.allowlist_only),
            allow_keys: Some(state.network.allow_keys.clone()),
            deny_keys: Some(state.network.deny_keys.clone()),
            allow_cidrs: Some(state.network.allow_cidrs.clone()),
            deny_cidrs: Some(state.network.deny_cidrs.clone()),
        }
    }
}

fn default_puzzle_params() -> SoranetPuzzle {
    SoranetPuzzle {
        memory_kib: NonZeroU32::new(64 * 1024).expect("non-zero memory"),
        time_cost: NonZeroU32::new(2).expect("non-zero time"),
        lanes: NonZeroU32::new(1).expect("non-zero lanes"),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        num::{NonZeroU32, NonZeroU64, NonZeroUsize},
        path::PathBuf,
        time::Duration,
    };

    use iroha_config::{
        base::WithOrigin,
        client_api::{
            ComputePricingUpdate, ConfidentialGas as ConfidentialGasDTO, Logger as LoggerDTO,
            NetworkUpdate, SoranetHandshakePuzzleUpdate,
        },
        parameters::{
            actual::{
                Acceleration, BlockSync, Common, Concurrency, Confidential, Connect,
                DataspaceGossip, FraudMonitoring, Genesis, Governance, Hijiri, IsoBridge, Ivm,
                Kura, LiveQueryStore, Logger, Network, Nexus, NodeRole, Queue, RbcSampling, Root,
                Settlement, SoranetHandshake as ActualSoranetHandshake, SoranetPow, SoranetPrivacy,
                Streaming, StreamingSoranet, Sumeragi, TieredState, Torii, TransactionGossiper,
                TrustedPeers,
            },
            defaults,
        },
    };
    use iroha_crypto::{
        KeyPair,
        soranet::handshake::{
            DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
        },
        streaming::StreamingKeyMaterial,
    };
    use iroha_data_model::{ChainId, peer::Peer, sorafs::pricing::PricingScheduleRecord};
    use iroha_logger::Level;
    use iroha_primitives::addr::socket_addr;

    use super::*;

    #[allow(clippy::too_many_lines)]
    fn test_config() -> Root {
        // Minimal, self-contained config for testing Kiso subscriptions.
        let key_pair = KeyPair::random();
        let peer = Peer::new(socket_addr!(127.0.0.1:0), key_pair.public_key().clone());
        let streaming_identity = key_pair.clone();

        Root {
            common: Common {
                chain: ChainId::from("test-chain"),
                key_pair,
                peer: peer.clone(),
                trusted_peers: WithOrigin::inline(TrustedPeers {
                    myself: peer,
                    others: <_>::default(),
                    pops: std::collections::BTreeMap::new(),
                }),
                default_account_domain_label: WithOrigin::inline(
                    iroha_data_model::account::address::DEFAULT_DOMAIN_NAME_FALLBACK.to_owned(),
                ),
                chain_discriminant: WithOrigin::inline(defaults::common::chain_discriminant()),
            },
            network: Network {
                address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
                public_address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
                relay_mode: iroha_config::parameters::actual::RelayMode::Disabled,
                relay_hub_address: None,
                relay_ttl: defaults::network::RELAY_TTL,
                soranet_handshake: ActualSoranetHandshake {
                    descriptor_commit: WithOrigin::inline(DEFAULT_DESCRIPTOR_COMMIT.to_vec()),
                    client_capabilities: WithOrigin::inline(DEFAULT_CLIENT_CAPABILITIES.to_vec()),
                    relay_capabilities: WithOrigin::inline(DEFAULT_RELAY_CAPABILITIES.to_vec()),
                    trust_gossip: defaults::network::TRUST_GOSSIP,
                    kem_id: 1,
                    sig_id: 1,
                    resume_hash: None,
                    pow: SoranetPow::default(),
                },
                soranet_privacy: SoranetPrivacy::default(),
                soranet_vpn: iroha_config::parameters::actual::SoranetVpn::default(),
                lane_profile: iroha_config::parameters::actual::LaneProfile::Core,
                require_sm_handshake_match: true,
                require_sm_openssl_preview_match: true,
                idle_timeout: std::time::Duration::from_secs(5),
                peer_gossip_period: defaults::network::PEER_GOSSIP_PERIOD,
                trust_decay_half_life: defaults::network::TRUST_DECAY_HALF_LIFE,
                trust_penalty_bad_gossip: defaults::network::TRUST_PENALTY_BAD_GOSSIP,
                trust_penalty_unknown_peer: defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
                trust_min_score: defaults::network::TRUST_MIN_SCORE,
                trust_gossip: defaults::network::TRUST_GOSSIP,
                dns_refresh_interval: None,
                dns_refresh_ttl: None,
                quic_enabled: false,
                tls_enabled: false,
                tls_listen_address: None,
                prefer_ws_fallback: false,
                p2p_queue_cap_high: NonZeroUsize::new(128).unwrap(),
                p2p_queue_cap_low: NonZeroUsize::new(512).unwrap(),
                p2p_post_queue_cap: NonZeroUsize::new(128).unwrap(),
                happy_eyeballs_stagger: Duration::from_millis(100),
                addr_ipv6_first: false,
                max_incoming: None,
                max_total_connections: None,
                accept_rate_per_ip_per_sec: None,
                accept_burst_per_ip: None,
                max_accept_buckets: defaults::network::MAX_ACCEPT_BUCKETS,
                accept_bucket_idle: defaults::network::ACCEPT_BUCKET_IDLE,
                accept_prefix_v4_bits: defaults::network::ACCEPT_PREFIX_V4_BITS,
                accept_prefix_v6_bits: defaults::network::ACCEPT_PREFIX_V6_BITS,
                accept_rate_per_prefix_per_sec: None,
                accept_burst_per_prefix: None,
                low_priority_rate_per_sec: None,
                low_priority_burst: None,
                low_priority_bytes_per_sec: None,
                low_priority_bytes_burst: None,
                allowlist_only: false,
                allow_keys: Vec::new(),
                deny_keys: Vec::new(),
                allow_cidrs: Vec::new(),
                deny_cidrs: Vec::new(),
                disconnect_on_post_overflow: false,
                max_frame_bytes:
                    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES.get(),
                tcp_nodelay: true,
                tcp_keepalive: None,
                max_frame_bytes_consensus:
                    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS.get(),
                max_frame_bytes_control:
                    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONTROL.get(),
                max_frame_bytes_block_sync:
                    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC.get(),
                max_frame_bytes_tx_gossip:
                    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_TX_GOSSIP.get(),
                max_frame_bytes_peer_gossip:
                    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_PEER_GOSSIP.get(),
                max_frame_bytes_health:
                    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_HEALTH.get(),
                max_frame_bytes_other:
                    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_OTHER.get(),
                tls_only_v1_3: true,
                quic_max_idle_timeout: None,
            },
            genesis: Genesis {
                public_key: KeyPair::random().public_key().clone(),
                file: None,
                manifest_json: None,
                expected_hash: None,
                bootstrap_allowlist: Vec::new(),
                bootstrap_max_bytes: iroha_config::parameters::defaults::genesis::BOOTSTRAP_MAX_BYTES
                    .get(),
                bootstrap_response_throttle: iroha_config::parameters::defaults::genesis::BOOTSTRAP_RESPONSE_THROTTLE,
                bootstrap_request_timeout: iroha_config::parameters::defaults::genesis::BOOTSTRAP_REQUEST_TIMEOUT,
                bootstrap_retry_interval: iroha_config::parameters::defaults::genesis::BOOTSTRAP_RETRY_INTERVAL,
                bootstrap_max_attempts: iroha_config::parameters::defaults::genesis::BOOTSTRAP_MAX_ATTEMPTS,
                bootstrap_enabled: true,
            },
            torii: Torii {
                address: WithOrigin::inline(socket_addr!(127.0.0.1:0)),
                api_versions: iroha_config::parameters::defaults::torii::api_supported_versions(),
                api_version_default:
                    iroha_config::parameters::defaults::torii::api_default_version(),
                api_min_proof_version:
                    iroha_config::parameters::defaults::torii::api_min_proof_version(),
                api_version_sunset_unix: iroha_torii_shared::API_VERSION_SUNSET_UNIX,
                max_content_len: 1_048_576u64.into(),
                data_dir: iroha_config::parameters::defaults::torii::data_dir(),
                query_rate_per_authority_per_sec: None,
                query_burst_per_authority: None,
                deploy_rate_per_origin_per_sec: None,
                deploy_burst_per_origin: None,
                require_api_token: false,
                api_tokens: Vec::new(),
                api_fee_asset_id: None,
                api_fee_amount: None,
                api_fee_receiver: None,
                api_allow_cidrs: Vec::new(),
                peer_telemetry_urls: Vec::new(),
                peer_geo: iroha_config::parameters::actual::ToriiPeerGeo::default(),
                soranet_privacy_ingest: iroha_config::parameters::actual::SoranetPrivacyIngest::default(),
                strict_addresses: true,
                debug_match_filters: false,
                operator_auth: iroha_config::parameters::actual::ToriiOperatorAuth::default(),
                preauth_max_connections: None,
                preauth_max_connections_per_ip: None,
                preauth_rate_per_ip_per_sec: None,
                preauth_burst_per_ip: None,
                preauth_temp_ban: None,
                preauth_allow_cidrs: Vec::new(),
                preauth_scheme_limits: Vec::new(),
                api_high_load_tx_threshold: None,
                api_high_load_stream_threshold: None,
                api_high_load_subscription_threshold: None,
                events_buffer_capacity: NonZeroUsize::new(
                    iroha_config::parameters::defaults::torii::EVENTS_BUFFER_CAPACITY,
                )
                .expect("non-zero events buffer capacity"),
                ws_message_timeout: Duration::from_millis(
                    iroha_config::parameters::defaults::torii::WS_MESSAGE_TIMEOUT_MS,
                ),
                attachments_ttl_secs: 3600,
                attachments_max_bytes: 4 * 1024 * 1024,
                attachments_per_tenant_max_count:
                    iroha_config::parameters::defaults::torii::ATTACHMENTS_PER_TENANT_MAX_COUNT,
                attachments_per_tenant_max_bytes:
                    iroha_config::parameters::defaults::torii::ATTACHMENTS_PER_TENANT_MAX_BYTES,
                attachments_allowed_mime_types:
                    iroha_config::parameters::defaults::torii::attachments_allowed_mime_types(),
                attachments_max_expanded_bytes:
                    iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_EXPANDED_BYTES,
                attachments_max_archive_depth:
                    iroha_config::parameters::defaults::torii::ATTACHMENTS_MAX_ARCHIVE_DEPTH,
                attachments_sanitizer_mode:
                    iroha_config::parameters::actual::AttachmentSanitizerMode::Subprocess,
                attachments_sanitize_timeout_ms:
                    iroha_config::parameters::defaults::torii::ATTACHMENTS_SANITIZE_TIMEOUT_MS,
                zk_prover_enabled: false,
                zk_prover_scan_period_secs: 60,
                zk_prover_reports_ttl_secs: 3600,
                zk_prover_max_inflight:
                    iroha_config::parameters::defaults::torii::ZK_PROVER_MAX_INFLIGHT,
                zk_prover_max_scan_bytes:
                    iroha_config::parameters::defaults::torii::ZK_PROVER_MAX_SCAN_BYTES,
                zk_prover_max_scan_millis:
                    iroha_config::parameters::defaults::torii::ZK_PROVER_MAX_SCAN_MILLIS,
                zk_prover_keys_dir:
                    iroha_config::parameters::defaults::torii::zk_prover_keys_dir(),
                zk_prover_allowed_backends:
                    iroha_config::parameters::defaults::torii::zk_prover_allowed_backends(),
                zk_prover_allowed_circuits:
                    iroha_config::parameters::defaults::torii::zk_prover_allowed_circuits(),
                rbc_sampling: RbcSampling::default(),
                da_ingest: iroha_config::parameters::actual::DaIngest::default(),
                connect: Connect {
                    enabled: false,
                    ws_max_sessions: 64,
                    ws_per_ip_max_sessions: 8,
                    ws_rate_per_ip_per_min: 60,
                    session_ttl: std::time::Duration::from_secs(300),
                    frame_max_bytes: 256 * 1024,
                    session_buffer_max_bytes: 512 * 1024,
                    ping_interval: std::time::Duration::from_secs(30),
                    ping_miss_tolerance: 3,
                    ping_min_interval: std::time::Duration::from_secs(15),
                    dedupe_ttl: std::time::Duration::from_secs(60),
                    dedupe_cap: 1024,
                    relay_enabled: false,
                    relay_strategy: "broadcast",
                    p2p_ttl_hops: 0,
                },
                iso_bridge: IsoBridge {
                    enabled: false,
                    dedupe_ttl_secs: 3600,
                    signer: None,
                    account_aliases: Vec::new(),
                    currency_assets: Vec::new(),
                    reference_data: iroha_config::parameters::actual::IsoReferenceData::default(),
                },
                sorafs_discovery: iroha_config::parameters::actual::SorafsDiscovery::default(),
                sorafs_storage: iroha_config::parameters::actual::SorafsStorage::default(),
                sorafs_quota: iroha_config::parameters::actual::SorafsQuota::default(),
                sorafs_alias_cache:
                    iroha_config::parameters::actual::SorafsAliasCachePolicy::default(),
                sorafs_gateway: iroha_config::parameters::actual::SorafsGateway::default(),
                sorafs_por: iroha_config::parameters::actual::SorafsPor::default(),
                transport: iroha_config::parameters::actual::ToriiTransport::default(),
                onboarding: None,
                offline_issuer: None,
                proof_api: iroha_config::parameters::actual::ProofApi {
                    rate_per_minute: iroha_config::parameters::defaults::torii::PROOF_RATE_PER_MIN
                        .and_then(NonZeroU32::new),
                    burst: iroha_config::parameters::defaults::torii::PROOF_BURST
                        .and_then(NonZeroU32::new),
                    max_body_bytes: iroha_config::parameters::defaults::torii::PROOF_MAX_BODY_BYTES,
                    egress_bytes_per_sec: iroha_config::parameters::defaults::torii::PROOF_EGRESS_BYTES_PER_SEC
                        .and_then(std::num::NonZeroU64::new),
                    egress_burst_bytes: iroha_config::parameters::defaults::torii::PROOF_EGRESS_BURST_BYTES
                        .and_then(std::num::NonZeroU64::new),
                    max_list_limit: NonZeroU32::new(
                        iroha_config::parameters::defaults::torii::PROOF_MAX_LIST_LIMIT,
                    )
                    .expect("non-zero list limit"),
                    request_timeout: Duration::from_millis(
                        iroha_config::parameters::defaults::torii::PROOF_REQUEST_TIMEOUT_MS,
                    ),
                    cache_max_age: Duration::from_secs(
                        iroha_config::parameters::defaults::torii::PROOF_CACHE_MAX_AGE_SECS,
                    ),
                    retry_after: Duration::from_secs(
                        iroha_config::parameters::defaults::torii::PROOF_RETRY_AFTER_SECS,
                    ),
                },
                app_api: iroha_config::parameters::actual::AppApi {
                    default_list_limit: NonZeroU32::new(
                        iroha_config::parameters::defaults::torii::APP_API_DEFAULT_LIST_LIMIT,
                    )
                    .expect("non-zero default list limit"),
                    max_list_limit: NonZeroU32::new(
                        iroha_config::parameters::defaults::torii::APP_API_MAX_LIST_LIMIT,
                    )
                    .expect("non-zero max list limit"),
                    max_fetch_size: NonZeroU32::new(
                        iroha_config::parameters::defaults::torii::APP_API_MAX_FETCH_SIZE,
                    )
                    .expect("non-zero max fetch size"),
                    rate_limit_cost_per_row: NonZeroU32::new(
                        iroha_config::parameters::defaults::torii::APP_API_RATE_LIMIT_COST_PER_ROW,
                    )
                    .expect("non-zero app-api rate limit cost"),
                },
                webhook: iroha_config::parameters::actual::Webhook {
                    queue_capacity: NonZeroUsize::new(
                        iroha_config::parameters::defaults::torii::WEBHOOK_QUEUE_CAPACITY,
                    )
                    .expect("non-zero webhook queue capacity"),
                    max_attempts: NonZeroU32::new(
                        iroha_config::parameters::defaults::torii::WEBHOOK_MAX_ATTEMPTS,
                    )
                    .expect("non-zero webhook max attempts"),
                    backoff_initial: Duration::from_millis(
                        iroha_config::parameters::defaults::torii::WEBHOOK_BACKOFF_INITIAL_MS,
                    ),
                    backoff_max: Duration::from_millis(
                        iroha_config::parameters::defaults::torii::WEBHOOK_BACKOFF_MAX_MS,
                    ),
                    connect_timeout: Duration::from_millis(
                        iroha_config::parameters::defaults::torii::WEBHOOK_CONNECT_TIMEOUT_MS,
                    ),
                    write_timeout: Duration::from_millis(
                        iroha_config::parameters::defaults::torii::WEBHOOK_WRITE_TIMEOUT_MS,
                    ),
                    read_timeout: Duration::from_millis(
                        iroha_config::parameters::defaults::torii::WEBHOOK_READ_TIMEOUT_MS,
                    ),
                },
                push: iroha_config::parameters::actual::Push {
                    enabled: iroha_config::parameters::defaults::torii::PUSH_ENABLED,
                    rate_per_minute: iroha_config::parameters::defaults::torii::PUSH_RATE_PER_MINUTE
                        .and_then(NonZeroU32::new),
                    burst: iroha_config::parameters::defaults::torii::PUSH_BURST
                        .and_then(NonZeroU32::new),
                    connect_timeout: Duration::from_millis(
                        iroha_config::parameters::defaults::torii::PUSH_CONNECT_TIMEOUT_MS,
                    ),
                    request_timeout: Duration::from_millis(
                        iroha_config::parameters::defaults::torii::PUSH_REQUEST_TIMEOUT_MS,
                    ),
                    max_topics_per_device: NonZeroUsize::new(
                        iroha_config::parameters::defaults::torii::PUSH_MAX_TOPICS_PER_DEVICE.max(1),
                    )
                    .expect("non-zero push topics cap"),
                    fcm_api_key: None,
                    apns_endpoint: None,
                    apns_auth_token: None,
                },
            },
            kura: Kura {
                init_mode: iroha_config::kura::InitMode::Strict,
                store_dir: WithOrigin::inline(std::env::temp_dir()),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: NonZeroUsize::new(10).unwrap(),
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            },
	            sumeragi: Sumeragi {
                debug_force_soft_fork: false,
                debug_disable_background_worker: false,
                debug_rbc_drop_every_nth_chunk: None,
                debug_rbc_shuffle_chunks: false,
                debug_rbc_duplicate_inits: false,
                debug_rbc_force_deliver_quorum_one: false,
                debug_rbc_corrupt_witness_ack: false,
                debug_rbc_corrupt_ready_signature: false,
                debug_rbc_drop_validator_mask: 0,
                debug_rbc_equivocate_chunk_mask: 0,
                debug_rbc_equivocate_validator_mask: 0,
                debug_rbc_conflicting_ready_mask: 0,
                debug_rbc_partial_chunk_mask: 0,
                role: NodeRole::Validator,
                enable_bls: true,
                allow_view0_slack: false,
                collectors_k: iroha_config::parameters::defaults::sumeragi::COLLECTORS_K,
                collectors_redundant_send_r:
                    iroha_config::parameters::defaults::sumeragi::COLLECTORS_REDUNDANT_SEND_R,
                block_max_transactions: iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_TRANSACTIONS,
                block_max_payload_bytes: iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES,
                msg_channel_cap_votes:
                    iroha_config::parameters::defaults::sumeragi::MSG_CHANNEL_CAP_VOTES,
                msg_channel_cap_block_payload:
                    iroha_config::parameters::defaults::sumeragi::MSG_CHANNEL_CAP_BLOCK_PAYLOAD,
                msg_channel_cap_rbc_chunks:
                    iroha_config::parameters::defaults::sumeragi::MSG_CHANNEL_CAP_RBC_CHUNKS,
                msg_channel_cap_blocks:
                    iroha_config::parameters::defaults::sumeragi::MSG_CHANNEL_CAP_BLOCKS,
                control_msg_channel_cap:
                    iroha_config::parameters::defaults::sumeragi::CONTROL_MSG_CHANNEL_CAP,
                consensus_mode: iroha_config::parameters::actual::ConsensusMode::Permissioned,
                mode_flip_enabled: iroha_config::parameters::defaults::sumeragi::MODE_FLIP_ENABLED,
                da_enabled: iroha_config::parameters::defaults::sumeragi::DA_ENABLED,
                da_quorum_timeout_multiplier:
                    iroha_config::parameters::defaults::sumeragi::DA_QUORUM_TIMEOUT_MULTIPLIER,
                da_availability_timeout_multiplier:
                    iroha_config::parameters::defaults::sumeragi::DA_AVAILABILITY_TIMEOUT_MULTIPLIER,
                da_availability_timeout_floor: std::time::Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::DA_AVAILABILITY_TIMEOUT_FLOOR_MS,
                ),
                kura_store_retry_interval: std::time::Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::KURA_STORE_RETRY_INTERVAL_MS,
                ),
                kura_store_retry_max_attempts:
                    iroha_config::parameters::defaults::sumeragi::KURA_STORE_RETRY_MAX_ATTEMPTS,
                commit_inflight_timeout: std::time::Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::COMMIT_INFLIGHT_TIMEOUT_MS,
                ),
                missing_block_signer_fallback_attempts:
                    iroha_config::parameters::defaults::sumeragi::MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS,
                membership_mismatch_alert_threshold:
                    iroha_config::parameters::defaults::sumeragi::MEMBERSHIP_MISMATCH_ALERT_THRESHOLD,
                membership_mismatch_fail_closed:
                    iroha_config::parameters::defaults::sumeragi::MEMBERSHIP_MISMATCH_FAIL_CLOSED,
                da_max_commitments_per_block:
                    iroha_config::parameters::defaults::sumeragi::DA_MAX_COMMITMENTS_PER_BLOCK,
                da_max_proof_openings_per_block:
                    iroha_config::parameters::defaults::sumeragi::DA_MAX_PROOF_OPENINGS_PER_BLOCK,
                proof_policy: iroha_config::parameters::actual::ProofPolicy::Off,
                commit_cert_history_cap:
                    iroha_config::parameters::defaults::sumeragi::COMMIT_CERT_HISTORY_CAP,
                zk_finality_k: 0,
                require_precommit_qc:
                    iroha_config::parameters::defaults::sumeragi::REQUIRE_PRECOMMIT_QC,
                rbc_chunk_max_bytes: 64 * 1024,
                rbc_pending_max_chunks:
                    iroha_config::parameters::defaults::sumeragi::RBC_PENDING_MAX_CHUNKS,
                rbc_pending_max_bytes:
                    iroha_config::parameters::defaults::sumeragi::RBC_PENDING_MAX_BYTES,
                rbc_pending_ttl: std::time::Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::RBC_PENDING_TTL_MS,
                ),
                rbc_session_ttl: std::time::Duration::from_secs(120),
                rbc_store_max_sessions:
                    iroha_config::parameters::defaults::sumeragi::RBC_STORE_MAX_SESSIONS,
                rbc_store_soft_sessions:
                    iroha_config::parameters::defaults::sumeragi::RBC_STORE_SOFT_SESSIONS,
                rbc_store_max_bytes:
                    iroha_config::parameters::defaults::sumeragi::RBC_STORE_MAX_BYTES,
                rbc_store_soft_bytes:
                    iroha_config::parameters::defaults::sumeragi::RBC_STORE_SOFT_BYTES,
                rbc_disk_store_ttl: std::time::Duration::from_secs(
                    iroha_config::parameters::defaults::sumeragi::RBC_DISK_STORE_TTL_SECS,
                ),
                rbc_disk_store_max_bytes:
                    iroha_config::parameters::defaults::sumeragi::RBC_DISK_STORE_MAX_BYTES,
                key_activation_lead_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_ACTIVATION_LEAD_BLOCKS,
                key_overlap_grace_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_OVERLAP_GRACE_BLOCKS,
                key_expiry_grace_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_EXPIRY_GRACE_BLOCKS,
                key_require_hsm: iroha_config::parameters::defaults::sumeragi::KEY_REQUIRE_HSM,
                key_allowed_algorithms: iroha_config::parameters::defaults::sumeragi::key_allowed_algorithms()
                    .into_iter()
                    .collect(),
                key_allowed_hsm_providers: iroha_config::parameters::defaults::sumeragi::key_allowed_hsm_providers()
                    .into_iter()
                    .collect(),
                npos: iroha_config::parameters::actual::SumeragiNpos {
                    block_time: std::time::Duration::from_millis(
                        iroha_config::parameters::defaults::sumeragi::npos::BLOCK_TIME_MS,
                    ),
                    timeouts: iroha_config::parameters::actual::SumeragiNposTimeouts {
                        propose: std::time::Duration::from_millis(
                            iroha_config::parameters::defaults::sumeragi::npos::TIMEOUT_PROPOSE_MS,
                        ),
                        prevote: std::time::Duration::from_millis(
                            iroha_config::parameters::defaults::sumeragi::npos::TIMEOUT_PREVOTE_MS,
                        ),
                        precommit: std::time::Duration::from_millis(
                            iroha_config::parameters::defaults::sumeragi::npos::TIMEOUT_PRECOMMIT_MS,
                        ),
                        exec: std::time::Duration::from_millis(
                            iroha_config::parameters::defaults::sumeragi::npos::TIMEOUT_EXEC_MS,
                        ),
                        witness: std::time::Duration::from_millis(
                            iroha_config::parameters::defaults::sumeragi::npos::TIMEOUT_WITNESS_MS,
                        ),
                        commit: std::time::Duration::from_millis(
                            iroha_config::parameters::defaults::sumeragi::npos::TIMEOUT_COMMIT_MS,
                        ),
                        da: std::time::Duration::from_millis(
                            iroha_config::parameters::defaults::sumeragi::npos::TIMEOUT_DA_MS,
                        ),
                        aggregator: std::time::Duration::from_millis(
                            iroha_config::parameters::defaults::sumeragi::npos::TIMEOUT_AGG_MS,
                        ),
                    },
                    pacemaker_backoff_multiplier:
                        iroha_config::parameters::defaults::sumeragi::PACEMAKER_BACKOFF_MULTIPLIER,
                    pacemaker_rtt_floor_multiplier:
                        iroha_config::parameters::defaults::sumeragi::PACEMAKER_RTT_FLOOR_MULTIPLIER,
                    pacemaker_max_backoff: std::time::Duration::from_millis(
                        iroha_config::parameters::defaults::sumeragi::PACEMAKER_MAX_BACKOFF_MS,
                    ),
                    pacemaker_jitter_frac_permille:
                        iroha_config::parameters::defaults::sumeragi::PACEMAKER_JITTER_FRAC_PERMILLE,
                    k_aggregators:
                        iroha_config::parameters::defaults::sumeragi::npos::K_AGGREGATORS,
                    redundant_send_r:
                        iroha_config::parameters::defaults::sumeragi::npos::REDUNDANT_SEND_R,
                    vrf: iroha_config::parameters::actual::SumeragiNposVrf {
                        commit_window_blocks: iroha_config::parameters::defaults::sumeragi::npos::VRF_COMMIT_WINDOW_BLOCKS,
                        reveal_window_blocks: iroha_config::parameters::defaults::sumeragi::npos::VRF_REVEAL_WINDOW_BLOCKS,
                    },
                    election: iroha_config::parameters::actual::SumeragiNposElection {
                        max_validators:
                            iroha_config::parameters::defaults::sumeragi::npos::MAX_VALIDATORS,
                        min_self_bond:
                            iroha_config::parameters::defaults::sumeragi::npos::MIN_SELF_BOND,
                        min_nomination_bond:
                            iroha_config::parameters::defaults::sumeragi::npos::MIN_NOMINATION_BOND,
                        max_nominator_concentration_pct:
                            iroha_config::parameters::defaults::sumeragi::npos::MAX_NOMINATOR_CONCENTRATION_PCT,
                        seat_band_pct:
                            iroha_config::parameters::defaults::sumeragi::npos::SEAT_BAND_PCT,
                        max_entity_correlation_pct:
                            iroha_config::parameters::defaults::sumeragi::npos::MAX_ENTITY_CORRELATION_PCT,
                        finality_margin_blocks:
                            iroha_config::parameters::defaults::sumeragi::npos::FINALITY_MARGIN_BLOCKS,
                    },
                    reconfig: iroha_config::parameters::actual::SumeragiNposReconfig {
                        evidence_horizon_blocks:
                            iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_EVIDENCE_HORIZON_BLOCKS,
                        activation_lag_blocks:
                            iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS,
                    },
                },
                use_stake_snapshot_roster: false,
                epoch_length_blocks:
                    iroha_config::parameters::defaults::sumeragi::EPOCH_LENGTH_BLOCKS,
                vrf_commit_deadline_offset:
                    iroha_config::parameters::defaults::sumeragi::VRF_COMMIT_DEADLINE_OFFSET,
                vrf_reveal_deadline_offset:
                    iroha_config::parameters::defaults::sumeragi::VRF_REVEAL_DEADLINE_OFFSET,
                pacemaker_backoff_multiplier:
                    iroha_config::parameters::defaults::sumeragi::PACEMAKER_BACKOFF_MULTIPLIER,
                pacemaker_rtt_floor_multiplier:
                    iroha_config::parameters::defaults::sumeragi::PACEMAKER_RTT_FLOOR_MULTIPLIER,
                pacemaker_max_backoff: std::time::Duration::from_millis(
                    iroha_config::parameters::defaults::sumeragi::PACEMAKER_MAX_BACKOFF_MS,
                ),
                pacemaker_jitter_frac_permille:
                    iroha_config::parameters::defaults::sumeragi::PACEMAKER_JITTER_FRAC_PERMILLE,
                adaptive_observability: iroha_config::parameters::actual::AdaptiveObservability::default(),
            },
            block_sync: BlockSync {
                gossip_period: std::time::Duration::from_millis(200),
                gossip_size: NonZeroU32::new(32).unwrap(),
            },
            transaction_gossiper: TransactionGossiper {
                gossip_period: std::time::Duration::from_millis(200),
                gossip_size: NonZeroU32::new(32).unwrap(),
                dataspace: DataspaceGossip::default(),
            },
            live_query_store: LiveQueryStore::default(),
            logger: Logger {
                level: Level::INFO,
                filter: None,
                format: iroha_config::logger::Format::default(),
                terminal_colors: false,
            },
            queue: Queue::default(),
            nexus: Nexus::default(),
            snapshot: iroha_config::parameters::user::Snapshot {
                mode: iroha_config::snapshot::Mode::Disabled,
                create_every_ms: iroha_config::base::util::DurationMs(
                    std::time::Duration::from_secs(60),
                ),
                store_dir: WithOrigin::inline(std::env::temp_dir()),
                merkle_chunk_size_bytes:
                    iroha_config::parameters::defaults::snapshot::MERKLE_CHUNK_SIZE_BYTES,
                verification_public_key: None,
                signing_private_key: None,
            },
            telemetry_enabled: false,
            telemetry_profile: iroha_config::parameters::actual::TelemetryProfile::Disabled,
            telemetry: None,
            telemetry_redaction: iroha_config::parameters::actual::TelemetryRedaction::default(),
            telemetry_integrity: iroha_config::parameters::actual::TelemetryIntegrity::default(),
            dev_telemetry: iroha_config::parameters::user::DevTelemetry {
                out_file: None,
                panic_on_duplicate_metrics:
                    iroha_config::parameters::defaults::telemetry::PANIC_ON_DUPLICATE_METRICS,
            },
            pipeline: iroha_config::parameters::actual::Pipeline {
                dynamic_prepass: false,
                access_set_cache_enabled:
                    iroha_config::parameters::defaults::pipeline::ACCESS_SET_CACHE_ENABLED,
                parallel_overlay: false,
                workers: iroha_config::parameters::defaults::pipeline::WORKERS,
                parallel_apply: true,
                ready_queue_heap: iroha_config::parameters::defaults::pipeline::READY_QUEUE_HEAP,
                gpu_key_bucket: iroha_config::parameters::defaults::pipeline::GPU_KEY_BUCKET,
                debug_trace_scheduler_inputs:
                    iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_SCHEDULER_INPUTS,
                debug_trace_tx_eval:
                    iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_TX_EVAL,
                signature_batch_max:
                    iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX,
                signature_batch_max_ed25519:
                    iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519,
                signature_batch_max_secp256k1:
                    iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_SECP256K1,
                signature_batch_max_pqc:
                    iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_PQC,
                signature_batch_max_bls:
                    iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_BLS,
                cache_size: iroha_config::parameters::defaults::pipeline::CACHE_SIZE,
                ivm_cache_max_decoded_ops:
                    iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS,
                ivm_cache_max_bytes:
                    iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_BYTES,
                ivm_prover_threads:
                    iroha_config::parameters::defaults::pipeline::IVM_PROVER_THREADS,
                overlay_max_instructions:
                    iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_INSTRUCTIONS,
                overlay_max_bytes: iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_BYTES,
                overlay_chunk_instructions:
                    iroha_config::parameters::defaults::pipeline::OVERLAY_CHUNK_INSTRUCTIONS,
                gas: iroha_config::parameters::actual::Gas {
                    tech_account_id:
                        iroha_config::parameters::defaults::pipeline::GAS_TECH_ACCOUNT_ID
                            .to_string(),
                    accepted_assets: Vec::new(),
                    units_per_gas: Vec::new(),
                },
                ivm_max_cycles_upper_bound:
                    iroha_config::parameters::defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
                ivm_max_decoded_instructions:
                    iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_INSTRUCTIONS,
                ivm_max_decoded_bytes:
                    iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_BYTES,
                quarantine_max_txs_per_block:
                    iroha_config::parameters::defaults::pipeline::QUARANTINE_MAX_TXS_PER_BLOCK,
                quarantine_tx_max_cycles:
                    iroha_config::parameters::defaults::pipeline::QUARANTINE_TX_MAX_CYCLES,
                quarantine_tx_max_millis:
                    iroha_config::parameters::defaults::pipeline::QUARANTINE_TX_MAX_MILLIS,
                query_default_cursor_mode:
                    iroha_config::parameters::actual::QueryCursorMode::Ephemeral,
                query_stored_min_gas_units:
                    iroha_config::parameters::defaults::pipeline::QUERY_STORED_MIN_GAS_UNITS,
                amx_per_dataspace_budget_ms:
                    iroha_config::parameters::defaults::pipeline::AMX_PER_DATASPACE_BUDGET_MS,
                amx_group_budget_ms:
                    iroha_config::parameters::defaults::pipeline::AMX_GROUP_BUDGET_MS,
                amx_per_instruction_ns:
                    iroha_config::parameters::defaults::pipeline::AMX_PER_INSTRUCTION_NS,
                amx_per_memory_access_ns:
                    iroha_config::parameters::defaults::pipeline::AMX_PER_MEMORY_ACCESS_NS,
                amx_per_syscall_ns:
                    iroha_config::parameters::defaults::pipeline::AMX_PER_SYSCALL_NS,
            },
            tiered_state: TieredState {
                enabled: false,
                hot_retained_keys: 0,
                hot_retained_bytes: iroha_config::parameters::defaults::tiered_state::HOT_RETAINED_BYTES,
                hot_retained_grace_snapshots:
                    iroha_config::parameters::defaults::tiered_state::HOT_RETAINED_GRACE_SNAPSHOTS,
                cold_store_root: None,
                da_store_root: None,
                max_snapshots: 2,
                max_cold_bytes: iroha_config::parameters::defaults::tiered_state::MAX_COLD_BYTES,
            },
            compute: iroha_config::parameters::actual::Compute {
                enabled: iroha_config::parameters::defaults::compute::ENABLED,
                namespaces: iroha_config::parameters::defaults::compute::default_namespaces()
                    .into_iter()
                    .collect(),
                default_ttl_slots:
                    iroha_config::parameters::defaults::compute::default_ttl_slots(),
                max_ttl_slots: iroha_config::parameters::defaults::compute::max_ttl_slots(),
                max_request_bytes: iroha_config::parameters::defaults::compute::MAX_REQUEST_BYTES,
                max_response_bytes: iroha_config::parameters::defaults::compute::MAX_RESPONSE_BYTES,
                max_gas_per_call: iroha_config::parameters::defaults::compute::max_gas_per_call(),
                resource_profiles:
                    iroha_config::parameters::defaults::compute::resource_profiles(),
                default_resource_profile: iroha_config::parameters::defaults::compute::default_resource_profile(),
                price_families: iroha_config::parameters::defaults::compute::price_families(),
                default_price_family: iroha_config::parameters::defaults::compute::default_price_family(),
                auth_policy: iroha_config::parameters::defaults::compute::default_auth_policy(),
                sandbox: iroha_config::parameters::defaults::compute::sandbox_rules(),
                economics: iroha_config::parameters::actual::ComputeEconomics {
                    max_cu_per_call: iroha_config::parameters::defaults::compute::max_cu_per_call(),
                    max_amplification_ratio: iroha_config::parameters::defaults::compute::max_amplification_ratio(),
                    fee_split: iroha_config::parameters::defaults::compute::fee_split(),
                    sponsor_policy: iroha_config::parameters::defaults::compute::sponsor_policy(),
                    price_bounds: iroha_config::parameters::defaults::compute::price_bounds(),
                    price_risk_classes: iroha_config::parameters::defaults::compute::price_risk_classes(),
                    price_family_baseline: iroha_config::parameters::defaults::compute::price_families(),
                    price_amplifiers: iroha_config::parameters::defaults::compute::price_amplifiers(),
                },
                slo: iroha_config::parameters::actual::ComputeSlo {
                    max_inflight_per_route: iroha_config::parameters::defaults::compute::max_inflight_per_route(),
                    queue_depth_per_route: iroha_config::parameters::defaults::compute::queue_depth_per_route(),
                    max_requests_per_second: iroha_config::parameters::defaults::compute::max_requests_per_second(),
                    target_p50_latency_ms: iroha_config::parameters::defaults::compute::target_p50_latency_ms(),
                    target_p95_latency_ms: iroha_config::parameters::defaults::compute::target_p95_latency_ms(),
                    target_p99_latency_ms: iroha_config::parameters::defaults::compute::target_p99_latency_ms(),
                },
            },
            content: iroha_config::parameters::actual::Content {
                max_bundle_bytes: iroha_config::parameters::defaults::content::MAX_BUNDLE_BYTES,
                max_files: iroha_config::parameters::defaults::content::MAX_FILES,
                max_path_len: iroha_config::parameters::defaults::content::MAX_PATH_LEN,
                max_retention_blocks: iroha_config::parameters::defaults::content::MAX_RETENTION_BLOCKS,
                chunk_size_bytes: iroha_config::parameters::defaults::content::CHUNK_SIZE_BYTES,
                publish_allow_accounts: Vec::new(),
                limits: iroha_config::parameters::actual::ContentLimits {
                    max_requests_per_second: std::num::NonZeroU32::new(
                        iroha_config::parameters::defaults::content::MAX_REQUESTS_PER_SECOND
                    )
                    .unwrap(),
                    request_burst: std::num::NonZeroU32::new(
                        iroha_config::parameters::defaults::content::REQUEST_BURST
                    )
                    .unwrap(),
                    max_egress_bytes_per_second: std::num::NonZeroU64::new(
                        u64::from(
                            iroha_config::parameters::defaults::content::MAX_EGRESS_BYTES_PER_SECOND,
                        )
                    )
                    .unwrap(),
                    egress_burst_bytes: std::num::NonZeroU64::new(
                        iroha_config::parameters::defaults::content::EGRESS_BURST_BYTES
                    )
                    .unwrap(),
                },
                default_cache_max_age_secs:
                    iroha_config::parameters::defaults::content::DEFAULT_CACHE_MAX_AGE_SECS,
                max_cache_max_age_secs: iroha_config::parameters::defaults::content::MAX_CACHE_MAX_AGE_SECS,
                immutable_bundles: iroha_config::parameters::defaults::content::IMMUTABLE_BUNDLES,
                default_auth_mode: iroha_data_model::content::ContentAuthMode::Public,
                slo: iroha_config::parameters::actual::ContentSlo {
                    target_p50_latency_ms: std::num::NonZeroU32::new(
                        iroha_config::parameters::defaults::content::TARGET_P50_LATENCY_MS
                    )
                    .unwrap(),
                    target_p99_latency_ms: std::num::NonZeroU32::new(
                        iroha_config::parameters::defaults::content::TARGET_P99_LATENCY_MS
                    )
                    .unwrap(),
                    target_availability_bps: std::num::NonZeroU32::new(
                        iroha_config::parameters::defaults::content::TARGET_AVAILABILITY_BPS
                    )
                    .unwrap(),
                },
                pow: iroha_config::parameters::actual::ContentPow {
                    difficulty_bits: iroha_config::parameters::defaults::content::POW_DIFFICULTY_BITS,
                    header_name: iroha_config::parameters::defaults::content::default_pow_header(),
                },
                stripe_layout: iroha_config::parameters::defaults::content::default_stripe_layout(),
            },
            oracle: iroha_config::parameters::actual::Oracle {
                history_depth: iroha_config::parameters::defaults::oracle::history_depth(),
                economics: iroha_config::parameters::actual::OracleEconomics {
                    reward_asset: iroha_config::parameters::defaults::oracle::reward_asset(),
                    reward_pool: iroha_config::parameters::defaults::oracle::reward_pool(),
                    reward_amount: iroha_config::parameters::defaults::oracle::reward_amount(),
                    slash_asset: iroha_config::parameters::defaults::oracle::slash_asset(),
                    slash_receiver: iroha_config::parameters::defaults::oracle::slash_receiver(),
                    slash_outlier_amount:
                        iroha_config::parameters::defaults::oracle::slash_outlier_amount(),
                    slash_error_amount: iroha_config::parameters::defaults::oracle::slash_error_amount(),
                    slash_no_show_amount:
                        iroha_config::parameters::defaults::oracle::slash_no_show_amount(),
                    dispute_bond_asset: iroha_config::parameters::defaults::oracle::dispute_bond_asset(),
                    dispute_bond_amount:
                        iroha_config::parameters::defaults::oracle::dispute_bond_amount(),
                    dispute_reward_amount:
                        iroha_config::parameters::defaults::oracle::dispute_reward_amount(),
                    frivolous_slash_amount:
                        iroha_config::parameters::defaults::oracle::frivolous_slash_amount(),
                },
                governance: iroha_config::parameters::actual::OracleGovernance {
                    intake_sla_blocks: iroha_config::parameters::defaults::oracle::intake_sla_blocks(),
                    rules_sla_blocks: iroha_config::parameters::defaults::oracle::rules_sla_blocks(),
                    cop_sla_blocks: iroha_config::parameters::defaults::oracle::cop_sla_blocks(),
                    technical_sla_blocks: iroha_config::parameters::defaults::oracle::technical_sla_blocks(),
                    policy_jury_sla_blocks: iroha_config::parameters::defaults::oracle::policy_jury_sla_blocks(),
                    enact_sla_blocks: iroha_config::parameters::defaults::oracle::enact_sla_blocks(),
                    intake_min_votes: iroha_config::parameters::defaults::oracle::intake_min_votes(),
                    rules_min_votes: iroha_config::parameters::defaults::oracle::rules_min_votes(),
                    cop_min_votes: iroha_config::parameters::actual::OracleChangeThresholds {
                        low: iroha_config::parameters::defaults::oracle::cop_low_votes(),
                        medium: iroha_config::parameters::defaults::oracle::cop_medium_votes(),
                        high: iroha_config::parameters::defaults::oracle::cop_high_votes(),
                    },
                    technical_min_votes: iroha_config::parameters::defaults::oracle::technical_min_votes(),
                policy_jury_min_votes: iroha_config::parameters::actual::OracleChangeThresholds {
                    low: iroha_config::parameters::defaults::oracle::policy_jury_low_votes(),
                    medium: iroha_config::parameters::defaults::oracle::policy_jury_medium_votes(),
                    high: iroha_config::parameters::defaults::oracle::policy_jury_high_votes(),
                },
            },
            twitter_binding: iroha_config::parameters::actual::OracleTwitterBinding {
                feed_id: iroha_config::parameters::defaults::oracle::twitter_binding_feed_id(),
                pepper_id: iroha_config::parameters::defaults::oracle::twitter_binding_pepper_id(),
                max_ttl_ms: iroha_config::parameters::defaults::oracle::twitter_binding_max_ttl_ms(),
                min_ttl_ms: iroha_config::parameters::defaults::oracle::twitter_binding_min_ttl_ms(),
                min_update_spacing_ms: iroha_config::parameters::defaults::oracle::twitter_binding_min_update_spacing_ms(),
            },
        },
            zk: iroha_config::parameters::actual::Zk {
                halo2: iroha_config::parameters::actual::Halo2 {
                    enabled: false,
                    curve: iroha_config::parameters::actual::ZkCurve::Pallas,
                    backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
                    max_k: 16,
                    verifier_budget_ms: 1000,
                    verifier_max_batch: 8,
                    ..iroha_config::parameters::actual::Halo2::default()
                },
                fastpq: iroha_config::parameters::actual::Fastpq {
                    execution_mode: iroha_config::parameters::actual::FastpqExecutionMode::Auto,
                    poseidon_mode: iroha_config::parameters::actual::FastpqPoseidonMode::Auto,
                    device_class: None,
                    chip_family: None,
                    gpu_kind: None,
                    metal_queue_fanout: None,
                    metal_queue_column_threshold: None,
                    metal_max_in_flight: None,
                    metal_threadgroup_width: None,
                    metal_trace: iroha_config::parameters::defaults::zk::fastpq::METAL_TRACE,
                    metal_debug_enum:
                        iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_ENUM,
                    metal_debug_fused:
                        iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_FUSED,
                },
                root_history_cap: iroha_config::parameters::defaults::zk::ledger::ROOT_HISTORY_CAP,
                ballot_history_cap:
                    iroha_config::parameters::defaults::zk::vote::BALLOT_HISTORY_CAP,
                empty_root_on_empty:
                    iroha_config::parameters::defaults::zk::ledger::EMPTY_ROOT_ON_EMPTY,
                merkle_depth: iroha_config::parameters::defaults::zk::ledger::EMPTY_ROOT_DEPTH,
                preverify_max_bytes: iroha_config::parameters::defaults::zk::preverify::MAX_BYTES,
                preverify_budget_bytes:
                    iroha_config::parameters::defaults::zk::preverify::BUDGET_BYTES,
                proof_history_cap:
                    iroha_config::parameters::defaults::zk::proof::RECORD_HISTORY_CAP,
                proof_retention_grace_blocks:
                    iroha_config::parameters::defaults::zk::proof::RETENTION_GRACE_BLOCKS,
                proof_prune_batch:
                    iroha_config::parameters::defaults::zk::proof::PRUNE_BATCH_SIZE,
                bridge_proof_max_range_len:
                    iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_RANGE_LEN,
                bridge_proof_max_past_age_blocks:
                    iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_PAST_AGE_BLOCKS,
                bridge_proof_max_future_drift_blocks:
                    iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_FUTURE_DRIFT_BLOCKS,
                poseidon_params_id:
                    iroha_config::parameters::defaults::confidential::POSEIDON_PARAMS_ID,
                pedersen_params_id:
                    iroha_config::parameters::defaults::confidential::PEDERSEN_PARAMS_ID,
                kaigi_roster_join_vk: None,
                kaigi_roster_leave_vk: None,
                kaigi_usage_vk: None,
                max_proof_size_bytes:
                    iroha_config::parameters::defaults::confidential::MAX_PROOF_SIZE_BYTES,
                max_nullifiers_per_tx:
                    iroha_config::parameters::defaults::confidential::MAX_NULLIFIERS_PER_TX,
                max_commitments_per_tx:
                    iroha_config::parameters::defaults::confidential::MAX_COMMITMENTS_PER_TX,
                max_confidential_ops_per_block:
                    iroha_config::parameters::defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
                verify_timeout: iroha_config::parameters::defaults::confidential::VERIFY_TIMEOUT,
                max_anchor_age_blocks:
                    iroha_config::parameters::defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
                max_proof_bytes_block:
                    iroha_config::parameters::defaults::confidential::MAX_PROOF_BYTES_BLOCK,
                max_verify_calls_per_tx:
                    iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
                max_verify_calls_per_block:
                    iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
                max_public_inputs:
                    iroha_config::parameters::defaults::confidential::MAX_PUBLIC_INPUTS,
                reorg_depth_bound:
                    iroha_config::parameters::defaults::confidential::REORG_DEPTH_BOUND,
                policy_transition_delay_blocks:
                    iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
                policy_transition_window_blocks:
                    iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
                tree_roots_history_len:
                    iroha_config::parameters::defaults::confidential::TREE_ROOTS_HISTORY_LEN,
                tree_frontier_checkpoint_interval:
                    iroha_config::parameters::defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
                registry_max_vk_entries:
                    iroha_config::parameters::defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
                registry_max_params_entries:
                    iroha_config::parameters::defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
                registry_max_delta_per_block:
                    iroha_config::parameters::defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
                gas: iroha_config::parameters::actual::ConfidentialGas {
                    proof_base: iroha_config::parameters::defaults::confidential::gas::PROOF_BASE,
                    per_public_input:
                        iroha_config::parameters::defaults::confidential::gas::PER_PUBLIC_INPUT,
                    per_proof_byte:
                        iroha_config::parameters::defaults::confidential::gas::PER_PROOF_BYTE,
                    per_nullifier:
                        iroha_config::parameters::defaults::confidential::gas::PER_NULLIFIER,
                    per_commitment:
                        iroha_config::parameters::defaults::confidential::gas::PER_COMMITMENT,
                },
            },
            norito: iroha_config::parameters::actual::Norito {
                min_compress_bytes_cpu:
                    iroha_config::parameters::defaults::norito::MIN_COMPRESS_BYTES_CPU,
                min_compress_bytes_gpu:
                    iroha_config::parameters::defaults::norito::MIN_COMPRESS_BYTES_GPU,
                zstd_level_small: iroha_config::parameters::defaults::norito::ZSTD_LEVEL_SMALL,
                zstd_level_large: iroha_config::parameters::defaults::norito::ZSTD_LEVEL_LARGE,
                zstd_level_gpu: iroha_config::parameters::defaults::norito::ZSTD_LEVEL_GPU,
                large_threshold: iroha_config::parameters::defaults::norito::LARGE_THRESHOLD,
                enable_compact_seq_len_up_to:
                    iroha_config::parameters::defaults::norito::ENABLE_COMPACT_SEQ_LEN_UP_TO,
                enable_varint_offsets_up_to:
                    iroha_config::parameters::defaults::norito::ENABLE_VARINT_OFFSETS_UP_TO,
                allow_gpu_compression:
                    iroha_config::parameters::defaults::norito::ALLOW_GPU_COMPRESSION,
                max_archive_len: iroha_config::parameters::defaults::norito::MAX_ARCHIVE_LEN,
                aos_ncb_small_n: iroha_config::parameters::defaults::norito::AOS_NCB_SMALL_N,
            },
            hijiri: Hijiri::new(None),
            fraud_monitoring: FraudMonitoring::new(
                iroha_config::parameters::defaults::fraud_monitoring::ENABLED,
                Vec::new(),
                iroha_config::parameters::defaults::fraud_monitoring::CONNECT_TIMEOUT,
                iroha_config::parameters::defaults::fraud_monitoring::REQUEST_TIMEOUT,
                iroha_config::parameters::defaults::fraud_monitoring::MISSING_ASSESSMENT_GRACE_SECS,
                None,
                Vec::new(),
            ),
            gov: Governance {
                vk_ballot: None,
                vk_tally: None,
                voting_asset_id: iroha_config::parameters::defaults::governance::voting_asset_id()
                    .parse()
                    .expect("valid default voting asset id"),
                citizenship_asset_id: iroha_config::parameters::defaults::governance::CITIZENSHIP_ASSET_ID
                    .parse()
                    .expect("valid default citizenship asset id"),
                citizenship_bond_amount:
                    iroha_config::parameters::defaults::governance::CITIZENSHIP_BOND_AMOUNT,
                citizenship_escrow_account: iroha_config::parameters::defaults::governance::CITIZENSHIP_ESCROW_ACCOUNT
                    .parse()
                    .expect("valid default citizenship escrow account id"),
                min_bond_amount: 150,
                bond_escrow_account: iroha_config::parameters::defaults::governance::bond_escrow_account()
                    .parse()
                    .expect("valid default bond escrow account id"),
                slash_receiver_account: iroha_config::parameters::defaults::governance::slash_receiver_account()
                    .parse()
                    .expect("valid default slash receiver account id"),
                slash_double_vote_bps: 0,
                slash_invalid_proof_bps: 0,
                slash_ineligible_proof_bps: 0,
                debug_trace_pipeline: iroha_config::parameters::defaults::governance::DEBUG_TRACE_PIPELINE,
                jdg_signature_schemes: iroha_config::parameters::defaults::governance::jdg_signature_schemes()
                    .into_iter()
                    .map(|scheme| {
                        scheme
                            .parse::<iroha_data_model::jurisdiction::JdgSignatureScheme>()
                            .expect("valid default JDG signature scheme")
                    })
                    .collect(),
                runtime_upgrade_provenance:
                    iroha_config::parameters::actual::RuntimeUpgradeProvenancePolicy::default(),
                citizen_service: iroha_config::parameters::actual::CitizenServiceDiscipline::default(),
                viral_incentives: iroha_config::parameters::actual::ViralIncentives::default(),
                sorafs_pin_policy: iroha_config::parameters::actual::SorafsPinPolicyConstraints::default(),
                sorafs_pricing: PricingScheduleRecord::launch_default(),
                alias_teu_minimum: iroha_config::parameters::defaults::governance::alias_teu_minimum(),
                alias_frontier_telemetry: iroha_config::parameters::defaults::governance::alias_frontier_telemetry(),
                sorafs_penalty: iroha_config::parameters::actual::SorafsPenaltyPolicy::default(),
                sorafs_telemetry: iroha_config::parameters::actual::SorafsTelemetryPolicy::default(),
                sorafs_provider_owners: std::collections::BTreeMap::new(),
                conviction_step_blocks: 100,
                max_conviction: 6,
                min_enactment_delay: 20,
                window_span: 100,
                plain_voting_enabled: false,
                approval_threshold_q_num: 1,
                approval_threshold_q_den: 2,
                min_turnout: 0,
                parliament_committee_size:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_COMMITTEE_SIZE,
                parliament_term_blocks:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_TERM_BLOCKS,
                parliament_min_stake:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_MIN_STAKE,
                parliament_eligibility_asset_id: iroha_config::parameters::defaults::governance::PARLIAMENT_ELIGIBILITY_ASSET_ID
                    .parse()
                    .expect("valid default governance asset id"),
                parliament_alternate_size:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_ALTERNATE_SIZE,
                parliament_quorum_bps:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_QUORUM_BPS,
                rules_committee_size:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_RULES_COMMITTEE_SIZE,
                agenda_council_size:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_AGENDA_COUNCIL_SIZE,
                interest_panel_size:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_INTEREST_PANEL_SIZE,
                review_panel_size:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_REVIEW_PANEL_SIZE,
                policy_jury_size:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_POLICY_JURY_SIZE,
                oversight_committee_size:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_OVERSIGHT_COMMITTEE_SIZE,
                fma_committee_size:
                    iroha_config::parameters::defaults::governance::PARLIAMENT_FMA_COMMITTEE_SIZE,
                pipeline_study_sla_blocks:
                    iroha_config::parameters::defaults::governance::PIPELINE_STUDY_SLA_BLOCKS,
                pipeline_review_sla_blocks:
                    iroha_config::parameters::defaults::governance::PIPELINE_REVIEW_SLA_BLOCKS,
                pipeline_decision_sla_blocks:
                    iroha_config::parameters::defaults::governance::PIPELINE_DECISION_SLA_BLOCKS,
                pipeline_enactment_sla_blocks:
                    iroha_config::parameters::defaults::governance::PIPELINE_ENACTMENT_SLA_BLOCKS,
                pipeline_rules_sla_blocks:
                    iroha_config::parameters::defaults::governance::PIPELINE_RULES_SLA_BLOCKS,
                pipeline_agenda_sla_blocks:
                    iroha_config::parameters::defaults::governance::PIPELINE_AGENDA_SLA_BLOCKS,
            },
            nts: iroha_config::parameters::actual::Nts {
                sample_interval: iroha_config::parameters::defaults::time::NTS_SAMPLE_INTERVAL,
                sample_cap_per_round:
                    iroha_config::parameters::defaults::time::NTS_SAMPLE_CAP_PER_ROUND,
                max_rtt_ms: iroha_config::parameters::defaults::time::NTS_MAX_RTT_MS,
                trim_percent: iroha_config::parameters::defaults::time::NTS_TRIM_PERCENT,
                per_peer_buffer: iroha_config::parameters::defaults::time::NTS_PER_PEER_BUFFER,
                smoothing_enabled: iroha_config::parameters::defaults::time::NTS_SMOOTHING_ENABLED,
                smoothing_alpha: iroha_config::parameters::defaults::time::NTS_SMOOTHING_ALPHA,
                max_adjust_ms_per_min:
                    iroha_config::parameters::defaults::time::NTS_MAX_ADJUST_MS_PER_MIN,
                min_samples: iroha_config::parameters::defaults::time::NTS_MIN_SAMPLES,
                max_offset_ms: iroha_config::parameters::defaults::time::NTS_MAX_OFFSET_MS,
                max_confidence_ms: iroha_config::parameters::defaults::time::NTS_MAX_CONFIDENCE_MS,
                enforcement_mode: iroha_config::parameters::actual::NtsEnforcementMode::Warn,
            },
            accel: Acceleration {
                enable_simd: iroha_config::parameters::defaults::accel::ENABLE_SIMD,
                enable_cuda: false,
                enable_metal: false,
                max_gpus: None,
                merkle_min_leaves_gpu:
                    iroha_config::parameters::defaults::accel::MERKLE_MIN_LEAVES_GPU,
                merkle_min_leaves_metal: None,
                merkle_min_leaves_cuda: None,
                prefer_cpu_sha2_max_leaves_aarch64: None,
                prefer_cpu_sha2_max_leaves_x86: None,
            },
            ivm: Ivm::default(),
            concurrency: Concurrency::from_defaults(),
            confidential: Confidential {
                enabled: iroha_config::parameters::defaults::confidential::ENABLED,
                assume_valid: iroha_config::parameters::defaults::confidential::ASSUME_VALID,
                verifier_backend: iroha_config::parameters::defaults::confidential::VERIFIER_BACKEND
                    .to_string(),
                max_proof_size_bytes:
                    iroha_config::parameters::defaults::confidential::MAX_PROOF_SIZE_BYTES,
                max_nullifiers_per_tx:
                    iroha_config::parameters::defaults::confidential::MAX_NULLIFIERS_PER_TX,
                max_commitments_per_tx:
                    iroha_config::parameters::defaults::confidential::MAX_COMMITMENTS_PER_TX,
                max_confidential_ops_per_block: iroha_config::parameters::defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
                verify_timeout: iroha_config::parameters::defaults::confidential::VERIFY_TIMEOUT,
                max_anchor_age_blocks:
                    iroha_config::parameters::defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
                max_proof_bytes_block:
                    iroha_config::parameters::defaults::confidential::MAX_PROOF_BYTES_BLOCK,
                max_verify_calls_per_tx:
                    iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
                max_verify_calls_per_block:
                    iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
                max_public_inputs:
                    iroha_config::parameters::defaults::confidential::MAX_PUBLIC_INPUTS,
                reorg_depth_bound:
                    iroha_config::parameters::defaults::confidential::REORG_DEPTH_BOUND,
                policy_transition_delay_blocks:
                    iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
                policy_transition_window_blocks:
                    iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
                tree_roots_history_len:
                    iroha_config::parameters::defaults::confidential::TREE_ROOTS_HISTORY_LEN,
                tree_frontier_checkpoint_interval:
                    iroha_config::parameters::defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
                registry_max_vk_entries:
                    iroha_config::parameters::defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
                registry_max_params_entries:
                    iroha_config::parameters::defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
                registry_max_delta_per_block:
                    iroha_config::parameters::defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
                gas: iroha_config::parameters::actual::ConfidentialGas {
                    proof_base: iroha_config::parameters::defaults::confidential::gas::PROOF_BASE,
                    per_public_input:
                        iroha_config::parameters::defaults::confidential::gas::PER_PUBLIC_INPUT,
                    per_proof_byte:
                        iroha_config::parameters::defaults::confidential::gas::PER_PROOF_BYTE,
                    per_nullifier:
                        iroha_config::parameters::defaults::confidential::gas::PER_NULLIFIER,
                    per_commitment:
                        iroha_config::parameters::defaults::confidential::gas::PER_COMMITMENT,
                },
            },
            crypto: iroha_config::parameters::actual::Crypto::default(),
            settlement: Settlement::default(),
            streaming: Streaming {
                key_material: StreamingKeyMaterial::new(streaming_identity)
                    .expect("streaming key material"),
                session_store_dir: PathBuf::from(
                    iroha_config::parameters::defaults::streaming::SESSION_STORE_DIR,
                ),
                feature_bits: iroha_config::parameters::defaults::streaming::FEATURE_BITS,
                soranet: StreamingSoranet::from_defaults(),
                soravpn: iroha_config::parameters::actual::StreamingSoravpn::from_defaults(),
                sync: iroha_config::parameters::actual::StreamingSync::from_defaults(),
                codec: iroha_config::parameters::actual::StreamingCodec::from_defaults(),
            },
        }
    }

    #[tokio::test]
    async fn subscription_on_log_level_works() {
        const INIT_LOG_LEVEL: Level = Level::WARN;
        const NEW_LOG_LEVEL: Level = Level::DEBUG;
        const WATCH_LAG_MILLIS: u64 = 30;

        let mut config = test_config();
        config.logger.level = INIT_LOG_LEVEL;
        let (kiso, _) = KisoHandle::start(config);

        let mut recv = kiso
            .subscribe_on_logger_updates()
            .await
            .expect("Subscription should be fine");

        let _err = tokio::time::timeout(Duration::from_millis(WATCH_LAG_MILLIS), recv.changed())
            .await
            .expect_err("Watcher should not be active initially");

        kiso.update_with_dto(ConfigUpdateDTO {
            logger: LoggerDTO {
                level: NEW_LOG_LEVEL,
                filter: Some("trace,trace,trace".parse().unwrap()),
            },
            network_acl: None,
            network: None,
            confidential_gas: None,
            soranet_handshake: None,
            transport: None,
            compute_pricing: None,
        })
        .await
        .expect("Update should work fine");

        let () = tokio::time::timeout(Duration::from_millis(WATCH_LAG_MILLIS), recv.changed())
            .await
            .expect("Watcher should resolve within timeout")
            .expect("Watcher should not be closed");

        let value = recv.borrow_and_update().clone();
        assert_eq!(value.level, NEW_LOG_LEVEL);
        assert_eq!(format!("{}", value.filter.unwrap()), "trace,trace,trace");
    }

    #[tokio::test]
    async fn confidential_gas_update_applies_to_state() {
        crate::gas::configure_confidential_gas(crate::gas::ConfidentialGasSchedule::default());
        let config = test_config();
        let (kiso, _) = KisoHandle::start(config);

        let updated_gas = ConfidentialGasDTO {
            proof_base: 321_000,
            per_public_input: 9_999,
            per_proof_byte: 77,
            per_nullifier: 55,
            per_commitment: 44,
        };

        kiso.update_with_dto(ConfigUpdateDTO {
            logger: LoggerDTO {
                level: iroha_logger::Level::INFO,
                filter: None,
            },
            network_acl: None,
            network: None,
            confidential_gas: Some(updated_gas),
            soranet_handshake: None,
            transport: None,
            compute_pricing: None,
        })
        .await
        .expect("update should succeed");

        let dto = kiso.get_dto().await.expect("fetch updated dto");
        assert_eq!(dto.confidential_gas.proof_base, updated_gas.proof_base);
        assert_eq!(
            dto.confidential_gas.per_public_input,
            updated_gas.per_public_input
        );
        assert_eq!(
            dto.confidential_gas.per_proof_byte,
            updated_gas.per_proof_byte
        );
        assert_eq!(
            dto.confidential_gas.per_nullifier,
            updated_gas.per_nullifier
        );
        assert_eq!(
            dto.confidential_gas.per_commitment,
            updated_gas.per_commitment
        );

        crate::gas::configure_confidential_gas(crate::gas::ConfidentialGasSchedule::default());
    }

    #[tokio::test]
    async fn network_acl_updates_are_canonical_and_allow_clearing() {
        use iroha_logger::Level;

        const WATCH_LAG_MILLIS: u64 = 30;

        let initial_allow_keys = vec![KeyPair::random().public_key().clone()];
        let initial_deny_keys = vec![KeyPair::random().public_key().clone()];
        let initial_allow_cidrs = vec!["10.0.0.0/8".to_owned()];
        let initial_deny_cidrs = vec!["192.168.0.0/16".to_owned()];

        let mut config = test_config();
        config.network.allowlist_only = true;
        config.network.allow_keys.clone_from(&initial_allow_keys);
        config.network.deny_keys.clone_from(&initial_deny_keys);
        config.network.allow_cidrs.clone_from(&initial_allow_cidrs);
        config.network.deny_cidrs.clone_from(&initial_deny_cidrs);

        let (kiso, _) = KisoHandle::start(config);
        let mut recv = kiso
            .subscribe_on_network_acl_updates()
            .await
            .expect("subscription should succeed");

        let initial = recv.borrow().clone();
        assert_eq!(initial.allowlist_only, Some(true));
        assert_eq!(initial.allow_keys.clone().unwrap(), initial_allow_keys);
        assert_eq!(
            initial.deny_keys.clone().unwrap(),
            initial_deny_keys.clone()
        );
        assert_eq!(
            initial.allow_cidrs.clone().unwrap(),
            initial_allow_cidrs.clone()
        );
        assert_eq!(
            initial.deny_cidrs.clone().unwrap(),
            initial_deny_cidrs.clone()
        );

        let replacement_key = KeyPair::random().public_key().clone();
        kiso.update_with_dto(ConfigUpdateDTO {
            logger: LoggerDTO {
                level: Level::INFO,
                filter: None,
            },
            network_acl: Some(NetworkAcl {
                allowlist_only: Some(false),
                allow_keys: Some(vec![replacement_key.clone()]),
                deny_keys: None,
                allow_cidrs: None,
                deny_cidrs: None,
            }),
            network: None,
            confidential_gas: None,
            soranet_handshake: None,
            transport: None,
            compute_pricing: None,
        })
        .await
        .expect("update should succeed");

        tokio::time::timeout(Duration::from_millis(WATCH_LAG_MILLIS), recv.changed())
            .await
            .expect("watcher should resolve within timeout")
            .expect("watcher should remain active");
        let updated = recv.borrow_and_update().clone();
        assert_eq!(updated.allowlist_only, Some(false));
        assert_eq!(
            updated.allow_keys.clone().unwrap(),
            vec![replacement_key.clone()]
        );
        assert_eq!(updated.deny_keys.clone().unwrap(), initial_deny_keys);
        assert_eq!(updated.allow_cidrs.clone().unwrap(), initial_allow_cidrs);
        assert_eq!(updated.deny_cidrs.clone().unwrap(), initial_deny_cidrs);

        kiso.update_with_dto(ConfigUpdateDTO {
            logger: LoggerDTO {
                level: Level::INFO,
                filter: None,
            },
            network_acl: Some(NetworkAcl {
                allowlist_only: None,
                allow_keys: Some(Vec::new()),
                deny_keys: Some(Vec::new()),
                allow_cidrs: Some(Vec::new()),
                deny_cidrs: Some(Vec::new()),
            }),
            network: None,
            confidential_gas: None,
            soranet_handshake: None,
            transport: None,
            compute_pricing: None,
        })
        .await
        .expect("clearing update should succeed");

        tokio::time::timeout(Duration::from_millis(WATCH_LAG_MILLIS), recv.changed())
            .await
            .expect("watcher should resolve within timeout")
            .expect("watcher should remain active");
        let cleared = recv.borrow_and_update().clone();
        assert_eq!(cleared.allowlist_only, Some(false));
        assert!(cleared.allow_keys.as_ref().unwrap().is_empty());
        assert!(cleared.deny_keys.as_ref().unwrap().is_empty());
        assert!(cleared.allow_cidrs.as_ref().unwrap().is_empty());
        assert!(cleared.deny_cidrs.as_ref().unwrap().is_empty());
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn soranet_handshake_update_applies() {
        let config = test_config();
        let (kiso, _) = KisoHandle::start(config);

        let descriptor_hex = "0123456789abcdef".to_string();
        let resume_hex = "feedface".to_string();
        let descriptor_bytes = hex::decode(&descriptor_hex).expect("descriptor hex");
        let resume_bytes = hex::decode(&resume_hex).expect("resume hex");

        let mut handshake_rx = kiso
            .subscribe_on_soranet_handshake_updates()
            .await
            .expect("subscribe handshake watcher");

        kiso.update_with_dto(ConfigUpdateDTO {
            logger: LoggerDTO {
                level: Level::INFO,
                filter: None,
            },
            network_acl: None,
            network: None,
            confidential_gas: None,
            soranet_handshake: Some(SoranetHandshakeUpdate {
                descriptor_commit_hex: Some(descriptor_hex.clone()),
                client_capabilities_hex: None,
                relay_capabilities_hex: None,
                kem_id: Some(3),
                sig_id: Some(7),
                resume_hash_hex: Some(ResumeHashDirective::Set(resume_hex.clone())),
                pow: Some(SoranetHandshakePowUpdate {
                    required: Some(true),
                    difficulty: Some(6),
                    max_future_skew_secs: Some(1200),
                    min_ticket_ttl_secs: Some(90),
                    ticket_ttl_secs: Some(240),
                    signed_ticket_public_key_hex: None,
                    puzzle: Some(SoranetHandshakePuzzleUpdate {
                        enabled: Some(true),
                        memory_kib: Some(131_072),
                        time_cost: Some(3),
                        lanes: Some(2),
                    }),
                }),
            }),
            transport: None,
            compute_pricing: None,
        })
        .await
        .expect("handshake update should succeed");

        tokio::time::timeout(Duration::from_millis(30), handshake_rx.changed())
            .await
            .expect("handshake watcher should resolve within timeout")
            .expect("handshake watcher should remain active");
        let observed = handshake_rx.borrow_and_update().clone();
        assert_eq!(
            observed.descriptor_commit.value(),
            descriptor_bytes.as_slice()
        );
        assert_eq!(observed.kem_id, 3);
        assert_eq!(observed.sig_id, 7);
        let resume = observed
            .resume_hash
            .as_ref()
            .expect("resume hash present")
            .value();
        assert_eq!(resume, resume_bytes.as_slice());
        assert!(observed.pow.required);
        assert_eq!(observed.pow.difficulty, 6);
        assert_eq!(observed.pow.max_future_skew.as_secs(), 1200);
        assert_eq!(observed.pow.min_ticket_ttl.as_secs(), 90);
        assert_eq!(observed.pow.ticket_ttl.as_secs(), 240);
        let puzzle_cfg = observed.pow.puzzle.expect("puzzle config present");
        assert_eq!(puzzle_cfg.memory_kib.get(), 131_072);
        assert_eq!(puzzle_cfg.time_cost.get(), 3);
        assert_eq!(puzzle_cfg.lanes.get(), 2);

        let dto = kiso.get_dto().await.expect("fetch handshake dto");
        let handshake = dto.network.soranet_handshake;
        assert_eq!(handshake.descriptor_commit_hex, descriptor_hex);
        assert_eq!(handshake.kem_id, 3);
        assert_eq!(handshake.sig_id, 7);
        assert_eq!(handshake.resume_hash_hex, Some(resume_hex.clone()));
        assert!(handshake.pow.required);
        assert_eq!(handshake.pow.difficulty, 6);
        assert_eq!(handshake.pow.max_future_skew_secs, 1200);
        assert_eq!(handshake.pow.min_ticket_ttl_secs, 90);
        assert_eq!(handshake.pow.ticket_ttl_secs, 240);
        let puzzle = handshake.pow.puzzle.expect("puzzle summary present");
        assert_eq!(puzzle.memory_kib, 131_072);
        assert_eq!(puzzle.time_cost, 3);
        assert_eq!(puzzle.lanes, 2);

        // Clear resume hash without touching other fields.
        kiso.update_with_dto(ConfigUpdateDTO {
            logger: LoggerDTO {
                level: Level::INFO,
                filter: None,
            },
            network_acl: None,
            network: None,
            confidential_gas: None,
            soranet_handshake: Some(SoranetHandshakeUpdate {
                descriptor_commit_hex: None,
                client_capabilities_hex: None,
                relay_capabilities_hex: None,
                kem_id: None,
                sig_id: None,
                resume_hash_hex: Some(ResumeHashDirective::Clear),
                pow: None,
            }),
            transport: None,
            compute_pricing: None,
        })
        .await
        .expect("resume hash clear should succeed");

        let dto = kiso.get_dto().await.expect("fetch updated dto");
        assert_eq!(dto.network.soranet_handshake.resume_hash_hex, None);

        let dto_default = kiso
            .get_dto()
            .await
            .expect("fetch dto before puzzle update");
        assert!(
            dto_default.network.soranet_handshake.pow.puzzle.is_some(),
            "puzzle gate should be enabled by default"
        );

        // Disable the puzzle gate.
        kiso.update_with_dto(ConfigUpdateDTO {
            logger: LoggerDTO {
                level: Level::INFO,
                filter: None,
            },
            network_acl: None,
            network: None,
            confidential_gas: None,
            soranet_handshake: Some(SoranetHandshakeUpdate {
                descriptor_commit_hex: None,
                client_capabilities_hex: None,
                relay_capabilities_hex: None,
                kem_id: None,
                sig_id: None,
                resume_hash_hex: None,
                pow: Some(SoranetHandshakePowUpdate {
                    required: None,
                    difficulty: None,
                    max_future_skew_secs: None,
                    min_ticket_ttl_secs: None,
                    ticket_ttl_secs: None,
                    signed_ticket_public_key_hex: None,
                    puzzle: Some(SoranetHandshakePuzzleUpdate {
                        enabled: Some(false),
                        memory_kib: None,
                        time_cost: None,
                        lanes: None,
                    }),
                }),
            }),
            transport: None,
            compute_pricing: None,
        })
        .await
        .expect("puzzle disable should succeed");

        let dto = kiso.get_dto().await.expect("fetch puzzle-disabled dto");
        assert!(dto.network.soranet_handshake.pow.puzzle.is_none());
    }

    #[tokio::test]
    async fn soranet_handshake_watch_updates_without_subscribers() {
        let (kiso, _) = KisoHandle::start(test_config());

        let updated_pow = SoranetHandshakePowUpdate {
            required: Some(true),
            difficulty: Some(9),
            max_future_skew_secs: Some(30),
            min_ticket_ttl_secs: Some(15),
            ticket_ttl_secs: Some(45),
            signed_ticket_public_key_hex: None,
            puzzle: Some(SoranetHandshakePuzzleUpdate {
                enabled: Some(true),
                memory_kib: Some(32 * 1024),
                time_cost: Some(1),
                lanes: Some(2),
            }),
        };

        // Apply the update before any watchers are subscribed.
        kiso.update_with_dto(ConfigUpdateDTO {
            logger: LoggerDTO {
                level: Level::INFO,
                filter: None,
            },
            network_acl: None,
            network: None,
            confidential_gas: None,
            soranet_handshake: Some(SoranetHandshakeUpdate {
                descriptor_commit_hex: None,
                client_capabilities_hex: None,
                relay_capabilities_hex: None,
                kem_id: None,
                sig_id: None,
                resume_hash_hex: None,
                pow: Some(updated_pow),
            }),
            transport: None,
            compute_pricing: None,
        })
        .await
        .expect("handshake update should succeed without subscribers");

        let rx = kiso
            .subscribe_on_soranet_handshake_updates()
            .await
            .expect("subscribe to handshake updates");
        let snapshot = rx.borrow().clone();

        assert_eq!(snapshot.pow.difficulty, 9);
        assert_eq!(snapshot.pow.ticket_ttl.as_secs(), 45);
        assert!(snapshot.pow.puzzle.is_some());
    }

    #[tokio::test]
    async fn soranet_sm_policy_update_applies() {
        let config = test_config();
        let (kiso, _) = KisoHandle::start(config);

        kiso.update_with_dto(ConfigUpdateDTO {
            logger: LoggerDTO {
                level: Level::INFO,
                filter: None,
            },
            network_acl: None,
            network: Some(NetworkUpdate {
                lane_profile: None,
                require_sm_handshake_match: Some(false),
                require_sm_openssl_preview_match: Some(false),
            }),
            confidential_gas: None,
            soranet_handshake: None,
            transport: None,
            compute_pricing: None,
        })
        .await
        .expect("SM toggle update should succeed");

        let dto = kiso.get_dto().await.expect("fetch updated dto");
        assert!(!dto.network.require_sm_handshake_match);
        assert!(!dto.network.require_sm_openssl_preview_match);
    }

    #[test]
    fn compute_pricing_updates_enforce_delta_bounds() {
        let config = test_config();
        let (logger_tx, _) = watch::channel(config.logger.clone());
        let (network_acl_tx, _) = watch::channel(Actor::snapshot_network_acl(&config));
        let (handshake_tx, _) = watch::channel(config.network.soranet_handshake.clone());
        let (confidential_gas_tx, _) = watch::channel(config.confidential.gas);
        let (_, handle_rx) = mpsc::channel(DEFAULT_CHANNEL_SIZE);
        let mut actor = Actor {
            handle: handle_rx,
            state: config,
            logger_update: logger_tx,
            network_acl_update: network_acl_tx,
            soranet_handshake_update: handshake_tx,
            confidential_gas_update: confidential_gas_tx,
        };

        let family = defaults::compute::default_price_family();
        let mut invalid = actor
            .state
            .compute
            .price_families
            .get(&family)
            .cloned()
            .expect("default price family present");
        invalid.cycles_per_unit =
            NonZeroU64::new(invalid.cycles_per_unit.get().saturating_mul(2)).expect("non-zero");

        let err = actor
            .apply_config_update(ConfigUpdateDTO {
                logger: LoggerDTO {
                    level: Level::INFO,
                    filter: None,
                },
                network_acl: None,
                network: None,
                confidential_gas: None,
                soranet_handshake: None,
                transport: None,
                compute_pricing: Some(ComputePricingUpdate {
                    price_families: [(family.clone(), invalid)].into_iter().collect(),
                    default_price_family: None,
                }),
            })
            .expect_err("delta beyond bounds should be rejected");
        assert!(matches!(err, Error::Validation(msg) if msg.contains("delta")));

        let mut ok = actor
            .state
            .compute
            .price_families
            .get(&family)
            .cloned()
            .expect("default price family present");
        ok.cycles_per_unit = NonZeroU64::new(
            ok.cycles_per_unit
                .get()
                .saturating_add(ok.cycles_per_unit.get() / 10),
        )
        .expect("non-zero");

        actor
            .apply_config_update(ConfigUpdateDTO {
                logger: LoggerDTO {
                    level: Level::INFO,
                    filter: None,
                },
                network_acl: None,
                network: None,
                confidential_gas: None,
                soranet_handshake: None,
                transport: None,
                compute_pricing: Some(ComputePricingUpdate {
                    price_families: [(family.clone(), ok.clone())].into_iter().collect(),
                    default_price_family: None,
                }),
            })
            .expect("delta within bounds should apply");
        assert_eq!(actor.state.compute.price_families.get(&family), Some(&ok));
    }
}
