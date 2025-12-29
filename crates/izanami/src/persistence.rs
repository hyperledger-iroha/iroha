//! Simple persistence helpers for storing and loading Izanami configurations.

use std::{fs, io, path::PathBuf, time::Duration};

use color_eyre::{Result, eyre::eyre};
use dirs::config_dir;
use norito::codec::{Decode, Encode};

use crate::config::{ChaosConfig, FaultArgs, FaultToggles, IzanamiArgs};

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
    fs::create_dir_all(dir)?;

    let mut args_clone = args.clone();
    args_clone.tui = false;
    let stored = StoredArgs::from_args(&args_clone)?;
    let bytes = stored.encode();
    fs::write(path, bytes)?;
    Ok(())
}

pub fn store_config(config: &ChaosConfig) -> Result<()> {
    store_args(&IzanamiArgs::from_config(config))
}
