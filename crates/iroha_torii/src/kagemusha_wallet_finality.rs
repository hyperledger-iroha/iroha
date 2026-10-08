//! Bounded server-only terminal Load proof worker over real committed native history.
//!
//! HTTP requests select no proving inputs or acceptance verdicts. The authenticated payer's
//! committed receipt selects the job; the worker independently reacquires that receipt and
//! its actual native event. Only the full installed finality graph produces output bytes.

use std::{
    collections::BTreeMap,
    num::{NonZeroU16, NonZeroU64},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use iroha_allocation::AllocationBudget;
use iroha_config::parameters::actual::KagemushaLoadFinality;
use iroha_core::{
    kagemusha_wallet_v1::CommittedLoadReceipts,
    state::{State as CoreState, StateReadOnly as _},
    sumeragi::finality::{self, NativeFinalityCursorV1},
};
use iroha_core_zk::{
    kagemusha_wallet_artifacts_v1::{
        InstallationV1, VERIFIER_PACK_MAX_BYTES_V1, producer_inventory::CATALOG_MAX_BYTES_V1,
    },
    kagemusha_wallet_finality_v1::server::{
        ServerFinalityCancellationV1, ServerFinalityLimitsV1, ServerFinalityStorageV1,
        ServerFinalityV1,
    },
};
use iroha_data_model::{
    account::AccountId, block::decode_framed_signed_block,
    isi::kagemusha_wallet::KagemushaWalletLoadReceiptV1,
    kagemusha::KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1,
    sumeragi_finality::SumeragiFinalityVerifier,
};

const RECORD_BYTES: usize = 16 << 10;
type Result<T> = std::result::Result<T, &'static str>;
type Key = [u8; 32];

struct Job {
    payer: AccountId,
    receipt: KagemushaWalletLoadReceiptV1,
    key: Key,
}
enum Status {
    Pending,
    Ready(Vec<u8>),
    Failed,
}
struct Queue {
    entries: BTreeMap<Key, Status>,
    maximum: usize,
}
impl Queue {
    fn observe(&mut self, key: Key) -> Result<Option<Option<Vec<u8>>>> {
        match self.entries.get(&key) {
            Some(Status::Pending) => Ok(Some(None)),
            Some(Status::Ready(_)) => match self.entries.remove(&key) {
                Some(Status::Ready(bytes)) => Ok(Some(Some(bytes))),
                _ => Err("proof result custody changed"),
            },
            Some(Status::Failed) => {
                self.entries.remove(&key);
                Err("previous proof attempt unavailable")
            }
            None if self.entries.len() >= self.maximum => Err("proof worker capacity unavailable"),
            None => {
                self.entries.insert(key, Status::Pending);
                Ok(None)
            }
        }
    }
}

/// One optional configured source owner, one proving worker and a finite request queue.
/// Completed output stays bounded until read. Every subsequent retry reacquires and
/// re-verifies the durable terminal original; no unbounded response cache is maintained.
pub(super) struct FinalityService {
    scheme: [u8; 32],
    maximum_height: u64,
    worker: Arc<worker::Worker>,
}
mod worker;
impl FinalityService {
    pub(super) fn open(
        state: Arc<CoreState>,
        config: KagemushaLoadFinality,
    ) -> std::io::Result<Self> {
        if !(1..=64).contains(&config.max_pending_requests) {
            return Err(std::io::Error::other("invalid finality queue bound"));
        }
        iroha_fs::PrivateDirectory::open_exact(&config.verifier_originals)?;
        iroha_fs::PrivateDirectory::open_exact(&config.proving_cache)?;
        iroha_fs::PrivateDirectory::open_exact(&config.journal_dir)?;
        iroha_fs::SelectedRegularFile::capture(&config.verifier_pack)?;
        iroha_fs::SelectedRegularFile::capture(&config.producer_inventory)?;
        let cancel = ServerFinalityCancellationV1::default();
        let worker_cancel = cancel.clone();
        let scheme = config.scheme_id;
        let maximum_height = config.maximum_receipt_height;
        let worker = worker::Worker::start_with(config.max_pending_requests, cancel, move || {
            let mut producer = None;
            move |job: &Job, stop: &AtomicBool| {
                if producer.is_none() {
                    producer = Some(mount(&state, &config, worker_cancel.clone(), stop)?);
                }
                prove(
                    &state,
                    &config,
                    producer.as_mut().ok_or("producer absent")?,
                    job,
                    stop,
                )
            }
        })?;
        Ok(Self {
            scheme,
            maximum_height,
            worker: Arc::new(worker),
        })
    }

    /// Receipt must have just been obtained from CommittedLoadReceipts for this authenticated
    /// payer. Worker reacquisition independently enforces that ownership before any proving.
    pub(super) fn read_or_schedule(
        &self,
        payer: AccountId,
        receipt: KagemushaWalletLoadReceiptV1,
    ) -> Result<Option<Vec<u8>>> {
        if receipt.scheme_id != self.scheme
            || !(2..=self.maximum_height).contains(&receipt.block_height)
        {
            return Err("receipt selection differs");
        }
        let key = receipt.receipt_digest().map_err(|_| "invalid receipt")?;
        self.worker.read_or_schedule(Job {
            payer,
            receipt,
            key,
        })
    }
}

/// Retain the proof owner in normal startup, rollback and test-router shutdown.
pub(super) fn register_worker(
    app: &crate::AppState,
    shutdown: iroha_futures::supervisor::ShutdownSignal,
    workers: &mut Vec<crate::ToriiCriticalWorker>,
) -> std::result::Result<(), &'static str> {
    if let Some(service) = app.kagemusha_load_finality.as_ref() {
        workers.push(crate::ToriiCriticalWorker {
            name: "kagemusha_load_finality",
            task: service.worker.supervise(shutdown)?,
        });
    }
    Ok(())
}

fn running(stop: &AtomicBool) -> Result<()> {
    if stop.load(Ordering::Acquire) {
        Err("owner stopped")
    } else {
        Ok(())
    }
}

fn mount(
    state: &CoreState,
    config: &KagemushaLoadFinality,
    cancel: ServerFinalityCancellationV1,
    stop: &AtomicBool,
) -> Result<ServerFinalityV1> {
    running(stop)?;
    let pack = iroha_fs::SelectedRegularFile::capture(&config.verifier_pack)
        .and_then(|file| file.read(VERIFIER_PACK_MAX_BYTES_V1))
        .map_err(|_| "verifier pack custody unavailable")?;
    running(stop)?;
    let inventory = iroha_fs::SelectedRegularFile::capture(&config.producer_inventory)
        .and_then(|file| file.read(CATALOG_MAX_BYTES_V1))
        .map_err(|_| "producer inventory custody unavailable")?;
    running(stop)?;
    let view = state.view();
    // This original comes from Core's actual configured signed genesis, never a request,
    // producer inventory anchor, remote checkpoint or an operator-supplied result projection.
    let proof = finality::build_proof(&view, 1).map_err(|_| "actual signed genesis unavailable")?;
    let genesis = norito::core::with_decode_limits_scope(
        norito::canonical_decode_limits(proof.block_wire.len()),
        || decode_framed_signed_block(&proof.block_wire),
    )
    .map_err(|_| "actual signed genesis framing differs")?;
    let verifier =
        SumeragiFinalityVerifier::new(&genesis, view.chain_id().as_str(), proof.committee)
            .map_err(|_| "actual signed genesis authority differs")?;
    if verifier.initial_epoch().network_id != *view.network_id() {
        return Err("actual network differs");
    }
    running(stop)?;
    ServerFinalityV1::open(
        &verifier,
        InstallationV1 {
            scheme_id: config.scheme_id,
            manifest_digest: config.manifest_digest,
        },
        &pack,
        &inventory,
        ServerFinalityStorageV1 {
            verifier_originals: &config.verifier_originals,
            proving_cache: &config.proving_cache,
            journal: &config.journal_dir,
        },
        ServerFinalityLimitsV1 {
            maximum_key_bytes: config.maximum_key_bytes,
            maximum_resident_proving_key_bytes: config.maximum_resident_proving_key_bytes,
            maximum_original_bytes: config.maximum_original_bytes,
            maximum_artifacts: config.maximum_artifacts,
            msm_bytes: config.msm_bytes,
            maximum_journal_entries: config.maximum_journal_entries,
            maximum_journal_bytes: config.maximum_journal_bytes,
        },
        cancel,
    )
    .map_err(|_| "authenticated complete server proof graph unavailable")
}

fn receipts<'a, 'b>(
    view: &'a iroha_core::state::StateView<'b>,
) -> Result<CommittedLoadReceipts<'a, 'b>> {
    CommittedLoadReceipts::new(
        view,
        RECORD_BYTES,
        norito::DecodeLimits::new(
            RECORD_BYTES,
            RECORD_BYTES,
            RECORD_BYTES * 8,
            RECORD_BYTES * 8,
            128,
        ),
    )
    .map_err(|_| "actual committed Load source unavailable")
}
fn prove(
    state: &CoreState,
    config: &KagemushaLoadFinality,
    producer: &mut ServerFinalityV1,
    job: &Job,
    stop: &AtomicBool,
) -> Result<Vec<u8>> {
    let selected = &job.receipt;
    let actual = receipts(&state.view())?
        .receipt_for(
            &job.payer,
            &selected.scheme_id,
            &selected.wallet_id,
            &selected.request_id,
        )
        .map_err(|_| "payer's original receipt unavailable")?;
    if actual != *selected {
        return Err("payer's original receipt changed");
    }
    if let Some(bytes) = producer
        .retained(&actual)
        .map_err(|_| "retained terminal proof refused")?
    {
        return Ok(bytes);
    }
    let budget = AllocationBudget::new(config.native_working_set_bytes);
    let mut cursor = NativeFinalityCursorV1::new();
    let mut prefix = producer
        .genesis()
        .map_err(|_| "genesis proof unavailable")?;
    for height in 2..=actual.block_height {
        if stop.load(Ordering::Acquire) {
            return Err("owner stopped");
        }
        let deadline = Instant::now()
            .checked_add(config.native_step_timeout)
            .ok_or("native deadline overflow")?;
        let target = NonZeroU64::new(height).ok_or("zero height")?;
        let native = loop {
            let view = state.view();
            let observation = cursor
                .advance_to_height(
                    &view,
                    target,
                    &budget,
                    deadline,
                    NonZeroU16::new(64).ok_or("zero native step bound")?,
                )
                .map_err(|_| "native original history refused")?;
            if let Some(native) = observation {
                break native;
            }
            if stop.load(Ordering::Acquire) {
                return Err("owner stopped");
            }
        };
        prefix = producer
            .append(&prefix, native.block())
            .map_err(|_| "genuine history proof unavailable")?;
        if height == actual.block_height {
            let view = state.view();
            let evidence = receipts(&view)?
                .event_evidence_for(
                    &native,
                    &job.payer,
                    &actual.scheme_id,
                    &actual.wallet_id,
                    &actual.request_id,
                )
                .map_err(|_| "actual native event refused")?;
            if evidence.verified().receipt() != &actual {
                return Err("actual native event receipt differs");
            }
            let path = evidence
                .path()
                .proof()
                .map_err(|_| "actual event path unavailable")?;
            drop(view);
            let bytes = producer
                .prove(&prefix, native.block(), &actual, &path)
                .map_err(|_| "genuine terminal receipt proof unavailable")?;
            if bytes.is_empty() || bytes.len() > KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1 {
                return Err("terminal proof extent differs");
            }
            return Ok(bytes);
        }
    }
    Err("non-genesis receipt absent")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_keeps_pending_exact_and_bounds_active_plus_unread_results() {
        let mut queue = Queue {
            entries: BTreeMap::new(),
            maximum: 1,
        };
        assert_eq!(queue.observe([1; 32]).unwrap(), None);
        assert_eq!(queue.observe([1; 32]).unwrap(), Some(None));
        assert!(queue.observe([2; 32]).is_err());
        queue.entries.insert([1; 32], Status::Ready(vec![1, 2, 3]));
        assert!(queue.observe([2; 32]).is_err());
        assert_eq!(queue.observe([1; 32]).unwrap(), Some(Some(vec![1, 2, 3])));
        assert_eq!(queue.observe([2; 32]).unwrap(), None);
    }
    #[test]
    fn failed_work_is_not_completed_evidence_and_requires_an_explicit_retry() {
        let mut queue = Queue {
            entries: BTreeMap::new(),
            maximum: 1,
        };
        queue.entries.insert([1; 32], Status::Failed);
        assert!(queue.observe([1; 32]).is_err());
        assert!(queue.entries.is_empty());
        assert_eq!(queue.observe([1; 32]).unwrap(), None);
    }
}
