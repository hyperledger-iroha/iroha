//! One-owner-per-seat genesis DKG relay for the signed public-reset deployment.

use super::*;
use iroha_core::beacon::{
    GlobalThresholdBeaconDkgSnapshotV1, RetainedGlobalThresholdBeaconDkgFinalizationV1,
};
use iroha_core::sumeragi::native_journal::NativeJournalCursor;
use iroha_data_model::{
    consensus::GlobalThresholdBeaconKeySessionV1, sumeragi::finality::NativeFinalityJournal,
};
use norito::NoritoSerialize;
use std::{
    fs,
    io::{ErrorKind, Read as _, Seek as _, SeekFrom, Write as _},
    os::{
        fd::{AsFd as _, AsRawFd as _, OwnedFd},
        unix::{
            fs::{MetadataExt as _, OpenOptionsExt as _},
            process::CommandExt as _,
        },
    },
    process::Stdio,
};

const KEY_FD: i32 = 198;
const PUBLIC_FD: i32 = 201;
const FINALITY_FD: i32 = 202;

struct SeatChild {
    child: Child,
    public_writer: File,
    finality_writer: File,
    config_file: File,
    _config_root: tempfile::TempDir,
    attempt_path: PathBuf,
    complete: bool,
}

impl Drop for SeatChild {
    fn drop(&mut self) {
        if !self.complete {
            let _ = terminate_owned_child(&mut self.child);
        }
        let _ = self.config_file.set_len(0);
        let _ = self.config_file.sync_all();
    }
}

impl SeatChild {
    #[allow(
        unsafe_code,
        reason = "only this seat's private config and two bounded FIFO inputs are inherited by its owned daemon child"
    )]
    fn spawn(
        program: &Path,
        root: &Path,
        session: GlobalThresholdBeaconDkgSessionV1,
        chain_id: &str,
        proof_args: &[OsString],
        signer_index: u16,
        config: &[u8],
        deadline: Instant,
    ) -> Result<Self> {
        ensure_local_deadline(Some(deadline))?;
        let credential_memory =
            configured_beacon_credential_memory(config, Path::new("genesis-seat-config.toml"))?;
        // TODO: Recovery must scrub an unconsumed per-seat config copy after a
        // supervisor crash before its daemon child opens FD 198. The signed
        // network attempt remains consumed even when this cleanup is needed.
        let config_root = tempfile::Builder::new()
            .prefix("genesis-seat-config-")
            .tempdir_in(root)?;
        let config_path = config_root.path().join("config.toml");
        reset::inputs::write_new_private(&config_path, config)?;
        let mut config_file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits())?)
            .open(&config_path)?;
        config_file.seek(SeekFrom::Start(0))?;
        let (public_reader, public_writer) = std::io::pipe()?;
        let (finality_reader, finality_writer) = std::io::pipe()?;
        let private = File::from(rustix::io::fcntl_dupfd_cloexec(
            config_file.as_fd(),
            KEY_FD,
        )?);
        let public = File::from(rustix::io::fcntl_dupfd_cloexec(
            public_reader.as_fd(),
            PUBLIC_FD,
        )?);
        let finality = File::from(rustix::io::fcntl_dupfd_cloexec(
            finality_reader.as_fd(),
            FINALITY_FD,
        )?);
        if private.as_raw_fd() != KEY_FD
            || public.as_raw_fd() != PUBLIC_FD
            || finality.as_raw_fd() != FINALITY_FD
        {
            return Err(eyre!("reserved genesis seat descriptor is occupied"));
        }
        let public_writer = File::from(OwnedFd::from(public_writer));
        let finality_writer = File::from(OwnedFd::from(finality_writer));
        for writer in [&public_writer, &finality_writer] {
            let flags = rustix::fs::fcntl_getfl(writer.as_fd())?;
            rustix::fs::fcntl_setfl(writer.as_fd(), flags | rustix::fs::OFlags::NONBLOCK)?;
        }
        let log = |name| -> Result<File> {
            Ok(OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(config_root.path().join(name))?)
        };
        let mut command = Command::new(program);
        command
            .args(beacon_native_args(
                credential_memory,
                "provision-genesis-seat",
                chain_id,
            )?)
            .args(proof_args)
            .arg("--signer-index")
            .arg(signer_index.to_string())
            .arg("--config-fd")
            .arg(KEY_FD.to_string())
            .arg("--public-fd")
            .arg(PUBLIC_FD.to_string())
            .arg("--finality-fd")
            .arg(FINALITY_FD.to_string())
            .arg("--attempt-root")
            .arg(root)
            .arg("--timeout-ms")
            .arg(
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .clamp(1, 3_600_000)
                    .to_string(),
            )
            .env_clear()
            .env("LC_ALL", "C")
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log("stdout.log")?))
            .stderr(Stdio::from(log("stderr.log")?))
            .process_group(0);
        unsafe {
            command.pre_exec(move || {
                for fd in [KEY_FD, PUBLIC_FD, FINALITY_FD] {
                    rustix::io::fcntl_setfd(
                        std::os::fd::BorrowedFd::borrow_raw(fd),
                        rustix::io::FdFlags::empty(),
                    )
                    .map_err(std::io::Error::from)?;
                }
                Ok(())
            });
        }
        let child = command.spawn()?;
        drop(private);
        drop(public);
        drop(finality);
        drop(public_reader);
        drop(finality_reader);
        Ok(Self {
            child,
            public_writer,
            finality_writer,
            config_file,
            _config_root: config_root,
            attempt_path: root.join(format!(
                "attempt-{}-seat-{signer_index}",
                hex::encode(session.attempt_id)
            )),
            complete: false,
        })
    }

    fn poll(&mut self) -> Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some_and(|value| !value.success()) {
            return Err(eyre!(
                "one genesis DKG seat failed; its attempt remains consumed"
            ));
        }
        Ok(status)
    }
}

fn write_frame(writer: &mut File, bytes: &[u8], maximum: usize, deadline: Instant) -> Result<()> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(eyre!("genesis DKG frame exceeds its exact bound"));
    }
    let mut frame = Vec::with_capacity(bytes.len() + 4);
    frame.extend_from_slice(&u32::try_from(bytes.len())?.to_be_bytes());
    frame.extend_from_slice(bytes);
    let mut cursor = 0;
    while cursor < frame.len() {
        ensure_local_deadline(Some(deadline))?;
        match writer.write(&frame[cursor..]) {
            Ok(0) => return Err(eyre!("genesis DKG FIFO closed before the complete frame")),
            Ok(count) => cursor += count,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(
                    PROCESS_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                );
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn read_snapshot(path: &Path) -> Result<Option<GlobalThresholdBeaconDkgSnapshotV1>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits())?)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > PUBLIC_LIMIT {
        return Err(eyre!("genesis seat public snapshot exceeds bound"));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len())?);
    file.take(PUBLIC_LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() > usize::try_from(PUBLIC_LIMIT)? {
        return Err(eyre!("genesis seat public snapshot grew past bound"));
    }
    Ok(
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .ok(),
    )
}

/// Relay signed public edges while each child retains exactly one private dealer state.
pub(super) struct GenesisRelay {
    session: GlobalThresholdBeaconDkgSessionV1,
    state: GlobalThresholdBeaconDkgStateV1,
    children: Vec<SeatChild>,
    clock: NativeJournalCursor,
    proofs: Vec<NativeFinalityJournal>,
    deadline: Instant,
}

impl GenesisRelay {
    pub(super) fn new(
        session: GlobalThresholdBeaconDkgSessionV1,
        clock: NativeJournalCursor,
        deadline: Instant,
    ) -> Result<Self> {
        if session.network_id != clock.network_id()
            || session.start_height != 1
            || clock.tip().is_some()
        {
            return Err(eyre!(
                "genesis relay requires its original signed-genesis clock"
            ));
        }
        Ok(Self {
            session,
            state: GlobalThresholdBeaconDkgStateV1::new(
                session,
                &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
                clock.allocation_budget(),
            )?,
            children: Vec::with_capacity(4),
            clock,
            proofs: Vec::with_capacity(3),
            deadline,
        })
    }

    pub(super) fn spawn_seat(
        &mut self,
        program: &Path,
        root: &Path,
        proof_args: &[OsString],
        signer_index: u16,
        config: &[u8],
    ) -> Result<()> {
        if signer_index != u16::try_from(self.children.len() + 1)? || self.children.len() >= 4 {
            return Err(eyre!("genesis seat order differs from signed roster"));
        }
        self.children.push(SeatChild::spawn(
            program,
            root,
            self.session,
            self.clock.chain_id().as_str(),
            proof_args,
            signer_index,
            config,
            self.deadline,
        )?);
        Ok(())
    }

    fn wait_snapshots(&mut self, name: &str) -> Result<Vec<GlobalThresholdBeaconDkgSnapshotV1>> {
        if self.children.len() != 4 {
            return Err(eyre!("genesis relay lacks one of the signed voting seats"));
        }
        let mut observed = (0..4).map(|_| None).collect::<Vec<_>>();
        loop {
            ensure_local_deadline(Some(self.deadline))?;
            for (index, child) in self.children.iter_mut().enumerate() {
                if observed[index].is_some() {
                    continue;
                }
                if let Some(snapshot) = read_snapshot(&child.attempt_path.join(name))? {
                    observed[index] = Some(snapshot);
                } else if child.poll()?.is_some() {
                    return Err(eyre!("genesis seat exited before its signed public edge"));
                }
            }
            if observed.iter().all(Option::is_some) {
                return observed
                    .into_iter()
                    .map(|snapshot| snapshot.ok_or_else(|| eyre!("missing genesis snapshot")))
                    .collect();
            }
            std::thread::sleep(
                PROCESS_POLL_INTERVAL.min(self.deadline.saturating_duration_since(Instant::now())),
            );
        }
    }

    fn broadcast_public<T: NoritoSerialize>(
        children: &mut [SeatChild],
        value: &T,
        deadline: Instant,
    ) -> Result<()> {
        let bytes = norito::encode_canonical(value)?;
        for child in children {
            write_frame(
                &mut child.public_writer,
                &bytes,
                usize::try_from(PUBLIC_LIMIT)?,
                deadline,
            )?;
        }
        Ok(())
    }

    pub(super) fn publications(&mut self) -> Result<()> {
        let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
        for (index, snapshot) in self
            .wait_snapshots("publication.norito")?
            .iter()
            .enumerate()
        {
            let seat = u16::try_from(index + 1)?;
            if snapshot.session != self.session
                || snapshot.last_updated_height != self.session.start_height
                || snapshot.recipient_keys.len() != 1
                || snapshot.dealer_commitments.len() != 1
                || snapshot.recipient_keys[0].recipient_index != seat
                || snapshot.dealer_commitments[0].dealer_index != seat
                || !snapshot.encrypted_shares.is_empty()
                || !snapshot.share_acceptances.is_empty()
            {
                return Err(eyre!("genesis publication is not one exact signed seat"));
            }
            let _ = GlobalThresholdBeaconDkgStateV1::from_snapshot(
                snapshot,
                &crypto,
                self.state.allocation_budget(),
            )?;
            self.state
                .record_recipient_key(self.session.start_height, &snapshot.recipient_keys[0])?;
            self.state.record_dealer_commitment(
                self.session.start_height,
                &snapshot.dealer_commitments[0],
                &crypto,
            )?;
        }
        let snapshot = self.state.public_snapshot()?;
        Self::broadcast_public(&mut self.children, snapshot.record(), self.deadline)
    }

    pub(super) fn require_running(&mut self) -> Result<()> {
        for child in &mut self.children {
            if child.poll()?.is_some() {
                return Err(eyre!(
                    "genesis seat exited before all required ledger operations"
                ));
            }
        }
        Ok(())
    }

    pub(super) fn advance(&mut self, height: u64, journal: &NativeFinalityJournal) -> Result<()> {
        let next = self
            .session
            .start_height
            .checked_add(u64::try_from(self.proofs.len() + 1)?)
            .ok_or_else(|| eyre!("genesis phase height overflow"))?;
        if height != next
            || u64::try_from(journal.blocks.len())? != height
            || height > self.session.acceptances_end_height
            || self.clock.tip().map(|tip| tip.height()).unwrap_or(1) != height - 1
        {
            return Err(eyre!("genesis phase finality is replayed or discontinuous"));
        }
        let limits = self.clock.limits();
        journal
            .validate_source(limits)
            .map_err(|error| eyre!(error))?;
        let bytes = norito::encode_canonical(journal)?;
        if bytes.is_empty() || bytes.len() > limits.journal_bytes {
            return Err(eyre!(
                "genesis native journal frame exceeds its exact bound"
            ));
        }
        // The original full-prefix cursor verifies native certificates, parent results,
        // epochs and retained-tip continuity before any child sees this phase.
        self.clock.advance(journal.into())?;
        for child in &mut self.children {
            write_frame(
                &mut child.finality_writer,
                &bytes,
                limits.journal_bytes,
                self.deadline,
            )?;
        }
        self.proofs.push(journal.clone());
        Ok(())
    }

    pub(super) fn deliveries(&mut self) -> Result<()> {
        let previous = self.state.public_snapshot()?;
        let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
        for (index, snapshot) in self.wait_snapshots("deliveries.norito")?.iter().enumerate() {
            if snapshot.session != self.session
                || snapshot.last_updated_height != self.session.commitments_end_height
                || snapshot.recipient_keys != previous.recipient_keys
                || snapshot.dealer_commitments != previous.dealer_commitments
                || !snapshot.share_acceptances.is_empty()
                || snapshot.encrypted_shares.len() != 4
                || snapshot
                    .encrypted_shares
                    .iter()
                    .any(|edge| edge.dealer_index != u16::try_from(index + 1).unwrap_or(0))
            {
                return Err(eyre!(
                    "genesis dealer did not publish every exact encrypted edge"
                ));
            }
            let _ = GlobalThresholdBeaconDkgStateV1::from_snapshot(
                snapshot,
                &crypto,
                self.state.allocation_budget(),
            )?;
            for edge in &snapshot.encrypted_shares {
                self.state
                    .record_encrypted_share(self.session.commitments_end_height, edge)?;
            }
        }
        let snapshot = self.state.public_snapshot()?;
        Self::broadcast_public(&mut self.children, snapshot.record(), self.deadline)
    }

    pub(super) fn acceptances(&mut self) -> Result<()> {
        let previous = self.state.public_snapshot()?;
        let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
        for (index, snapshot) in self
            .wait_snapshots("acceptances.norito")?
            .iter()
            .enumerate()
        {
            if snapshot.session != self.session
                || snapshot.last_updated_height != self.session.deliveries_end_height
                || snapshot.recipient_keys != previous.recipient_keys
                || snapshot.dealer_commitments != previous.dealer_commitments
                || snapshot.encrypted_shares != previous.encrypted_shares
                || snapshot.share_acceptances.len() != 4
                || snapshot.share_acceptances.iter().any(|acceptance| {
                    acceptance.recipient_index != u16::try_from(index + 1).unwrap_or(0)
                })
            {
                return Err(eyre!(
                    "genesis recipient did not sign every exact edge acceptance"
                ));
            }
            let _ = GlobalThresholdBeaconDkgStateV1::from_snapshot(
                snapshot,
                &crypto,
                self.state.allocation_budget(),
            )?;
            for acceptance in &snapshot.share_acceptances {
                self.state
                    .record_share_acceptance(self.session.deliveries_end_height, acceptance)?;
            }
        }
        let assembled = self
            .state
            .finalize(self.session.acceptances_end_height, &crypto)?;
        Self::broadcast_public(&mut self.children, assembled, self.deadline)
    }

    pub(super) fn finish(
        mut self,
    ) -> Result<(
        RetainedGlobalThresholdBeaconDkgFinalizationV1,
        Vec<PathBuf>,
        Vec<NativeFinalityJournal>,
    )> {
        if self.proofs.len() != 3 {
            return Err(eyre!(
                "genesis relay lacks complete authenticated h2–h4 finality"
            ));
        }
        // Consume the reducer only after authenticated completion. This moves
        // its exact finalized graph and original ledger; no raw DTO clone escapes.
        let assembled = self.state.into_finalized()?;
        let mut providers = Vec::with_capacity(4);
        for child in &mut self.children {
            loop {
                ensure_local_deadline(Some(self.deadline))?;
                if child.poll()?.is_some() {
                    break;
                }
                std::thread::sleep(
                    PROCESS_POLL_INTERVAL
                        .min(self.deadline.saturating_duration_since(Instant::now())),
                );
            }
            let path = child.attempt_path.join("public-session.norito");
            let (file, snapshot) = open_pinned_regular(&path, "genesis seat public session")?;
            let bytes = read_pinned_bytes(
                &path,
                "genesis seat public session",
                file,
                &snapshot,
                PUBLIC_LIMIT,
            )?;
            let public: GlobalThresholdBeaconKeySessionV1 = norito::decode_canonical_with_limits(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )?;
            if &public != assembled.record() {
                return Err(eyre!("genesis seat finalized another public transcript"));
            }
            let credential = child.attempt_path.join(CREDENTIAL_FILE);
            let metadata = fs::symlink_metadata(&credential)?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.uid() != rustix::process::geteuid().as_raw()
                || metadata.mode() & 0o7777 != 0o600
                || metadata.len() == 0
                || metadata.len() > PUBLIC_LIMIT
            {
                return Err(eyre!("genesis seat private credential is not owner-ready"));
            }
            providers.push(child.attempt_path.join("provider.json"));
            child.complete = true;
        }
        Ok((assembled, providers, self.proofs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::{
        beacon::ceremony::global_beacon_genesis_dkg_session_v1,
        state::World,
        sumeragi::{
            native_journal::{NativeJournalError, authenticate_signed_genesis},
            test_chain::{CertifiedTestChain, TestChainConfig},
        },
    };
    use iroha_data_model::{
        block::{SignedBlock, consensus::SumeragiRootScope},
        sumeragi::finality::{NativeFinalityArtifact, NativeFinalityLimits},
    };
    use iroha_model_base::chain::ChainId;

    struct NativePhaseFixture {
        chain: CertifiedTestChain,
        chain_id: ChainId,
        limits: NativeFinalityLimits,
        journal: NativeFinalityJournal,
        session: GlobalThresholdBeaconDkgSessionV1,
    }

    impl NativePhaseFixture {
        fn new(times: [u64; 3]) -> Self {
            let config = TestChainConfig::new(World::new(), 1_000);
            let chain_id = config.chain_id.clone();
            let mut chain = CertifiedTestChain::start(config).expect("actual signed genesis");
            for time in times {
                chain.commit_at(time, Vec::new());
            }
            let budget = chain.state().ivm_execution_budget();
            let limits = beacon_native_finality_limits(
                NonZeroUsize::new(budget.limit_bytes()).expect("original physical pool"),
            )
            .expect("current native source limits");
            let journal = NativeFinalityJournal {
                blocks: (1..=4)
                    .map(|height| {
                        NativeFinalityArtifact::from_block(chain.committed(height).block(), limits)
                            .expect("original canonical native block")
                    })
                    .collect(),
            };
            let (_, epoch) = authenticate_signed_genesis(
                &journal.blocks[0].block_wire,
                chain.network_id(),
                limits,
            )
            .expect("original signed-genesis epoch");
            let roster = epoch
                .committee
                .iter()
                .map(|seat| seat.validator.clone())
                .collect::<Vec<_>>();
            let session = global_beacon_genesis_dkg_session_v1(chain.network_id(), &roster)
                .expect("current canonical genesis session");
            Self {
                chain,
                chain_id,
                limits,
                journal,
                session,
            }
        }

        fn prefix(&self, height: usize) -> NativeFinalityJournal {
            NativeFinalityJournal {
                blocks: self.journal.blocks[..height].to_vec(),
            }
        }

        fn relay_with_limits(&self, limits: NativeFinalityLimits) -> GenesisRelay {
            let clock = NativeJournalCursor::new(
                self.chain_id.clone(),
                self.chain.network_id(),
                SumeragiRootScope::Global,
                limits,
                &self.chain.state().ivm_execution_budget(),
            )
            .expect("original native clock");
            GenesisRelay::new(
                self.session,
                clock,
                Instant::now() + Duration::from_secs(30),
            )
            .expect("relay owns the same native pool")
        }

        fn relay(&self) -> GenesisRelay {
            self.relay_with_limits(self.limits)
        }
    }

    #[test]
    fn native_phase_prefixes_authenticate_h2_h3_h4_without_h1_finalized_receipt() {
        // These actual native blocks test relay admission and the current node envelope.
        // No seat child or completed DKG ceremony is fabricated by this fixture.
        let fixture = NativePhaseFixture::new([2_000, 3_000, 4_000]);
        let mut relay = fixture.relay();
        assert!(relay.clock.tip().is_none());
        assert!(relay.advance(1, &fixture.prefix(1)).is_err());
        assert!(relay.clock.tip().is_none());
        assert!(relay.proofs.is_empty());
        for height in 2..=4 {
            let prefix = fixture.prefix(height);
            let bytes = norito::encode_canonical(&prefix).expect("current native journal frame");
            let decoded = NativeFinalityJournal::decode(&bytes, fixture.limits)
                .expect("the native node accepts the exact journal envelope");
            assert_eq!(decoded, prefix);
            relay
                .advance(height as u64, &decoded)
                .expect("actual native phase");
            assert_eq!(
                relay.clock.tip().unwrap().block_hash(),
                fixture.chain.committed(height as u64).block_hash()
            );
            assert_eq!(relay.proofs.len(), height - 1);
            assert_eq!(relay.proofs.last(), Some(&prefix));
            assert!(
                relay.advance(height as u64, &prefix).is_err(),
                "replayed phase"
            );
            assert_eq!(relay.proofs.len(), height - 1);
        }
    }

    #[test]
    fn native_phase_refusals_keep_the_original_tip_and_retained_prefix() {
        let fixture = NativePhaseFixture::new([2_000, 3_000, 4_000]);
        let mut relay = fixture.relay();
        relay.advance(2, &fixture.prefix(2)).unwrap();
        let original_hash = relay.clock.tip().unwrap().block_hash();
        let original_result = relay.clock.tip().unwrap().result();
        let original_prefix = relay.proofs[0].clone();

        let mut forged = fixture.prefix(3);
        let mut value = json::to_value(fixture.chain.committed(3).block().as_ref()).unwrap();
        let result = value
            .as_object_mut()
            .unwrap()
            .get_mut("result")
            .unwrap()
            .as_object_mut()
            .unwrap();
        let actual = result
            .get("committed_fragment_count")
            .unwrap()
            .as_u64()
            .unwrap();
        result.insert(
            "committed_fragment_count".into(),
            json::Value::from(actual + 1),
        );
        let block: SignedBlock =
            json::from_value(value).expect("altered result retains its original certificate");
        forged.blocks[2] = NativeFinalityArtifact::from_block(&block, fixture.limits).unwrap();
        assert!(
            relay.advance(3, &forged).is_err(),
            "uncertified result mutation"
        );

        let mut gap = fixture.prefix(4);
        gap.blocks.remove(1);
        assert!(
            relay.advance(3, &gap).is_err(),
            "three entries cannot hide missing H2"
        );
        let suffix = NativeFinalityJournal {
            blocks: fixture.journal.blocks[1..].to_vec(),
        };
        assert!(
            relay.advance(3, &suffix).is_err(),
            "a suffix is not a signed-genesis prefix"
        );
        assert!(
            relay.advance(4, &fixture.prefix(4)).is_err(),
            "phase H3 cannot be skipped"
        );

        let fork = NativePhaseFixture::new([2_001, 3_001, 4_001]);
        assert_eq!(fork.chain.network_id(), fixture.chain.network_id());
        assert_ne!(fork.chain.committed(2).block_hash(), original_hash);
        let mut fork_relay = fork.relay();
        fork_relay.advance(2, &fork.prefix(2)).unwrap();
        fork_relay.advance(3, &fork.prefix(3)).unwrap();
        assert!(
            relay.advance(3, &fork.prefix(3)).is_err(),
            "a genuinely certified fork cannot replace the retained tip"
        );
        assert_eq!(relay.clock.tip().unwrap().block_hash(), original_hash);
        assert_eq!(relay.clock.tip().unwrap().result(), original_result);
        assert_eq!(relay.proofs, vec![original_prefix]);
        relay
            .advance(3, &fixture.prefix(3))
            .expect("original prefix still advances");
    }

    #[test]
    fn native_phase_source_and_original_pool_bounds_refuse_without_advancing() {
        let fixture = NativePhaseFixture::new([2_000, 3_000, 4_000]);
        let mut relay = fixture.relay();
        relay.advance(2, &fixture.prefix(2)).unwrap();
        let original_hash = relay.clock.tip().unwrap().block_hash();
        let pool = fixture.chain.state().ivm_execution_budget();
        let blocker = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
            .unwrap();
        let error = relay
            .advance(3, &fixture.prefix(3))
            .expect_err("original pool is occupied");
        assert!(matches!(
            error.downcast_ref::<NativeJournalError>(),
            Some(NativeJournalError::Block(_))
        ));
        assert_eq!(relay.clock.tip().unwrap().block_hash(), original_hash);
        assert_eq!(relay.proofs.len(), 1);
        drop(blocker);
        relay
            .advance(3, &fixture.prefix(3))
            .expect("same original source retries after original pool releases");

        let mut count_bound = fixture.relay_with_limits(NativeFinalityLimits {
            block_count: 2,
            ..fixture.limits
        });
        count_bound.advance(2, &fixture.prefix(2)).unwrap();
        assert!(count_bound.advance(3, &fixture.prefix(3)).is_err());
        assert_eq!(count_bound.clock.tip().unwrap().height(), 2);
        assert_eq!(count_bound.proofs.len(), 1);
        let mut decode_bound = fixture.relay_with_limits(NativeFinalityLimits {
            allocated_bytes: 1,
            ..fixture.limits
        });
        assert!(decode_bound.advance(2, &fixture.prefix(2)).is_err());
        assert!(decode_bound.clock.tip().is_none());
        assert!(decode_bound.proofs.is_empty());
        let mut oversized = fixture.prefix(2);
        oversized.blocks[1]
            .block_wire
            .resize(fixture.limits.block_bytes + 1, 0);
        assert!(decode_bound.advance(2, &oversized).is_err());
        assert!(decode_bound.clock.tip().is_none());
    }

    #[test]
    fn public_frame_rejects_empty_and_oversized_payloads() {
        let (_reader, writer) = std::io::pipe().expect("local FIFO");
        let mut writer = File::from(OwnedFd::from(writer));
        let deadline = Instant::now() + Duration::from_secs(1);
        assert!(write_frame(&mut writer, b"", 4, deadline).is_err());
        assert!(write_frame(&mut writer, b"oversized", 4, deadline).is_err());
    }

    #[test]
    fn malformed_public_snapshot_never_becomes_a_signed_edge() {
        let root = reset::private_custody_test_dir("beacon-relay-");
        let path = root.path().join("publication.norito");
        assert!(read_snapshot(&path).expect("absent snapshot").is_none());
        reset::inputs::write_new_private(&path, &[1, 2, 3]).expect("write malformed public bytes");
        assert!(read_snapshot(&path).expect("malformed snapshot").is_none());
    }
}
