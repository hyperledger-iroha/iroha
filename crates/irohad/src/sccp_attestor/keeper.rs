//! In-node inbound light-client keeper (`specs/sccp.md` §4.13.4).
//!
//! Every validator with an active or pending bridge key keeps Taira's inbound light clients
//! fresh: when a light client's head is older than `advance_after` (default `ws_bound_ms / 4`),
//! the keeper builds an advance from the configured (or compiled public) endpoints and submits
//! it fee-exempt from its bridge key's account. A built advance larger than `max_advance_bytes`
//! is dropped with a warning instead of being submitted. Several keepers are harmless: the
//! advance carries the expected state hash, so only the first one moves the head and the rest
//! fail admission instead of paying.
//!
//! Ethereum, BSC and TRON advances use HTTP endpoints, TON advances ADNL liteservers.

use std::time::Instant;

use iroha_config::parameters::actual::SccpLightClientKeeper;
use iroha_data_model::{
    bridge::SccpNetworkV1,
    isi::sccp::AdvanceSccpLightClientV1,
    sccp::light_client::{SccpLcAdvanceBytesV1, SccpLightClientV1},
};
use iroha_sccp_rpc::{
    BeaconClient, EvmClient, HttpEndpointKind, HttpTransport, TronClient,
    builders::{bsc::BscBuilder, ethereum::EthereumBuilder, ton::TonBuilder, tron::TronBuilder},
    ton::LiteClient,
};

/// The keeper's builders and cadence.
pub(super) struct Keeper {
    config: SccpLightClientKeeper,
    ethereum: Option<EthereumBuilder>,
    bsc: Option<BscBuilder>,
    tron: Option<TronBuilder>,
    ton: Option<TonBuilder>,
    last_poll: Option<Instant>,
}

impl Keeper {
    /// Build the keeper from `config` on a blocking worker of the current Tokio runtime.
    ///
    /// The HTTP transports own blocking `reqwest` clients. Building one starts the client's
    /// internal runtime thread and blocks until it runs, which must never happen on an async
    /// worker thread: debug builds panic there and release builds stall the worker. A panic
    /// while building leaves the keeper idle, because the node never aborts over SCCP.
    pub(super) async fn build(config: SccpLightClientKeeper) -> Self {
        let idle = SccpLightClientKeeper {
            enabled: false,
            ..config.clone()
        };
        match crate::panic_recovery::join_recoverable(
            crate::panic_recovery::spawn_blocking_recoverable(move || Self::new(config)),
        )
        .await
        {
            Ok(keeper) => keeper,
            Err(_panic) => {
                iroha_logger::error!(
                    "SCCP keeper: building the endpoint clients panicked; the keeper stays idle"
                );
                Self::new(idle)
            }
        }
    }

    /// Build the keeper from `config`; a disabled keeper or unusable endpoints leave it idle.
    ///
    /// Blocks while the HTTP clients start, so async callers use [`Self::build`].
    fn new(config: SccpLightClientKeeper) -> Self {
        let seed = u64::from(std::process::id());
        let ethereum = if config.enabled {
            let beacon =
                HttpTransport::from_keeper_config(&config, HttpEndpointKind::EthereumBeacon, seed);
            let execution = HttpTransport::from_keeper_config(
                &config,
                HttpEndpointKind::EthereumExecution,
                seed,
            );
            match (beacon, execution) {
                (Ok(beacon), Ok(execution)) => Some(EthereumBuilder::new(
                    BeaconClient::new(beacon),
                    EvmClient::new(execution),
                )),
                (Err(error), _) | (_, Err(error)) => {
                    iroha_logger::warn!(%error, "SCCP keeper: Ethereum endpoints are unusable");
                    None
                }
            }
        } else {
            None
        };
        let bsc = if config.enabled {
            match HttpTransport::from_keeper_config(&config, HttpEndpointKind::Bsc, seed) {
                Ok(transport) => Some(BscBuilder::new(EvmClient::new(transport))),
                Err(error) => {
                    iroha_logger::warn!(%error, "SCCP keeper: BSC endpoints are unusable");
                    None
                }
            }
        } else {
            None
        };
        let tron = if config.enabled {
            match HttpTransport::from_keeper_config(&config, HttpEndpointKind::Tron, seed) {
                Ok(transport) => Some(TronBuilder::new(TronClient::new(transport))),
                Err(error) => {
                    iroha_logger::warn!(%error, "SCCP keeper: TRON endpoints are unusable");
                    None
                }
            }
        } else {
            None
        };
        let ton = if config.enabled {
            match LiteClient::from_keeper_config(&config, seed) {
                Ok(lite) => Some(TonBuilder::new(lite)),
                Err(error) => {
                    iroha_logger::warn!(%error, "SCCP keeper: TON liteservers are unusable");
                    None
                }
            }
        } else {
            None
        };
        Self {
            config,
            ethereum,
            bsc,
            tron,
            ton,
            last_poll: None,
        }
    }

    /// Whether a poll is due now (at most once per `poll_interval`).
    pub(super) fn due(&mut self) -> bool {
        if self.ethereum.is_none()
            && self.bsc.is_none()
            && self.tron.is_none()
            && self.ton.is_none()
        {
            return false;
        }
        let now = Instant::now();
        if self
            .last_poll
            .is_some_and(|last| now.duration_since(last) < self.config.poll_interval)
        {
            return false;
        }
        self.last_poll = Some(now);
        true
    }

    /// Build the advances of every stale light client this keeper can serve at Taira time
    /// `now_ms`.
    pub(super) fn advances(
        &self,
        light_clients: &[SccpLightClientV1],
        now_ms: u64,
    ) -> Vec<AdvanceSccpLightClientV1> {
        light_clients
            .iter()
            .filter(|light_client| is_stale(&self.config, light_client, now_ms))
            .filter_map(|light_client| {
                let network = light_client.params.network;
                let max_updates =
                    usize::try_from(light_client.params.max_updates_per_advance).unwrap_or(1);
                let latest = light_client.head.latest_set_id;
                let built = match network {
                    SccpNetworkV1::EthereumMainnet => {
                        self.ethereum.as_ref()?.advance(latest, max_updates)
                    }
                    SccpNetworkV1::BscMainnet => self.bsc.as_ref()?.advance(latest, max_updates),
                    SccpNetworkV1::TronMainnet => self.tron.as_ref()?.advance(latest, max_updates),
                    SccpNetworkV1::TonMainnet => self.ton.as_ref()?.advance(latest, max_updates),
                    SccpNetworkV1::SoraTaira => return None,
                };
                match built {
                    Ok(advance) if fits_advance_bound(&self.config, network, &advance) => {
                        Some(AdvanceSccpLightClientV1 {
                            network,
                            expected_state_hash: Some(light_client.state_hash),
                            advance,
                        })
                    }
                    Ok(_) => None,
                    Err(error) => {
                        iroha_logger::warn!(
                            %error,
                            network = network.profile_key(),
                            "SCCP keeper: building an advance failed"
                        );
                        None
                    }
                }
            })
            .collect()
    }
}

/// Whether a built `advance` fits the configured `max_advance_bytes`. An oversized advance is
/// dropped with a warning instead of being submitted.
fn fits_advance_bound(
    config: &SccpLightClientKeeper,
    network: SccpNetworkV1,
    advance: &SccpLcAdvanceBytesV1,
) -> bool {
    let max_advance_bytes = config.max_advance_bytes.get();
    let fits = advance.len() <= max_advance_bytes;
    if !fits {
        iroha_logger::warn!(
            network = network.profile_key(),
            advance_bytes = advance.len(),
            max_advance_bytes,
            "SCCP keeper: dropping an advance larger than max_advance_bytes"
        );
    }
    fits
}

/// How often the keeper looks for a new TON key block: TON light clients have no
/// `ws_bound_ms` (their freshness comes from the epoch's own validity).
const TON_ADVANCE_AFTER_MS: u64 = 3_600_000;

/// Whether `light_client` is unfrozen and its head is older than the keeper's staleness bound.
fn is_stale(config: &SccpLightClientKeeper, light_client: &SccpLightClientV1, now_ms: u64) -> bool {
    let after_ms = if light_client.params.ws_bound_ms == 0 {
        TON_ADVANCE_AFTER_MS
    } else {
        let after = config.advance_after_for(light_client.params.ws_bound_ms);
        u64::try_from(after.as_millis()).unwrap_or(u64::MAX)
    };
    !light_client.is_frozen()
        && now_ms.saturating_sub(light_client.head.last_progress_taira_ms) >= after_ms
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::sccp::light_client::{
        SccpLcHeadV1, SccpLcPointV1, SccpLightClientParamsV1,
    };
    use std::{num::NonZeroUsize, time::Duration};

    fn light_client(last_progress_taira_ms: u64) -> SccpLightClientV1 {
        SccpLightClientV1 {
            params: SccpLightClientParamsV1::defaults_for(SccpNetworkV1::EthereumMainnet)
                .expect("ethereum defaults"),
            head: SccpLcHeadV1 {
                latest_set_id: 1,
                latest_finalized: SccpLcPointV1 {
                    source_height: 1,
                    block_hash: [1; 32],
                    source_time_ms: 1,
                },
                last_progress_taira_ms,
            },
            frozen: None,
            state_hash: [0; 32],
        }
    }

    #[test]
    fn staleness_follows_the_configured_or_derived_bound() {
        let mut config = SccpLightClientKeeper::default();
        config.advance_after = Some(Duration::from_secs(10));
        assert!(!is_stale(&config, &light_client(5_000), 14_000));
        assert!(is_stale(&config, &light_client(5_000), 15_000));
        config.advance_after = None;
        let derived = light_client(0).params.ws_bound_ms / 4;
        assert!(!is_stale(&config, &light_client(0), derived - 1));
        assert!(is_stale(&config, &light_client(0), derived));
    }

    #[test]
    fn ton_light_clients_are_polled_hourly() {
        let config = SccpLightClientKeeper::default();
        let mut ton = light_client(0);
        ton.params =
            SccpLightClientParamsV1::defaults_for(SccpNetworkV1::TonMainnet).expect("ton defaults");
        assert!(!is_stale(&config, &ton, TON_ADVANCE_AFTER_MS - 1));
        assert!(is_stale(&config, &ton, TON_ADVANCE_AFTER_MS));
    }

    #[test]
    fn advances_above_max_advance_bytes_are_dropped() {
        let mut config = SccpLightClientKeeper::default();
        config.max_advance_bytes = NonZeroUsize::new(4).expect("nonzero bound");
        let advance = |len: usize| SccpLcAdvanceBytesV1::new(vec![0; len]).expect("advance bytes");
        let network = SccpNetworkV1::EthereumMainnet;
        assert!(fits_advance_bound(&config, network, &advance(1)));
        assert!(fits_advance_bound(&config, network, &advance(4)));
        assert!(!fits_advance_bound(&config, network, &advance(5)));
        let default = SccpLightClientKeeper::default();
        let bound = default.max_advance_bytes.get();
        assert!(fits_advance_bound(&default, network, &advance(bound)));
        assert!(!fits_advance_bound(&default, network, &advance(bound + 1)));
    }

    #[test]
    fn a_disabled_keeper_never_polls() {
        let mut config = SccpLightClientKeeper::default();
        config.enabled = false;
        let mut keeper = Keeper::new(config);
        assert!(!keeper.due());
        assert!(keeper.advances(&[light_client(0)], u64::MAX).is_empty());
    }

    /// Every chain of the default configuration has a builder, and the first poll is due.
    fn assert_default_keeper_serves_every_chain(keeper: &mut Keeper) {
        assert!(keeper.ethereum.is_some(), "Ethereum builder");
        assert!(keeper.bsc.is_some(), "BSC builder");
        assert!(keeper.tron.is_some(), "TRON builder");
        assert!(keeper.ton.is_some(), "TON builder");
        assert!(keeper.due());
        assert!(!keeper.due(), "a poll is due at most once per poll_interval");
    }

    #[tokio::test]
    async fn the_default_keeper_builds_inside_a_current_thread_runtime() {
        let mut keeper = Keeper::build(SccpLightClientKeeper::default()).await;
        assert_default_keeper_serves_every_chain(&mut keeper);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_default_keeper_builds_inside_a_multi_thread_runtime() {
        let mut keeper = Keeper::build(SccpLightClientKeeper::default()).await;
        assert_default_keeper_serves_every_chain(&mut keeper);
        // The keeper, like the attestor that owns it, is dropped on an async worker.
        drop(keeper);
    }

    #[tokio::test]
    async fn a_disabled_keeper_builds_idle_inside_a_runtime() {
        let mut config = SccpLightClientKeeper::default();
        config.enabled = false;
        let mut keeper = Keeper::build(config).await;
        assert!(keeper.ethereum.is_none() && keeper.bsc.is_none());
        assert!(keeper.tron.is_none() && keeper.ton.is_none());
        assert!(!keeper.due());
    }
}
