//! Genuine bounded epoch maintenance runs independently of application operations.
//! Only native Kagami handles seed derivation; only the shipping CLI dispatches.
use super::*;
use iroha_crypto::PublicKey;
use iroha_data_model::{
    NetworkId,
    bridge::BridgeFinalityVerifier,
    isi::kagemusha_v1::KagemushaMintFinalityEpochRosterV1,
    parameter::{Parameter, system::KagemushaMintFinalityNextEpochParameterV1},
    transaction::SignedTransaction,
};
use iroha_model_base::peer::PeerId;
use iroha_version::codec::DecodeVersioned as _;
use std::{collections::BTreeMap, io::Write, num::NonZeroU64};

#[cfg(target_os = "linux")]
#[path = "production_epoch_supervisor.rs"]
mod supervisor;

pub(super) enum Driver {
    Finite,
    #[cfg(target_os = "linux")]
    Supervised,
}

impl Driver {
    // Pure admission runs before fixture paths, credentials, genesis or children
    // exist. Do not substitute the development label into a supervisor policy.
    pub(super) fn admit_build_identity(
        &self,
        identity: iroha_core::release_identity::BuildIdentity,
    ) -> Result<iroha_core::release_identity::BuildIdentity> {
        match self {
            Self::Finite => Ok(identity),
            #[cfg(target_os = "linux")]
            Self::Supervised => {
                identity.release_source_commit().wrap_err(
                    "supervised beacon fixture requires an exact compiled Git source commit before setup; use maintained Taira checks, not --stable-local-metadata",
                )?;
                Ok(identity)
            }
        }
    }
}

const EPOCH_LENGTH: u64 = 11;
const SCHEDULE_EPOCHS: u64 = 8;
// The existing fifteen application operations, at most eight real maintenance
// operations, and five retained replay/snapshot restart phases each retain
// their own original 180-second bound. This is only the monitor's outer bound.
const MONITOR_PHASES: u32 = 15 + SCHEDULE_EPOCHS as u32 + 5;

fn seed_pipe() -> Result<(std::os::fd::OwnedFd, std::os::fd::OwnedFd)> {
    // nix::pipe2 is unavailable on macOS. Protect both owned ends immediately,
    // before supplying seed bytes or spawning any child.
    let descriptors = nix::unistd::pipe()?;
    for descriptor in [&descriptors.0, &descriptors.1] {
        let flags = nix::fcntl::FdFlag::from_bits_truncate(nix::fcntl::fcntl(
            descriptor,
            nix::fcntl::FcntlArg::F_GETFD,
        )?);
        nix::fcntl::fcntl(
            descriptor,
            nix::fcntl::FcntlArg::F_SETFD(flags | nix::fcntl::FdFlag::FD_CLOEXEC),
        )?;
    }
    Ok(descriptors)
}

fn copy_fixture_seed(path: &Path, output: &mut impl Write) -> Result<()> {
    let before = fs::symlink_metadata(path)?;
    ensure!(
        before.is_file()
            && !before.file_type().is_symlink()
            && before.uid() == nix::unistd::geteuid().as_raw()
            && before.mode() & 0o777 == 0o600
            && before.nlink() == 1
            && before.len() == 32,
        "invalid original fixture mint-finality seed custody"
    );
    let mut input = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)?;
    let stamp = |m: &fs::Metadata| (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec());
    ensure!(
        stamp(&input.metadata()?) == stamp(&before),
        "seed changed before owned pipe transfer"
    );
    ensure!(
        std::io::copy(&mut std::io::Read::take(&mut input, 33), output)? == 32,
        "seed length changed during owned pipe transfer"
    );
    ensure!(
        stamp(&input.metadata()?) == stamp(&before),
        "seed changed during owned pipe transfer"
    );
    Ok(())
}

fn signed_genesis_roster(
    directory: &Path,
    network: NetworkId,
    public_key: &PublicKey,
) -> Result<KagemushaMintFinalityEpochRosterV1> {
    let (hash, metadata) = iroha_core::release_identity::genesis_identity(
        &fs::read(directory.join("genesis.signed.nrt"))?,
        public_key,
    )?;
    ensure!(
        hash == iroha_crypto::Hash::from(network.into_genesis_hash()),
        "epoch fixture genesis has another network identity"
    );
    Ok(metadata
        .kagemusha_mint_finality
        .epoch_roster
        .bind_network_id(network)?)
}

fn schedule_parameters(
    bytes: &[u8],
    network: NetworkId,
    roster: &[PeerId],
    genesis_roster: &KagemushaMintFinalityEpochRosterV1,
) -> Result<Vec<KagemushaMintFinalityNextEpochParameterV1>> {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let value: Value = json::from_slice(bytes)?;
    ensure!(
        field(&value, "schema_version")?.as_u64() == Some(1)
            && json::from_value::<NetworkId>(field(&value, "network_id")?.clone())? == network,
        "native epoch schedule belongs to another schema or network"
    );
    let derived_genesis: KagemushaMintFinalityEpochRosterV1 =
        json::from_value(field(&value, "genesis_roster")?.clone())?;
    ensure!(
        derived_genesis == *genesis_roster
            && genesis_roster.network_id == network
            && genesis_roster.epoch == 0,
        "native epoch schedule was not derived from the signed genesis mint seeds"
    );
    let parameters: Vec<Parameter> = json::from_value(field(&value, "parameters")?.clone())?;
    ensure!(
        parameters.len() == SCHEDULE_EPOCHS as usize,
        "native epoch schedule has the wrong bound"
    );
    let expected = roster.iter().cloned().collect::<BTreeSet<_>>();
    ensure!(
        expected.len() == 4,
        "epoch schedule requires four distinct genesis validators"
    );
    parameters
        .into_iter()
        .enumerate()
        .map(|(index, parameter)| {
            let Parameter::Custom(custom) = parameter else {
                return Err(eyre!("epoch schedule contains a non-custom parameter"));
            };
            let parameter =
                KagemushaMintFinalityNextEpochParameterV1::from_custom_parameter(&custom)
                    .ok_or_else(|| {
                        eyre!("native epoch schedule contains an invalid roster parameter")
                    })?;
            ensure!(
                parameter.roster.network_id == network
                    && parameter.roster.epoch == index as u64 + 1
                    && parameter.roster.validators.len() == 4
                    && parameter
                        .roster
                        .validators
                        .iter()
                        .map(|entry| entry.validator.clone())
                        .collect::<BTreeSet<_>>()
                        == expected,
                "native epoch schedule has a gap, foreign network or different voters"
            );
            Ok(parameter)
        })
        .collect()
}

pub(super) async fn prepare_schedule(
    directory: &Path,
    kagami: &Path,
    roster: &[PeerId],
    network: NetworkId,
    genesis_public_key: &PublicKey,
    deadline: Instant,
) -> Result<PathBuf> {
    let mut ordered = BTreeMap::new();
    let mut payment_asset = None;
    for index in 0..4 {
        let native = config(&directory.join(format!("peer{index}.toml")))?;
        ensure!(
            ordered
                .insert(native.common.peer.id.clone(), index)
                .is_none(),
            "duplicate native validator"
        );
        if let Some(expected) = &payment_asset {
            ensure!(
                expected == &native.nexus.fees.fee_asset_id,
                "validator payment assets differ"
            );
        } else {
            payment_asset = Some(native.nexus.fees.fee_asset_id.clone());
        }
    }
    ensure!(
        ordered.keys().cloned().collect::<BTreeSet<_>>() == roster.iter().cloned().collect(),
        "seed sources do not match authenticated genesis voters"
    );
    let (read, write) = seed_pipe()?;
    let mut write = File::from(write);
    // This tiny pipe is filled only inside the runtime fixture; no secret is
    // formatted, logged, returned, or written into the public schedule.
    for index in ordered.values() {
        copy_fixture_seed(
            &directory.join(format!("runtime/mint-finality-signers/peer{index}.seed")),
            &mut write,
        )?;
    }
    drop(write);
    let mut derive = command(kagami, directory);
    derive
        .args([
            "kagemusha",
            "derive-mint-finality-epoch-schedule-v1",
            "--network-id",
        ])
        .arg(network.to_string())
        .args([
            "--epoch",
            "1",
            "--epoch-count",
            "8",
            "--seed-fd",
            "197",
            "--payment-asset",
        ])
        .arg(payment_asset.ok_or_else(|| eyre!("native payment asset absent"))?)
        .args(["--transaction-fee-maximum", "100"])
        .stderr(private_file(
            &directory.join("epoch-schedule.stderr.log"),
            &[],
        )?);
    for peer in ordered.keys() {
        derive.arg("--validator").arg(peer.to_string());
    }
    inherit(&mut derive, &[(read.as_raw_fd(), 197)])?;
    let bytes = run(derive, deadline).await?;
    drop(read);
    let genesis_roster = signed_genesis_roster(
        &directory.join("final-genesis"),
        network,
        genesis_public_key,
    )?;
    schedule_parameters(&bytes, network, roster, &genesis_roster)?;
    let path = directory.join("epoch-schedule.json");
    private_file(&path, &bytes)?;
    Ok(path)
}

pub(super) struct Maintenance {
    binary: PathBuf,
    directory: PathBuf,
    trust: PathBuf,
    schedule: PathBuf,
    journal: PathBuf,
    network: NetworkId,
    child: Child,
    stopped: bool,
    #[cfg(target_os = "linux")]
    supervisor: Option<supervisor::Supervisor>,
}
impl Maintenance {
    pub(super) fn start(
        binary: &Path,
        prepared: &prepare::Prepared,
        trust: PathBuf,
    ) -> Result<Self> {
        let directory = prepared.directory.clone();
        let journal = directory.join("epoch-maintenance");
        fs::create_dir(&journal)?;
        fs::set_permissions(&journal, fs::Permissions::from_mode(0o700))?;
        let mut child = Self::base_command(binary, &directory);
        child
            .args(["maintain", "--trust"])
            .arg(&trust)
            .arg("--schedule")
            .arg(&prepared.epoch_schedule)
            .arg("--journal-dir")
            .arg(&journal)
            .args([
                "--stop-after-epoch",
                "8",
                "--operation-timeout-ms",
                "180000",
                "--timeout-ms",
            ])
            .arg((PHASE_BUDGET * MONITOR_PHASES).as_millis().to_string())
            .stdout(private_file(&journal.join("stdout.log"), &[])?)
            .stderr(private_file(&journal.join("stderr.log"), &[])?);
        Ok(Self {
            binary: binary.into(),
            directory,
            trust,
            schedule: prepared.epoch_schedule.clone(),
            journal,
            network: prepared.network_id,
            child: child.spawn()?,
            stopped: false,
            #[cfg(target_os = "linux")]
            supervisor: None,
        })
    }
    #[cfg(target_os = "linux")]
    pub(super) fn start_supervisor(
        binary: &Path,
        kagami: &Path,
        prepared: &prepare::Prepared,
        trust: PathBuf,
        build_identity: iroha_core::release_identity::BuildIdentity,
    ) -> Result<Self> {
        supervisor::start(binary, kagami, prepared, trust, build_identity)
    }

    fn base_command(binary: &Path, directory: &Path) -> Command {
        let mut child = command(binary, directory);
        child
            .arg("--machine")
            .arg("--config")
            .arg(directory.join("client.toml"))
            .arg("--operator-private-key-file")
            .arg(directory.join("runtime/operator-signer.key"))
            .args(["--fee-payer", "authority", "taira", "epoch-maintenance"]);
        child
    }
    fn operation(&self, epoch: u64) -> PathBuf {
        self.journal.join(format!("epoch-{}-{epoch}", self.network))
    }
    async fn status(&self, epoch: u64, deadline: Instant) -> Result<Value> {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            ensure!(
                !remaining.is_zero(),
                "epoch status exceeded its original deadline"
            );
            let mut child = Self::base_command(&self.binary, &self.directory);
            child
                .args(["status", "--trust"])
                .arg(&self.trust)
                .arg("--schedule")
                .arg(&self.schedule)
                .arg("--journal-dir")
                .arg(&self.journal)
                .arg("--target-epoch")
                .arg(epoch.to_string())
                .arg("--timeout-ms")
                .arg(remaining.as_millis().to_string());
            let bytes = run(child, deadline).await?;
            // Status is strictly read-side. A typed Pending result never
            // authorizes resubmission or a fresh transaction.
            let value: Value = json::from_slice(&bytes)?;
            ensure!(
                field(&value, "target_epoch")?.as_u64() == Some(epoch)
                    && json::from_value::<NetworkId>(field(&value, "network_id")?.clone())?
                        == self.network,
                "native epoch status changed its selected network or target"
            );
            if value
                .get("applied_height")
                .and_then(Value::as_u64)
                .is_some()
            {
                return Ok(value);
            }
            ensure!(
                value.get("state").and_then(Value::as_str) == Some("pending"),
                "native epoch status returned neither verified completion nor typed Pending"
            );
            sleep(Duration::from_millis(200)).await;
        }
    }
    fn transaction(&self, epoch: u64) -> Result<SignedTransaction> {
        let value: Value =
            json::from_slice(&fs::read(self.operation(epoch).join("prepared.json"))?)?;
        let wire = text(&value, "signed_transaction_wire_hex")?;
        ensure!(
            wire.len() % 2 == 0 && wire.is_ascii(),
            "invalid native maintenance wire"
        );
        let bytes = (0..wire.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&wire[index..index + 2], 16))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(hex(&bytes) == wire, "noncanonical native maintenance wire");
        Ok(SignedTransaction::decode_all_versioned(&bytes)?)
    }
    /// Provisional owned-child progress only. Overall acceptance always
    /// reauthenticates epoch1 with native Status after the monitor is stopped.
    pub(super) async fn first_progress(
        &mut self,
        deadline: Instant,
    ) -> Result<HashOf<TransactionEntrypoint>> {
        let transaction = timeout_at(deadline, async {
            while !self.operation(1).join("completion.json").try_exists()? {
                ensure!(
                    self.child.try_wait()?.is_none(),
                    "native epoch monitor exited before first completion"
                );
                sleep(Duration::from_millis(200)).await;
            }
            // The child can still hold its exclusive journal lock briefly
            // after installing this receipt. Do not race a Status process here.
            // This barrier cannot authorize final success or replace the final
            // stopped-monitor native proof audit.
            let receipt: Value = json::from_slice(&fs::read(self.operation(1).join("completion.json"))?)?;
            let transaction = self.transaction(1)?;
            transaction.verify_signature()?;
            ensure!(field(&receipt, "schema_version")?.as_u64() == Some(1)
                && field(&receipt, "target_epoch")?.as_u64() == Some(1)
                && json::from_value::<NetworkId>(field(&receipt, "network_id")?.clone())? == self.network
                && transaction.network_id() == Some(&self.network)
                && text(&receipt, "transaction_hash")? == hex(transaction.hash().as_ref())
                && field(&receipt, "applied_height")?.as_u64() == Some(10),
                "first maintenance progress differs from the exact retained network/transaction/pulse height");
            Ok::<_, eyre::Report>(transaction.hash_as_entrypoint())
        })
        .await
        .wrap_err("first genuine epoch maintenance exceeded the original phase deadline")??;
        #[cfg(target_os = "linux")]
        if self.supervisor.is_some() {
            supervisor::restart(self, deadline).await?;
        }
        Ok(transaction)
    }
    pub(super) async fn stop(&mut self, deadline: Instant) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        if let Some(status) = self.child.try_wait()? {
            ensure!(
                status.success(),
                "native epoch maintenance failed; inspect retained monitor stderr"
            );
        } else {
            self.child.start_kill()?;
            timeout_at(deadline, self.child.wait())
                .await
                .wrap_err("owned epoch monitor failed to stop")??;
        }
        self.stopped = true;
        Ok(())
    }
    pub(super) async fn verify(
        &self,
        prepared: &prepare::Prepared,
        clients: &[iroha::client::Client],
        deadline: Instant,
    ) -> Result<()> {
        let genesis_roster = signed_genesis_roster(
            &prepared.genesis_directory,
            self.network,
            &prepared.genesis_public_key,
        )?;
        let parameters = schedule_parameters(
            &fs::read(&self.schedule)?,
            self.network,
            &prepared.roster,
            &genesis_roster,
        )?;
        ensure!(
            self.stopped,
            "operator must be stopped before the final read-only audit"
        );
        let mut completed = BTreeSet::new();
        let mut receipts = Vec::new();
        for epoch in 1..=SCHEDULE_EPOCHS {
            let completion = self.operation(epoch).join("completion.json");
            if !self.operation(epoch).join("submitted.json").try_exists()? {
                ensure!(
                    !completion.try_exists()?,
                    "completion exists without a retained dispatch claim"
                );
                continue;
            }
            // Cancellation may interrupt only receipt publication. Native
            // status authenticates the retained transaction without creating
            // journal files or re-dispatching; keep that proof result here.
            let receipt = self.status(epoch, deadline).await?;
            if completion.try_exists()? {
                ensure!(
                    receipt == json::from_slice::<Value>(&fs::read(completion)?)?,
                    "native status changed retained maintenance completion"
                );
            }
            ensure!(
                field(&receipt, "applied_height")?
                    .as_u64()
                    .is_some_and(|height| height > 1 && height < epoch * EPOCH_LENGTH),
                "maintenance did not execute before its exact epoch boundary"
            );
            completed.insert(epoch);
            receipts.push(receipt);
        }
        private_file(
            &self.journal.join("fixture-verified-completions.json"),
            &json::to_vec(&receipts)?,
        )?;
        let height = status_height(clients, deadline).await?;
        ensure!(
            height < SCHEDULE_EPOCHS * EPOCH_LENGTH,
            "fixture exceeded its explicit epoch schedule bound"
        );
        ensure!(
            (1..=height / EPOCH_LENGTH).all(|epoch| completed.contains(&epoch)),
            "crossed epoch lacks authenticated completed maintenance"
        );
        #[cfg(target_os = "linux")]
        if let Some(supervisor) = &self.supervisor {
            supervisor::verify(self, supervisor, height)?;
        }
        verify_boundary_chain(prepared, clients, &parameters, height, deadline).await
    }
}

async fn verify_boundary_chain(
    prepared: &prepare::Prepared,
    clients: &[iroha::client::Client],
    parameters: &[KagemushaMintFinalityNextEpochParameterV1],
    height: u64,
    deadline: Instant,
) -> Result<()> {
    let genesis = iroha_genesis::decode_signed_genesis(&fs::read(
        prepared.genesis_directory.join("genesis.signed.nrt"),
    )?)?;
    let pops = iroha_genesis::signed_genesis_validator_pops(&genesis)?;
    let network = prepared.network_id;
    let genesis_hash = genesis.hash();
    let mut expected_roster = pops
        .into_iter()
        .map(|(key, pop)| (PeerId::new(key), pop))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        expected_roster.len() == 4,
        "genesis proof authority is not four peers"
    );
    let (roster, pops): (Vec<_>, Vec<_>) = std::mem::take(&mut expected_roster)
        .into_iter()
        .map(|(validator, pop)| {
            (
                iroha_data_model::block::consensus_v2::ValidatorPower {
                    validator,
                    power: 1,
                },
                pop,
            )
        })
        .unzip();
    let tips = try_join_all(clients.iter().enumerate().map(|(index, client)| {
        let client = client.clone();
        let roster = roster.clone();
        let pops = pops.clone();
        let parameters = parameters.to_vec();
        let path = prepared
            .directory
            .join(format!("epoch-boundary-proof-peer{index}.json"));
        iroha_test_network::read_on_dedicated_thread(move || {
            let bounded = || -> Result<iroha::client::Client> {
                let remaining = deadline.saturating_duration_since(Instant::now());
                ensure!(
                    !remaining.is_zero(),
                    "epoch proof chain exceeded its original audit deadline"
                );
                let mut builder = client.to_builder();
                builder.torii_request_timeout =
                    iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT.min(remaining);
                Ok(builder.build()?)
            };
            let (first, hash) =
                bounded()?.get_bridge_finality_anchor(NonZeroU64::new(1).unwrap(), network)?;
            ensure!(
                hash == genesis_hash
                    && first.block_header.hash() == genesis_hash
                    && first.finality_artifact.height_context.roster == roster
                    && first.finality_artifact.validator_set_pops == pops,
                "epoch proof chain is not anchored to exact signed genesis authority"
            );
            let mut verifier =
                BridgeFinalityVerifier::with_context(network, first.finality_artifact.context_id());
            verifier.verify(&first)?;
            let mut proofs = vec![first];
            for next in 2..=height {
                let proof = bounded()?.get_next_bridge_finality_proof(
                    NonZeroU64::new(next).unwrap(),
                    &mut verifier,
                )?;
                if next % EPOCH_LENGTH == 0 {
                    let target = next / EPOCH_LENGTH;
                    let transition = proof
                        .finality_artifact
                        .height_context
                        .next_epoch_snapshot
                        .as_ref()
                        .ok_or_else(|| eyre!("authenticated epoch boundary omitted next roster"))?;
                    ensure!(
                        transition.epoch == target
                            && transition.kagemusha_mint_finality_epoch_roster
                                == parameters[(target - 1) as usize].roster,
                        "authenticated epoch transition differs from native seed-derived schedule"
                    );
                }
                proofs.push(proof);
            }
            let tip = proofs.last().unwrap().block_header.hash();
            private_file(&path, &json::to_vec(&proofs)?)?;
            Ok(tip)
        })
    }))
    .await?;
    ensure!(
        tips.len() == 4 && tips.iter().all(|tip| *tip == tips[0]),
        "epoch proof tips differ across validators"
    );
    Ok(())
}

#[test]
fn production_epoch_driver_admits_required_build_identity_before_setup() -> Result<()> {
    use iroha_core::release_identity::{BuildIdentity, BuildIdentityError};

    let development = BuildIdentity::from_compiled_parts(
        "fixture-test",
        Some("local-fast-build"),
        None,
        None,
        None,
        None,
    )?;
    assert_eq!(
        Driver::Finite.admit_build_identity(development)?,
        development
    );

    // This known public source revision tests admission syntax only. It is never
    // selected as executable metadata or claimed as authenticated provenance.
    let source = BuildIdentity::from_compiled_parts(
        "fixture-test",
        Some("592c6e0e5adcd2ff5e0492d971bfbb179f591b53"),
        None,
        None,
        None,
        None,
    )?;
    assert_eq!(Driver::Finite.admit_build_identity(source)?, source);
    #[cfg(target_os = "linux")]
    {
        let error = Driver::Supervised
            .admit_build_identity(development)
            .expect_err("supervised custody must reject a development identity before setup");
        assert_eq!(
            error.downcast_ref::<BuildIdentityError>(),
            Some(&BuildIdentityError::DevelopmentSource)
        );
        assert!(error.to_string().contains("before setup"));
        assert_eq!(Driver::Supervised.admit_build_identity(source)?, source);
    }
    #[cfg(not(target_os = "linux"))]
    assert_eq!(
        development.release_source_commit(),
        Err(BuildIdentityError::DevelopmentSource)
    );
    Ok(())
}

#[test]
fn production_epoch_seed_pipe_rejects_shared_or_wrong_length_custody() -> Result<()> {
    let (read, write) = seed_pipe()?;
    for descriptor in [&read, &write] {
        let flags = nix::fcntl::fcntl(descriptor, nix::fcntl::FcntlArg::F_GETFD)?;
        assert_ne!(
            flags & nix::fcntl::FdFlag::FD_CLOEXEC.bits(),
            0,
            "original pipe descriptors must not survive exec alongside the sole inherited target"
        );
    }
    drop((read, write));
    let root = tempfile::tempdir()?;
    let path = root.path().join("seed");
    private_file(&path, &[7; 32])?;
    let mut output = Vec::new();
    copy_fixture_seed(&path, &mut output)?;
    assert_eq!(output, vec![7; 32]);
    fs::hard_link(&path, root.path().join("alias"))?;
    assert!(copy_fixture_seed(&path, &mut Vec::new()).is_err());
    fs::remove_file(root.path().join("alias"))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
    assert!(copy_fixture_seed(&path, &mut Vec::new()).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    fs::write(&path, [7; 31])?;
    assert!(copy_fixture_seed(&path, &mut Vec::new()).is_err());
    Ok(())
}

#[test]
fn production_epoch_schedule_requires_exact_network_roster_and_contiguous_bound() -> Result<()> {
    use iroha_crypto::{Algorithm, Hash};
    use iroha_data_model::isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterV1,
    };
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"epoch-fixture-schedule",
    )));
    let mut roster = (1..=4u8)
        .map(|marker| {
            PeerId::new(
                KeyPair::from_seed(vec![marker; 32], Algorithm::BlsNormal)
                    .public_key()
                    .clone(),
            )
        })
        .collect::<Vec<_>>();
    roster.sort();
    let parameters = (1..=SCHEDULE_EPOCHS).map(|epoch| {
        let validators = roster.iter().enumerate().map(|(index, peer)| {
            iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &[index as u8 + 1;32], epoch, peer.clone()).map_err(|error| eyre!("native test keys: {error:?}"))
        }).collect::<Result<Vec<_>>>()?;
        Ok(Parameter::Custom(KagemushaMintFinalityNextEpochParameterV1 {
            roster: KagemushaMintFinalityEpochRosterV1 { version: KAGEMUSHA_CHAIN_VERSION_V1, network_id: network, epoch, validators }
        }.into_custom_parameter()))
    }).collect::<Result<Vec<_>>>()?;
    let genesis_roster = KagemushaMintFinalityEpochRosterV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1, network_id: network, epoch: 0,
        validators: roster.iter().enumerate().map(|(index, peer)| {
            iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &[index as u8 + 1; 32], 0, peer.clone()).map_err(|error| eyre!("native genesis keys: {error:?}"))
        }).collect::<Result<Vec<_>>>()?,
    };
    let native = norito::json!({"schema_version":1, "network_id":network, "genesis_roster":genesis_roster, "parameters":parameters});
    assert_eq!(
        schedule_parameters(&json::to_vec(&native)?, network, &roster, &genesis_roster)?.len(),
        8
    );
    let foreign = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"different-epoch-fixture",
    )));
    assert!(
        schedule_parameters(&json::to_vec(&native)?, foreign, &roster, &genesis_roster).is_err()
    );
    assert!(
        schedule_parameters(
            &json::to_vec(&native)?,
            network,
            &roster[..3],
            &genesis_roster
        )
        .is_err()
    );
    let mut gap = native.clone();
    gap.get_mut("parameters")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .swap(1, 2);
    assert!(schedule_parameters(&json::to_vec(&gap)?, network, &roster, &genesis_roster).is_err());
    let mut substituted = native.clone();
    let mut other_keys = genesis_roster.clone();
    other_keys.validators[0] =
        iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
            &[99; 32],
            0,
            roster[0].clone(),
        )
        .map_err(|error| eyre!("substituted seed keys: {error:?}"))?;
    *substituted.get_mut("genesis_roster").unwrap() = json::to_value(&other_keys)?;
    assert!(
        schedule_parameters(
            &json::to_vec(&substituted)?,
            network,
            &roster,
            &genesis_roster
        )
        .is_err()
    );
    let mut absent_genesis = native.clone();
    absent_genesis
        .as_object_mut()
        .unwrap()
        .remove("genesis_roster");
    assert!(
        schedule_parameters(
            &json::to_vec(&absent_genesis)?,
            network,
            &roster,
            &genesis_roster
        )
        .is_err()
    );
    let mut missing = native;
    missing
        .get_mut("parameters")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(
        schedule_parameters(&json::to_vec(&missing)?, network, &roster, &genesis_roster).is_err()
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn production_epoch_supervisor_renews_and_resumes_after_owned_restart() -> Result<()> {
    super::run_fresh_custody_bootstrap(Driver::Supervised).await
}
