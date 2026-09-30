//! Original-funded durable lane frames with retained startup, read and publication owners.
//!
//! One exclusive instance owner validates its entire recovered prefix before exposing a tip.
//! Complete signed availability and the original CommitQC share one canonical atomic frame.

use crate::sumeragi::{
    availability_schedule::{AvailabilitySchedule, resolve_source},
    body_read::{BodyReadError, BodyReadJob, BodyReader},
    driver::{SharedCrypto, traits::BlockStore},
    durable_artifact,
    lanes::record::PreparedLaneWrite,
    records::{Faults, NoFaults},
};
use iroha_allocation::AllocationBudget;
use iroha_sumeragi::{
    availability::{AvailabilitySource, AvailableBody},
    crypto::AttestationVerifier,
    message::{PayloadManifest, Qc, SyncEntry},
    types::Hash32,
};
use parking_lot::{Condvar, Mutex};
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

#[path = "store/read.rs"]
mod read;
#[path = "store/recovery.rs"]
mod recovery;
use read::{LaneBodyRead, RestoreFrame, certified_source, record_error};
pub use recovery::LaneStoreOpen;

const FRAME_SUFFIX: &str = "frame";
const LOCK_NAME: &str = ".custody.lock";
// Disk-corruption/allocation bound, not a consensus parameter.
const MAX_FRAME_FILE_BYTES: usize = 80 * 1024 * 1024;

/// Read-only inspection of one original committed lane frame under independent authority.
/// Uses the same canonical decoder, certificate verification and signed RS16 restoration as
/// store recovery, without acquiring a writer lock, creating files or changing a store tip.
/// Keep this job across local resource refusals to retain its original allocation custody.
pub struct LaneFrameRead {
    budget: AllocationBudget,
    job: RestoreFrame,
}

impl LaneFrameRead {
    /// Begin inspecting an independently selected path and native height. The historical
    /// schedule and verifier must come from authenticated authority, never the inspected frame.
    ///
    /// # Errors
    /// I/O, a non-regular artifact or an oversized frame.
    pub fn open(
        path: &Path,
        height: u64,
        crypto: SharedCrypto,
        budget: AllocationBudget,
        schedule: Arc<dyn AvailabilitySchedule>,
        verifier: Arc<dyn AttestationVerifier + Send + Sync>,
    ) -> io::Result<Self> {
        let job = RestoreFrame::open(path, height, budget.clone(), schedule, crypto, verifier)?;
        Ok(Self { budget, job })
    }

    /// Authenticate the original certificate and restore complete available payload custody.
    /// Moves the exact restored body and certificate owners without a metadata or bulk clone.
    /// A successful job is consumed and cannot be polled again.
    ///
    /// # Errors
    /// Invalid bytes, wrong historical authority, missing artifacts, I/O or original-pool
    /// refusal. `WouldBlock` retains the exact pending read and may be retried.
    pub fn poll(&mut self) -> io::Result<(AvailableBody, Qc)> {
        let prepared = self.job.poll(&self.budget)?;
        Ok(prepared.into_parts())
    }
}

struct PendingRead {
    height: u64,
    job: RestoreFrame,
    ready: Option<PreparedLaneWrite>,
}
struct StoreState {
    tip: u64,
    write: Option<PreparedLaneWrite>,
    read: Option<PendingRead>,
}

/// One fully recovered lane incarnation. Share this owner through Arc; second opens fail.
pub struct FileLaneBlockStore {
    dir: PathBuf,
    instance: Hash32,
    _ownership: File,
    crypto: SharedCrypto,
    faults: Arc<dyn Faults>,
    budget: AllocationBudget,
    schedule: Arc<dyn AvailabilitySchedule>,
    verifier: Arc<dyn AttestationVerifier + Send + Sync>,
    state: Mutex<StoreState>,
    grown: Condvar,
}
impl core::fmt::Debug for FileLaneBlockStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FileLaneBlockStore")
            .field("dir", &self.dir)
            .field("height", &self.state.lock().tip)
            .finish_non_exhaustive()
    }
}
fn frame_name(height: u64) -> String {
    format!("{height:020}.{FRAME_SUFFIX}")
}
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn lock_instance(dir: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
        options
            .mode(0o600)
            .custom_flags(i32::from_ne_bytes(flags.bits().to_ne_bytes()));
    }
    let file = options.open(dir.join(LOCK_NAME))?;
    if !file.metadata()?.is_file() {
        return Err(invalid("lane ownership lock is not a regular file"));
    }
    file.try_lock().map_err(io::Error::from)?;
    Ok(file)
}

impl FileLaneBlockStore {
    /// Acquire exclusive ownership and enumerate a canonical contiguous prefix, without reading
    /// or allocating body bytes. Keep the returned owner until recovery completes or is abandoned.
    ///
    /// # Errors
    /// Filesystem, existing owner, foreign schedule, or malformed directory population.
    pub fn begin_open(
        root: &Path,
        instance: &Hash32,
        crypto: SharedCrypto,
        budget: AllocationBudget,
        schedule: Arc<dyn AvailabilitySchedule>,
        verifier: Arc<dyn AttestationVerifier + Send + Sync>,
    ) -> io::Result<LaneStoreOpen> {
        Self::begin_open_with_faults(
            root,
            instance,
            crypto,
            budget,
            schedule,
            verifier,
            Arc::new(NoFaults),
        )
    }
    /// Begin exclusive recovery with deterministic filesystem fault injection.
    ///
    /// # Errors
    /// See `begin_open`.
    pub fn begin_open_with_faults(
        root: &Path,
        instance: &Hash32,
        crypto: SharedCrypto,
        budget: AllocationBudget,
        schedule: Arc<dyn AvailabilitySchedule>,
        verifier: Arc<dyn AttestationVerifier + Send + Sync>,
        faults: Arc<dyn Faults>,
    ) -> io::Result<LaneStoreOpen> {
        if schedule.instance() != *instance {
            return Err(invalid("lane schedule belongs to another instance"));
        }
        let dir = root.join(hex::encode(instance.0));
        durable_artifact::establish_dir(&*faults, &dir)?;
        let ownership = lock_instance(&dir)?;
        let mut heights = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == LOCK_NAME {
                continue;
            }
            if !entry.file_type()?.is_file() {
                return Err(invalid(format!("non-regular lane artifact {name}")));
            }
            match name.strip_suffix(&format!(".{FRAME_SUFFIX}")) {
                Some(number) => {
                    let height = number
                        .parse::<u64>()
                        .map_err(|_| invalid(format!("unexpected lane frame {name}")))?;
                    if name != frame_name(height) {
                        return Err(invalid(format!("non-canonical lane frame name {name}")));
                    }
                    heights.push(height);
                }
                None if name.ends_with(".tmp") => fs::remove_file(entry.path())?,
                None => return Err(invalid(format!("unexpected lane file {name}"))),
            }
        }
        heights.sort_unstable();
        for (i, height) in heights.iter().enumerate() {
            if *height != i as u64 + 1 {
                return Err(invalid("lane heights are not contiguous from one"));
            }
        }
        let tip = heights.last().copied().unwrap_or(0);
        let store = Self {
            dir,
            instance: *instance,
            _ownership: ownership,
            crypto,
            faults,
            budget,
            schedule,
            verifier,
            state: Mutex::new(StoreState {
                tip: 0,
                write: None,
                read: None,
            }),
            grown: Condvar::new(),
        };
        Ok(LaneStoreOpen::new(store, tip))
    }
    /// Take a fully authenticated committed body and its original certificate for lane merge.
    /// The store retains every read/restoration allocation across errors and resource refusals.
    ///
    /// # Errors
    /// Invalid stored artifacts, unavailable historical authority, I/O or original-pool refusal.
    pub(super) fn committed_body(&self, height: u64) -> io::Result<Option<(AvailableBody, Qc)>> {
        let mut state = self.state.lock();
        if height == 0 || height > state.tip {
            return Ok(None);
        }
        self.read_prepared(&mut state, height)?;
        let prepared = state
            .read
            .take()
            .and_then(|read| read.ready)
            .expect("completed original lane read");
        Ok(Some(prepared.into_parts()))
    }
    /// Wait until the durable validated tip reaches `height`, bounded by `timeout`.
    #[must_use]
    pub fn wait_for(&self, height: u64, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock();
        while state.tip < height {
            if self.grown.wait_until(&mut state, deadline).timed_out() {
                return state.tip >= height;
            }
        }
        true
    }
    fn restore(&self, height: u64) -> io::Result<RestoreFrame> {
        RestoreFrame::open(
            &self.dir.join(frame_name(height)),
            height,
            self.budget.clone(),
            Arc::clone(&self.schedule),
            Arc::clone(&self.crypto),
            Arc::clone(&self.verifier),
        )
    }
    fn read_prepared<'a>(
        &self,
        state: &'a mut StoreState,
        height: u64,
    ) -> io::Result<&'a mut PreparedLaneWrite> {
        if state
            .read
            .as_ref()
            .is_some_and(|read| read.height != height)
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "another original lane read is pending",
            ));
        }
        if state.read.is_none() {
            state.read = Some(PendingRead {
                height,
                job: self.restore(height)?,
                ready: None,
            });
        }
        let read = state.read.as_mut().expect("retained original read");
        if read.ready.is_none() {
            read.ready = Some(read.job.poll(&self.budget)?);
        }
        Ok(read.ready.as_mut().expect("restored original body and QC"))
    }
}

impl BlockStore for FileLaneBlockStore {
    fn committed_body(&self, height: u64) -> io::Result<Option<(AvailableBody, Qc)>> {
        Self::committed_body(self, height)
    }
    fn height(&self) -> u64 {
        self.state.lock().tip
    }
    fn entry(&self, height: u64) -> io::Result<Option<SyncEntry>> {
        let mut state = self.state.lock();
        if height == 0 || height > state.tip {
            return Ok(None);
        }
        let prepared = self.read_prepared(&mut state, height)?;
        let entry = SyncEntry {
            manifest: PayloadManifest {
                header: prepared.body().header().clone(),
                availability: prepared.body().availability().clone(),
            },
            commit_qc: prepared.commit_qc().clone(),
        };
        state.read = None;
        Ok(Some(entry))
    }
    fn availability_source(
        &self,
        height: u64,
        block_hash: Hash32,
    ) -> io::Result<Option<AvailabilitySource>> {
        resolve_source(&*self.schedule, self.instance, height, block_hash)
    }
    fn append(&self, body: &AvailableBody, qc: &Qc) -> io::Result<()> {
        if !body.admitted_to(&self.budget)
            || qc
                .attestation_witness
                .as_ref()
                .is_some_and(|w| !w.admitted_to(&self.budget))
        {
            return Err(invalid("lane append original pool mismatch"));
        }
        let height = body.header().height;
        let source = certified_source(
            &*self.schedule,
            &*self.crypto,
            &*self.verifier,
            height,
            body.header(),
            qc,
        )?;
        if body.source() != &source {
            return Err(invalid(
                "lane body was authenticated under another historical source",
            ));
        }
        let mut state = self.state.lock();
        if height <= state.tip {
            let stored = self.read_prepared(&mut state, height)?;
            if stored.body() != body || stored.commit_qc().result != qc.result {
                // This healthy stored read is complete; reject the incoming decision without
                // pinning unrelated future reads behind it. Pending publication is untouched.
                state.read = None;
                return Err(invalid("another lane decision is already durable"));
            }
            let bytes = stored.prepare(&self.budget).map_err(record_error)?;
            durable_artifact::publish(&*self.faults, &self.dir, &frame_name(height), bytes)?;
            state.read = None;
            return Ok(());
        }
        if height != state.tip.saturating_add(1) {
            return Err(invalid("lane append is not the next durable height"));
        }
        if let Some(original) = &state.write {
            if original.body() != body || original.commit_qc() != qc {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "different prepared artifact cannot replace original pending lane publication",
                ));
            }
        } else {
            state.write = Some(PreparedLaneWrite::new(body.clone(), qc.clone()));
        }
        let pending = state.write.as_mut().expect("original publication retained");
        pending
            .check_context(&source, &*self.crypto)
            .map_err(record_error)?;
        let bytes = pending.prepare(&self.budget).map_err(record_error)?;
        if bytes.len() > MAX_FRAME_FILE_BYTES {
            return Err(invalid("lane frame exceeds disk custody bound"));
        }
        durable_artifact::publish(&*self.faults, &self.dir, &frame_name(height), bytes)?;
        state.write = None;
        state.tip = height;
        self.grown.notify_all();
        Ok(())
    }
}
impl BodyReader for FileLaneBlockStore {
    fn begin_read(
        &self,
        source: AvailabilitySource,
    ) -> Result<Box<dyn BodyReadJob>, BodyReadError> {
        if source.instance() != self.instance {
            return Err(BodyReadError::Io(invalid(
                "lane read source belongs to another instance",
            )));
        }
        Ok(Box::new(
            LaneBodyRead::open(
                source.clone(),
                &self.dir.join(frame_name(source.height())),
                self.budget.clone(),
                Arc::clone(&self.crypto),
            )
            .map_err(BodyReadError::Io)?,
        ))
    }
}

#[cfg(test)]
#[path = "store/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "store/publication_tests.rs"]
mod publication_tests;
