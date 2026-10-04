//! One-owner-per-seat genesis DKG relay for the signed public-reset deployment.

use super::*;
use iroha_core::beacon::{
    GlobalThresholdBeaconDkgSnapshotV1, RetainedGlobalThresholdBeaconDkgFinalizationV1,
};
use iroha_data_model::{
    consensus::GlobalThresholdBeaconKeySessionV1, sumeragi_finality::SumeragiFinalityVerifier,
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
const MAX_PROOF_FRAME: usize = 4 * 1024 * 1024;

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
            ))
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
    verifier: SumeragiFinalityVerifier,
    proofs: Vec<SumeragiFinalityProof>,
    deadline: Instant,
}

impl GenesisRelay {
    pub(super) fn new(
        session: GlobalThresholdBeaconDkgSessionV1,
        first_finality: &SumeragiFinalityProof,
        mut verifier: SumeragiFinalityVerifier,
        deadline: Instant,
        budget: &iroha_core::state::AllocationBudget,
    ) -> Result<Self> {
        if first_finality.block_header.height().get() != 1 {
            return Err(eyre!("genesis relay requires authenticated h1 finality"));
        }

        verifier.verify(first_finality)?;
        Ok(Self {
            session,
            state: GlobalThresholdBeaconDkgStateV1::new(
                session,
                &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
                budget,
            )?,
            children: Vec::with_capacity(4),
            verifier,
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

    pub(super) fn advance(&mut self, height: u64, proof: &SumeragiFinalityProof) -> Result<()> {
        if height != self.session.start_height + u64::try_from(self.proofs.len() + 1)?
            || proof.block_header.height().get() != height
            || height > self.session.acceptances_end_height
        {
            return Err(eyre!("genesis phase finality is replayed or discontinuous"));
        }
        self.verifier.verify(proof)?;
        let bytes = norito::encode_canonical(proof)?;
        for child in &mut self.children {
            write_frame(
                &mut child.finality_writer,
                &bytes,
                MAX_PROOF_FRAME,
                self.deadline,
            )?;
        }
        self.proofs.push(proof.clone());
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
        Vec<SumeragiFinalityProof>,
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
