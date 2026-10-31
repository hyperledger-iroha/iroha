//! Exact-root Goldilocks transforms shared by bounded prover adapters.
//!
//! This is arithmetic dispatch, not a proof-profile selector. Public dimensions,
//! canonical inputs and the full order of the supplied root are checked before
//! any mutation. GPU staging has clearing owners; input and completed output
//! columns remain the caller's responsibility.

use core::fmt;
use rayon::prelude::*;

use crate::{backend::ExecutionMode, cyclotomic, poseidon::FIELD_MODULUS};

/// Maximum resident columns accepted by one exact-root transform call.
pub const MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1: usize = 8;

/// Maximum columns in the separately admitted, prepared forward-only interface.
/// Ordinary transforms and prepared inverse transforms retain their eight-column limit.
pub const MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1: usize = 16;

// Measured native batches benefit from the vectorizable serial fold; waking a
// wide pool for them can cost more than the scan. The common22 four-column
// extent benefits from parallel tiles. Selection depends only on public size.
/// Public minimum extent for parallel canonicality validation.
pub(crate) const CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1: usize = 1 << 24;
/// Maximum words in one independent parallel canonicality tile.
pub(crate) const CANONICAL_SCAN_TILE_WORDS_V1: usize = 1 << 14;

/// Select parallel validation solely from public extent and the existing pool.
pub(crate) fn parallel_canonical_scan_v1(words: usize) -> bool {
    words >= CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1 && rayon::current_num_threads() > 1
}

/// Validate every word in one borrowed slice with a vectorizable reduction.
/// No private scratch is allocated and no output is published by this helper.
pub(crate) fn canonical_word_slice_v1(words: &[u64]) -> bool {
    words.iter().fold(0_u64, |invalid, &word| {
        invalid | u64::from(word >= FIELD_MODULUS)
    }) == 0
}

fn canonical_word_columns_v1<T: AsRef<[u64]> + Sync>(columns: &[T], words: usize) -> bool {
    if parallel_canonical_scan_v1(words) {
        columns.par_iter().all(|column| {
            column
                .as_ref()
                .par_chunks(CANONICAL_SCAN_TILE_WORDS_V1)
                .all(canonical_word_slice_v1)
        })
    } else {
        columns.iter().all(|column| {
            column
                .as_ref()
                .chunks(crate::gpu_memory::METAL_PAGE_BYTES / core::mem::size_of::<u64>())
                .all(canonical_word_slice_v1)
        })
    }
}

/// Direction of an exact-root transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoldilocksTransformDirectionV1 {
    /// Evaluate coefficients at successive powers of the supplied root.
    Forward,
    /// Interpolate evaluations, including multiplication by the inverse length.
    Inverse,
}

/// Backend that actually completed this transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoldilocksTransformBackendV1 {
    /// Deterministic scalar arithmetic, parallel across independent columns.
    Cpu,
    /// The existing Metal Goldilocks kernel completed successfully.
    Metal,
    /// The existing CUDA Goldilocks kernel completed successfully.
    Cuda,
}

/// Whether device completion is uncertain and private staging may remain live.
///
/// This state is sticky for the process. Memory-limited proof producers must
/// reject before allocating sources, even when selecting CPU execution; switching
/// backends does not release storage still owned by an unfinished command.
#[must_use]
pub fn goldilocks_transform_completion_uncertain_v1() -> bool {
    #[cfg(feature = "fastpq-gpu")]
    {
        crate::gpu::transform_completion_uncertain_v1()
    }
    #[cfg(not(feature = "fastpq-gpu"))]
    false
}

/// Return the hardware backend eligible for exact-root transform dispatch.
///
/// Detection is shared with the existing process-wide backend owner. CPU-only
/// builds return `None`; a quarantined CUDA backend is unavailable. This does
/// not execute a transform or qualify a complete proof pipeline.
#[must_use]
pub fn available_goldilocks_transform_backend_v1() -> Option<GoldilocksTransformBackendV1> {
    if goldilocks_transform_completion_uncertain_v1() {
        return None;
    }
    #[cfg(feature = "fastpq-gpu")]
    {
        if !matches!(ExecutionMode::Auto.resolve(), ExecutionMode::Gpu) {
            return None;
        }
        match crate::backend::current_gpu_backend() {
            Some(crate::backend::GpuBackend::Metal) => Some(GoldilocksTransformBackendV1::Metal),
            Some(crate::backend::GpuBackend::Cuda) => Some(GoldilocksTransformBackendV1::Cuda),
            _ => None,
        }
    }
    #[cfg(not(feature = "fastpq-gpu"))]
    None
}

// Shared with Metal's actual pool policy when this module is wired. Live pooled
// allocations have exactly the requested page capacity, independently of a full
// idle pool. The separate twiddle cache also holds factorized exact-root tables;
// charge its largest entry regardless of the current layout.
use crate::gpu_memory::{METAL_PAGE_BYTES, METAL_POOL_MAX_CACHED_BYTES};

/// Smallest power-of-two column whose words occupy complete shared Metal pages.
/// Smaller retained outputs require explicit page-padding admission.
pub const MIN_UNPADDED_GOLDILOCKS_COLUMN_ROWS_V1: usize =
    METAL_PAGE_BYTES / core::mem::size_of::<u64>();

/// Conservative additional host/shared payload for one bounded Metal FFT.
///
/// The caller's columns are excluded. The returned allowance includes one
/// rounded private staging allocation shared without copying with Metal, the complete 64MiB
/// pool cache, all cached stage twiddles, and 1MiB for dispatch metadata. It does
/// not replace a process reserve for allocator/driver/thread overhead or imply
/// a measured RSS bound. CUDA has a different pool and is deliberately outside
/// this Metal-specific accounting contract.
///
/// # Errors
/// Returns [`GoldilocksTransformErrorV1::InvalidShape`] for an unsupported row
/// or column count, or when the checked byte allowance overflows.
pub fn metal_goldilocks_transform_extra_payload_v1(
    rows: usize,
    columns: usize,
) -> Result<usize, GoldilocksTransformErrorV1> {
    metal_extra_payload_with_limit_v1(rows, columns, MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1)
}

/// Complete Metal allowance for the separately bounded prepared forward interface.
/// Includes its sole shared backing, full idle pool, twiddles and dispatch metadata.
///
/// # Errors
/// Returns invalid shape for an unsupported extent or checked byte overflow.
pub fn metal_goldilocks_forward_extra_payload_v1(
    rows: usize,
    columns: usize,
) -> Result<usize, GoldilocksTransformErrorV1> {
    metal_extra_payload_with_limit_v1(rows, columns, MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1)
}

fn metal_extra_payload_with_limit_v1(
    rows: usize,
    columns: usize,
    limit: usize,
) -> Result<usize, GoldilocksTransformErrorV1> {
    if rows < 2
        || !rows.is_power_of_two()
        || rows.trailing_zeros() > 32
        || columns == 0
        || columns > limit
    {
        return Err(GoldilocksTransformErrorV1::InvalidShape);
    }
    let column_bytes = rows
        .checked_mul(8)
        .ok_or(GoldilocksTransformErrorV1::InvalidShape)?;
    // Exact-root dispatch owns one unpublished PooledBuffer; its Metal buffer
    // retains the same pages through newBufferWithBytesNoCopy. The caller is
    // mutated only after that sole batch completes, so neither this path nor
    // the facade allocates a rollback snapshot. Ordinary multi-batch FFT APIs
    // have different owners and must not use this exact-root payload forecast.
    let private = column_bytes
        .checked_mul(columns)
        .ok_or(GoldilocksTransformErrorV1::InvalidShape)?;
    let padding = columns
        .checked_mul(METAL_PAGE_BYTES - 1)
        .ok_or(GoldilocksTransformErrorV1::InvalidShape)?;
    [
        private,
        padding,
        METAL_POOL_MAX_CACHED_BYTES,
        crate::gpu_memory::METAL_TWIDDLE_PAYLOAD_ALLOWANCE,
        1 << 20,
    ]
    .into_iter()
    .try_fold(0_usize, |total, bytes| {
        total
            .checked_add(bytes)
            .ok_or(GoldilocksTransformErrorV1::InvalidShape)
    })
}

/// Transform validation, dispatch or checked-publication failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoldilocksTransformErrorV1 {
    /// Columns do not have one supported public power-of-two extent.
    InvalidShape,
    /// The root is noncanonical or does not have exactly the requested order.
    InvalidRoot,
    /// An input word is not a canonical Goldilocks residue.
    NonCanonicalInput,
    /// A completed device output contains a noncanonical Goldilocks residue.
    /// The original destination columns have not been changed.
    NonCanonicalOutput,
    /// Required GPU execution is unavailable in this build or on this host.
    DeviceUnavailable,
    /// The required device rejected or failed the transform.
    DeviceFailure(String),
    /// An unfinished device command may still own private staging. No automatic
    /// fallback or subsequent memory-limited proof admission is safe.
    CompletionUncertain,
}

impl fmt::Display for GoldilocksTransformErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidShape => formatter.write_str("invalid bounded Goldilocks transform shape"),
            Self::InvalidRoot => formatter.write_str("Goldilocks root has another exact order"),
            Self::NonCanonicalInput => {
                formatter.write_str("noncanonical Goldilocks transform input")
            }
            Self::NonCanonicalOutput => {
                formatter.write_str("noncanonical Goldilocks transform output")
            }
            Self::DeviceUnavailable => {
                formatter.write_str("required Goldilocks transform device unavailable")
            }
            Self::DeviceFailure(message) => {
                write!(formatter, "Goldilocks transform device failure: {message}")
            }
            Self::CompletionUncertain => formatter.write_str(
                "Goldilocks device completion is uncertain; private staging remains reserved",
            ),
        }
    }
}
impl std::error::Error for GoldilocksTransformErrorV1 {}

/// One completed, unpublished Metal transform over borrowed input columns.
///
/// The private backing is the same clearing allocation used by ordinary exact-root
/// transforms. This owner retains the Metal view and GPU lane until publication or
/// drop. It is intentionally neither `Clone` nor `Debug`; it cannot outlive its
/// owning thread or expose mutable device output.
#[must_use = "publish the checked result or drop it to erase the completed backing"]
pub struct PreparedGoldilocksColumnsV1 {
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    inner: crate::metal::exact_root::PreparedColumns,
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    _lane: std::sync::MutexGuard<'static, ()>,
    // Cell is non-Copy; the Rc marker keeps the sealed owner !Send and !Sync
    // even in builds where no device-backed token can be constructed. Both
    // wrappers are zero-sized and allocate nothing.
    _sealed: core::cell::Cell<core::marker::PhantomData<std::rc::Rc<()>>>,
}

/// Fully validated immutable transform output with clearing host-only custody.
///
/// Completion and whole-output canonicality are established before this owner
/// exists. It retains no GPU lane or Metal view. Shared column borrows cannot
/// outlive it; the last backing owner erases every allocated page on drop.
pub struct CompletedGoldilocksColumnsV1 {
    _sealed: (),
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    inner: crate::metal::exact_root::CompletedReadColumns,
}
// Preserve consuming ownership even when no device-backed owner can be created.
// The empty representation remains sealed, Send + Sync, and allocation-free.
#[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
impl Drop for CompletedGoldilocksColumnsV1 {
    fn drop(&mut self) {}
}

impl CompletedGoldilocksColumnsV1 {
    /// Consume a successfully used output and erase its complete backing.
    /// Large exclusive allocations may be cleared by independent worker tiles.
    /// Shared device retention, failure cleanup and unwind retain ordinary Drop.
    pub fn erase_completed_v1(self) {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        self.inner.erase_completed_v1();
    }

    /// Exact number of words in each completed column.
    #[must_use]
    pub fn rows_v1(&self) -> usize {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        return self.inner.rows();
        #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
        0
    }
    /// Number of completed columns sharing the backing.
    #[must_use]
    pub fn columns_v1(&self) -> usize {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        return self.inner.columns();
        #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
        0
    }
    /// Borrow one canonical column; invalid public indices return `None`.
    #[must_use]
    pub fn column_v1(&self, index: usize) -> Option<&[u64]> {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        return self.inner.column(index);
        #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
        {
            let _ = index;
            None
        }
    }
    /// Actual page/backing-control capacity plus this inline owner.
    /// This excludes the caller's Arc header and existing cache/driver reserves.
    ///
    /// # Errors
    /// Returns invalid shape on byte overflow, or unavailable without Metal.
    pub fn allocated_payload_bytes_v1(&self) -> Result<usize, GoldilocksTransformErrorV1> {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        return self
            .inner
            .backing_payload_bytes()?
            .checked_add(core::mem::size_of::<Self>())
            .ok_or(GoldilocksTransformErrorV1::InvalidShape);
        #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
        Err(GoldilocksTransformErrorV1::DeviceUnavailable)
    }
}

/// Closed private-allocation bound for a completed transactional Metal result.
///
/// Counts the exact page-rounded data extent, a 256-byte backing/control-header
/// allowance, and the complete public token. This is separate from—and does not
/// replace—the full idle-pool, twiddle and driver reservations in
/// [`metal_goldilocks_transform_extra_payload_v1`].
///
/// # Errors
/// Returns invalid shape when dimensions or checked byte arithmetic are invalid.
pub fn prepared_goldilocks_columns_payload_bound_v1(
    rows: usize,
    columns: usize,
) -> Result<usize, GoldilocksTransformErrorV1> {
    metal_goldilocks_transform_extra_payload_v1(rows, columns)?;
    prepared_payload_bound_v1(rows, columns)
}

/// Exact backing/control bound for a prepared forward transform of at most sixteen columns.
/// This does not replace [`metal_goldilocks_forward_extra_payload_v1`].
///
/// # Errors
/// Returns invalid shape for an unsupported extent or checked byte overflow.
pub fn prepared_goldilocks_forward_payload_bound_v1(
    rows: usize,
    columns: usize,
) -> Result<usize, GoldilocksTransformErrorV1> {
    metal_goldilocks_forward_extra_payload_v1(rows, columns)?;
    prepared_payload_bound_v1(rows, columns)
}

fn prepared_payload_bound_v1(
    rows: usize,
    columns: usize,
) -> Result<usize, GoldilocksTransformErrorV1> {
    rows.checked_mul(columns)
        .and_then(|words| words.checked_mul(core::mem::size_of::<u64>()))
        .and_then(|bytes| bytes.checked_add(METAL_PAGE_BYTES - 1))
        .map(|bytes| bytes / METAL_PAGE_BYTES * METAL_PAGE_BYTES)
        .and_then(|bytes| bytes.checked_add(256))
        .and_then(|bytes| bytes.checked_add(core::mem::size_of::<PreparedGoldilocksColumnsV1>()))
        .ok_or(GoldilocksTransformErrorV1::InvalidShape)
}

impl PreparedGoldilocksColumnsV1 {
    /// Validate complete output and transfer it into immutable host-only custody.
    /// The device view and lane are released before the result reaches the caller.
    ///
    /// # Errors
    /// Returns uncertain completion, invalid shape or noncanonical output before
    /// exposing a borrow. Failed conversion erases the completed backing.
    pub fn into_readonly_v1(
        self,
    ) -> Result<CompletedGoldilocksColumnsV1, GoldilocksTransformErrorV1> {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        {
            let Self {
                inner,
                _lane,
                _sealed,
            } = self;
            let inner = inner.into_readonly()?;
            drop(_lane);
            Ok(CompletedGoldilocksColumnsV1 { inner, _sealed: () })
        }
        #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
        Err(GoldilocksTransformErrorV1::DeviceUnavailable)
    }

    /// Exact public row count of every completed column.
    #[must_use]
    pub fn rows_v1(&self) -> usize {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        return self.inner.rows();
        #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
        0
    }

    /// Exact public number of completed columns, at most sixteen for prepared forward work.
    #[must_use]
    pub fn columns_v1(&self) -> usize {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        return self.inner.columns();
        #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
        0
    }

    /// Actual private backing capacity, Arc allocation and this complete owner.
    ///
    /// This excludes idle caches and opaque driver allocations. Callers must still
    /// reserve [`metal_goldilocks_transform_extra_payload_v1`] (ordinary transforms)
    /// or [`metal_goldilocks_forward_extra_payload_v1`] (prepared forward work)
    /// in full, plus their own borrowed descriptors and `size_of::<Self>()`; this getter is an actual
    /// capacity check, never a replacement for that conservative admission.
    ///
    /// # Errors
    /// Returns an invalid-shape error if checked byte arithmetic overflows.
    pub fn allocated_payload_bytes_v1(&self) -> Result<usize, GoldilocksTransformErrorV1> {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        return self
            .inner
            .backing_payload_bytes()?
            .checked_add(core::mem::size_of::<Self>())
            .ok_or(GoldilocksTransformErrorV1::InvalidShape);
        #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
        Err(GoldilocksTransformErrorV1::DeviceUnavailable)
    }

    /// Validate the entire completed output, then publish into existing slices.
    ///
    /// Sticky uncertain completion, every destination shape, and every output
    /// residue are checked before the first write. No user callback, allocation,
    /// or fallible check occurs after publication starts. All failures preserve
    /// every destination; dropping this owner erases its completed private pages.
    ///
    /// # Errors
    /// Returns uncertain completion, invalid destination shape, or noncanonical
    /// output without changing any destination column.
    pub fn publish_into_v1(
        self,
        destinations: &mut [&mut [u64]],
    ) -> Result<GoldilocksTransformBackendV1, GoldilocksTransformErrorV1> {
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        {
            self.inner.publish_into(destinations)?;
            Ok(GoldilocksTransformBackendV1::Metal)
        }
        #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
        {
            let _ = destinations;
            Err(GoldilocksTransformErrorV1::DeviceUnavailable)
        }
    }
}

/// Result of preparing a Metal transform under the caller's execution policy.
/// A CPU outcome leaves every borrowed input untouched for deterministic fallback.
#[must_use]
pub enum PreparedGoldilocksColumnsOutcomeV1 {
    /// The existing Metal command completed; output is still unpublished.
    Metal(PreparedGoldilocksColumnsV1),
    /// The caller must transform the unchanged originals on the CPU.
    Cpu,
}

/// Prepare one Metal transform without mutating borrowed inputs.
///
/// Input views are copied directly into the existing guarded Metal backing. They
/// are not retained by the result, so callers can end their input borrows before
/// creating destination views of the same allocations. Domain validation, GPU
/// barriers, completion deadlines and uncertain-command quarantine are shared
/// with the ordinary exact-root path. Auto mode returns a CPU outcome when Metal
/// is unavailable, its lane is busy, or an ordinary failure drains completely.
/// Required GPU mode reports these failures. CPU mode never touches the device.
/// This transactional interface stages only Metal; callers selecting another
/// device should use the ordinary transform interface instead.
///
/// # Errors
/// Returns invalid shape/root/input, unavailable Metal, device failure, or sticky
/// uncertain completion. A failure never returns an owner that can publish.
pub fn prepare_goldilocks_columns_v1(
    columns: &[&[u64]],
    root: u64,
    direction: GoldilocksTransformDirectionV1,
    execution: ExecutionMode,
) -> Result<PreparedGoldilocksColumnsOutcomeV1, GoldilocksTransformErrorV1> {
    prepare_with_limit_v1(columns, root, direction, execution, false)
}

/// Prepare at most sixteen forward columns under a separately charged caller budget.
/// Publication, completion, canonicality and Auto fallback use the ordinary transaction.
/// No inverse or ordinary transform limit is widened by this interface.
///
/// # Errors
/// Returns the same shape, root, canonicality, device and completion errors as
/// [`prepare_goldilocks_columns_v1`], without mutating any input.
pub fn prepare_goldilocks_forward_columns_v1(
    columns: &[&[u64]],
    root: u64,
    execution: ExecutionMode,
) -> Result<PreparedGoldilocksColumnsOutcomeV1, GoldilocksTransformErrorV1> {
    prepare_with_limit_v1(
        columns,
        root,
        GoldilocksTransformDirectionV1::Forward,
        execution,
        true,
    )
}

/// Borrow native prefixes and prepare common-domain output without a full host input.
/// The CPU outcome leaves all inputs unchanged for the original CPU fallback.
/// The caller must charge the same complete prepared-transform allowance.
///
/// # Errors
/// Returns invalid shape/root/input, device failure/unavailability or uncertain
/// completion without mutating input or returning an output owner.
pub fn prepare_goldilocks_prefix_columns_v1(
    prefixes: &[&[u64]],
    rows: usize,
    root: u64,
    powers: &[u64],
    execution: ExecutionMode,
) -> Result<PreparedGoldilocksColumnsOutcomeV1, GoldilocksTransformErrorV1> {
    let domain = validate_prefixes_v1(prefixes, rows, root, powers)?;
    if matches!(execution, ExecutionMode::Cpu) {
        return Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu);
    }
    if goldilocks_transform_completion_uncertain_v1() {
        return Err(GoldilocksTransformErrorV1::CompletionUncertain);
    }
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    {
        use crate::backend::{self, GpuBackend};
        if matches!(execution.resolve(), ExecutionMode::Gpu)
            && backend::current_gpu_backend() == Some(GpuBackend::Metal)
            && let Some(lane) = transform_lane_v1(execution)
        {
            let result = crate::metal::exact_root::prepare_prefixes_v1(
                prefixes,
                domain.log_size,
                root,
                powers,
            );
            if let Some(inner) = classify_dispatch_result_v1(
                result,
                execution,
                goldilocks_transform_completion_uncertain_v1(),
            )? {
                return Ok(PreparedGoldilocksColumnsOutcomeV1::Metal(
                    PreparedGoldilocksColumnsV1 {
                        inner,
                        _lane: lane,
                        _sealed: core::cell::Cell::new(core::marker::PhantomData),
                    },
                ));
            }
        }
    }
    #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
    let _ = domain;
    if matches!(execution, ExecutionMode::Gpu) {
        return Err(GoldilocksTransformErrorV1::DeviceUnavailable);
    }
    Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu)
}

/// Validate domain, complete input prefixes, and the complete public power table.
fn validate_prefixes_v1(
    prefixes: &[&[u64]],
    rows: usize,
    root: u64,
    powers: &[u64],
) -> Result<cyclotomic::Domain, GoldilocksTransformErrorV1> {
    if prefixes.is_empty()
        || prefixes.len() > MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1
        || rows < 2
        || !rows.is_power_of_two()
        || rows.trailing_zeros() > 32
        || powers.is_empty()
        || powers.len() > rows
        || prefixes
            .iter()
            .any(|column| column.is_empty() || column.len() > powers.len())
        || rows
            .checked_mul(prefixes.len())
            .and_then(|n| n.checked_mul(8))
            .is_none()
    {
        return Err(GoldilocksTransformErrorV1::InvalidShape);
    }
    if root == 0
        || root >= FIELD_MODULUS
        || power_v1(root, rows as u64) != 1
        || power_v1(root, rows as u64 / 2) == 1
    {
        return Err(GoldilocksTransformErrorV1::InvalidRoot);
    }
    let words = prefixes.iter().map(|column| column.len()).sum();
    if !canonical_word_columns_v1(prefixes, words) || !canonical_word_slice_v1(powers) {
        return Err(GoldilocksTransformErrorV1::NonCanonicalInput);
    }
    Ok(cyclotomic::Domain {
        log_size: rows.trailing_zeros(),
        generator: root,
    })
}

fn prepare_with_limit_v1(
    columns: &[&[u64]],
    root: u64,
    direction: GoldilocksTransformDirectionV1,
    execution: ExecutionMode,
    wide_forward: bool,
) -> Result<PreparedGoldilocksColumnsOutcomeV1, GoldilocksTransformErrorV1> {
    let limit = if wide_forward {
        MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1
    } else {
        MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1
    };
    let domain = validate_with_limit_v1(columns, root, limit)?;
    // Match the ordinary CPU branch, which performs no device discovery.
    if matches!(execution, ExecutionMode::Cpu) {
        return Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu);
    }
    if goldilocks_transform_completion_uncertain_v1() {
        return Err(GoldilocksTransformErrorV1::CompletionUncertain);
    }
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    {
        use crate::backend::{self, GpuBackend};
        if matches!(execution.resolve(), ExecutionMode::Gpu)
            && backend::current_gpu_backend() == Some(GpuBackend::Metal)
            && let Some(lane) = transform_lane_v1(execution)
        {
            let result = if wide_forward {
                crate::metal::exact_root::prepare_forward(columns, domain.log_size, root)
            } else {
                crate::metal::exact_root::prepare(
                    columns,
                    domain.log_size,
                    root,
                    matches!(direction, GoldilocksTransformDirectionV1::Inverse),
                )
            };
            if let Some(inner) = classify_dispatch_result_v1(
                result,
                execution,
                goldilocks_transform_completion_uncertain_v1(),
            )? {
                // Publication performs completed-output validation separately
                // from dispatch classification; it can never request fallback.
                return Ok(PreparedGoldilocksColumnsOutcomeV1::Metal(
                    PreparedGoldilocksColumnsV1 {
                        inner,
                        _lane: lane,
                        _sealed: core::cell::Cell::new(core::marker::PhantomData),
                    },
                ));
            }
        }
    }
    #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
    let _ = (domain, direction);
    if matches!(execution, ExecutionMode::Gpu) {
        return Err(GoldilocksTransformErrorV1::DeviceUnavailable);
    }
    Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu)
}

/// Transform at most eight columns using the exact caller-supplied root.
///
/// Each column must have the same length, from `2` through `2^32`, and contain
/// only canonical residues. CPU mode does not discover or touch a GPU. Auto
/// mode uses the configured backend when its lane is free and falls back to the
/// CPU if dispatch is unavailable or fails. Required GPU mode waits for that
/// lane and reports an error instead of substituting CPU work.
///
/// Backend failures preserve the original columns: both Metal and CUDA publish
/// their guarded staging only after a successful wait. The Metal path orders
/// independent tiles and butterflies with device resource barriers. Automatic
/// fallback therefore transforms the original inputs after a
/// fully drained failure. Uncertain completion is terminal, including in Auto
/// mode, because switching arithmetic does not release device-owned storage.
///
/// # Errors
/// Returns a shape, root or noncanonical-input error before any transform, a
/// checked-publication or uncertain-completion error before any Metal output is
/// copied, or, in required GPU mode, a device unavailability or failure error.
pub fn transform_goldilocks_columns_v1(
    columns: &mut [Vec<u64>],
    root: u64,
    direction: GoldilocksTransformDirectionV1,
    execution: ExecutionMode,
) -> Result<GoldilocksTransformBackendV1, GoldilocksTransformErrorV1> {
    let domain = validate_v1(columns, root)?;
    if matches!(execution, ExecutionMode::Cpu) {
        transform_cpu_v1(columns, domain, direction);
        return Ok(GoldilocksTransformBackendV1::Cpu);
    }
    if goldilocks_transform_completion_uncertain_v1() {
        return Err(GoldilocksTransformErrorV1::CompletionUncertain);
    }
    #[cfg(feature = "fastpq-gpu")]
    {
        use crate::backend::{self, GpuBackend};
        if matches!(execution.resolve(), ExecutionMode::Gpu) {
            let backend = backend::current_gpu_backend();
            let actual = match backend {
                Some(GpuBackend::Metal) => Some(GoldilocksTransformBackendV1::Metal),
                Some(GpuBackend::Cuda) => Some(GoldilocksTransformBackendV1::Cuda),
                _ => None,
            };
            if let (Some(backend), Some(actual)) = (backend, actual) {
                let lane = transform_lane_v1(execution);
                if let Some(_lane) = lane {
                    #[cfg(target_os = "macos")]
                    if backend == GpuBackend::Metal {
                        let count = columns.len();
                        let prepared = {
                            let views: [&[u64]; MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1] =
                                core::array::from_fn(|index| -> &[u64] {
                                    columns.get(index).map_or(&[], Vec::as_slice)
                                });
                            crate::metal::exact_root::prepare(
                                &views[..count],
                                domain.log_size,
                                root,
                                matches!(direction, GoldilocksTransformDirectionV1::Inverse),
                            )
                        };
                        // Dispatch failures may fall back only while originals
                        // are unchanged. Completed-output refusals stay typed
                        // and terminal; they are never ordinary device errors.
                        let published = publish_dispatch_result_v1(
                            prepared,
                            execution,
                            goldilocks_transform_completion_uncertain_v1(),
                            |prepared| {
                                let mut destinations: [&mut [u64];
                                    MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1] =
                                    core::array::from_fn(|_| -> &mut [u64] { &mut [] });
                                for (destination, column) in
                                    destinations.iter_mut().zip(columns.iter_mut())
                                {
                                    *destination = column;
                                }
                                prepared.publish_into(&mut destinations[..count])
                            },
                        )?;
                        if published.is_some() {
                            return Ok(actual);
                        }
                        drop(_lane);
                        transform_cpu_v1(columns, domain, direction);
                        return Ok(GoldilocksTransformBackendV1::Cpu);
                    }
                    let result = match direction {
                        GoldilocksTransformDirectionV1::Forward => {
                            crate::gpu::fft_columns_async(columns, domain.log_size, root, backend)
                        }
                        GoldilocksTransformDirectionV1::Inverse => {
                            crate::gpu::ifft_columns_async(columns, domain.log_size, root, backend)
                        }
                    }
                    .and_then(crate::gpu::ColumnDispatch::wait);
                    // Partial-batch cleanup can discover uncertain completion
                    // while an earlier ordinary error is being returned. Inspect
                    // the sticky owner state after every pending guard drops.
                    if classify_dispatch_result_v1(
                        result,
                        execution,
                        goldilocks_transform_completion_uncertain_v1(),
                    )?
                    .is_some()
                    {
                        return Ok(actual);
                    }
                }
            }
        }
    }
    if matches!(execution, ExecutionMode::Gpu) {
        return Err(GoldilocksTransformErrorV1::DeviceUnavailable);
    }
    transform_cpu_v1(columns, domain, direction);
    Ok(GoldilocksTransformBackendV1::Cpu)
}

#[cfg(feature = "fastpq-gpu")]
fn transform_lane_v1(execution: ExecutionMode) -> Option<std::sync::MutexGuard<'static, ()>> {
    if matches!(execution, ExecutionMode::Gpu) {
        Some(crate::backend::acquire_gpu_lane())
    } else {
        crate::backend::try_acquire_gpu_lane()
    }
}

#[cfg(feature = "fastpq-gpu")]
fn classify_dispatch_result_v1<T>(
    result: Result<T, crate::gpu::GpuError>,
    execution: ExecutionMode,
    uncertain_owner: bool,
) -> Result<Option<T>, GoldilocksTransformErrorV1> {
    check_dispatch_completion_v1(&result, uncertain_owner)?;
    match result {
        Ok(output) => Ok(Some(output)),
        Err(error) if matches!(execution, ExecutionMode::Gpu) => {
            Err(GoldilocksTransformErrorV1::DeviceFailure(error.to_string()))
        }
        Err(error) => {
            tracing::warn!(target: "fastpq::transform", %error,
                "exact-root device transform failed; using original columns on CPU");
            Ok(None)
        }
    }
}

/// Keep checked publication outside the ordinary dispatch fallback classifier.
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn publish_dispatch_result_v1<T, U>(
    result: Result<T, crate::gpu::GpuError>,
    execution: ExecutionMode,
    uncertain_owner: bool,
    publish: impl FnOnce(T) -> Result<U, GoldilocksTransformErrorV1>,
) -> Result<Option<U>, GoldilocksTransformErrorV1> {
    match classify_dispatch_result_v1(result, execution, uncertain_owner)? {
        Some(completed) => publish(completed).map(Some),
        None => Ok(None),
    }
}

#[cfg(feature = "fastpq-gpu")]
fn check_dispatch_completion_v1<T>(
    result: &Result<T, crate::gpu::GpuError>,
    uncertain_owner: bool,
) -> Result<(), GoldilocksTransformErrorV1> {
    if uncertain_owner
        || matches!(
            result,
            Err(crate::gpu::GpuError::CompletionUncertain { .. })
        )
    {
        Err(GoldilocksTransformErrorV1::CompletionUncertain)
    } else {
        Ok(())
    }
}

fn validate_v1<T: AsRef<[u64]> + Sync>(
    columns: &[T],
    root: u64,
) -> Result<cyclotomic::Domain, GoldilocksTransformErrorV1> {
    validate_with_limit_v1(columns, root, MAX_GOLDILOCKS_TRANSFORM_COLUMNS_V1)
}

fn validate_with_limit_v1<T: AsRef<[u64]> + Sync>(
    columns: &[T],
    root: u64,
    limit: usize,
) -> Result<cyclotomic::Domain, GoldilocksTransformErrorV1> {
    let rows = columns.first().map_or(0, |column| column.as_ref().len());
    if columns.is_empty()
        || columns.len() > limit
        || rows < 2
        || !rows.is_power_of_two()
        || rows.trailing_zeros() > 32
        || columns.iter().any(|column| column.as_ref().len() != rows)
        || rows
            .checked_mul(columns.len())
            .and_then(|n| n.checked_mul(8))
            .is_none()
    {
        return Err(GoldilocksTransformErrorV1::InvalidShape);
    }
    let order = rows as u64;
    if root == 0
        || root >= FIELD_MODULUS
        || power_v1(root, order) != 1
        || power_v1(root, order / 2) == 1
    {
        return Err(GoldilocksTransformErrorV1::InvalidRoot);
    }
    // The shape checks above established this public product and byte extent.
    if !canonical_word_columns_v1(columns, rows * columns.len()) {
        return Err(GoldilocksTransformErrorV1::NonCanonicalInput);
    }
    Ok(cyclotomic::Domain {
        log_size: rows.trailing_zeros(),
        generator: root,
    })
}

fn transform_cpu_v1(
    columns: &mut [Vec<u64>],
    domain: cyclotomic::Domain,
    direction: GoldilocksTransformDirectionV1,
) {
    columns.par_iter_mut().for_each(|column| match direction {
        GoldilocksTransformDirectionV1::Forward => cyclotomic::fft(column, domain),
        GoldilocksTransformDirectionV1::Inverse => cyclotomic::ifft(column, domain),
    });
}

fn power_v1(mut value: u64, mut exponent: u64) -> u64 {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = mul_mod_v1(result, value);
        }
        value = mul_mod_v1(value, value);
        exponent >>= 1;
    }
    result
}

/// Multiply two words modulo the Goldilocks prime with exact 128-bit arithmetic.
fn mul_mod_v1(left: u64, right: u64) -> u64 {
    let product = (u128::from(left) * u128::from(right)) % u128::from(FIELD_MODULUS);
    u64::try_from(product).expect("a residue modulo the Goldilocks prime fits u64")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_scan_policy_depends_only_on_public_extent_and_workers() {
        for workers in [1, 4, 20] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    assert!(!parallel_canonical_scan_v1(0));
                    assert!(!parallel_canonical_scan_v1(
                        CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1 - 1
                    ));
                    assert_eq!(
                        parallel_canonical_scan_v1(CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1),
                        workers > 1
                    );
                    assert_eq!(
                        parallel_canonical_scan_v1(2 * CANONICAL_SCAN_PARALLEL_MIN_WORDS_V1),
                        workers > 1
                    );
                });
        }
    }

    #[test]
    fn canonical_word_scans_preserve_all_small_positions_boundaries_and_refusal_order() {
        for rows in [0, 67, CANONICAL_SCAN_TILE_WORDS_V1 + 3] {
            let mut columns = vec![vec![FIELD_MODULUS - 1; rows]; 3];
            assert!(canonical_word_columns_v1(&columns, rows * 3));
            let positions = if rows <= 67 {
                (0..rows).collect::<Vec<_>>()
            } else {
                vec![
                    0,
                    2047,
                    2048,
                    CANONICAL_SCAN_TILE_WORDS_V1 - 1,
                    CANONICAL_SCAN_TILE_WORDS_V1,
                    rows - 1,
                ]
            };
            for column in 0..columns.len() {
                for &position in &positions {
                    for invalid in [FIELD_MODULUS, u64::MAX] {
                        columns[column][position] = invalid;
                        assert!(!canonical_word_slice_v1(&columns[column]));
                        assert!(!canonical_word_columns_v1(&columns, rows * 3));
                    }
                    columns[column][position] = FIELD_MODULUS - 1;
                }
            }
        }
        assert_eq!(
            validate_v1(&[vec![FIELD_MODULUS; 3]], 0).err(),
            Some(GoldilocksTransformErrorV1::InvalidShape)
        );
        assert_eq!(
            validate_v1(&[vec![FIELD_MODULUS; 8]], 0).err(),
            Some(GoldilocksTransformErrorV1::InvalidRoot)
        );
        assert_eq!(
            validate_v1(&[vec![FIELD_MODULUS; 8]], root_v1(3, 1)).err(),
            Some(GoldilocksTransformErrorV1::NonCanonicalInput)
        );
    }

    fn root_v1(log: u32, odd_power: u64) -> u64 {
        power_v1(power_v1(7, (FIELD_MODULUS - 1) >> log), odd_power)
    }

    fn horner_v1(coefficients: &[u64], point: u64) -> u64 {
        coefficients.iter().rev().fold(0, |value, coefficient| {
            u64::try_from(
                (u128::from(value) * u128::from(point) + u128::from(*coefficient))
                    % u128::from(FIELD_MODULUS),
            )
            .unwrap()
        })
    }

    #[test]
    fn prepared_cpu_and_input_refusals_preserve_borrowed_originals() {
        let original = vec![vec![17; 8]; 2];
        let views = original.iter().map(Vec::as_slice).collect::<Vec<_>>();
        assert!(matches!(
            prepare_goldilocks_columns_v1(
                &views,
                root_v1(3, 1),
                GoldilocksTransformDirectionV1::Inverse,
                ExecutionMode::Cpu,
            ),
            Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu)
        ));
        for (values, root, expected) in [
            (
                vec![vec![17; 3]],
                7,
                GoldilocksTransformErrorV1::InvalidShape,
            ),
            (
                vec![vec![17; 8]],
                1,
                GoldilocksTransformErrorV1::InvalidRoot,
            ),
            (
                vec![vec![FIELD_MODULUS; 8]],
                root_v1(3, 1),
                GoldilocksTransformErrorV1::NonCanonicalInput,
            ),
        ] {
            let before = values.clone();
            assert_eq!(
                prepare_goldilocks_columns_v1(
                    &values.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    root,
                    GoldilocksTransformDirectionV1::Inverse,
                    ExecutionMode::Auto,
                )
                .err()
                .unwrap(),
                expected
            );
            assert_eq!(values, before);
        }
        assert_eq!(original, vec![vec![17; 8]; 2]);
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn shared_prepared_dispatch_classification_preserves_auto_and_required_semantics() {
        use crate::{backend::GpuBackend, gpu::GpuError};
        for mode in [ExecutionMode::Auto, ExecutionMode::Gpu] {
            assert_eq!(
                classify_dispatch_result_v1(Ok(17), mode, false),
                Ok(Some(17))
            );
            let ordinary = classify_dispatch_result_v1::<()>(
                Err(GpuError::InvalidInput("drained fixture")),
                mode,
                false,
            );
            if matches!(mode, ExecutionMode::Auto) {
                assert_eq!(ordinary, Ok(None));
            } else {
                assert!(matches!(
                    ordinary,
                    Err(GoldilocksTransformErrorV1::DeviceFailure(_))
                ));
            }
            for result in [
                Ok(()),
                Err(GpuError::InvalidInput("drained fixture")),
                Err(GpuError::CompletionUncertain {
                    backend: GpuBackend::Metal,
                }),
            ] {
                assert_eq!(
                    classify_dispatch_result_v1(result, mode, true),
                    Err(GoldilocksTransformErrorV1::CompletionUncertain)
                );
            }
            assert_eq!(
                classify_dispatch_result_v1::<()>(
                    Err(GpuError::CompletionUncertain {
                        backend: GpuBackend::Metal
                    }),
                    mode,
                    false
                ),
                Err(GoldilocksTransformErrorV1::CompletionUncertain)
            );
        }
        let _lane = crate::backend::acquire_gpu_lane();
        assert!(transform_lane_v1(ExecutionMode::Auto).is_none());
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    #[test]
    fn completed_publication_refusals_never_become_auto_cpu_fallback() {
        for mode in [ExecutionMode::Auto, ExecutionMode::Gpu] {
            for error in [
                GoldilocksTransformErrorV1::InvalidShape,
                GoldilocksTransformErrorV1::NonCanonicalOutput,
                GoldilocksTransformErrorV1::CompletionUncertain,
            ] {
                let expected = error.clone();
                assert_eq!(
                    publish_dispatch_result_v1(Ok(()), mode, false, |()| { Err::<(), _>(error) }),
                    Err(expected)
                );
            }
            assert_eq!(
                publish_dispatch_result_v1(Ok(17), mode, false, |value| Ok(value + 1)),
                Ok(Some(18))
            );
        }
        assert_eq!(
            publish_dispatch_result_v1::<(), ()>(
                Err(crate::gpu::GpuError::InvalidInput(
                    "ordinary drained failure"
                )),
                ExecutionMode::Auto,
                false,
                |()| panic!("failed dispatch cannot publish"),
            ),
            Ok(None)
        );
        assert_eq!(
            publish_dispatch_result_v1(Ok(()), ExecutionMode::Auto, true, |()| {
                panic!("uncertain dispatch cannot publish")
            }),
            Err::<Option<()>, _>(GoldilocksTransformErrorV1::CompletionUncertain)
        );
    }

    #[test]
    fn prepared_payload_bound_covers_exact_pages_and_closed_metadata() {
        for rows in [2usize, 32, 1 << 19, 1 << 22] {
            for width in [1, 4, 8] {
                let expected = (rows * width * 8).div_ceil(METAL_PAGE_BYTES) * METAL_PAGE_BYTES
                    + 256
                    + core::mem::size_of::<PreparedGoldilocksColumnsV1>();
                assert_eq!(
                    prepared_goldilocks_columns_payload_bound_v1(rows, width).unwrap(),
                    expected
                );
            }
        }
        assert!(prepared_goldilocks_columns_payload_bound_v1(3, 1).is_err());
        assert!(prepared_goldilocks_columns_payload_bound_v1(8, 9).is_err());
        assert!(prepared_goldilocks_columns_payload_bound_v1(usize::MAX, 8).is_err());
    }

    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn uncertain_completion_is_terminal_even_after_an_ordinary_error_or_success() {
        use crate::{backend::GpuBackend, gpu::GpuError};
        let ordinary = Err(GpuError::InvalidInput("injected submission failure"));
        assert!(check_dispatch_completion_v1(&ordinary, false).is_ok());
        for result in [
            Ok(()),
            ordinary,
            Err(GpuError::CompletionUncertain {
                backend: GpuBackend::Metal,
            }),
        ] {
            assert_eq!(
                check_dispatch_completion_v1(&result, true),
                Err(GoldilocksTransformErrorV1::CompletionUncertain)
            );
        }
        assert_eq!(
            check_dispatch_completion_v1::<()>(
                &Err(GpuError::CompletionUncertain {
                    backend: GpuBackend::Cuda
                }),
                false
            ),
            Err(GoldilocksTransformErrorV1::CompletionUncertain)
        );
    }

    #[test]
    fn metal_payload_forecast_charges_one_staging_full_caches_and_rejects_shapes() {
        for (rows, columns) in [2, 1 << 8, 1 << 19, 1 << 22]
            .into_iter()
            .flat_map(|rows| [1, 2, 4, 8].into_iter().map(move |columns| (rows, columns)))
        {
            let bytes = metal_goldilocks_transform_extra_payload_v1(rows, columns).unwrap();
            assert_eq!(
                bytes,
                columns * rows * 8 + columns * 16383 + (64 << 20) + 66 * 4 * 256 * 8 + (1 << 20)
            );
        }
        for (rows, columns) in [(0, 1), (1, 1), (3, 1), (8, 0), (8, 9), (usize::MAX, 1)] {
            assert!(metal_goldilocks_transform_extra_payload_v1(rows, columns).is_err());
        }
        #[cfg(not(feature = "fastpq-gpu"))]
        assert_eq!(available_goldilocks_transform_backend_v1(), None);
    }

    #[test]
    fn explicit_roots_match_independent_evaluation_and_inverse() {
        for log in 1..=5 {
            for odd in [1, 3, 5] {
                let root = root_v1(log, odd);
                let original = (0_u64..3)
                    .map(|column| {
                        (0..(1_u64 << log))
                            .map(|row| row * 31 + column * 17 + 1)
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let expected = original
                    .iter()
                    .map(|column| {
                        (0..column.len())
                            .map(|row| horner_v1(column, power_v1(root, row as u64)))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let mut actual = original.clone();
                assert_eq!(
                    transform_goldilocks_columns_v1(
                        &mut actual,
                        root,
                        GoldilocksTransformDirectionV1::Forward,
                        ExecutionMode::Cpu
                    )
                    .unwrap(),
                    GoldilocksTransformBackendV1::Cpu
                );
                assert_eq!(actual, expected);
                transform_goldilocks_columns_v1(
                    &mut actual,
                    root,
                    GoldilocksTransformDirectionV1::Inverse,
                    ExecutionMode::Cpu,
                )
                .unwrap();
                assert_eq!(actual, original);
            }
        }
    }

    #[test]
    fn all_preflight_failures_preserve_every_input_word() {
        let root = root_v1(3, 1);
        let good = vec![vec![11; 8]; 2];
        let mut cases = vec![
            (Vec::new(), root),
            (vec![vec![11]], 1),
            (vec![vec![11; 3]], root),
            (vec![vec![11; 8]; 9], root),
            (vec![vec![11; 8], vec![11; 4]], root),
            (good.clone(), 0),
            (good.clone(), 1),
            (good.clone(), FIELD_MODULUS),
            (good.clone(), root_v1(2, 1)),
        ];
        let mut malformed = good;
        malformed[1][7] = FIELD_MODULUS;
        cases.push((malformed, root));
        for (mut columns, root) in cases {
            let before = columns.clone();
            assert!(
                transform_goldilocks_columns_v1(
                    &mut columns,
                    root,
                    GoldilocksTransformDirectionV1::Forward,
                    ExecutionMode::Gpu
                )
                .is_err()
            );
            assert_eq!(columns, before);
        }
    }

    #[cfg(not(feature = "fastpq-gpu"))]
    #[test]
    fn build_without_device_uses_explicit_cpu_fallback_and_rejects_required_gpu() {
        let root = root_v1(3, 1);
        let source = vec![vec![3, 5, 7, 11, 13, 17, 19, 23]];
        let mut automatic = source.clone();
        assert_eq!(
            transform_goldilocks_columns_v1(
                &mut automatic,
                root,
                GoldilocksTransformDirectionV1::Forward,
                ExecutionMode::Auto
            )
            .unwrap(),
            GoldilocksTransformBackendV1::Cpu
        );
        let mut required = source.clone();
        assert_eq!(
            transform_goldilocks_columns_v1(
                &mut required,
                root,
                GoldilocksTransformDirectionV1::Forward,
                ExecutionMode::Gpu
            ),
            Err(GoldilocksTransformErrorV1::DeviceUnavailable)
        );
        assert_eq!(required, source);
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    #[test]
    #[ignore = "actual required-Metal eight-column native19/common22 parity and cost diagnostic"]
    fn required_metal_native19_common22_exact_root_parity_and_timing() {
        use std::time::Instant;

        const COLUMNS: usize = 8;
        const NATIVE_LOG: u32 = 19;
        const COMMON_LOG: u32 = 22;
        const MASK_COEFFICIENTS: usize = 1816;
        assert_eq!(
            crate::backend::current_gpu_backend(),
            Some(crate::backend::GpuBackend::Metal),
            "this receipt requires the real Metal backend"
        );
        let native_root = root_v1(NATIVE_LOG, 1);
        let common_root = root_v1(COMMON_LOG, 1);
        let native_rows = 1 << NATIVE_LOG;
        let common_rows = 1 << COMMON_LOG;
        assert_metal_uses_supplied_roots();

        let source = (0..COLUMNS)
            .map(|column| {
                (0..native_rows)
                    .map(|row| (row * 31 + column * 17 + 1) as u64)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut coefficients = source.clone();
        let started = Instant::now();
        transform_goldilocks_columns_v1(
            &mut coefficients,
            native_root,
            GoldilocksTransformDirectionV1::Inverse,
            ExecutionMode::Cpu,
        )
        .unwrap();
        let cpu_inverse = started.elapsed();
        let mut actual = source;
        let started = Instant::now();
        assert_eq!(
            transform_goldilocks_columns_v1(
                &mut actual,
                native_root,
                GoldilocksTransformDirectionV1::Inverse,
                ExecutionMode::Gpu
            )
            .unwrap(),
            GoldilocksTransformBackendV1::Metal,
        );
        let metal_inverse = started.elapsed();
        assert_eq!(actual, coefficients);
        drop(actual);

        // Use every mask coefficient, including the highest X^(N+1815) term.
        apply_vanishing_masks(&mut coefficients, native_rows, MASK_COEFFICIENTS);
        let shifted = coset_shifted(&coefficients, common_rows);
        let mut expected = shifted.clone();
        let started = Instant::now();
        transform_goldilocks_columns_v1(
            &mut expected,
            common_root,
            GoldilocksTransformDirectionV1::Forward,
            ExecutionMode::Cpu,
        )
        .unwrap();
        let cpu_forward = started.elapsed();
        for (coefficients, values) in coefficients.iter().zip(&expected) {
            for index in [0, 1, 17, common_rows / 2 + 3, common_rows - 1] {
                let point = mul_mod_v1(7, power_v1(common_root, index as u64));
                assert_eq!(values[index], horner_v1(coefficients, point));
            }
        }
        for device_batch in [8, 4, 2] {
            let mut actual = shifted.clone();
            let started = Instant::now();
            for batch in actual.chunks_mut(device_batch) {
                assert_eq!(
                    transform_goldilocks_columns_v1(
                        batch,
                        common_root,
                        GoldilocksTransformDirectionV1::Forward,
                        ExecutionMode::Gpu
                    )
                    .unwrap(),
                    GoldilocksTransformBackendV1::Metal,
                );
            }
            let elapsed = started.elapsed();
            assert_eq!(actual, expected);
            eprintln!(
                "exact-root Metal transform: native_rows={native_rows}, common_rows={common_rows}, resident_columns={COLUMNS}, dispatch_columns={device_batch}, mask_coefficients={MASK_COEFFICIENTS}, cpu_inverse={cpu_inverse:?}, metal_inverse={metal_inverse:?}, cpu_forward={cpu_forward:?}, metal_forward={elapsed:?}; includes dispatch/staging/wait/clearing, excludes source construction, coset packing, row hashing, constraints and full proof; concurrent host load must be recorded"
            );
        }
    }

    /// Non-default primitive roots establish that dispatch uses the supplied
    /// root, not a catalog root hidden in a kernel or planner.
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn assert_metal_uses_supplied_roots() {
        for odd in [1, 3, 5] {
            let root = root_v1(4, odd);
            let mut values = vec![(0_u64..16).map(|i| i * 31 + 17).collect::<Vec<_>>()];
            let expected = (0..16)
                .map(|i| horner_v1(&values[0], power_v1(root, i)))
                .collect::<Vec<_>>();
            assert_eq!(
                transform_goldilocks_columns_v1(
                    &mut values,
                    root,
                    GoldilocksTransformDirectionV1::Forward,
                    ExecutionMode::Gpu
                )
                .unwrap(),
                GoldilocksTransformBackendV1::Metal,
            );
            assert_eq!(values[0], expected);
        }
    }

    /// Extend each coefficient column by `masks` terms: subtract each mask from
    /// coefficient `i` and place it at coefficient `native_rows + i`.
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn apply_vanishing_masks(coefficients: &mut [Vec<u64>], native_rows: usize, masks: usize) {
        for (column, coefficients) in coefficients.iter_mut().enumerate() {
            coefficients.resize(native_rows + masks, 0);
            for index in 0..masks {
                let mask = (index * 13 + column * 19 + 1) as u64;
                coefficients[index] = u64::try_from(
                    (u128::from(coefficients[index]) + u128::from(FIELD_MODULUS)
                        - u128::from(mask))
                        % u128::from(FIELD_MODULUS),
                )
                .expect("a residue modulo the Goldilocks prime fits u64");
                coefficients[native_rows + index] = mask;
            }
        }
    }

    /// Scale coefficient `i` of every column by `7^i` into a zero-padded
    /// `common_rows` vector, so a forward transform evaluates on the 7-coset.
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn coset_shifted(coefficients: &[Vec<u64>], common_rows: usize) -> Vec<Vec<u64>> {
        coefficients
            .iter()
            .map(|coefficients| {
                let mut values = vec![0; common_rows];
                let mut shift = 1;
                for (output, coefficient) in values.iter_mut().zip(coefficients) {
                    *output = mul_mod_v1(*coefficient, shift);
                    shift = mul_mod_v1(shift, 7);
                }
                values
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "goldilocks_prepared_forward_tests.rs"]
mod prepared_forward_tests;

#[cfg(test)]
#[path = "goldilocks_prefix_tests.rs"]
pub(crate) mod prefix_tests;

#[cfg(test)]
mod retained_prefix_api_tests {
    use super::*;

    #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
    #[test]
    fn cpu_only_completed_owner_keeps_consuming_and_unavailable_contract() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CompletedGoldilocksColumnsV1>();
        assert!(core::mem::needs_drop::<CompletedGoldilocksColumnsV1>());
        assert_eq!(core::mem::size_of::<CompletedGoldilocksColumnsV1>(), 0);
        let owner = CompletedGoldilocksColumnsV1 { _sealed: () };
        assert_eq!((owner.rows_v1(), owner.columns_v1()), (0, 0));
        assert_eq!(owner.column_v1(0), None);
        assert_eq!(
            owner.allocated_payload_bytes_v1(),
            Err(GoldilocksTransformErrorV1::DeviceUnavailable)
        );
        owner.erase_completed_v1();
    }

    #[test]
    fn production_prefix_cpu_validation_covers_entire_input_power_table_and_domain() {
        let rows = 4096;
        let root = power_v1(7, (FIELD_MODULUS - 1) >> 12);
        let original = vec![7; 2048 + 1816];
        let mut prefix = original.clone();
        let mut powers = vec![1; original.len()];
        assert!(matches!(
            prepare_goldilocks_prefix_columns_v1(
                &[&prefix],
                rows,
                root,
                &powers,
                ExecutionMode::Cpu
            ),
            Ok(PreparedGoldilocksColumnsOutcomeV1::Cpu)
        ));
        assert_eq!(prefix, original);
        *prefix.last_mut().unwrap() = FIELD_MODULUS;
        assert_eq!(
            prepare_goldilocks_prefix_columns_v1(
                &[&prefix],
                rows,
                root,
                &powers,
                ExecutionMode::Cpu
            )
            .err(),
            Some(GoldilocksTransformErrorV1::NonCanonicalInput)
        );
        assert_eq!(
            prepare_goldilocks_prefix_columns_v1(&[&prefix], rows, 1, &powers, ExecutionMode::Cpu)
                .err(),
            Some(GoldilocksTransformErrorV1::InvalidRoot)
        );
        assert_eq!(
            prepare_goldilocks_prefix_columns_v1(
                &[&prefix],
                rows - 1,
                1,
                &powers,
                ExecutionMode::Cpu
            )
            .err(),
            Some(GoldilocksTransformErrorV1::InvalidShape)
        );
        prefix.copy_from_slice(&original);
        *powers.last_mut().unwrap() = FIELD_MODULUS;
        assert_eq!(
            prepare_goldilocks_prefix_columns_v1(
                &[&prefix],
                rows,
                root,
                &powers,
                ExecutionMode::Cpu
            )
            .err(),
            Some(GoldilocksTransformErrorV1::NonCanonicalInput)
        );
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CompletedGoldilocksColumnsV1>();
    }

    #[test]
    fn retained_output_minimum_rows_match_actual_page_padding_and_metadata() {
        let rows = MIN_UNPADDED_GOLDILOCKS_COLUMN_ROWS_V1;
        assert!(rows.is_power_of_two());
        assert_eq!(rows * core::mem::size_of::<u64>(), METAL_PAGE_BYTES);
        // Half a column still occupies the complete page; the admission bound
        // accounts for those otherwise uncharged cells in generic callers.
        assert_eq!(
            prepared_goldilocks_columns_payload_bound_v1(rows / 2, 1),
            prepared_goldilocks_columns_payload_bound_v1(rows, 1)
        );
        let header =
            prepared_goldilocks_columns_payload_bound_v1(rows, 1).unwrap() - METAL_PAGE_BYTES;
        assert_eq!(
            prepared_goldilocks_columns_payload_bound_v1(rows, 8).unwrap(),
            8 * METAL_PAGE_BYTES + header
        );
    }
}
