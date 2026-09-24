//! Fresh four-seat custody through the real native provisioning and provider boundary.
//! The running daemon uses the explicit Core-only test seam; this is not Linux/Inrou qualification.
use super::*;
use iroha_config::base::read::ConfigReader;
use iroha_core::{
    beacon,
    kura::{BlockIndex, BlockStore},
};
use iroha_crypto::{ExposedPrivateKey, HashOf, KeyPair, MerkleTree};
use iroha_data_model::{
    block::{SignedBlock, decode_framed_signed_block},
    consensus::GlobalThresholdBeaconChainAnchorV1,
    isi::consensus_keys::{
        ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
        ThresholdKeyLifecycleCertificateV1,
    },
    parameter::system::SumeragiNposParameters,
    transaction::TransactionEntrypoint,
};
use iroha_test_network::{ReleasePrebuiltBinary, revalidate_release_prebuilt_binary};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Seek as _, Write as _},
    os::{
        fd::{AsRawFd, RawFd},
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Component, PathBuf},
    process::Stdio,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    process::{Child, Command},
};

#[path = "production_beacon_canary_receipt.rs"]
mod canary_receipt;
#[path = "production_epoch_retention.rs"]
mod epoch_retention;
#[path = "production_beacon_prepare.rs"]
mod prepare;
#[path = "public_transaction_sequence.rs"]
mod public_sequence;

const CHAIN: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const CREDENTIAL: &str = "iroha-global-beacon-partial-signer-v1.norito";
const PHASE_BUDGET: Duration = Duration::from_secs(180);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    value
        .get(name)
        .ok_or_else(|| eyre!("native output omitted {name}"))
}
fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    field(value, name)?
        .as_str()
        .ok_or_else(|| eyre!("native {name} is not text"))
}
fn private_file(path: &Path, bytes: &[u8]) -> Result<File> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(file)
}

// A build target may be inside Git. Only the explicit runtime root (or the
// established external artifact root) admits the fixture's generated custody.
fn validate_runtime_root(path: &Path) -> Result<PathBuf> {
    ensure!(
        path.is_absolute()
            && path
                .components()
                .all(|c| !matches!(c, Component::ParentDir | Component::CurDir)),
        "runtime root must be an absolute direct path"
    );
    let canonical = fs::canonicalize(path)?;
    ensure!(
        canonical == path,
        "runtime root must have no symlink or alias components"
    );
    let owner = nix::unistd::geteuid().as_raw();
    for (index, ancestor) in path.ancestors().enumerate() {
        let meta = fs::symlink_metadata(ancestor)?;
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink() && meta.mode() & 0o022 == 0,
            "unsafe runtime root ancestor"
        );
        ensure!(
            !ancestor.join(".git").exists(),
            "generated custody must remain outside Git"
        );
        if index == 0 {
            ensure!(
                meta.uid() == owner && meta.mode() & 0o077 == 0,
                "runtime root must be owner-only"
            );
        }
    }
    Ok(canonical)
}
fn runtime_workspace() -> Result<tempfile::TempDir> {
    let root = std::env::var_os("TAIRA_TESTNET_BEACON_FIXTURE_DIR")
        .or_else(|| std::env::var_os("IROHA_RELEASE_ARTIFACT_ROOT"))
        .ok_or_else(|| eyre!("an explicit owner-only external beacon fixture root is required"))?;
    let root = validate_runtime_root(Path::new(&root))?;
    Ok(tempfile::Builder::new()
        .prefix("beacon-production-")
        .tempdir_in(root)?)
}
fn binary(variable: &str, kind: ReleasePrebuiltBinary) -> Result<PathBuf> {
    let path = std::env::var_os(variable)
        .map(PathBuf::from)
        .ok_or_else(|| eyre!("{variable} must name the exact prebuilt binary"))?;
    ensure!(
        path.is_absolute() && path.is_file(),
        "prebuilt executable is absent"
    );
    revalidate_release_prebuilt_binary(kind, &path)?;
    Ok(path)
}

/// Descriptor duplication is confined to the child between fork and exec.
#[allow(unsafe_code)]
fn inherit(command: &mut Command, descriptors: &[(RawFd, RawFd)]) -> Result<()> {
    let mut targets = BTreeSet::new();
    ensure!(
        descriptors.iter().all(|(source, target)| *source >= 3
            && *target >= 3
            && source != target
            && targets.insert(*target)),
        "invalid inherited descriptor map"
    );
    ensure!(
        descriptors
            .iter()
            .all(|(source, _)| !targets.contains(source)),
        "descriptor mapping would overwrite a source"
    );
    let descriptors = descriptors.to_vec();
    // SAFETY: the closure only invokes async-signal-safe libc descriptor calls.
    unsafe {
        command.pre_exec(move || {
            for &(source, target) in &descriptors {
                if nix::libc::dup2(source, target) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    Ok(())
}
fn consumed_copy(source: &Path, target: &Path, maximum: u64) -> Result<File> {
    let before = fs::symlink_metadata(source)?;
    ensure!(
        before.is_file()
            && !before.file_type().is_symlink()
            && before.uid() == nix::unistd::geteuid().as_raw()
            && before.mode() & 0o777 == 0o600
            && before.nlink() == 1
            && (1..=maximum).contains(&before.len()),
        "invalid fixture-owned credential source"
    );
    let mut input = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(source)?;
    let opened = input.metadata()?;
    ensure!(
        (opened.dev(), opened.ino(), opened.len()) == (before.dev(), before.ino(), before.len()),
        "credential source changed before copy"
    );
    let mut output = private_file(target, &[])?;
    ensure!(
        std::io::copy(&mut input, &mut output)? == before.len(),
        "credential source changed during copy"
    );
    output.sync_all()?;
    let after = input.metadata()?;
    ensure!(
        (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec()
        ) == (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec()
        ),
        "credential source changed after copy"
    );
    output.rewind()?;
    Ok(output)
}
fn command(binary: &Path, directory: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .env_clear()
        .current_dir(directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    command
}
async fn run(mut command: Command, deadline: Instant) -> Result<Vec<u8>> {
    let output = timeout_at(deadline, command.spawn()?.wait_with_output())
        .await
        .wrap_err("native fixture child exceeded its fixed deadline")??;
    ensure!(
        output.status.success(),
        "native fixture child exited {}; stderr is retained",
        output.status
    );
    Ok(output.stdout)
}
fn config(path: &Path) -> Result<iroha_config::parameters::actual::Root> {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let parsed: iroha_config::parameters::actual::Root = ConfigReader::new()
        .without_env()
        .read_toml_with_extends(path.to_owned())
        .map_err(|_| eyre!("cannot read native fixture config"))?
        .read_and_complete::<iroha_config::parameters::user::Root>()
        .map_err(|_| eyre!("cannot decode native fixture config"))?
        .parse()
        .map_err(|_| eyre!("cannot validate native fixture config"))?;
    // These are the original retained-catalog fixture prerequisites. Verify
    // the generated native values; never rewrite a differing protocol mode.
    ensure!(
        matches!(parsed.kura.init_mode, iroha_config::kura::InitMode::Strict),
        "native fixture must retain strict Kura replay"
    );
    ensure!(
        matches!(
            parsed.nexus.staking.restricted_validator_mode,
            iroha_config::parameters::actual::LaneValidatorMode::AdminManaged
        ),
        "native restricted lanes must retain admin-managed validator admission"
    );
    Ok(parsed)
}
fn client(path: &Path, port: u16) -> Result<iroha::client::Client> {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let mut config = iroha::config::Config::load_file(path)
        .map_err(|error| eyre!("native fixture client configuration failed: {error:?}"))?;
    config.torii_api_url = format!("http://127.0.0.1:{port}/").parse()?;
    config.transaction_status_timeout = PHASE_BUDGET;
    Ok(iroha::client::Client::builder(config).build()?)
}
async fn status_height(clients: &[iroha::client::Client], deadline: Instant) -> Result<u64> {
    timeout_at(deadline, async {
        loop {
            let statuses = try_join_all(
                clients
                    .iter()
                    .map(|client| validator_status_until(client, deadline)),
            )
            .await?;
            if statuses
                .iter()
                .all(|s| s.blocks > 0 && s.blocks == statuses[0].blocks && s.queue_size == 0)
            {
                return Ok(statuses[0].blocks);
            }
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .wrap_err("four validators did not reach the same drained committed height")?
}
fn exact_height_reached(
    statuses: &[iroha_torii_shared::status::Status],
    expected: u64,
) -> Result<bool> {
    ensure!(
        statuses.len() == 4 && expected > 0,
        "exact height requires four validators and a positive target"
    );
    for (peer, status) in statuses.iter().enumerate() {
        ensure!(
            status.blocks <= expected,
            "validator {peer} advanced beyond exact height {expected}: observed {}",
            status.blocks
        );
    }
    Ok(statuses
        .iter()
        .all(|status| status.blocks == expected && status.queue_size == 0))
}

fn exact_meshed_height_reached(
    statuses: &[iroha_torii_shared::status::Status],
    expected: u64,
) -> Result<bool> {
    Ok(exact_height_reached(statuses, expected)?
        && statuses.iter().all(|status| status.peers == 3))
}

async fn wait_for_exact_meshed_height(
    clients: &[iroha::client::Client],
    expected: u64,
    deadline: Instant,
) -> Result<()> {
    let mut last = Vec::new();
    timeout_at(deadline, async {
        loop {
            let statuses = try_join_all(
                clients.iter().map(|client| validator_status_until(client, deadline)),
            ).await?;
            last = statuses
                .iter()
                .map(|status| (status.blocks, status.queue_size, status.peers))
                .collect::<Vec<_>>();
            if exact_meshed_height_reached(&statuses, expected)? {
                return Ok::<_, eyre::Report>(());
            }
            sleep(Duration::from_millis(200)).await;
        }
    }).await.wrap_err_with(|| format!(
        "four validators did not form a drained full mesh at exact height {expected}; last (height, queue, connected peers) observations={last:?}"
    ))?
}

async fn wait_for_exact_height(
    clients: &[iroha::client::Client],
    expected: u64,
    deadline: Instant,
) -> Result<()> {
    ensure!(
        clients.len() == 4 && expected > 0,
        "exact height requires four validators and a positive target"
    );
    let mut last = Vec::new();
    timeout_at(deadline, async {
        loop {
            let statuses = try_join_all(
                clients.iter().map(|client| validator_status_until(client, deadline)),
            ).await?;
            last = statuses.iter().map(|status| (status.blocks, status.queue_size)).collect::<Vec<_>>();
            if exact_height_reached(&statuses, expected)? {
                return Ok::<_, eyre::Report>(());
            }
            sleep(Duration::from_millis(200)).await;
        }
    }).await.wrap_err_with(|| format!(
        "four validators did not reach exact drained height {expected}; last (height, queue) observations={last:?}"
    ))?
}

async fn ready(port: u16, expected: u16, deadline: Instant) -> Result<()> {
    timeout_at(deadline, async {
        loop {
            let address = format!("127.0.0.1:{port}");
            if let Ok(mut stream) = tokio::net::TcpStream::connect(&address).await {
                stream.write_all(format!("GET /readyz HTTP/1.1\r\nHost: {address}\r\nAccept: */*\r\nConnection: close\r\n\r\n").as_bytes()).await?;
                let mut line = String::new();
                tokio::io::BufReader::new(stream).read_line(&mut line).await?;
                if line.starts_with(&format!("HTTP/1.1 {expected} ")) { return Ok::<_, eyre::Report>(()); }
                ensure!(!line.starts_with("HTTP/1.1 200 ") || expected == 200, "validator claimed ready without installed production custody");
            }
            sleep(Duration::from_millis(200)).await;
        }
    }).await.wrap_err("native readiness did not match the authenticated custody stage")?
}

async fn listeners_started(peers: &mut Peers, api: u16, deadline: Instant) -> Result<()> {
    let started = Instant::now();
    eprintln!(
        "beacon fixture waiting for public listeners: remaining={:.3}s",
        deadline.saturating_duration_since(started).as_secs_f64()
    );
    timeout_at(deadline, async {
        for offset in 0..4 {
            loop {
                for (index, child) in peers.children.iter_mut().enumerate() {
                    if let Some(status) = child.try_wait()? {
                        return Err(eyre!(
                            "fixture validator {index} exited before public listeners: {status}; inspect its retained stderr log"
                        ));
                    }
                }
                if tokio::net::TcpStream::connect(("127.0.0.1", api + offset))
                    .await
                    .is_ok()
                {
                    eprintln!(
                        "beacon fixture validator {offset} listener bound: elapsed={:.3}s",
                        started.elapsed().as_secs_f64()
                    );
                    break;
                }
                sleep(Duration::from_millis(200)).await;
            }
        }
        Ok::<_, eyre::Report>(())
    })
    .await
    .wrap_err_with(|| {
        format!(
            "fixture validators did not bind their public listeners after {:.3}s of listener wait",
            started.elapsed().as_secs_f64()
        )
    })??;
    Ok(())
}
struct Peers {
    children: Vec<Child>,
}
impl Peers {
    async fn stop(&mut self, deadline: Instant) -> Result<()> {
        for child in &mut self.children {
            if let Some(pid) = child.id() {
                nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(i32::try_from(pid)?),
                    nix::sys::signal::Signal::SIGTERM,
                )?;
            }
        }
        for child in &mut self.children {
            timeout_at(deadline, child.wait())
                .await
                .wrap_err("fixture-owned validator did not stop")??;
        }
        self.children.clear();
        Ok(())
    }
}
fn spawn_peers(
    directory: &Path,
    daemon: &Path,
    roster: &[iroha_model_base::peer::PeerId],
    ceremony: Option<&Path>,
    run_number: u16,
) -> Result<Peers> {
    let mut peers = Peers {
        children: Vec::new(),
    };
    for index in 0..4 {
        let path = directory.join(format!("peer{index}.toml"));
        let native = config(&path)?;
        let seat = roster
            .iter()
            .position(|peer| peer == &native.common.peer.id)
            .ok_or_else(|| eyre!("peer is outside signed genesis roster"))?
            + 1;
        let signer = consumed_copy(
            &directory.join(format!(
                "runtime/taira-runtime-signers/peer{index}.private_key"
            )),
            &directory.join(format!("runtime/peer{index}-run{run_number}.fd198")),
            71,
        )?;
        let mint = consumed_copy(
            &directory.join(format!("runtime/mint-finality-signers/peer{index}.seed")),
            &directory.join(format!("runtime/peer{index}-run{run_number}.fd199")),
            32,
        )?;
        let beacon = ceremony
            .map(|root| {
                consumed_copy(
                    &root.join(format!("seat-{seat}/{CREDENTIAL}")),
                    &directory.join(format!("runtime/peer{index}-run{run_number}.fd200")),
                    16 * 1024 * 1024,
                )
            })
            .transpose()?;
        let mut child = command(daemon, directory);
        child
            .arg("--sora")
            .arg("--config")
            .arg(&path)
            .arg("--test-network-production-beacon-custody")
            .stdout(private_file(
                &directory.join(format!("peer{index}-run{run_number}-stdout.log")),
                &[],
            )?)
            .stderr(private_file(
                &directory.join(format!("peer{index}-run{run_number}-stderr.log")),
                &[],
            )?);
        let mut descriptors = vec![(signer.as_raw_fd(), 198), (mint.as_raw_fd(), 199)];
        if let Some(beacon) = &beacon {
            descriptors.push((beacon.as_raw_fd(), 200));
        }
        inherit(&mut child, &descriptors)?;
        peers.children.push(child.spawn()?);
    }
    Ok(peers)
}

fn reserve_ports() -> Result<(u16, u16, Vec<std::net::TcpListener>)> {
    fn range() -> Result<(u16, Vec<std::net::TcpListener>)> {
        for _ in 0..100 {
            let first = std::net::TcpListener::bind(("127.0.0.1", 0))?;
            let base = first.local_addr()?.port();
            if base > u16::MAX - 3 {
                continue;
            }
            let mut held = vec![first];
            for offset in 1..4 {
                if let Ok(listener) = std::net::TcpListener::bind(("127.0.0.1", base + offset)) {
                    held.push(listener);
                } else {
                    break;
                }
            }
            if held.len() == 4 {
                return Ok((base, held));
            }
        }
        Err(eyre!("cannot reserve four contiguous loopback ports"))
    }
    let (api, mut held) = range()?;
    let (p2p, peers) = range()?;
    held.extend(peers);
    Ok((api, p2p, held))
}

fn fresh_client(directory: &Path, network_id: &iroha_data_model::NetworkId) -> Result<PathBuf> {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let key = KeyPair::try_random()?;
    let path = directory.join("fresh-client.toml");
    let private = directory.join("runtime/fresh-client.private_key");
    private_file(
        &private,
        format!("{}\n", ExposedPrivateKey(key.private_key().clone())).as_bytes(),
    )?;
    let mut table: toml::Table =
        toml::from_str(&fs::read_to_string(directory.join("client.toml"))?)?;
    table.remove("network_id_file");
    table.insert(
        "network_id".into(),
        toml::Value::String(network_id.to_string()),
    );
    let account = table
        .get_mut("account")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| eyre!("generated client omitted account"))?;
    account.remove("private_key");
    account.insert(
        "public_key".into(),
        toml::Value::String(key.public_key().to_string()),
    );
    account.insert(
        "private_key_file".into(),
        toml::Value::String(private.to_string_lossy().into_owned()),
    );
    private_file(&path, toml::to_string(&table)?.as_bytes())?;
    Ok(path)
}
fn idempotency(nonce: &str, kind: &str) -> String {
    let mut bytes = Vec::new();
    for frame in [
        b"iroha:taira:public-reset:child-idempotency:v1\0".as_slice(),
        nonce.as_bytes(),
        b"pre_edge",
        kind.as_bytes(),
    ] {
        bytes.extend_from_slice(&(frame.len() as u64).to_be_bytes());
        bytes.extend_from_slice(frame);
    }
    hex(&iroha_crypto::sha256(bytes))
}
struct Canary<'a> {
    binary: &'a Path,
    directory: &'a Path,
    config: &'a Path,
    root: String,
    nonce: String,
    authorization: String,
    expires_ms: u64,
    faucet: [String; 3],
}
impl Canary<'_> {
    async fn operation(
        &self,
        operation: &str,
        predecessor: Option<&Path>,
        deadline: Instant,
    ) -> Result<(PathBuf, u64)> {
        let envelope = self.directory.join(format!("{operation}.prepared.json"));
        let mut proved_height = None;
        let kind = if operation == "final-canary" {
            "write_canary"
        } else {
            operation
        };
        for prepare in [true, false] {
            let mut child = command(self.binary, self.directory);
            child
                .args(["--machine", "--config"])
                .arg(self.config)
                .arg("--operator-private-key-file")
                .arg(self.directory.join("runtime/operator-signer.key"))
                .args(["--fee-payer", "authority"])
                .args([
                    "taira",
                    "write-canary",
                    "--public-root",
                    &self.root,
                    "--operation",
                    operation,
                    "--authorization-sha256",
                    &self.authorization,
                    "--authorization-nonce",
                    &self.nonce,
                    "--mutation-phase",
                    "pre_edge",
                    "--idempotency-key",
                    &idempotency(&self.nonce, kind),
                    "--execution-expires-at-unix-ms",
                    &self.expires_ms.to_string(),
                    "--timeout-secs",
                    "120",
                    "--json",
                ]);
            if operation == "onboarding" {
                child
                    .arg("--onboarding-token-file")
                    .arg(self.directory.join("runtime/onboarding.token"));
            }
            if operation == "faucet" {
                child.args([
                    "--faucet-authority",
                    &self.faucet[0],
                    "--faucet-asset-id",
                    &self.faucet[1],
                    "--faucet-amount",
                    &self.faucet[2],
                ]);
            } else if operation == "final-canary" && prepare {
                child.args([
                    "--predecessor-faucet-authority",
                    &self.faucet[0],
                    "--predecessor-faucet-asset-id",
                    &self.faucet[1],
                    "--predecessor-faucet-amount",
                    &self.faucet[2],
                ]);
            }
            let file = if prepare {
                child.args(["--prepare-envelope", "--prepared-output-fd", "100"]);
                private_file(&envelope, &[])?
            } else {
                child.args(["--submit-prepared-envelope-fd", "100"]);
                File::open(&envelope)?
            };
            let previous = if prepare {
                predecessor.map(File::open).transpose()?
            } else {
                None
            };
            let mut descriptors = vec![(file.as_raw_fd(), 100)];
            if let Some(previous) = &previous {
                child.args(["--prerequisite-envelope-fd", "101"]);
                descriptors.push((previous.as_raw_fd(), 101));
            }
            inherit(&mut child, &descriptors)?;
            let phase = if prepare { "prepare" } else { "submit" };
            let started = Instant::now();
            eprintln!(
                "beacon fixture {operation} {phase} started: remaining={:.3}s",
                deadline.saturating_duration_since(started).as_secs_f64()
            );
            let bytes = run(child, deadline).await.wrap_err_with(|| {
                format!(
                    "native {operation} {phase} failed after {:.3}s",
                    started.elapsed().as_secs_f64()
                )
            })?;
            eprintln!(
                "beacon fixture {operation} {phase} complete: elapsed={:.3}s",
                started.elapsed().as_secs_f64()
            );
            if let Some(height) = self.retain_receipt(operation, kind, prepare, &bytes)? {
                proved_height = Some(height);
            }
        }
        Ok((
            envelope,
            proved_height.ok_or_else(|| eyre!("missing native canary proof receipt"))?,
        ))
    }

    fn assert_committed_prepared_replay(
        &self,
        operation: &str,
        client: &iroha::client::Client,
    ) -> Result<()> {
        let envelope_path = self.directory.join(format!("{operation}.prepared.json"));
        let envelope: Value = json::from_slice(&fs::read(&envelope_path)?)?;
        let prepared_operation = field(&envelope, "operation")?;
        let prepared = field(prepared_operation, "envelope")?;
        let (response, transaction_hash_hex) = match operation {
            "onboarding" => {
                ensure!(
                    text(prepared_operation, "kind")? == "onboarding_prepared",
                    "retained onboarding envelope has the wrong operation"
                );
                let prepared: iroha::client::AccountOnboardingPreparedTransactionV1 =
                    json::from_value(prepared.clone())?;
                let token = fs::read_to_string(self.directory.join("runtime/onboarding.token"))?;
                let response = client.post_prepared_account_onboarding(
                    &prepared.receipt.body.request,
                    &prepared,
                    &prepared.fee_payment,
                    &token,
                )?;
                (response, prepared.transaction_hash_hex)
            }
            "faucet" => {
                ensure!(
                    text(prepared_operation, "kind")? == "faucet_prepared",
                    "retained faucet envelope has the wrong operation"
                );
                let prepared: iroha::client::AccountFaucetPreparedTransactionV1 =
                    json::from_value(prepared.clone())?;
                let policy = iroha::client::AccountFaucetPolicyV1::try_new(
                    AccountId::parse_encoded(&self.faucet[0])?,
                    self.faucet[1].parse()?,
                    self.faucet[2].parse()?,
                )?;
                let response = client.post_prepared_account_faucet(
                    &prepared,
                    &prepared.fee_payment,
                    &policy,
                )?;
                (response, prepared.transaction_hash_hex)
            }
            _ => {
                return Err(eyre!(
                    "committed prepared replay requires onboarding or faucet"
                ));
            }
        };
        let replay_path = self
            .directory
            .join(format!("{operation}-committed-replay.json"));
        private_file(&replay_path, response.body())?;
        ensure!(
            response.status().as_u16() == 200,
            "committed {operation} prepared-envelope replay returned HTTP {}; retained response: {}",
            response.status(),
            replay_path.display()
        );
        let replay: Value = json::from_slice(response.body())?;
        ensure!(
            text(&replay, "outcome")? == "Applied"
                && text(&replay, "transaction_hash_hex")? == transaction_hash_hex,
            "committed {operation} replay did not return Applied for the exact retained transaction; retained response: {}",
            replay_path.display()
        );
        Ok(())
    }
}

fn install_provider_configs(
    directory: &Path,
    bundle: &Value,
    roster: &[iroha_model_base::peer::PeerId],
) -> Result<Vec<PathBuf>> {
    let providers = field(bundle, "providers")?
        .as_array()
        .ok_or_else(|| eyre!("public provider inventory is not an array"))?;
    ensure!(
        providers.len() == 4,
        "public bundle must bind all four providers"
    );
    let mut paths = Vec::new();
    for index in 0..4 {
        let path = directory.join(format!("peer{index}.toml"));
        let native = config(&path)?;
        let seat = roster
            .iter()
            .position(|peer| peer == &native.common.peer.id)
            .ok_or_else(|| eyre!("unbound peer"))?;
        let provider = &providers[seat];
        let peer: iroha_model_base::peer::PeerId =
            json::from_value(field(provider, "validator")?.clone())?;
        ensure!(
            peer == native.common.peer.id
                && field(provider, "signer_index")?.as_u64() == Some((seat + 1) as u64),
            "provider inventory changed roster seat"
        );
        let digest: [u8; 32] = json::from_value(field(provider, "policy_digest")?.clone())?;
        let revision = field(provider, "revision")?
            .as_u64()
            .ok_or_else(|| eyre!("provider revision absent"))?;
        let mut table: toml::Table = toml::from_str(&fs::read_to_string(&path)?)?;
        let sumeragi = table
            .entry("sumeragi")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| eyre!("sumeragi is not a table"))?;
        sumeragi.insert(
            "global_beacon_partial_signer_provider_handle".into(),
            toml::Value::String(text(provider, "handle")?.into()),
        );
        sumeragi.insert(
            "global_beacon_partial_signer_provider_revision".into(),
            toml::Value::Integer(i64::try_from(revision)?),
        );
        sumeragi.insert(
            "global_beacon_partial_signer_provider_policy_digest_hex".into(),
            toml::Value::String(hex(&digest)),
        );
        // These are exclusively fixture-owned files. The signed public provider
        // binding comes from native assemble-install validation, never discovery.
        fs::write(&path, toml::to_string(&table)?)?;
        paths.push(path);
    }
    Ok(paths)
}
async fn sign_and_assemble(
    binary: &Path,
    directory: &Path,
    ceremony: &Path,
    roster: &[iroha_model_base::peer::PeerId],
    deadline: Instant,
) -> Result<PathBuf> {
    let bundle = ceremony.join("public-bundle.json");
    let mut signatures = Vec::new();
    for seat in 0..3 {
        let source = (0..4)
            .map(|index| directory.join(format!("peer{index}.toml")))
            .find(|path| config(path).is_ok_and(|native| native.common.peer.id == roster[seat]))
            .ok_or_else(|| eyre!("authorization seat lacks a generated validator config"))?;
        let copy = consumed_copy(
            &source,
            &directory.join(format!("runtime/lifecycle-seat{seat}.fd198")),
            1024 * 1024,
        )?;
        let output = directory.join(format!("lifecycle-signature-{seat}.json"));
        let mut child = command(binary, directory);
        child
            .args(["beacon-bootstrap", "sign-install", "--bundle"])
            .arg(&bundle)
            .args([
                "--signer-index",
                &seat.to_string(),
                "--config-fd",
                "198",
                "--output",
            ])
            .arg(&output);
        inherit(&mut child, &[(copy.as_raw_fd(), 198)])?;
        run(child, deadline).await?;
        ensure!(
            copy.metadata()?.len() == 0,
            "native signing did not consume its descriptor copy"
        );
        signatures.push(output);
    }
    let output = directory.join("install-instruction.json");
    let mut child = command(binary, directory);
    child
        .args(["beacon-bootstrap", "assemble-install", "--bundle"])
        .arg(bundle)
        .arg("--signature")
        .args(&signatures)
        .arg("--output")
        .arg(&output);
    run(child, deadline).await?;
    Ok(output)
}
async fn submit_install(
    client: &iroha::client::Client,
    instructions_path: &Path,
    expected_certificate: &ThresholdKeyLifecycleCertificateV1,
    deadline: Instant,
) -> Result<u64> {
    let instructions: Vec<InstructionBox> = json::from_slice(&fs::read(instructions_path)?)?;
    ensure!(
        instructions.len() == 1,
        "native installation must be one certificate instruction"
    );
    let installation = instructions[0]
        .as_any()
        .downcast_ref::<ApplyThresholdKeyLifecycleCertificateV1>()
        .ok_or_else(|| eyre!("native installation is not a lifecycle certificate"))?;
    let mut unsigned_certificate = installation.certificate.clone();
    unsigned_certificate.signatures.clear();
    ensure!(
        unsigned_certificate == *expected_certificate
            && unsigned_certificate.action == ThresholdKeyLifecycleActionV1::InstallGlobalBeaconKey,
        "assembled installation differs from the native bootstrap certificate"
    );
    let install_height = installation.certificate.effective_height;
    let client = client.with_request_deadline(deadline.into_std());
    let account = client.account_client()?;
    let mut payload = account.prepare_transaction(
        AccountTransactionDraft::new(
            instructions,
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        )
        // This sole exact-height lifecycle certificate uses authenticated
        // Ordinary ingress; useful canaries and paid deployment keep QueuePlanSynced.
        .with_admission_intent(TransactionAdmissionIntent::Ordinary),
    )?;
    let quote = account
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    ensure!(
        quote.observation.next_block_height == install_height,
        "install quote changed the certificate's exact next height"
    );
    ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "install quote changed payer or gas bound"
    );
    payload.fee_payment = quote.intent;
    let transaction = account.sign_transaction(payload)?;
    // Persist the exact signed transaction before the sole submission attempt.
    private_file(
        &instructions_path.with_extension("submitted.nrt"),
        &transaction.encode_wire_v1()?,
    )?;
    let expected = transaction.hash();
    ensure!(
        timeout_at(deadline, account.submit_transaction_and_wait(&transaction)).await?? == expected,
        "installation hash changed"
    );
    Ok(install_height)
}

fn read_block(store: &mut BlockStore, height: u64) -> Result<SignedBlock> {
    ensure!(height > 0, "genesis is height one");
    let mut index = [BlockIndex {
        start: 0,
        length: 0,
    }];
    store.read_block_indices(height - 1, &mut index)?;
    ensure!(
        (1..=iroha_data_model::block::consensus_v2::MAX_EXECUTED_BLOCK_WIRE_BYTES)
            .contains(&index[0].length),
        "invalid canonical block length"
    );
    let mut bytes = vec![0; usize::try_from(index[0].length)?];
    store.read_block_data(index[0].start, &mut bytes)?;
    Ok(decode_framed_signed_block(&bytes)?)
}
fn verify_pulse(
    peer_configs: &[PathBuf],
    bundle: &Value,
    catalog_entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> Result<()> {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    iroha_genesis::init_instruction_registry();
    let record: beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1 =
        json::from_value(field(bundle, "record")?.clone())?;
    record.validate()?;
    let genesis = field(bundle, "genesis")?;
    let manifest: iroha_genesis::RawGenesisTransaction =
        json::from_value(field(genesis, "manifest")?.clone())?;
    let signed_wire: Vec<u8> = json::from_value(field(genesis, "signed_wire")?.clone())?;
    let public_key: iroha_crypto::PublicKey =
        json::from_value(field(genesis, "public_key")?.clone())?;
    iroha_genesis::validate_prepared_genesis_bundle(
        &signed_wire,
        &manifest,
        &public_key,
        record.session.network_id.into_genesis_hash(),
    )?;
    let parameters = manifest.effective_parameters()?;
    let npos = parameters
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .ok_or_else(|| eyre!("validated signed genesis omitted NPoS parameters"))?;
    let epoch_length = npos.epoch_length_blocks().get();
    ensure!(
        epoch_length == epoch_retention::EPOCH_LENGTH,
        "fixture must exercise the real catalog merge at mandatory height 10"
    );
    let catalog_tree: MerkleTree<TransactionEntrypoint> =
        [catalog_entrypoint_hash].into_iter().collect();
    let pulse_height = epoch_length
        .checked_sub(1)
        .filter(|height| *height > 1)
        .ok_or_else(|| eyre!("signed genesis has no first mandatory pulse anchor"))?;
    let anchor_height = pulse_height - 1;
    let session = beacon::validate_global_threshold_beacon_session_v1(
        record.session.clone(),
        &beacon::GlobalThresholdBeaconSessionBindingV1 {
            network_id: record.session.network_id,
            session_id: record.session.session_id,
            roster_hash: record.session.roster_hash,
            transcript_hash: record.session.transcript_hash,
        },
    )?;
    let mut common = None;
    for config_path in peer_configs {
        // All fixture children have stopped. This is a strict read-only native
        // journal reader, so validation cannot repair or rewrite the evidence.
        let native = config(config_path)?;
        let mut store = BlockStore::open_read_only(native.kura.store_dir.value())?;
        ensure!(
            store.read_index_count()? >= epoch_length,
            "paid deployment did not cross the mandatory epoch boundary"
        );
        let anchor = read_block(&mut store, anchor_height)?;
        let block = read_block(&mut store, pulse_height)?;
        // Native completion has already authenticated this exact native catalog
        // transaction as Applied on all four peers. Bind it to the sole leaf of
        // the execution-bearing merge at the mandatory pulse height, excluding
        // unrelated transactions, QueuePlan admissions and anchor padding.
        let context = block
            .execution_context()
            .ok_or_else(|| eyre!("mandatory pulse has no certified execution context"))?;
        let reference = context.merge_entry.as_ref().ok_or_else(|| {
            eyre!("catalog transaction did not execute on the mandatory pulse carrier")
        })?;
        ensure!(
            reference.execution_batch_hash.is_some()
                && reference.entrypoint_count == Some(1)
                && reference.entrypoint_merkle_root == catalog_tree.root()
                && block.external_entrypoint_count() == 0
                && context.queue_plan_admissions.is_empty()
                && context.autonomous_lane_payloads.is_empty(),
            "mandatory pulse carrier is not the exact one-transaction native catalog merge"
        );
        // Canonical QueuePlan admissions and autonomous anchors are genuine
        // protocol content even when they contain no external transaction row.
        ensure!(
            !block.is_empty()
                && (block.external_entrypoint_count() > 0
                    || block
                        .execution_context()
                        .is_some_and(|context| !context.is_empty())),
            "mandatory pulse lacks useful canonical content independent of its effects"
        );
        let pulse = block
            .npos_consensus_effects()
            .and_then(|effects| effects.finalized_global_beacon_pulse.as_ref())
            .ok_or_else(|| {
                eyre!("mandatory pulse is absent from signed-genesis height {pulse_height}")
            })?;
        ensure!(
            pulse.height == pulse_height,
            "mandatory pulse height differs"
        );
        beacon::verify_finalized_global_threshold_beacon_pulse_v1(
            &session,
            pulse,
            GlobalThresholdBeaconChainAnchorV1 {
                height: anchor_height,
                block_hash: anchor.hash(),
            },
        )?;
        let proof = (block.hash(), pulse.clone());
        if let Some(expected) = &common {
            ensure!(
                expected == &proof,
                "validators disagree on the real threshold pulse"
            );
        } else {
            common = Some(proof);
        }
    }
    ensure!(
        peer_configs.len() == 4,
        "pulse verification needs every validator"
    );
    Ok(())
}

fn set_snapshot_mode(peer_configs: &[PathBuf], mode: &str) -> Result<()> {
    ensure!(
        matches!(mode, "disabled" | "read_write"),
        "invalid fixture snapshot mode"
    );
    for path in peer_configs {
        let mut table: toml::Table = fs::read_to_string(path)?.parse()?;
        let snapshot = table
            .get_mut("snapshot")
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| eyre!("fixture snapshot configuration missing"))?;
        snapshot.insert("mode".into(), toml::Value::String(mode.into()));
        fs::write(path, toml::to_string(&table)?)?;
    }
    Ok(())
}
fn peer_logs(directory: &Path, peer: usize, run: u16) -> [PathBuf; 2] {
    [
        directory.join(format!("peer{peer}-run{run}-stdout.log")),
        directory.join(format!("peer{peer}-run{run}-stderr.log")),
    ]
}
struct Runtime<'a> {
    directory: &'a Path,
    daemon: &'a Path,
    roster: &'a [iroha_model_base::peer::PeerId],
    ceremony: &'a Path,
    api: u16,
    clients: &'a [iroha::client::Client],
    peers: &'a mut Peers,
    run: u16,
}
impl Runtime<'_> {
    async fn restart(&mut self, deadline: Instant) -> Result<()> {
        self.peers.stop(deadline).await?;
        self.run = self
            .run
            .checked_add(1)
            .ok_or_else(|| eyre!("fixture run counter overflow"))?;
        *self.peers = spawn_peers(
            self.directory,
            self.daemon,
            self.roster,
            Some(self.ceremony),
            self.run,
        )?;
        listeners_started(self.peers, self.api, deadline).await?;
        for index in 0..4 {
            ready(self.api + index, 200, deadline).await?;
        }
        Ok(())
    }
    async fn signed_snapshot_restart(&mut self, applied_height: u64) -> Result<()> {
        let snapshot_deadline = Instant::now() + Duration::from_secs(60);
        timeout_at(snapshot_deadline, async {
            loop {
                let mut complete = true;
                for peer in 0..4 {
                    let root = self.directory.join(format!("state/peer{peer}/snapshot"));
                    let height = public_sequence::snapshot_height(&root)?;
                    complete &= if let Some(height) = height {
                        height >= applied_height && public_sequence::snapshot_log_contains_height(&peer_logs(self.directory, peer, self.run), "Successfully created a snapshot of state", height)?
                    } else { false };
                }
                if complete { return Ok::<_, eyre::Report>(()); }
                sleep(Duration::from_millis(200)).await;
            }
        }).await.wrap_err("all validators must publish complete signed snapshots after the exact Applied transaction")??;
        let restart = Instant::now() + PHASE_BUDGET;
        self.peers.stop(restart).await?;
        let heights = (0..4)
            .map(|peer| {
                public_sequence::snapshot_height(
                    &self.directory.join(format!("state/peer{peer}/snapshot")),
                )?
                .ok_or_else(|| eyre!("snapshot disappeared during shutdown"))
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            heights.iter().all(|height| *height >= applied_height),
            "shutdown snapshot regressed"
        );
        // Restart uses newly consumed FD198/199/200 copies of the same retained
        // production credentials, never the already-truncated launch copies.
        self.restart(restart).await?;
        timeout_at(restart, async {
            loop {
                let mut complete = true;
                for peer in 0..4 {
                    let status = validator_status_until(&self.clients[peer], restart).await?;
                    complete &= status.blocks >= heights[peer]
                        && public_sequence::snapshot_log_contains_height(
                            &peer_logs(self.directory, peer, self.run),
                            "Successfully loaded the state from a snapshot",
                            heights[peer],
                        )?;
                }
                if complete {
                    return Ok::<_, eyre::Report>(());
                }
                sleep(Duration::from_millis(200)).await;
            }
        })
        .await
        .wrap_err("native restart did not authenticate each retained signed snapshot")??;
        Ok(())
    }
}

fn assert_native_routes(
    peer_configs: &[PathBuf],
    universal: &iroha::client::Client,
    routed: &iroha::client::Client,
) -> Result<()> {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let universal = universal
        .to_builder()
        .account
        .to_i105_for_discriminant(369)?;
    let routed = routed.to_builder().account.to_i105_for_discriminant(369)?;
    ensure!(
        universal != routed,
        "route scenarios must use different canonical accounts"
    );
    for path in peer_configs {
        let native = config(path)?;
        let policy = &native.nexus.routing_policy;
        ensure!(
            policy.default_lane.as_u32() == 0,
            "universal scenario lost its native default route"
        );
        ensure!(
            policy
                .rules
                .iter()
                .all(|rule| rule.matcher.account.as_deref() != Some(universal.as_str())),
            "default-route account gained an exact override"
        );
        let selected = policy
            .rules
            .iter()
            .find(|rule| rule.matcher.account.as_deref() == Some(routed.as_str()))
            .ok_or_else(|| eyre!("explicit routed fixture account absent"))?;
        let lane = native
            .nexus
            .lane_catalog
            .lanes()
            .iter()
            .find(|lane| lane.id.as_u32() == 3)
            .ok_or_else(|| eyre!("native PayNet lane absent"))?;
        ensure!(
            selected.lane == lane.id
                && selected.dataspace == Some(lane.dataspace_id)
                && selected.matcher.instruction.is_none(),
            "explicit account route changed native lane or dataspace"
        );
    }
    Ok(())
}

// The preceding paid helper has already authenticated the exact three retained
// transactions and completion on all four peers. Reconstruct its catalog result
// from that local plan and the unchanged generated startup authority, never from
// an HTTP state response that the next scenario is meant to verify.
fn catalog_fixture(
    prepared: &prepare::Prepared,
    peer_configs: &[PathBuf],
    clients: &[iroha::client::Client],
    api: u16,
) -> Result<super::runtime_catalog_transition::real_custody::CatalogFixture> {
    use super::runtime_catalog_transition::real_custody::{CatalogFixture, CatalogPeer};
    use iroha_core::governance::manifest::LaneManifestRegistry;
    use iroha_crypto::Hash;
    use iroha_data_model::nexus::{
        LaneCatalog, LaneLifecycleParameterV1, LaneLifecycleStatusV1, NexusCatalogTransitionV1,
        NexusRuntimeCatalogV1, dataspace_catalog_hash,
    };
    use iroha_model_base::topology::{DataSpaceId, LaneId};
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    ensure!(
        peer_configs.len() == 4 && clients.len() == 4,
        "catalog requires four native configurations"
    );
    let configs = peer_configs
        .iter()
        .map(|path| config(path))
        .collect::<Result<Vec<_>>>()?;
    let first = &configs[0].nexus;
    let registry = LaneManifestRegistry::from_config(
        &first.configured_lane_catalog,
        &first.governance,
        &first.registry,
    );
    let manifests_hash = Hash::prehashed(registry.baseline_consensus_policy_digest());
    for config in &configs {
        let nexus = &config.nexus;
        let registry = LaneManifestRegistry::from_config(
            &nexus.configured_lane_catalog,
            &nexus.governance,
            &nexus.registry,
        );
        ensure!(
            nexus.configured_lane_catalog == first.configured_lane_catalog
                && nexus.configured_dataspace_catalog == first.configured_dataspace_catalog
                && Hash::prehashed(registry.baseline_consensus_policy_digest()) == manifests_hash,
            "four generated immutable catalog authorities differ"
        );
    }
    let plan: Value = json::from_slice(&fs::read(
        prepared
            .directory
            .join("paid-deployment/journal/clean-client-dpn/plan.json"),
    )?)?;
    let baseline: LaneLifecycleStatusV1 = json::from_value(field(&plan, "baseline")?.clone())?;
    let transition: NexusCatalogTransitionV1 =
        json::from_value(field(&plan, "catalog_transition")?.clone())?;
    transition.validate_structure()?;
    ensure!(
        text(&plan, "operation_id")? == "clean-client-dpn"
            && field(&plan, "baseline_overlay")?.is_null()
            && baseline.validate()? == first.configured_lane_catalog
            && baseline.catalog_hash
                == LaneLifecycleParameterV1::catalog_hash(&first.configured_lane_catalog)
            && transition.expected_catalog_hash == baseline.catalog_hash
            && transition.expected_incarnation_root == baseline.incarnation_root
            && transition.expected_runtime_catalog_hash.is_none(),
        "retained paid transition is not bound to the exact native baseline"
    );
    ensure!(
        transition.lane_additions.len() == 1
            && transition.lane_additions[0].id == LaneId::new(6)
            && transition.dataspace_additions.len() == 1
            && transition.manifest_additions.len() == 1,
        "paid fixture no longer has its exact single catalog addition"
    );
    let mut lanes = first.configured_lane_catalog.lanes().to_vec();
    lanes.extend(transition.lane_additions.clone());
    let lane_count = lanes
        .iter()
        .map(|lane| lane.id.as_u32() + 1)
        .max()
        .unwrap()
        .max(baseline.lane_count);
    let lanes = LaneCatalog::new(
        std::num::NonZeroU32::new(lane_count).ok_or_else(|| eyre!("zero namespace"))?,
        lanes,
    )?;
    let runtime = NexusRuntimeCatalogV1 {
        version: NexusRuntimeCatalogV1::VERSION,
        baseline_dataspaces_hash: dataspace_catalog_hash(&first.configured_dataspace_catalog),
        baseline_manifests_hash: manifests_hash,
        dataspaces: transition.dataspace_additions,
        manifests: transition.manifest_additions,
    };
    runtime.validate_structure()?;
    let old = first
        .configured_lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == first.routing_policy.default_lane)
        .ok_or_else(|| eyre!("configured default lane missing"))?;
    let writer = client(&prepared.directory.join("client.toml"), api)?;
    let grantee = client(&prepared.routed_client, api)?.to_builder().account;
    let manifest = iroha_genesis::RawGenesisTransaction::from_path(
        prepared.genesis_directory.join("genesis.json"),
    )?;
    let genesis = iroha_genesis::validate_prepared_genesis_bundle(
        &fs::read(prepared.genesis_directory.join("genesis.signed.nrt"))?,
        &manifest,
        &prepared.genesis_public_key,
        prepared.network_id.into_genesis_hash(),
    )?;
    let peers = configs
        .iter()
        .zip(clients)
        .map(|(config, client)| CatalogPeer {
            client: client.clone(),
            peer_id: config.common.peer.id.clone(),
            kura_store: config.kura.store_dir.value().to_path_buf(),
        })
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| eyre!("catalog reader count differs"))?;
    Ok(CatalogFixture {
        peers,
        delegator: writer.to_builder().account,
        writer,
        genesis,
        baseline_dataspaces: first.configured_dataspace_catalog.clone(),
        baseline_lanes: lanes.lanes().to_vec(),
        previous_runtime: Some(runtime),
        old_route: (old.id, old.dataspace_id),
        added_lane: LaneId::new(5),
        added_dataspace: DataSpaceId::new(56_005),
        grantee,
    })
}

async fn retained_catalog_recovery(
    runtime: &mut Runtime<'_>,
    prepared: &prepare::Prepared,
    peer_configs: &[PathBuf],
) -> Result<()> {
    use super::runtime_catalog_transition::real_custody::{
        CatalogScenario, replayed_complete_history,
    };
    let mut scenario = CatalogScenario::begin(catalog_fixture(
        prepared,
        peer_configs,
        runtime.clients,
        runtime.api,
    )?)
    .await?;
    let replay_height = scenario.replay_height();
    let restart_deadline = Instant::now() + FUNCTIONAL_FINALITY_TIMEOUT;
    runtime.peers.stop(restart_deadline).await?;
    for peer in 0..4 {
        ensure!(
            public_sequence::snapshot_height(
                &runtime.directory.join(format!("state/peer{peer}/snapshot"))
            )?
            .is_none(),
            "retained full replay must precede every signed snapshot publication"
        );
    }
    // Enabling the writer on this restart matches the retained contract: state
    // first reconstructs the complete Kura prefix, then may publish snapshots.
    set_snapshot_mode(peer_configs, "read_write")?;
    let previous_run = runtime.run;
    runtime.restart(restart_deadline).await?;
    ensure!(
        runtime.run > previous_run,
        "full replay reused a prior daemon run"
    );
    timeout_at(restart_deadline, async {
        loop {
            let mut complete = true;
            for peer in 0..4 {
                complete &= validator_status_until(&runtime.clients[peer], restart_deadline)
                    .await?
                    .blocks
                    >= replay_height
                    && replayed_complete_history(
                        &peer_logs(runtime.directory, peer, runtime.run),
                        replay_height,
                    )?;
            }
            if complete {
                return Ok::<_, eyre::Report>(());
            }
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .wrap_err("all four validators must replay the authenticated complete Kura prefix")??;
    let snapshot_height = scenario.after_full_replay().await?;
    runtime.signed_snapshot_restart(snapshot_height).await?;
    scenario.after_snapshot_restore().await?;
    eprintln!(
        "catalog expansion retained exact four-peer execution, committee and permission history through full replay and signed snapshot restore"
    );
    Ok(())
}

async fn both_public_sequences(
    runtime: &mut Runtime<'_>,
    peer_configs: &[PathBuf],
    routed_config: &Path,
) -> Result<()> {
    let universal = client(&runtime.directory.join("client.toml"), runtime.api)?;
    let routed = client(routed_config, runtime.api)?;
    assert_native_routes(peer_configs, &universal, &routed)?;
    set_snapshot_mode(peer_configs, "read_write")?;
    runtime.restart(Instant::now() + PHASE_BUDGET).await?;
    for (scope, client) in [("universal", universal), ("explicit PayNet", routed)] {
        let mut height = status_height(runtime.clients, Instant::now() + PHASE_BUDGET).await?;
        for sequence in 1..=3 {
            height =
                public_sequence::submit_and_observe(&client, runtime.clients, sequence, height)
                    .await?;
            if sequence == 2 {
                runtime.signed_snapshot_restart(height).await?;
            }
        }
        eprintln!(
            "retained {scope} three-transaction contract passed on real custody, including all-four signed-snapshot restore and post-restart Applied"
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn four_peer_fresh_custody_bootstrap_reaches_mandatory_pulse() -> Result<()> {
    run_fresh_custody_bootstrap().await
}

async fn run_fresh_custody_bootstrap() -> Result<()> {
    // Validate the same immutable identity used by the paid trust helper before
    // artifact reads, custody creation, genesis generation, or child startup.
    // The complete fixture binds every proof to this exact release source.
    let build_identity = epoch_retention::admit_build_identity(
        iroha_core::compiled_build_identity!()
            .wrap_err("production beacon fixture has invalid compiled build metadata")?,
    )?;
    init_instruction_registry();
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let daemon = binary(
        "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL",
        ReleasePrebuiltBinary::IrohadMessageControl,
    )?;
    let launcher = binary(
        "TEST_NETWORK_BIN_IROHAD_TAIRA",
        ReleasePrebuiltBinary::IrohadTaira,
    )?;
    let cli = binary("TEST_NETWORK_BIN_IROHA", ReleasePrebuiltBinary::Iroha)?;
    let kagami = binary("KAGAMI_BIN", ReleasePrebuiltBinary::Kagami)?;
    let workspace = runtime_workspace()?;
    let (api, p2p, reservations) = reserve_ports()?;
    let preparation_started = Instant::now();
    let preparation_deadline = preparation_started + PHASE_BUDGET;
    eprintln!(
        "beacon fixture preparing fresh genesis: budget={:.3}s",
        PHASE_BUDGET.as_secs_f64()
    );
    let prepared = match prepare::prepare(
        workspace.path(),
        &kagami,
        &cli,
        api,
        p2p,
        preparation_deadline,
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!(
                "beacon fixture preparation retained at {}",
                workspace.keep().display()
            );
            return Err(error);
        }
    };
    eprintln!(
        "beacon fixture fresh genesis prepared: elapsed={:.3}s",
        preparation_started.elapsed().as_secs_f64()
    );
    let directory = &prepared.directory;
    let fresh = fresh_client(directory, &prepared.network_id)?;
    let clients = (0..4)
        .map(|index| client(&directory.join("client.toml"), api + index))
        .collect::<Result<Vec<_>>>()?;
    let public_config = config(&directory.join("peer0.toml"))?;
    ensure!(
        public_config.common.chain.to_string() == CHAIN,
        "fixture must use the exact Taira chain"
    );
    // Read only the fixture's generated public policy as independent authority;
    // an HTTP discovery response does not select faucet identity.
    let table: toml::Table = toml::from_str(&fs::read_to_string(directory.join("peer0.toml"))?)?;
    let faucet = &table["torii"]["faucet"];
    let faucet = ["authority", "asset_definition_id", "amount"]
        .map(|key| {
            faucet
                .get(key)
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| eyre!("native fixture faucet {key} absent"))
        })
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    let faucet: [String; 3] = faucet
        .try_into()
        .map_err(|_| eyre!("faucet policy length"))?;
    drop(table);
    drop(reservations);
    // Native genesis generation is a separate bounded phase. Listener availability
    // and exact committed genesis are separate observations under one startup deadline.
    let startup_started = Instant::now();
    let startup = startup_started + PHASE_BUDGET;
    eprintln!(
        "beacon fixture starting four validators: budget={:.3}s",
        PHASE_BUDGET.as_secs_f64()
    );
    let mut peers = spawn_peers(directory, &daemon, &prepared.roster, None, 1)?;
    let outcome: Result<()> = async {
        listeners_started(&mut peers, api, startup).await?;
        wait_for_exact_height(&clients, 1, startup).await?;
        for offset in 0..4 { ready(api + offset, 503, startup).await?; }
        eprintln!("beacon fixture initial startup complete: elapsed={:.3}s", startup_started.elapsed().as_secs_f64());
        let ceremony_deadline = Instant::now() + PHASE_BUDGET;
        let ceremony = directory.join("beacon-ceremony");
        let (height_read, height_write) = nix::unistd::pipe()?;
        let mut height_write = File::from(height_write);
        let mut provision = command(&launcher, directory);
        provision.args(["beacon-bootstrap", "provision", "--request"]).arg(&prepared.request)
            .arg("--genesis-manifest").arg(prepared.genesis_directory.join("genesis.json"))
            .arg("--genesis-signed").arg(prepared.genesis_directory.join("genesis.signed.nrt"))
            .arg("--genesis-public-key").arg(prepared.genesis_directory.join("genesis.public_key"))
            .args(["--observed-height", "1", "--height-fd", "197", "--output"]).arg(&ceremony)
            .args(["--timeout-ms", "180000"]);
        inherit(&mut provision, &[(height_read.as_raw_fd(), 197)])?;
        let mut provision = provision.spawn()?;
        drop(height_read);
        let mut progress = tokio::io::BufReader::new(provision.stdout.take().ok_or_else(|| eyre!("native provision progress missing"))?);
        let mut line = String::new();
        timeout_at(ceremony_deadline, progress.read_line(&mut line)).await??;
        ensure!(text(&json::from_str::<Value>(&line)?, "state")? == "sharing-ready", "native ceremony did not publish its sharing barrier");
        let nonce = hex(&iroha_crypto::sha256(fs::read(&prepared.request)?))[..32].to_owned();
        let network_id = prepared.network_id;
        let request_sha256 = hex(&iroha_crypto::sha256(fs::read(&prepared.request)?));
        let authorization = hex(&iroha_crypto::sha256(json::to_vec(&norito::json!({"network_id": network_id, "request_sha256": request_sha256, "fixture": "fresh-production-beacon"}))?));
        let expires_ms = u64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis())? + 180_000;
        let canary = Canary { binary: &cli, directory, config: &fresh, root: format!("http://127.0.0.1:{api}"), nonce, authorization, expires_ms, faucet };
        let mut previous = None;
        let mut last_proved_height = 1;
        for operation in ["onboarding", "faucet", "final-canary"] {
            let (envelope, proved_height) = canary.operation(operation, previous.as_deref(), ceremony_deadline).await?;
            previous = Some(envelope);
            ensure!(proved_height > last_proved_height, "proved useful operation did not strictly advance height");
            wait_for_exact_height(&clients, proved_height, ceremony_deadline).await?;
            last_proved_height = proved_height;
            if matches!(operation, "onboarding" | "faucet") {
                // The SDK's synchronous HTTP client refuses a Tokio runtime
                // thread. Replay the retained envelope on a scoped OS thread
                // so the test still checks the exact committed transaction.
                std::thread::scope(|scope| {
                    scope
                        .spawn(|| {
                            // The network discriminant override is thread-local;
                            // scoped workers do not inherit the Tokio task's
                            // configured testnet profile.
                            let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
                            canary.assert_committed_prepared_replay(operation, &clients[0])
                        })
                        .join()
                        .map_err(|_| eyre!("committed prepared replay worker panicked"))?
                })?;
                wait_for_exact_height(&clients, proved_height, ceremony_deadline).await?;
            }
        }
        // The native provisioner finalizes on the first height that reaches the
        // response boundary. Send only the final authenticated canary height so
        // its certificate is effective at the exact next proved height.
        writeln!(height_write, "{last_proved_height}")?;
        height_write.flush()?;
        drop(height_write);
        let status = timeout_at(ceremony_deadline, provision.wait()).await??;
        ensure!(status.success(), "fresh native DKG provisioning failed");
        let bundle: Value = json::from_slice(&fs::read(ceremony.join("public-bundle.json"))?)?;
        ensure!(field(&bundle, "finalized_observed_height")?.as_u64() == Some(last_proved_height), "ceremony backdated its finalization");
        let certificate: ThresholdKeyLifecycleCertificateV1 = json::from_value(field(&bundle, "certificate")?.clone())?;
        let expected_install_height = last_proved_height.checked_add(1).ok_or_else(|| eyre!("installation height overflow"))?;
        ensure!(certificate.effective_height == expected_install_height, "native certificate is not effective at the exact next proved height");
        let instruction = sign_and_assemble(&launcher, directory, &ceremony, &prepared.roster, ceremony_deadline).await?;
        // Match the maintained controller: commit the certificate without a
        // local beacon provider, then activate custody on the same four ledgers.
        // Installation and provider restart share one unchanged phase deadline.
        let restart = Instant::now() + PHASE_BUDGET;
        let install_height = submit_install(&clients[0], &instruction, &certificate, restart).await?;
        wait_for_exact_height(&clients, install_height, restart).await?;
        for offset in 0..4 { ready(api + offset, 503, restart).await?; }
        peers.stop(restart).await?;
        let peer_configs = install_provider_configs(directory, &bundle, &prepared.roster)?;
        peers = spawn_peers(directory, &daemon, &prepared.roster, Some(&ceremony), 2)?;
        listeners_started(&mut peers, api, restart).await?;
        wait_for_exact_height(&clients, install_height, restart).await?;
        for offset in 0..4 { ready(api + offset, 200, restart).await?; }
        wait_for_exact_meshed_height(&clients, install_height, restart).await?;
        let mut doctor = command(&cli, directory);
        doctor.args(["--machine", "taira", "doctor", "--scope", "basic", "--public-root", &format!("http://127.0.0.1:{api}"), "--json"]);
        let doctor_deadline = (Instant::now() + Duration::from_secs(60)).min(restart);
        let doctor_report: Value = json::from_slice(&run(doctor, doctor_deadline).await?)
            .wrap_err("basic public doctor returned invalid JSON")?;
        ensure!(doctor_report.get("status").and_then(Value::as_str) == Some("ok")
            && doctor_report.get("scope").and_then(Value::as_str) == Some("basic"),
            "basic public doctor omitted its successful scope");
        // The complete prior clean-client assertions run on real custody.
        // Their genuine operations cross the signed genesis's mandatory pulse.
        let genesis_wire = fs::read(prepared.genesis_directory.join("genesis.signed.nrt"))?;
        // Deployment uses the generated genesis-authorized client. The fresh
        // public account remains the onboarding/faucet/canary actor and receives
        // no deployment administration permissions.
        // The first genuine paid catalog transaction admits at 8, anchors at 9,
        // and executes at the mandatory pulse height 10. Its native completion
        // verifies the exact signed operation independently on all four peers.
        let catalog_entrypoint_hash = super::dataspace_deploy_cli::run_paid_deployment(super::dataspace_deploy_cli::PaidDeploymentFixture {
            binary: &cli, build_identity, config: &directory.join("client.toml"), operator: &directory.join("runtime/operator-signer.key"),
            root: &directory.join("paid-deployment"), genesis_wire: &genesis_wire,
            genesis_public_key: &prepared.genesis_public_key, peer_configs: &peer_configs, clients: &clients,
        }).await?;
        {
            let mut runtime = Runtime { directory, daemon: &daemon, roster: &prepared.roster,
                ceremony: &ceremony, api, clients: &clients, peers: &mut peers, run: 2 };
            retained_catalog_recovery(&mut runtime, &prepared, &peer_configs).await?;
            both_public_sequences(&mut runtime, &peer_configs, &prepared.routed_client).await?;
        }
        let installed: beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1 =
            json::from_value(field(&bundle, "record")?.clone())?;
        epoch_retention::verify_boundary_chain(
            &prepared, &clients, &installed, Instant::now() + PHASE_BUDGET,
        ).await?;
        peers.stop(Instant::now() + PHASE_BUDGET).await?;
        verify_pulse(&peer_configs, &bundle, catalog_entrypoint_hash)?;
        eprintln!("four fresh production-custody validators completed native onboarding/faucet/canary/install and paid deployment across a verified mandatory pulse");
        Ok(())
    }.await;
    if !peers.children.is_empty() {
        let stopped = peers.stop(Instant::now() + Duration::from_secs(30)).await;
        if outcome.is_ok() {
            stopped?;
        }
    }
    // Keep failure diagnostics and generated custody owner-private outside Git.
    if outcome.is_err() {
        eprintln!(
            "beacon fixture evidence retained at {}",
            workspace.keep().display()
        );
    }
    outcome
}

#[test]
fn production_beacon_exact_height_wait_preserves_retained_tip() -> Result<()> {
    use iroha_torii_shared::status::Status;
    let mut statuses: [Status; 4] = std::array::from_fn(|_| Status {
        blocks: 6,
        ..Status::default()
    });
    // A shared drained replay prefix is not the retained tip. Require every
    // peer to finish its pending tip rather than returning the common prefix.
    assert!(!exact_height_reached(&statuses, 7)?);
    for peer in 0..3 {
        statuses[peer].blocks = 7;
        assert!(!exact_height_reached(&statuses, 7)?);
    }
    statuses[3].blocks = 7;
    statuses[2].queue_size = 1;
    assert!(!exact_height_reached(&statuses, 7)?);
    statuses[2].queue_size = 0;
    assert!(exact_height_reached(&statuses, 7)?);
    // Overshoot is fatal even while another peer is behind or has queued work.
    statuses[0].blocks = 8;
    statuses[1].blocks = 6;
    statuses[1].queue_size = 1;
    assert!(exact_height_reached(&statuses, 7).is_err());
    assert!(exact_height_reached(&statuses[..3], 7).is_err());
    assert!(exact_height_reached(&statuses, 0).is_err());
    Ok(())
}

#[test]
fn production_beacon_paid_deployment_waits_for_full_mesh_at_exact_height() -> Result<()> {
    use iroha_torii_shared::status::Status;
    let mut statuses: [Status; 4] = std::array::from_fn(|_| Status {
        blocks: 8,
        peers: 3,
        ..Status::default()
    });
    assert!(exact_meshed_height_reached(&statuses, 8)?);
    statuses[0].peers = 0;
    assert!(!exact_meshed_height_reached(&statuses, 8)?);
    statuses[0].peers = 3;
    statuses[1].queue_size = 1;
    assert!(!exact_meshed_height_reached(&statuses, 8)?);
    statuses[1].queue_size = 0;
    statuses[2].blocks = 7;
    assert!(!exact_meshed_height_reached(&statuses, 8)?);
    statuses[2].blocks = 9;
    assert!(exact_meshed_height_reached(&statuses, 8).is_err());
    Ok(())
}

#[test]
fn production_beacon_fixture_root_rejects_git_symlink_and_shared_custody() -> Result<()> {
    let home = std::env::var_os("HOME").ok_or_else(|| eyre!("HOME absent"))?;
    let base = fs::canonicalize(home)?;
    let root = tempfile::Builder::new()
        .prefix("beacon-root-contract-")
        .tempdir_in(base)?;
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))?;
    assert_eq!(validate_runtime_root(root.path())?, root.path());
    fs::create_dir(root.path().join(".git"))?;
    assert!(validate_runtime_root(root.path()).is_err());
    fs::remove_dir(root.path().join(".git"))?;
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(root.path(), &alias)?;
    assert!(validate_runtime_root(&alias).is_err());
    fs::remove_file(alias)?;
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o777))?;
    assert!(validate_runtime_root(root.path()).is_err());
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))?;
    Ok(())
}
