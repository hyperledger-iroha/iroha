//! Simple persistence helpers for storing and loading Izanami configurations.

use std::{fs, io, path::PathBuf, time::Duration};

use color_eyre::{Result, eyre::eyre};
use dirs::config_dir;
use norito::codec::{Decode, Encode};
use tracing::warn;

use crate::config::{
    ChaosConfig, FaultArgs, FaultToggles, IzanamiArgs, DEFAULT_PROGRESS_INTERVAL,
    DEFAULT_PROGRESS_TIMEOUT,
};

const APP_DIR: &str = "izanami";
const CONFIG_FILE: &str = "config.bin";

#[derive(Clone, Encode, Decode)]
struct StoredArgs {
    peers: u32,
    faulty: u32,
    duration_ms: u64,
    seed: Option<u64>,
    tps: f64,
    max_inflight: u32,
    log_filter: String,
    fault_min_ms: u64,
    fault_max_ms: u64,
    fault_flags: u8,
    nexus: bool,
    allow_net: bool,
}

impl StoredArgs {
    fn from_args(args: &IzanamiArgs) -> Result<Self> {
        let peers = u32::try_from(args.peers)
            .map_err(|_| eyre!("peer count {} exceeds persistence limits", args.peers))?;
        let faulty = u32::try_from(args.faulty)
            .map_err(|_| eyre!("faulty count {} exceeds persistence limits", args.faulty))?;
        let duration_ms = u64::try_from(args.duration.as_millis())
            .map_err(|_| eyre!("duration {:?} too large to persist", args.duration))?;
        let max_inflight = u32::try_from(args.max_inflight).map_err(|_| {
            eyre!(
                "max_inflight {} exceeds persistence limits",
                args.max_inflight
            )
        })?;
        let fault_min_ms = u64::try_from(args.fault_interval_min.as_millis()).map_err(|_| {
            eyre!(
                "fault interval min {:?} too large to persist",
                args.fault_interval_min
            )
        })?;
        let fault_max_ms = u64::try_from(args.fault_interval_max.as_millis()).map_err(|_| {
            eyre!(
                "fault interval max {:?} too large to persist",
                args.fault_interval_max
            )
        })?;
        Ok(Self {
            peers,
            faulty,
            duration_ms,
            seed: args.seed,
            tps: args.tps,
            max_inflight,
            log_filter: args.log_filter.clone(),
            fault_min_ms,
            fault_max_ms,
            fault_flags: args.faults.to_toggles().bits(),
            nexus: args.nexus,
            allow_net: args.allow_net,
        })
    }

    fn into_args(self) -> Result<IzanamiArgs> {
        let to_duration = |ms: u64| -> Result<Duration> { Ok(Duration::from_millis(ms)) };
        let fault_toggles = FaultToggles::from_bits(self.fault_flags);
        Ok(IzanamiArgs {
            tui: false,
            allow_net: self.allow_net,
            peers: self.peers as usize,
            faulty: self.faulty as usize,
            duration: to_duration(self.duration_ms)?,
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: self.seed,
            tps: self.tps,
            max_inflight: self.max_inflight as usize,
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

#[cfg(all(unix, target_os = "linux"))]
mod tests {
    use std::{
        env,
        fs,
        os::unix::fs::PermissionsExt,
        path::PathBuf,
    };

    use super::*;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(prev) = &self.previous {
                env::set_var(self.key, prev);
            } else {
                env::remove_var(self.key);
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
}
