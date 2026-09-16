//! Fresh four-seat custody through the real native provisioning and provider boundary.
//! The running daemon uses the explicit Core-only test seam; this is not Linux/Inrou qualification.
use super::*;
use iroha_config::base::read::ConfigReader;
use iroha_core::{
    beacon,
    kura::{BlockIndex, BlockStore},
};
use iroha_crypto::{ExposedPrivateKey, KeyPair};
use iroha_data_model::{
    block::{SignedBlock, decode_framed_signed_block},
    consensus::GlobalThresholdBeaconChainAnchorV1,
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

#[path = "production_beacon_prepare.rs"]
mod prepare;

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
    ConfigReader::new()
        .without_env()
        .read_toml_with_extends(path.to_owned())
        .map_err(|_| eyre!("cannot read native fixture config"))?
        .read_and_complete::<iroha_config::parameters::user::Root>()
        .map_err(|_| eyre!("cannot decode native fixture config"))?
        .parse()
        .map_err(|_| eyre!("cannot validate native fixture config"))
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

async fn listeners_started(api: u16, deadline: Instant) -> Result<()> {
    timeout_at(deadline, async {
        for offset in 0..4 {
            loop {
                if tokio::net::TcpStream::connect(("127.0.0.1", api + offset))
                    .await
                    .is_ok()
                {
                    break;
                }
                sleep(Duration::from_millis(200)).await;
            }
        }
    })
    .await
    .wrap_err("fixture validators did not bind their public listeners")?;
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
    ) -> Result<PathBuf> {
        let envelope = self.directory.join(format!("{operation}.prepared.json"));
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
            let bytes = run(child, deadline).await?;
            let receipt: Value = json::from_slice(&bytes)?;
            ensure!(
                text(&receipt, "status")? == "ok",
                "native prepared canary did not succeed"
            );
            private_file(
                &self.directory.join(format!(
                    "{operation}-{}.json",
                    if prepare { "prepare" } else { "submit" }
                )),
                &bytes,
            )?;
        }
        Ok(envelope)
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
    deadline: Instant,
) -> Result<()> {
    let instructions: Vec<InstructionBox> = json::from_slice(&fs::read(instructions_path)?)?;
    ensure!(
        instructions.len() == 1,
        "native installation must be one certificate instruction"
    );
    let client = client.with_request_deadline(deadline.into_std());
    let account = client.account_client()?;
    let mut payload = account.prepare_transaction(
        AccountTransactionDraft::new(
            instructions,
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        )
        .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced),
    )?;
    let quote = account
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
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
    Ok(())
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
fn verify_pulse(peer_configs: &[PathBuf], bundle: &Value) -> Result<()> {
    let record: beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1 =
        json::from_value(field(bundle, "record")?.clone())?;
    record.validate()?;
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
            store.read_index_count()? >= 8,
            "paid deployment did not cross the mandatory epoch boundary"
        );
        let anchor = read_block(&mut store, 6)?;
        let block = read_block(&mut store, 7)?;
        ensure!(
            !block.is_empty() && block.external_entrypoint_count() > 0,
            "mandatory pulse was carried by an empty block"
        );
        let pulse = block
            .npos_consensus_effects()
            .and_then(|effects| effects.finalized_global_beacon_pulse.as_ref())
            .ok_or_else(|| eyre!("mandatory pulse is absent from block seven"))?;
        ensure!(pulse.height == 7, "mandatory pulse height differs");
        beacon::verify_finalized_global_threshold_beacon_pulse_v1(
            &session,
            pulse,
            GlobalThresholdBeaconChainAnchorV1 {
                height: 6,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn four_peer_fresh_custody_bootstrap_reaches_mandatory_pulse() -> Result<()> {
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
    let startup = Instant::now() + PHASE_BUDGET;
    let prepared = prepare::prepare(workspace.path(), &kagami, api, p2p, startup).await?;
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
    let mut peers = spawn_peers(directory, &daemon, &prepared.roster, None, 1)?;
    let outcome: Result<()> = async {
        listeners_started(api, startup).await?;
        ensure!(status_height(&clients, startup).await? == 1, "fresh network produced unsolicited blocks");
        for offset in 0..4 { ready(api + offset, 503, startup).await?; }
        let ceremony_deadline = Instant::now() + PHASE_BUDGET;
        let ceremony = directory.join("beacon-ceremony");
        let (height_read, height_write) = nix::unistd::pipe()?;
        let mut height_write = File::from(height_write);
        let mut provision = command(&launcher, directory);
        provision.args(["beacon-bootstrap", "provision", "--request"]).arg(&prepared.request)
            .arg("--genesis-manifest").arg(directory.join("genesis.json"))
            .arg("--genesis-signed").arg(directory.join("genesis.signed.nrt"))
            .arg("--genesis-public-key").arg(directory.join("genesis.public_key"))
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
        for (index, operation) in ["onboarding", "faucet", "final-canary"].iter().enumerate() {
            previous = Some(canary.operation(operation, previous.as_deref(), ceremony_deadline).await?);
            let height = status_height(&clients, ceremony_deadline).await?;
            ensure!(height == (index as u64) + 2, "useful bootstrap operation did not advance exactly once");
            writeln!(height_write, "{height}")?;
            height_write.flush()?;
        }
        drop(height_write);
        let status = timeout_at(ceremony_deadline, provision.wait()).await??;
        ensure!(status.success(), "fresh native DKG provisioning failed");
        let bundle: Value = json::from_slice(&fs::read(ceremony.join("public-bundle.json"))?)?;
        ensure!(field(&bundle, "finalized_observed_height")?.as_u64() == Some(4), "ceremony backdated its finalization");
        let instruction = sign_and_assemble(&launcher, directory, &ceremony, &prepared.roster, ceremony_deadline).await?;
        // Install real credentials before the on-chain certificate. The same
        // four ledgers are retained; this cannot manufacture a bootstrap pulse.
        let restart = Instant::now() + PHASE_BUDGET;
        peers.stop(restart).await?;
        let peer_configs = install_provider_configs(directory, &bundle, &prepared.roster)?;
        peers = spawn_peers(directory, &daemon, &prepared.roster, Some(&ceremony), 2)?;
        listeners_started(api, restart).await?;
        ensure!(status_height(&clients, restart).await? == 4, "credential installation changed the retained chain");
        for offset in 0..4 { ready(api + offset, 503, restart).await?; }
        submit_install(&clients[0], &instruction, restart).await?;
        ensure!(status_height(&clients, restart).await? == 5, "certificate did not install at its exact next height");
        for offset in 0..4 { ready(api + offset, 200, restart).await?; }
        let mut doctor = command(&cli, directory);
        doctor.args(["--machine", "taira", "doctor", "--scope", "basic", "--public-root", &format!("http://127.0.0.1:{api}"), "--json"]);
        run(doctor, restart).await?;
        // The complete prior clean-client assertions run on real custody. Its
        // paid bootstrap transaction carries height seven's mandatory pulse.
        let genesis_wire = fs::read(directory.join("genesis.signed.nrt"))?;
        super::dataspace_deploy_cli::run_paid_deployment(super::dataspace_deploy_cli::PaidDeploymentFixture {
            binary: &cli, config: &fresh, operator: &directory.join("runtime/operator-signer.key"),
            root: &directory.join("paid-deployment"), genesis_wire: &genesis_wire,
            genesis_public_key: &prepared.genesis_public_key, peer_configs: &peer_configs, clients: &clients,
        }).await?;
        peers.stop(Instant::now() + PHASE_BUDGET).await?;
        verify_pulse(&peer_configs, &bundle)?;
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
