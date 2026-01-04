//! Kagami localnet bootstrap coverage for permissioned Sumeragi.

use std::{
    any::Any,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    config::{Config, LoadPath},
    data_model::{Level, isi::Log},
};
use iroha_test_network::{
    Program, fslock_ports::AllocatedPortBlock, init_instruction_registry, repo_root,
};
use tempfile::TempDir;
use tokio::time::sleep;

const LOCALNET_PEERS: u16 = 4;
const READY_TIMEOUT: Duration = Duration::from_secs(180);
const READY_POLL: Duration = Duration::from_millis(200);
const LOCALNET_BLOCK_TIME_MS: u64 = 2_000;
const LOCALNET_COMMIT_TIME_MS: u64 = 2_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kagami_localnet_bootstrap_produces_blocks() -> Result<()> {
    init_instruction_registry();
    let _guard = sandbox::serial_guard();

    let temp_dir = localnet_tempdir()?;
    let out_dir = temp_dir.path().join("localnet");
    let result: Result<()> = async {
        let api_ports = alloc_port_block(LOCALNET_PEERS)?;
        let p2p_ports = alloc_port_block(LOCALNET_PEERS)?;
        let base_api_port = api_ports.base();
        let base_p2p_port = p2p_ports.base();
        generate_localnet(&out_dir, base_api_port, base_p2p_port)?;
        let irohad_bin = Program::Irohad
            .resolve()
            .wrap_err("resolve irohad binary")?;
        let _localnet = KagamiLocalnet::start(
            &out_dir,
            &irohad_bin,
            LOCALNET_PEERS,
            (api_ports, p2p_ports),
        )?;

        let client = load_localnet_client(&out_dir)?;
        wait_for_status_ready(&client, READY_TIMEOUT).await?;
        let baseline = client.get_status()?.blocks_non_empty;
        client.submit_blocking(Log::new(Level::INFO, "kagami localnet smoke".to_string()))?;
        let status =
            wait_for_blocks_non_empty(&client, baseline.saturating_add(1), READY_TIMEOUT).await?;
        ensure!(
            status.blocks_non_empty >= baseline.saturating_add(1),
            "expected non-empty block in kagami localnet (baseline={baseline}, current={})",
            status.blocks_non_empty
        );
        Ok(())
    }
    .await;

    if let Err(err) = result {
        if let Some(reason) = sandbox::sandbox_reason(&err) {
            eprintln!(
                "sandboxed network restriction detected while running kagami localnet test; skipping ({reason})"
            );
            return Ok(());
        }
        if std::env::var_os("IROHA_KAGAMI_LOCALNET_KEEP").is_some() {
            eprintln!(
                "keeping kagami localnet artifacts at {}",
                temp_dir.path().display()
            );
            let _ = temp_dir.keep();
        }
        return Err(err);
    }
    Ok(())
}

fn alloc_port_block(count: u16) -> Result<AllocatedPortBlock> {
    std::panic::catch_unwind(|| AllocatedPortBlock::new(count))
        .map_err(|panic| eyre!(panic_message(panic)))
}

fn panic_message(panic: Box<dyn Any + Send>) -> String {
    if let Some(message) = panic.as_ref().downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = panic.as_ref().downcast_ref::<String>() {
        message.clone()
    } else {
        "port allocation panicked".to_string()
    }
}

struct KagamiLocalnet {
    dir: PathBuf,
    children: Vec<Child>,
    _port_reservations: (AllocatedPortBlock, AllocatedPortBlock),
}

impl KagamiLocalnet {
    fn start(
        out_dir: &Path,
        irohad_bin: &Path,
        peers: u16,
        port_reservations: (AllocatedPortBlock, AllocatedPortBlock),
    ) -> Result<Self> {
        let mut children = Vec::with_capacity(peers as usize);
        for idx in 0..peers {
            let config_path = out_dir.join(format!("peer{idx}.toml"));
            let snapshot_dir = out_dir
                .join("storage")
                .join(format!("peer{idx}"))
                .join("snapshot");
            fs::create_dir_all(&snapshot_dir)
                .wrap_err_with(|| format!("create snapshot dir {}", snapshot_dir.display()))?;
            let log_path = out_dir.join(format!("peer{idx}.log"));
            let log_file = fs::File::create(&log_path)
                .wrap_err_with(|| format!("create log file {}", log_path.display()))?;
            let log_file_err = log_file
                .try_clone()
                .wrap_err_with(|| format!("clone log file {}", log_path.display()))?;

            let mut cmd = Command::new(irohad_bin);
            cmd.arg("--config").arg(&config_path);
            cmd.current_dir(out_dir);
            cmd.env("SNAPSHOT_STORE_DIR", &snapshot_dir);
            if std::env::var_os("RUST_LOG").is_none() {
                cmd.env("RUST_LOG", "info");
            }
            let child = cmd
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_file_err))
                .spawn()
                .wrap_err_with(|| format!("spawn irohad for peer {idx}"))?;
            children.push(child);
        }

        Ok(Self {
            dir: out_dir.to_path_buf(),
            children,
            _port_reservations: port_reservations,
        })
    }
}

impl Drop for KagamiLocalnet {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        if cfg!(unix) {
            let script = self.dir.join("stop.sh");
            if script.exists() {
                let _ = Command::new("bash")
                    .arg(script)
                    .current_dir(&self.dir)
                    .output();
            }
        }
    }
}

fn localnet_tempdir() -> Result<TempDir> {
    let root = repo_root().join("target").join("kagami-localnet");
    fs::create_dir_all(&root)
        .wrap_err_with(|| format!("create localnet temp root {}", root.display()))?;
    tempfile::tempdir_in(&root).wrap_err("create localnet temp dir")
}

fn generate_localnet(out_dir: &Path, base_api_port: u16, base_p2p_port: u16) -> Result<()> {
    let kagami_bin = resolve_kagami_bin()?;
    let peers = LOCALNET_PEERS.to_string();
    let api_port = base_api_port.to_string();
    let p2p_port = base_p2p_port.to_string();
    let block_time = LOCALNET_BLOCK_TIME_MS.to_string();
    let commit_time = LOCALNET_COMMIT_TIME_MS.to_string();
    let out_dir = out_dir.to_string_lossy().to_string();
    let output = Command::new(kagami_bin)
        .arg("localnet")
        .arg("--peers")
        .arg(peers)
        .arg("--seed")
        .arg("kagami-localnet")
        .arg("--bind-host")
        .arg("127.0.0.1")
        .arg("--public-host")
        .arg("127.0.0.1")
        .arg("--base-api-port")
        .arg(api_port)
        .arg("--base-p2p-port")
        .arg(p2p_port)
        .arg("--block-time-ms")
        .arg(block_time)
        .arg("--commit-time-ms")
        .arg(commit_time)
        .arg("--out-dir")
        .arg(out_dir)
        .output()
        .wrap_err("run kagami localnet")?;
    ensure!(
        output.status.success(),
        "kagami localnet failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn resolve_kagami_bin() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("KAGAMI_BIN") {
        return Ok(PathBuf::from(path));
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_kagami") {
        return Ok(PathBuf::from(path));
    }

    let repo = repo_root();
    let target_dir = resolve_target_dir(&repo);
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let bin = bin_name("kagami");
    let mut candidates = vec![
        target_dir.join(format!("{profile}/{bin}")),
        target_dir.join(format!("debug/{bin}")),
        target_dir.join(format!("release/{bin}")),
        repo.join(format!("target/{profile}/{bin}")),
        repo.join(format!("target/debug/{bin}")),
        repo.join(format!("target/release/{bin}")),
    ];

    if let Some(found) = try_candidates(&candidates) {
        return Ok(found);
    }

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command
        .current_dir(&repo)
        .arg("build")
        .arg("-p")
        .arg("iroha_kagami")
        .arg("--bin")
        .arg("kagami");
    if profile != "debug" {
        command.arg("--profile").arg(&profile);
    }
    let output = command.output().wrap_err("build kagami binary")?;
    ensure!(
        output.status.success(),
        "failed to build kagami: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    candidates.insert(0, target_dir.join(format!("{profile}/{bin}")));
    try_candidates(&candidates).ok_or_else(|| eyre!("kagami binary not found after build"))
}

fn resolve_target_dir(repo: &Path) -> PathBuf {
    std::env::var("CARGO_TARGET_DIR").map_or_else(
        |_| repo.join("target"),
        |path| {
            let candidate = PathBuf::from(path);
            if candidate.is_absolute() {
                candidate
            } else {
                repo.join(candidate)
            }
        },
    )
}

fn bin_name(raw: &str) -> String {
    if cfg!(windows) {
        format!("{raw}.exe")
    } else {
        raw.to_owned()
    }
}

fn try_candidates(candidates: &[PathBuf]) -> Option<PathBuf> {
    for candidate in candidates {
        if let Ok(path) = candidate.canonicalize() {
            return Some(path);
        }
    }
    None
}

fn load_localnet_client(out_dir: &Path) -> Result<Client> {
    let client_path = out_dir.join("client.toml");
    let mut config = Config::load(LoadPath::Explicit(client_path.clone())).map_err(|err| {
        eyre!(
            "load localnet client config {}: {err:?}",
            client_path.display()
        )
    })?;
    config.transaction_status_timeout = READY_TIMEOUT;
    Ok(Client::new(config))
}

async fn wait_for_status_ready(client: &Client, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() >= deadline {
            return Err(eyre!("timed out waiting for localnet status"));
        }
        if client.get_status().is_ok() {
            return Ok(());
        }
        sleep(READY_POLL).await;
    }
}

async fn wait_for_blocks_non_empty(
    client: &Client,
    target: u64,
    timeout: Duration,
) -> Result<iroha::client::Status> {
    let deadline = Instant::now() + timeout;
    loop {
        let status = client.get_status()?;
        if status.blocks_non_empty >= target {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for non-empty block target {target}"
            ));
        }
        sleep(READY_POLL).await;
    }
}
