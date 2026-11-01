//! Native destination-side delivery for the bounded independent-root game prototype.
//!
//! Committed State is the only queue. Registered validator endpoints are untrusted
//! discovery transports; core verifies the original certified export before any signature.
//! There is no relay account, custody file or workflow journal. The current validator key
//! pays ordinary network fees from its pre-funded account; no exemption is introduced.

use eyre::{Result, WrapErr as _, ensure};
use iroha_core::{
    executor::quote_nexus_fee_admission_draft,
    queue::Queue,
    smartcontracts::isi::game_partition::validate_game_partition_import,
    state::{State, StateReadOnly, WorldReadOnly},
    sumeragi::schedule,
};
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    game_partition::{GamePartitionStateV1, GameUnitImportProofV1},
    isi::{InstructionBox, game_partition::ImportGameUnitV1},
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_model_base::peer::PeerId;
use mv::storage::StorageReadOnly as _;
use std::{sync::Arc, time::Duration};

const POLL: Duration = Duration::from_secs(2);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const RETRY_AFTER: Duration = Duration::from_secs(30);

/// Start a native, dormant-until-installed destination delivery task.
pub fn start(
    state: Arc<State>,
    queue: Arc<Queue>,
    key: KeyPair,
    shutdown: ShutdownSignal,
) -> Child {
    let task = tokio::spawn(async move {
        let http = match reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
        {
            Ok(http) => http,
            Err(error) => {
                iroha_logger::error!(%error, "native game delivery HTTP initialization failed");
                return;
            }
        };
        let mut interval = tokio::time::interval(POLL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut cursor = 0_usize;
        let mut pending: Option<(Hash, Hash, std::time::Instant)> = None;
        loop {
            tokio::select! {
                () = shutdown.receive() => return,
                _ = interval.tick() => {},
            }
            // Never retain World locks over network IO. Rotation is a local fairness hint,
            // not delivery authority; restart reconstructs every candidate from State.
            let candidates = {
                let view = match state.try_view_once() {
                    Ok(view) => view,
                    Err(error) => {
                        iroha_logger::debug!(%error, "native game discovery snapshot deferred");
                        continue;
                    }
                };
                let next = u64::try_from(view.height())
                    .unwrap_or(u64::MAX)
                    .saturating_add(1);
                let peer = PeerId::new(key.public_key().clone());
                let Ok(committee) = schedule::scheduled_committee(view.world(), next) else {
                    continue;
                };
                if !committee.contains(&peer) {
                    continue;
                }
                view.world()
                    .game_partitions()
                    .iter()
                    .flat_map(|(_, world)| {
                        world.peers.iter().flat_map(move |peer| {
                            peer.endpoints.iter().map(move |endpoint| {
                                (world.clone(), peer.tracker.instance, endpoint.clone())
                            })
                        })
                    })
                    .collect::<Vec<_>>()
            };
            if candidates.is_empty() {
                continue;
            }
            let (local, remote_instance, endpoint) = &candidates[cursor % candidates.len()];
            cursor = cursor.wrapping_add(1);
            if let Some((world_id, receipt, since)) = pending {
                let imported = {
                    let view = match state.try_view_once() {
                        Ok(view) => view,
                        Err(error) => {
                            iroha_logger::debug!(%error, "native game pending snapshot deferred");
                            continue;
                        }
                    };
                    view.world()
                        .game_partitions()
                        .get(&world_id)
                        .is_some_and(|world| world.imported.contains(&receipt))
                };
                if imported || since.elapsed() >= RETRY_AFTER {
                    pending = None;
                } else {
                    continue;
                }
            }
            let result = async {
                let remote: GamePartitionStateV1 = get_json(
                    &http,
                    endpoint,
                    &format!("v1/game-partitions/{}/state", local.world_id),
                )
                .await?;
                remote.validate().map_err(eyre::Report::msg)?;
                ensure!(
                    remote.world_id == local.world_id
                        && remote.kernel_hash == local.kernel_hash
                        && remote.local_instance == *remote_instance,
                    "remote discovery world/kernel mismatch"
                );
                for record in &remote.exports {
                    let receipt = record.receipt_id();
                    if record.destination != local.local_instance
                        || local.imported.contains(&receipt)
                    {
                        continue;
                    }
                    let proof: GameUnitImportProofV1 = get_json(
                        &http,
                        endpoint,
                        &format!(
                            "v1/game-partitions/{}/exports/{receipt}/proof",
                            local.world_id
                        ),
                    )
                    .await?;
                    ensure!(
                        proof.record == *record,
                        "receipt differs from discovery; retry a consistent view"
                    );
                    // Proof checking and ordinary ingress can take CPU or admission locks.
                    // Never run those paths on a Tokio reactor, and retain no StateView
                    // while awaiting them or while entering the queue.
                    let state = Arc::clone(&state);
                    let queue = Arc::clone(&queue);
                    let key = key.clone();
                    let submitted = tokio::task::spawn_blocking(move || {
                        let valid = {
                            let view = state.try_view_once().map_err(|error| {
                                eyre::eyre!("game proof snapshot deferred: {error}")
                            })?;
                            validate_game_partition_import(&view, &proof)
                                .map_err(eyre::Report::msg)?
                        };
                        if !valid {
                            return Ok::<_, eyre::Report>(false);
                        }
                        submit(&state, &queue, &key, proof)?;
                        Ok(true)
                    })
                    .await
                    .wrap_err("native game import worker failed")??;
                    if !submitted {
                        // One proof fetch per poll, even if a concurrent validator
                        // imported it after the discovery snapshot was sampled.
                        return Ok(None);
                    }
                    return Ok::<_, eyre::Report>(Some(receipt));
                }
                Ok(None)
            };
            let result = tokio::select! {
                () = shutdown.receive() => return,
                result = result => result,
            };
            match result {
                Ok(Some(receipt)) => {
                    pending = Some((local.world_id, receipt, std::time::Instant::now()));
                    iroha_logger::info!(%receipt, "native game export import submitted");
                }
                Ok(None) => {}
                Err(error) => {
                    iroha_logger::debug!(error = %format_args!("{error:#}"), world = %local.world_id, %endpoint, "native game delivery deferred")
                }
            }
        }
    });
    Child::new(task, OnShutdown::Wait(Duration::from_secs(1)))
}

async fn get_json<T: norito::json::JsonDeserializeOwned>(
    http: &reqwest::Client,
    endpoint: &str,
    path: &str,
) -> Result<T> {
    let mut base = reqwest::Url::parse(endpoint)?;
    ensure!(
        matches!(base.scheme(), "http" | "https")
            && base.username().is_empty()
            && base.password().is_none(),
        "game peer endpoint must be an uncredentialed HTTP(S) validator URL"
    );
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    let url = base.join(path)?;
    let mut response = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?;
    ensure!(
        response
            .content_length()
            .is_none_or(|n| n <= RESPONSE_BYTES as u64),
        "game response too large"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes
                .len()
                .checked_add(chunk.len())
                .is_some_and(|n| n <= RESPONSE_BYTES),
            "game response too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    norito::json::from_slice(&bytes).wrap_err("decode bounded game response")
}

fn submit(state: &State, queue: &Queue, key: &KeyPair, proof: GameUnitImportProofV1) -> Result<()> {
    // This is ordinary ingress. It gets no SCCP local-keeper or pairing-budget bypass.
    let authority = AccountId::new(key.public_key().clone());
    let mut payload = TransactionBuilder::new(
        *state.network_id_ref(),
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(ImportGameUnitV1 { proof })])
    .into_payload()?;
    payload.time_to_live_ms = std::num::NonZeroU64::new(RETRY_AFTER.as_millis().try_into()?);
    let route = queue.route_payload_plan_with_state(&payload, state)?;
    let iroha_core::queue::RoutingPlan::Single(route) = route else {
        eyre::bail!("game import requires one root");
    };
    let view = state
        .try_view_once()
        .map_err(|error| eyre::eyre!("game fee snapshot deferred: {error}"))?;
    let time = iroha_core::smartcontracts::isi::sccp::hold::committed_time_ms(&view)
        .map_err(|e| eyre::eyre!("invalid committed fee time: {e:?}"))?;
    let next = u64::try_from(view.height())?
        .checked_add(1)
        .ok_or_else(|| eyre::eyre!("height overflow"))?;
    let quote = quote_nexus_fee_admission_draft(
        view.world(),
        view.nexus(),
        view.pipeline(),
        &view.zk().gas,
        &payload,
        time.unwrap_or(0),
        time,
        next,
        Some(route.route.dataspace_id),
    )
    .map_err(|e| eyre::eyre!("native game import fee quote failed: {e:?}"))?;
    drop(view);
    payload.fee_payment = quote.recommended_intent;
    let signed = TransactionBuilder::from_payload(payload)?
        .try_sign(key.private_key())
        .map_err(|e| eyre::eyre!("native game import signing failed: {e:?}"))?;
    let accepted = crate::accept_internal_transaction(
        signed,
        state.network_id_ref(),
        &state.transaction_admission_parameters(),
    )?;
    queue
        .push_with_lane_with_state(accepted, state)
        .map(|_| ())
        .map_err(|e| eyre::eyre!("native game import admission failed: {}", e.err))
}
