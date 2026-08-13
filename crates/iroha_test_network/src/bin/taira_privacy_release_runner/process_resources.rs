//! Exact process-resource controls and accounting for isolated release stages.
#[cfg(target_os = "linux")]
use std::{fs, path::PathBuf};
use std::{
    mem::MaybeUninit,
    os::unix::process::ExitStatusExt,
    process::{Child, ExitStatus},
    time::Duration,
};
use iroha_core::privacy_release_evidence::{
    PRIVACY_RELEASE_STAGE_STACK_BYTES_V1, privacy_release_process_profile_v1,
};
use iroha_data_model::privacy::PrivacyProtocolIdV1;
use nix::{
    libc,
    sys::{
        resource::{Resource, getrlimit, rlim_t, setrlimit},
        signal::{Signal, kill},
    },
    unistd::Pid,
};
use super::{
    DynError, MAX_CHILD_RESULT_BYTES, MAX_STAGE_ADDRESS_SPACE_BYTES, MAX_STAGE_ELAPSED_MILLIS,
    MAX_STAGE_PEAK_RSS_BYTES, MAX_STAGE_SETUP_OPEN_FILES_V1, MAX_STAGE_TASKS_V1,
    MIN_STAGE_ADDRESS_SPACE_BYTES, MIN_STAGE_PEAK_RSS_BYTES,
};
pub(super) fn stage_option_names() -> Vec<&'static str> {
    vec![
        "protocol",
        "case",
        "out-fd",
        "elapsed-ceiling-ms",
        "peak-rss-ceiling-bytes",
        "address-space-ceiling-bytes",
    ]
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StageProcessCeilingsV1 {
    pub(super) elapsed_millis: u64,
    pub(super) peak_rss_bytes: u64,
    pub(super) address_space_bytes: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SampledProcessMemoryV1 {
    pub(super) peak_rss_bytes: u64,
    pub(super) peak_address_space_bytes: u64,
}
pub(super) struct WaitedStageChildV1 {
    pub(super) status: ExitStatus,
    pub(super) peak_rss_bytes: u64,
}
pub(super) struct StageChildGuardV1 {
    _child: Child,
    pid_raw: i32,
    reaped: bool,
}
impl StageChildGuardV1 {
    pub(super) fn new(mut child: Child) -> Result<Self, DynError> {
        let pid_raw = match i32::try_from(child.id()) {
            Ok(pid) if pid > 0 => pid,
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("child PID is outside the positive i32 process-id domain".into());
            }
        };
        Ok(Self {
            _child: child,
            pid_raw,
            reaped: false,
        })
    }
    pub(super) const fn pid_raw(&self) -> i32 {
        self.pid_raw
    }
    pub(super) fn try_wait4(&mut self) -> Result<Option<WaitedStageChildV1>, DynError> {
        let Some((status, usage)) = wait4_exact_pid(self.pid_raw, libc::WNOHANG)
            .map_err(|error| format!("wait4 failed for isolated stage: {error}"))?
        else {
            return Ok(None);
        };
        self.reaped = true;
        Ok(Some(WaitedStageChildV1 {
            status: ExitStatus::from_raw(status),
            peak_rss_bytes: rusage_peak_rss_bytes(&usage)?,
        }))
    }
}
impl Drop for StageChildGuardV1 {
    fn drop(&mut self) {
        if self.reaped {
            return;
        }
        let pid = Pid::from_raw(self.pid_raw);
        let _ = kill_stage_process_group(self.pid_raw, pid);
        // A blocking exact-PID wait after SIGKILL guarantees that no `?`
        // return path can leak a zombie or a running direct child.
        let _ = wait4_exact_pid(self.pid_raw, 0);
        self.reaped = true;
    }
}
pub(super) fn kill_stage_process_group(pid_raw: i32, pid: Pid) -> bool {
    let process_group = Pid::from_raw(pid_raw.saturating_neg());
    kill(process_group, Signal::SIGKILL)
        .or_else(|_| kill(pid, Signal::SIGKILL))
        .is_ok()
}
pub(super) fn wait4_exact_pid(
    pid_raw: i32,
    options: libc::c_int,
) -> std::io::Result<Option<(libc::c_int, libc::rusage)>> {
    loop {
        let mut status = 0;
        let mut usage = MaybeUninit::<libc::rusage>::uninit();
        // SAFETY: `status` and `usage` point to valid writable storage, the
        // positive PID selects exactly one owned child, and `wait4` initializes
        // `usage` whenever it returns that PID.
        let observed = unsafe { libc::wait4(pid_raw, &mut status, options, usage.as_mut_ptr()) };
        if observed == 0 {
            return Ok(None);
        }
        if observed == pid_raw {
            // SAFETY: a positive exact-PID `wait4` result initializes rusage.
            return Ok(Some((status, unsafe { usage.assume_init() })));
        }
        if observed < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("wait4 returned unexpected PID {observed} for child {pid_raw}"),
        ));
    }
}
pub(super) fn rusage_peak_rss_bytes(usage: &libc::rusage) -> Result<u64, DynError> {
    let raw = usage.ru_maxrss;
    let unsigned = u64::try_from(raw).map_err(|_| "kernel returned negative wait4 ru_maxrss")?;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        Ok(unsigned)
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        unsigned
            .checked_mul(1024)
            .ok_or_else(|| "kernel wait4 ru_maxrss byte conversion overflowed".into())
    }
}
pub(super) fn current_process_id_v1() -> Result<i32, DynError> {
    // SAFETY: `getpid` has no preconditions and cannot fail on Unix.
    let pid = unsafe { libc::getpid() };
    if pid <= 1 {
        return Err("release runner PID is outside the governed process domain".into());
    }
    Ok(pid)
}
#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub(super) fn install_pre_exec_stage_stack_limit_v1() -> std::io::Result<()> {
    let stack_limit: libc::rlim_t = PRIVACY_RELEASE_STAGE_STACK_BYTES_V1
        .try_into()
        .map_err(|_| std::io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    let mut inherited = MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: successful getrlimit initializes the complete output structure.
    if unsafe { libc::getrlimit(libc::RLIMIT_STACK, inherited.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: successful getrlimit initialized the structure.
    let inherited = unsafe { inherited.assume_init() };
    if inherited.rlim_max != libc::RLIM_INFINITY && inherited.rlim_max < stack_limit {
        return Err(std::io::Error::from_raw_os_error(libc::EPERM));
    }
    let exact = libc::rlimit {
        rlim_cur: stack_limit,
        rlim_max: stack_limit,
    };
    // SAFETY: `exact` is fully initialized and affects only the calling child.
    if unsafe { libc::setrlimit(libc::RLIMIT_STACK, &exact) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut observed = MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: successful getrlimit initializes the complete output structure.
    if unsafe { libc::getrlimit(libc::RLIMIT_STACK, observed.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: successful getrlimit initialized the structure.
    let observed = unsafe { observed.assume_init() };
    if observed.rlim_cur != stack_limit || observed.rlim_max != stack_limit {
        return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
    }
    Ok(())
}
pub(super) fn install_hidden_stage_resource_limits(
    elapsed_ceiling_millis: u64,
    address_space_ceiling_bytes: u64,
) -> Result<(), DynError> {
    // The sealed Taira runner is Linux-only. RLIMIT_AS is a separately frozen
    // virtual-address-space containment bound; it must not be conflated with
    // the measured resident-memory ceiling.
    let allocation_limit: rlim_t = address_space_ceiling_bytes
        .try_into()
        .map_err(|_| "stage address-space ceiling exceeds rlim_t")?;
    let cpu_seconds = checked_stage_cpu_limit_seconds_v1(elapsed_ceiling_millis)
        .ok_or("stage CPU ceiling overflowed")?;
    let cpu_limit: rlim_t = cpu_seconds
        .try_into()
        .map_err(|_| "stage CPU ceiling exceeds rlim_t")?;
    let file_size_limit: rlim_t = MAX_CHILD_RESULT_BYTES
        .try_into()
        .map_err(|_| "stage output ceiling exceeds rlim_t")?;
    let stack_limit: rlim_t = PRIVACY_RELEASE_STAGE_STACK_BYTES_V1
        .try_into()
        .map_err(|_| "stage stack size exceeds rlim_t")?;
    setrlimit(Resource::RLIMIT_CORE, 0, 0)
        .map_err(|error| format!("cannot disable hidden-stage core dumps: {error}"))?;
    setrlimit(Resource::RLIMIT_FSIZE, file_size_limit, file_size_limit)
        .map_err(|error| format!("cannot install hidden-stage file-size limit: {error}"))?;
    let (actual_stack_soft, actual_stack_hard) = getrlimit(Resource::RLIMIT_STACK)
        .map_err(|error| format!("cannot verify inherited hidden-stage stack limit: {error}"))?;
    if actual_stack_soft != stack_limit || actual_stack_hard != stack_limit {
        return Err(format!(
            "inherited hidden-stage stack limit is not exact: soft={actual_stack_soft}, hard={actual_stack_hard}, expected={stack_limit}"
        )
        .into());
    }
    setrlimit(Resource::RLIMIT_AS, allocation_limit, allocation_limit)
        .map_err(|error| format!("cannot install hidden-stage address-space limit: {error}"))?;
    setrlimit(Resource::RLIMIT_CPU, cpu_limit, cpu_limit)
        .map_err(|error| format!("cannot install hidden-stage CPU limit: {error}"))?;
    let open_file_limit: rlim_t = MAX_STAGE_SETUP_OPEN_FILES_V1
        .try_into()
        .map_err(|_| "stage open-file ceiling exceeds rlim_t")?;
    setrlimit(Resource::RLIMIT_NOFILE, open_file_limit, open_file_limit)
        .map_err(|error| format!("cannot install hidden-stage open-file limit: {error}"))?;
    Ok(())
}
pub(super) fn checked_stage_cpu_limit_seconds_v1(elapsed_ceiling_millis: u64) -> Option<u64> {
    if elapsed_ceiling_millis == 0 || elapsed_ceiling_millis > MAX_STAGE_ELAPSED_MILLIS {
        return None;
    }
    // RLIMIT_CPU accounts aggregate process CPU time across every thread.
    // Scale the wall-clock allowance by the exact task ceiling so four busy
    // Rayon workers cannot exhaust the fallback CPU guard before the parent
    // reaches the authoritative wall-clock deadline.
    elapsed_ceiling_millis
        .div_ceil(1_000)
        .checked_mul(MAX_STAGE_TASKS_V1)?
        .checked_add(1)
}
pub(super) fn validate_process_ceilings(
    elapsed_ceiling_millis: u64,
    peak_rss_ceiling_bytes: u64,
    address_space_ceiling_bytes: u64,
) -> Result<(), DynError> {
    if elapsed_ceiling_millis == 0 || elapsed_ceiling_millis > MAX_STAGE_ELAPSED_MILLIS {
        return Err(
            format!("stage elapsed ceiling must be within 1..={MAX_STAGE_ELAPSED_MILLIS}").into(),
        );
    }
    if !(MIN_STAGE_PEAK_RSS_BYTES..=MAX_STAGE_PEAK_RSS_BYTES).contains(&peak_rss_ceiling_bytes) {
        return Err(format!(
            "stage peak RSS ceiling must be within {MIN_STAGE_PEAK_RSS_BYTES}..={MAX_STAGE_PEAK_RSS_BYTES}"
        )
        .into());
    }
    if !(MIN_STAGE_ADDRESS_SPACE_BYTES..=MAX_STAGE_ADDRESS_SPACE_BYTES)
        .contains(&address_space_ceiling_bytes)
    {
        return Err(format!(
            "stage address-space ceiling must be within {MIN_STAGE_ADDRESS_SPACE_BYTES}..={MAX_STAGE_ADDRESS_SPACE_BYTES}"
        )
        .into());
    }
    if address_space_ceiling_bytes < peak_rss_ceiling_bytes {
        return Err(
            "stage address-space ceiling must not be smaller than its peak RSS ceiling".into(),
        );
    }
    Ok(())
}
pub(super) fn canonical_stage_process_ceilings_v1(
    protocol_id: PrivacyProtocolIdV1,
    elapsed_ceiling_millis: u64,
    peak_rss_ceiling_bytes: u64,
    address_space_ceiling_bytes: u64,
) -> Result<StageProcessCeilingsV1, DynError> {
    // A fixed protocol profile governs all three process boundaries exactly.
    // Generic protocols retain the reviewed caller-supplied limits.
    let ceilings = match privacy_release_process_profile_v1(protocol_id) {
        Some(profile) => {
            if profile.protocol_id != protocol_id {
                return Err("core returned a mismatched privacy release process profile".into());
            }
            StageProcessCeilingsV1 {
                elapsed_millis: profile.elapsed_ceiling_millis,
                peak_rss_bytes: profile.peak_rss_ceiling_bytes,
                address_space_bytes: profile.address_space_ceiling_bytes,
            }
        }
        None => StageProcessCeilingsV1 {
            elapsed_millis: elapsed_ceiling_millis,
            peak_rss_bytes: peak_rss_ceiling_bytes,
            address_space_bytes: address_space_ceiling_bytes,
        },
    };
    validate_process_ceilings(
        ceilings.elapsed_millis,
        ceilings.peak_rss_bytes,
        ceilings.address_space_bytes,
    )?;
    Ok(ceilings)
}
pub(super) fn validate_stage_process_ceilings_v1(
    protocol_id: PrivacyProtocolIdV1,
    elapsed_ceiling_millis: u64,
    peak_rss_ceiling_bytes: u64,
    address_space_ceiling_bytes: u64,
) -> Result<(), DynError> {
    let supplied = StageProcessCeilingsV1 {
        elapsed_millis: elapsed_ceiling_millis,
        peak_rss_bytes: peak_rss_ceiling_bytes,
        address_space_bytes: address_space_ceiling_bytes,
    };
    let canonical = canonical_stage_process_ceilings_v1(
        protocol_id,
        elapsed_ceiling_millis,
        peak_rss_ceiling_bytes,
        address_space_ceiling_bytes,
    )?;
    if supplied != canonical {
        return Err(format!(
            "stage {} does not match its canonical protocol-specific process profile",
            protocol_id.canonical_label()
        )
        .into());
    }
    Ok(())
}
fn parse_proc_status_kib_v1(value: &str, field: &str) -> Result<u64, DynError> {
    let mut fields = value.split_ascii_whitespace();
    let Some(decimal) = fields.next() else {
        return Err(format!("child {field} has an unexpected kernel format").into());
    };
    if fields.next() != Some("kB") || fields.next().is_some() {
        return Err(format!("child {field} has an unexpected kernel format").into());
    }
    if decimal.is_empty()
        || !decimal.bytes().all(|byte| byte.is_ascii_digit())
        || (decimal.len() > 1 && decimal.starts_with('0'))
    {
        return Err(format!("child {field} is not canonical unsigned decimal").into());
    }
    let kib = decimal
        .parse::<u64>()
        .map_err(|_| format!("child {field} is outside u64"))?;
    kib.checked_mul(1024)
        .ok_or_else(|| format!("child {field} byte conversion overflowed").into())
}
pub(super) fn parse_process_status_memory_v1(
    bytes: &[u8],
) -> Result<SampledProcessMemoryV1, DynError> {
    if bytes.len() > 1024 * 1024 {
        return Err("child /proc status unexpectedly exceeds 1 MiB".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "child /proc status is not UTF-8")?;
    let mut peak_rss_bytes = None;
    let mut peak_address_space_bytes = None;
    for line in text.lines() {
        let Some((field, value)) = line.split_once(':') else {
            continue;
        };
        let slot = match field {
            "VmHWM" => &mut peak_rss_bytes,
            "VmPeak" => &mut peak_address_space_bytes,
            _ => continue,
        };
        if slot.is_some() {
            return Err(format!("child /proc status contains duplicate {field}").into());
        }
        *slot = Some(parse_proc_status_kib_v1(value, field)?);
    }
    Ok(SampledProcessMemoryV1 {
        peak_rss_bytes: peak_rss_bytes.ok_or("child /proc status omitted VmHWM")?,
        peak_address_space_bytes: peak_address_space_bytes
            .ok_or("child /proc status omitted VmPeak")?,
    })
}
pub(super) fn sample_process_memory_v1(pid: i32) -> Result<SampledProcessMemoryV1, DynError> {
    #[cfg(target_os = "linux")]
    {
        if pid <= 0 {
            return Err("child PID must be positive for exact /proc sampling".into());
        }
        let status_path = PathBuf::from(format!("/proc/{pid}/status"));
        let bytes = match fs::read(&status_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SampledProcessMemoryV1::default());
            }
            Err(error) => {
                return Err(format!("failed to sample child process memory: {error}").into());
            }
        };
        parse_process_status_memory_v1(&bytes)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        Ok(SampledProcessMemoryV1::default())
    }
}
pub(super) fn elapsed_millis_ceil(duration: Duration) -> Result<u64, DynError> {
    let nanos = duration.as_nanos();
    let millis = nanos
        .checked_add(999_999)
        .ok_or("elapsed duration overflow")?
        / 1_000_000;
    u64::try_from(millis).map_err(|_| "elapsed milliseconds exceed u64".into())
}
