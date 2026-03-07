//! Simple persistence helpers for storing and loading Izanami configurations.

use std::{fs, io, path::PathBuf, time::Duration};

use color_eyre::{Result, eyre::eyre};
use dirs::config_dir;
use norito::codec::{Decode, Encode};
use tracing::warn;

use crate::config::{ChaosConfig, FaultArgs, FaultToggles, IzanamiArgs, WorkloadProfile};

const APP_DIR: &str = "izanami";
const CONFIG_FILE: &str = "config.bin";
const SUPPORTED_FAULT_FLAGS_MASK: u8 = 0b0000_1111;

#[derive(Clone, Encode, Decode)]
struct StoredArgs {
    peers: u32,
    faulty: u32,
    duration_ms: u64,
    seed: Option<u64>,
    tps: f64,
    max_inflight: u32,
    workload_profile: u8,
    allow_contract_deploy_in_stable: bool,
    log_filter: String,
    fault_min_ms: u64,
    fault_max_ms: u64,
    fault_flags: u8,
    nexus: bool,
    allow_net: bool,
    pipeline_time_ms: Option<u64>,
    target_blocks: Option<u64>,
    progress_interval_ms: u64,
    progress_timeout_ms: u64,
}

fn workload_profile_to_u8(profile: WorkloadProfile) -> u8 {
    match profile {
        WorkloadProfile::Stable => 0,
        WorkloadProfile::Chaos => 1,
    }
}

fn workload_profile_from_u8(value: u8) -> Result<WorkloadProfile> {
    match value {
        0 => Ok(WorkloadProfile::Stable),
        1 => Ok(WorkloadProfile::Chaos),
        _ => Err(eyre!(
            "unsupported persisted workload profile value {value}; expected 0 (stable) or 1 (chaos)"
        )),
    }
}

fn fault_toggles_from_bits(bits: u8) -> Result<FaultToggles> {
    if bits & !SUPPORTED_FAULT_FLAGS_MASK != 0 {
        return Err(eyre!(
            "unsupported persisted fault flag bits 0x{bits:02x}; only low 4 bits are allowed"
        ));
    }
    Ok(FaultToggles::from_bits(bits))
}

fn duration_to_ms(duration: Duration, label: &str) -> Result<u64> {
    u64::try_from(duration.as_millis())
        .map_err(|_| eyre!("{label} {duration:?} too large to persist"))
}

fn maybe_duration_to_ms(duration: Option<Duration>, label: &str) -> Result<Option<u64>> {
    duration
        .map(|value| duration_to_ms(value, label))
        .transpose()
}

impl StoredArgs {
    fn from_args(args: &IzanamiArgs) -> Result<Self> {
        let peers = u32::try_from(args.peers)
            .map_err(|_| eyre!("peer count {} exceeds persistence limits", args.peers))?;
        let faulty = u32::try_from(args.faulty)
            .map_err(|_| eyre!("faulty count {} exceeds persistence limits", args.faulty))?;
        let duration_ms = duration_to_ms(args.duration, "duration")?;
        let pipeline_time_ms = maybe_duration_to_ms(args.pipeline_time, "pipeline time")?;
        let progress_interval_ms = duration_to_ms(args.progress_interval, "progress interval")?;
        let progress_timeout_ms = duration_to_ms(args.progress_timeout, "progress timeout")?;
        let max_inflight = u32::try_from(args.max_inflight).map_err(|_| {
            eyre!(
                "max_inflight {} exceeds persistence limits",
                args.max_inflight
            )
        })?;
        let fault_min_ms = duration_to_ms(args.fault_interval_min, "fault interval min")?;
        let fault_max_ms = duration_to_ms(args.fault_interval_max, "fault interval max")?;
        Ok(Self {
            peers,
            faulty,
            duration_ms,
            seed: args.seed,
            tps: args.tps,
            max_inflight,
            workload_profile: workload_profile_to_u8(args.workload_profile),
            allow_contract_deploy_in_stable: args.allow_contract_deploy_in_stable,
            log_filter: args.log_filter.clone(),
            fault_min_ms,
            fault_max_ms,
            fault_flags: args.faults.to_toggles().bits(),
            nexus: args.nexus,
            allow_net: args.allow_net,
            pipeline_time_ms,
            target_blocks: args.target_blocks,
            progress_interval_ms,
            progress_timeout_ms,
        })
    }

    fn into_args(self) -> Result<IzanamiArgs> {
        let to_duration = |ms: u64| -> Result<Duration> { Ok(Duration::from_millis(ms)) };
        let fault_toggles = fault_toggles_from_bits(self.fault_flags)?;
        let workload_profile = workload_profile_from_u8(self.workload_profile)?;
        Ok(IzanamiArgs {
            tui: false,
            allow_net: self.allow_net,
            peers: self.peers as usize,
            faulty: self.faulty as usize,
            duration: to_duration(self.duration_ms)?,
            pipeline_time: self.pipeline_time_ms.map(Duration::from_millis),
            target_blocks: self.target_blocks,
            progress_interval: Duration::from_millis(self.progress_interval_ms),
            progress_timeout: Duration::from_millis(self.progress_timeout_ms),
            seed: self.seed,
            tps: self.tps,
            max_inflight: self.max_inflight as usize,
            workload_profile,
            allow_contract_deploy_in_stable: self.allow_contract_deploy_in_stable,
            log_filter: self.log_filter,
            fault_interval_min: to_duration(self.fault_min_ms)?,
            fault_interval_max: to_duration(self.fault_max_ms)?,
            faults: FaultArgs::from(fault_toggles),
            nexus: self.nexus,
        })
    }
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(APP_DIR).join(CONFIG_FILE))
}

pub fn load_args() -> Result<Option<IzanamiArgs>> {
    let Some(path) = config_path() else {
        return Ok(None);
    };
    let data = match fs::read(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            warn!(
                target: "izanami::persistence",
                path = %path.display(),
                "config dir not readable; skipping persisted settings"
            );
            return Ok(None);
        }
        Err(err) => return Err(err.into()),
    };

    let mut reader = &data[..];
    let stored = StoredArgs::decode(&mut reader).map_err(|e| eyre!("decode failed: {e}"))?;
    stored.into_args().map(Some)
}

pub fn store_args(args: &IzanamiArgs) -> Result<()> {
    let Some(path) = config_path() else {
        return Ok(());
    };
    let dir = path.parent().unwrap();
    if let Err(err) = fs::create_dir_all(dir) {
        if err.kind() == io::ErrorKind::PermissionDenied {
            warn!(
                target: "izanami::persistence",
                path = %dir.display(),
                "config dir not writable; skipping persistence"
            );
            return Ok(());
        }
        return Err(err.into());
    }

    let mut args_clone = args.clone();
    args_clone.tui = false;
    let stored = StoredArgs::from_args(&args_clone)?;
    let bytes = stored.encode();
    if let Err(err) = fs::write(&path, bytes) {
        if err.kind() == io::ErrorKind::PermissionDenied {
            warn!(
                target: "izanami::persistence",
                path = %path.display(),
                "config file not writable; skipping persistence"
            );
            return Ok(());
        }
        return Err(err.into());
    }
    Ok(())
}

pub fn store_config(config: &ChaosConfig) -> Result<()> {
    store_args(&IzanamiArgs::from_config(config))
}

#[cfg(test)]
mod decode_strict_tests {
    use super::*;

    fn stored_args_fixture() -> StoredArgs {
        StoredArgs {
            peers: 4,
            faulty: 1,
            duration_ms: 120_000,
            seed: Some(42),
            tps: 12.5,
            max_inflight: 32,
            workload_profile: 0,
            allow_contract_deploy_in_stable: false,
            log_filter: "info".to_owned(),
            fault_min_ms: 5_000,
            fault_max_ms: 20_000,
            fault_flags: SUPPORTED_FAULT_FLAGS_MASK,
            nexus: false,
            allow_net: true,
            pipeline_time_ms: Some(2_000),
            target_blocks: Some(100),
            progress_interval_ms: 15_000,
            progress_timeout_ms: 120_000,
        }
    }

    #[test]
    fn stored_args_rejects_unknown_workload_profile_value() {
        let mut stored = stored_args_fixture();
        stored.workload_profile = 2;
        let err = stored
            .into_args()
            .expect_err("unknown workload profile must be rejected");
        assert!(
            err.to_string()
                .contains("unsupported persisted workload profile"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn stored_args_rejects_unknown_fault_flag_bits() {
        let mut stored = stored_args_fixture();
        stored.fault_flags = SUPPORTED_FAULT_FLAGS_MASK | 0b1000_0000;
        let err = stored
            .into_args()
            .expect_err("unknown fault flag bits must be rejected");
        assert!(
            err.to_string()
                .contains("unsupported persisted fault flag bits"),
            "unexpected error: {err}"
        );
    }
}

#[cfg(all(unix, target_os = "linux"))]
mod tests {
    use std::{env, fs, os::unix::fs::PermissionsExt, path::PathBuf};

    use super::*;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        #[allow(unsafe_code)]
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            // Safety: test-only environment changes are scoped to the guard.
            unsafe {
                env::set_var(key, value);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            if let Some(prev) = &self.previous {
                // Safety: test-only environment changes are scoped to the guard.
                unsafe {
                    env::set_var(self.key, prev);
                }
            } else {
                // Safety: test-only environment changes are scoped to the guard.
                unsafe {
                    env::remove_var(self.key);
                }
            }
        }
    }

    fn readonly_dir(label: &str) -> Result<PathBuf> {
        let dir = env::temp_dir().join(format!("izanami-{label}-{}", std::process::id()));
        fs::create_dir_all(&dir)?;
        let mut perms = fs::metadata(&dir)?.permissions();
        perms.set_mode(0o500);
        fs::set_permissions(&dir, perms)?;
        Ok(dir)
    }

    fn restore_dir(path: &PathBuf) -> Result<()> {
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(path, perms)?;
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    fn temp_config_dir(label: &str) -> Result<PathBuf> {
        let dir = env::temp_dir().join(format!("izanami-{label}-{}", std::process::id()));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    #[test]
    fn store_args_skips_permission_denied() -> Result<()> {
        let dir = readonly_dir("perm-store")?;
        let _guard = EnvGuard::set("XDG_CONFIG_HOME", dir.to_string_lossy().as_ref());

        let args = IzanamiArgs::defaults();
        assert!(store_args(&args).is_ok());

        restore_dir(&dir)?;
        Ok(())
    }

    #[test]
    fn load_args_skips_permission_denied() -> Result<()> {
        let dir = env::temp_dir().join(format!("izanami-perm-load-{}", std::process::id()));
        fs::create_dir_all(&dir)?;
        let config_dir = dir.join(APP_DIR);
        fs::create_dir_all(&config_dir)?;
        let config_file = config_dir.join(CONFIG_FILE);
        fs::write(&config_file, b"locked")?;
        let mut perms = fs::metadata(&config_file)?.permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&config_file, perms)?;

        let _guard = EnvGuard::set("XDG_CONFIG_HOME", dir.to_string_lossy().as_ref());
        let loaded = load_args()?;
        assert!(loaded.is_none());

        let mut perms = fs::metadata(&config_file)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&config_file, perms)?;
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn store_and_load_roundtrip_persists_progress_settings() -> Result<()> {
        let dir = temp_config_dir("roundtrip")?;
        let _guard = EnvGuard::set("XDG_CONFIG_HOME", dir.to_string_lossy().as_ref());

        let args = IzanamiArgs {
            tui: false,
            allow_net: true,
            peers: 5,
            faulty: 1,
            duration: Duration::from_secs(90),
            pipeline_time: Some(Duration::from_millis(250)),
            target_blocks: Some(42),
            progress_interval: Duration::from_secs(7),
            progress_timeout: Duration::from_secs(55),
            seed: Some(123),
            tps: 12.5,
            max_inflight: 64,
            workload_profile: WorkloadProfile::Chaos,
            allow_contract_deploy_in_stable: true,
            log_filter: "debug".to_string(),
            fault_interval_min: Duration::from_secs(3),
            fault_interval_max: Duration::from_secs(9),
            faults: FaultArgs {
                network_latency: false,
                network_partition: true,
                cpu_stress: false,
                disk_saturation: true,
            },
            nexus: true,
        };

        store_args(&args)?;
        let loaded = load_args()?.expect("persisted args should load");

        assert_eq!(loaded.allow_net, args.allow_net);
        assert_eq!(loaded.peers, args.peers);
        assert_eq!(loaded.faulty, args.faulty);
        assert_eq!(loaded.duration, args.duration);
        assert_eq!(loaded.pipeline_time, args.pipeline_time);
        assert_eq!(loaded.target_blocks, args.target_blocks);
        assert_eq!(loaded.progress_interval, args.progress_interval);
        assert_eq!(loaded.progress_timeout, args.progress_timeout);
        assert_eq!(loaded.seed, args.seed);
        assert_eq!(loaded.tps, args.tps);
        assert_eq!(loaded.max_inflight, args.max_inflight);
        assert_eq!(loaded.workload_profile, args.workload_profile);
        assert_eq!(
            loaded.allow_contract_deploy_in_stable,
            args.allow_contract_deploy_in_stable
        );
        assert_eq!(loaded.log_filter, args.log_filter);
        assert_eq!(loaded.fault_interval_min, args.fault_interval_min);
        assert_eq!(loaded.fault_interval_max, args.fault_interval_max);
        assert_eq!(
            loaded.faults.to_toggles().bits(),
            args.faults.to_toggles().bits()
        );
        assert_eq!(loaded.nexus, args.nexus);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
