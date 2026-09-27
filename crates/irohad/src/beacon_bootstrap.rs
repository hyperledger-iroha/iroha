//! Per-seat authenticated genesis and rotation beacon DKG with exact-quorum finalization.
//!
//! Each provision process owns one dealer secret and one recipient key. Public
//! frames are signed, phase heights are independently finality-verified, and a
//! failed attempt is never rerolled in the same owner-private journal root.

use crate::external_software_signer::{
    GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1, RuntimeGlobalBeaconShareProvisioningV1,
    encode_global_beacon_partial_signer_credential_v1,
    global_beacon_partial_signer_inventory_digest_v1,
    global_beacon_partial_signer_public_inventory_digest_v1,
};
use clap::{Parser, Subcommand};
use iroha_core::beacon::{
    AdaptiveGlobalThresholdBeaconDkgCryptoV1, FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    GlobalThresholdBeaconDkgSnapshotV1, GlobalThresholdBeaconDkgStateV1,
    LocalGlobalThresholdBeaconDkgSeatV1, global_threshold_beacon_roster_hash_v1,
};
use iroha_core::state::{
    THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
    threshold_key_lifecycle_certificate_preimage_v1, verify_threshold_key_lifecycle_certificate_v1,
};
use iroha_core::validator_committee_evidence::{
    COMMITTEE_PROVISIONING_EVIDENCE_MAX_BYTES_V1, ValidatorCommitteeSelectionEvidenceV1,
    VerifiedValidatorCommitteeSelectionV1, verify_validator_committee_selection_evidence_v1,
};
use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, HashOf, KeyPair, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::HeightContextId,
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    consensus::{
        GlobalThresholdBeaconDkgSessionV1, GlobalThresholdBeaconKeySessionV1,
        v2::is_valid_committee_size,
    },
    isi::{
        InstructionBox,
        consensus_keys::{
            ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
            ThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleSignatureV1,
        },
    },
    nexus::ValidatorCommitteePreparationV1,
};
use iroha_model_base::peer::PeerId;
use norito::derive::{JsonDeserialize, JsonSerialize};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, File},
    io::{Read as _, Write as _},
    os::{fd::BorrowedFd, unix::fs::MetadataExt as _},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};
use zeroize::{Zeroize as _, Zeroizing};

mod genesis_seat;
mod rotation_seat;
use rotation_seat::provision_rotation_seat_command;

const MAX_PUBLIC_BYTES: usize = 32 * 1024 * 1024;
const MAX_ROTATION_PHASE_PROOF_BYTES: usize = 4 * 1024 * 1024;
const MAX_TIMEOUT_MS: u64 = 3_600_000;
const ROTATION_PENDING_SHARE_NAME: &str = "pending-share.bin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    InvalidInput,
    InvalidCustody,
    Crypto,
    Height,
    Deadline,
    Io,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "beacon bootstrap public input is invalid",
            Self::InvalidCustody => "beacon bootstrap custody path is invalid",
            Self::Crypto => "beacon bootstrap cryptographic validation failed",
            Self::Height => "beacon bootstrap committed-height observation is invalid or closed",
            Self::Deadline => "beacon bootstrap operation deadline elapsed",
            Self::Io => "beacon bootstrap bounded I/O failed",
        })
    }
}
type Result<T> = std::result::Result<T, Error>;

#[derive(Parser)]
#[command(
    name = "iroha3d_taira beacon-bootstrap",
    about = "Run one-seat beacon DKG and exact-quorum lifecycle finalization; never submits a transaction"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Run one signed-genesis voting seat's authenticated, one-shot DKG process.
    ProvisionGenesisSeat {
        #[command(flatten)]
        genesis: GenesisProofArgs,
        #[arg(long)]
        signer_index: u16,
        #[arg(
            long,
            conflicts_with = "config_fd",
            required_unless_present = "config_fd"
        )]
        key_fd: Option<i32>,
        #[arg(long, conflicts_with = "key_fd", required_unless_present = "key_fd")]
        config_fd: Option<i32>,
        #[arg(long)]
        public_fd: i32,
        #[arg(long)]
        finality_fd: i32,
        #[arg(long)]
        attempt_root: PathBuf,
        #[arg(long, default_value_t = 180_000)]
        timeout_ms: u64,
    },
    /// Assemble only the all-seat signed genesis DKG and unsigned install draft.
    AssembleGenesisDkg {
        #[command(flatten)]
        genesis: GenesisProofArgs,
        #[arg(long, required = true, num_args = 1..)]
        phase_proof: Vec<PathBuf>,
        #[arg(long)]
        public_session: PathBuf,
        #[arg(long, required = true, num_args = 1..)]
        provider: Vec<PathBuf>,
        #[arg(long)]
        certificate_height: u64,
        #[arg(long)]
        output: PathBuf,
    },
    /// Sign one signed-genesis-roster FinalizeGlobalBeaconKey draft.
    SignGenesisInstall {
        #[arg(long)]
        network_id: NetworkId,
        #[arg(long)]
        chain_discriminant: u16,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        signer_index: u16,
        #[arg(
            long,
            conflicts_with = "config_fd",
            required_unless_present = "config_fd"
        )]
        key_fd: Option<i32>,
        #[arg(long, conflicts_with = "key_fd", required_unless_present = "key_fd")]
        config_fd: Option<i32>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify the exact genesis quorum and emit only the native install instruction.
    AssembleGenesisInstall {
        #[arg(long)]
        network_id: NetworkId,
        #[arg(long)]
        chain_discriminant: u16,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, required = true, num_args = 1..)]
        signature: Vec<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Run one target seat's authenticated, one-shot rotation DKG process.
    ProvisionRotationSeat {
        #[command(flatten)]
        proof: RotationProofArgs,
        #[arg(long)]
        signer_index: u16,
        #[arg(
            long,
            conflicts_with = "config_fd",
            required_unless_present = "config_fd"
        )]
        key_fd: Option<i32>,
        #[arg(long, conflicts_with = "key_fd", required_unless_present = "key_fd")]
        config_fd: Option<i32>,
        #[arg(long)]
        public_fd: i32,
        #[arg(long)]
        finality_fd: i32,
        #[arg(long)]
        provider_handle: String,
        #[arg(long)]
        provider_revision: u64,
        /// Existing owner-private root; the exact attempt/seat child is derived by the daemon.
        #[arg(long)]
        attempt_root: PathBuf,
        #[arg(long, default_value_t = 180_000)]
        timeout_ms: u64,
    },
    /// Assemble only signed all-seat public DKG material for current-quorum review.
    AssembleRotationDkg {
        #[command(flatten)]
        proof: RotationProofArgs,
        #[arg(long)]
        public_session: PathBuf,
        /// Canonical finality proofs for every height after selection through DKG finalization.
        #[arg(long, required = true, num_args = 1..)]
        phase_proof: Vec<PathBuf>,
        #[arg(long, required = true, num_args = 1..)]
        provider: Vec<PathBuf>,
        #[arg(long)]
        certificate_height: u64,
        #[arg(long)]
        output: PathBuf,
    },
    /// Sign one exact current-quorum FinalizeGlobalBeaconKey draft.
    SignRotation {
        #[command(flatten)]
        proof: RotationProofArgs,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        signer_index: u16,
        #[arg(
            long,
            conflicts_with = "config_fd",
            required_unless_present = "config_fd"
        )]
        key_fd: Option<i32>,
        #[arg(long, conflicts_with = "key_fd", required_unless_present = "key_fd")]
        config_fd: Option<i32>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify the current exact quorum and emit only the native finalization instruction.
    AssembleRotation {
        #[command(flatten)]
        proof: RotationProofArgs,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, required = true, num_args = 1..)]
        signature: Vec<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(clap::Args)]
struct GenesisProofArgs {
    #[arg(long)]
    network_id: NetworkId,
    #[arg(long)]
    chain_discriminant: u16,
    #[arg(long)]
    request: PathBuf,
    #[arg(long)]
    genesis_manifest: PathBuf,
    #[arg(long)]
    genesis_signed: PathBuf,
    #[arg(long)]
    genesis_public_key: PathBuf,
    #[arg(long)]
    genesis_finality: PathBuf,
}

#[derive(clap::Args)]
struct RotationProofArgs {
    #[arg(long)]
    selection_evidence: PathBuf,
    #[arg(long)]
    network_id: NetworkId,
    #[arg(long)]
    trusted_context_id: Hash,
    #[arg(long)]
    anchor_height: u64,
    #[arg(long)]
    target_epoch: u64,
    #[arg(long)]
    transition_id: Hash,
}

#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Provider {
    signer_index: u16,
    validator: PeerId,
    handle: String,
    revision: u64,
    policy_digest: [u8; 32],
}
#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RotationPublicBundle {
    schema: String,
    preparation: ValidatorCommitteePreparationV1,
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
    finalized_observed_height: u64,
    phase_proofs: Vec<BridgeFinalityProof>,
    record: FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    finalization_draft: ThresholdKeyLifecycleCertificateV1,
    providers: Vec<Provider>,
}

/// Dispatch only the explicit offline bootstrap subcommand, before runtime-key loading.
pub(crate) fn dispatch_if_requested() -> bool {
    let mut args = std::env::args_os();
    let _ = args.next();
    if args.next().as_deref() != Some(std::ffi::OsStr::new("beacon-bootstrap")) {
        return false;
    }
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(
        crate::taira_runtime_signer::TAIRA_CHAIN_DISCRIMINANT_V1,
    );
    let parsed = Args::parse_from(std::iter::once(OsString::from("beacon-bootstrap")).chain(args));
    let result = match parsed.command {
        Command::ProvisionGenesisSeat {
            genesis,
            signer_index,
            key_fd,
            config_fd,
            public_fd,
            finality_fd,
            attempt_root,
            timeout_ms,
        } => genesis_seat::provision_genesis_seat_command(
            genesis.network_id,
            genesis.chain_discriminant,
            &genesis.request,
            &genesis.genesis_manifest,
            &genesis.genesis_signed,
            &genesis.genesis_public_key,
            &genesis.genesis_finality,
            signer_index,
            key_fd,
            config_fd,
            public_fd,
            finality_fd,
            &attempt_root,
            timeout_ms,
        ),
        Command::AssembleGenesisDkg {
            genesis,
            phase_proof,
            public_session,
            provider,
            certificate_height,
            output,
        } => genesis_seat::assemble_genesis_dkg_command(
            genesis.network_id,
            genesis.chain_discriminant,
            &genesis.request,
            &genesis.genesis_manifest,
            &genesis.genesis_signed,
            &genesis.genesis_public_key,
            &genesis.genesis_finality,
            &phase_proof,
            &public_session,
            &provider,
            certificate_height,
            &output,
        ),
        Command::SignGenesisInstall {
            network_id,
            chain_discriminant,
            bundle,
            signer_index,
            key_fd,
            config_fd,
            output,
        } => genesis_seat::sign_genesis_install_command(
            network_id,
            chain_discriminant,
            &bundle,
            signer_index,
            key_fd,
            config_fd,
            &output,
        ),
        Command::AssembleGenesisInstall {
            network_id,
            chain_discriminant,
            bundle,
            signature,
            output,
        } => genesis_seat::assemble_genesis_install_command(
            network_id,
            chain_discriminant,
            &bundle,
            &signature,
            &output,
        ),
        Command::ProvisionRotationSeat {
            proof,
            signer_index,
            key_fd,
            config_fd,
            public_fd,
            finality_fd,
            provider_handle,
            provider_revision,
            attempt_root,
            timeout_ms,
        } => provision_rotation_seat_command(
            &proof,
            signer_index,
            key_fd,
            config_fd,
            public_fd,
            finality_fd,
            &provider_handle,
            provider_revision,
            &attempt_root,
            timeout_ms,
        ),
        Command::AssembleRotationDkg {
            proof,
            public_session,
            phase_proof,
            provider,
            certificate_height,
            output,
        } => rotation_seat::assemble_rotation_dkg_command(
            &proof,
            &public_session,
            &phase_proof,
            &provider,
            certificate_height,
            &output,
        ),
        Command::SignRotation {
            proof,
            bundle,
            signer_index,
            key_fd,
            config_fd,
            output,
        } => sign_rotation_command(&proof, &bundle, signer_index, key_fd, config_fd, &output),
        Command::AssembleRotation {
            proof,
            bundle,
            signature,
            output,
        } => assemble_rotation_command(&proof, &bundle, &signature, &output),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
    true
}

fn require_budget(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}
fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.is_file()
        && b.is_file()
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.mode() == b.mode()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
// Resolve each ancestor relative to its already opened directory. Holding the
// selected directory keeps a concurrent pathname replacement from redirecting
// credential writes; revalidation refuses to publish success after a replacement.
pub(crate) struct Directory {
    path: PathBuf,
    file: File,
}
impl Directory {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        use rustix::fs::{Mode, OFlags};
        if !path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        {
            return Err(Error::InvalidCustody);
        }
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mut file =
            File::from(rustix::fs::open("/", flags, Mode::empty()).map_err(|_| Error::Io)?);
        validate_directory(&file.metadata().map_err(|_| Error::Io)?)?;
        for component in path.components() {
            if let Component::Normal(name) = component {
                file = File::from(
                    rustix::fs::openat(&file, name, flags, Mode::empty())
                        .map_err(|_| Error::InvalidCustody)?,
                );
                validate_directory(&file.metadata().map_err(|_| Error::Io)?)?;
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }
    pub(crate) fn revalidate(&self) -> Result<()> {
        let current = Self::open(&self.path)?;
        let held = self.file.metadata().map_err(|_| Error::Io)?;
        let named = current.file.metadata().map_err(|_| Error::Io)?;
        validate_directory(&held)?;
        if held.dev() != named.dev()
            || held.ino() != named.ino()
            || held.uid() != named.uid()
            || held.gid() != named.gid()
            || held.mode() != named.mode()
        {
            return Err(Error::InvalidCustody);
        }
        Ok(())
    }
    pub(crate) fn child(&self, name: &std::ffi::OsStr) -> Result<Self> {
        use rustix::fs::{Mode, OFlags};
        single_name(name)?;
        self.revalidate()?;
        rustix::fs::mkdirat(&self.file, name, Mode::from_raw_mode(0o700)).map_err(|_| Error::Io)?;
        let file = File::from(
            rustix::fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Io)?,
        );
        let child = Self {
            path: self.path.join(name),
            file,
        };
        let m = child.file.metadata().map_err(|_| Error::Io)?;
        if m.uid() != rustix::process::geteuid().as_raw() || m.mode() & 0o7777 != 0o700 {
            return Err(Error::InvalidCustody);
        }
        self.file.sync_all().map_err(|_| Error::Io)?;
        self.revalidate()?;
        child.revalidate()?;
        Ok(child)
    }
    /// Publish a complete staged generation without replacing any prior name.
    pub(crate) fn publish_child(&self, child: &Self, name: &std::ffi::OsStr) -> Result<()> {
        single_name(name)?;
        self.revalidate()?;
        child.revalidate()?;
        if child.path.parent() != Some(self.path.as_path()) {
            return Err(Error::InvalidCustody);
        }
        let old_name = child.path.file_name().ok_or(Error::InvalidCustody)?;
        child.file.sync_all().map_err(|_| Error::Io)?;
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        ))]
        rustix::fs::renameat_with(
            &self.file,
            old_name,
            &self.file,
            name,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|_| Error::Io)?;
        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )))]
        return Err(Error::InvalidCustody);
        self.file.sync_all().map_err(|_| Error::Io)?;
        self.revalidate()?;
        let published = Self::open(&self.path.join(name))?;
        let held = child.file.metadata().map_err(|_| Error::Io)?;
        let named = published.file.metadata().map_err(|_| Error::Io)?;
        if held.dev() != named.dev() || held.ino() != named.ino() {
            return Err(Error::InvalidCustody);
        }
        Ok(())
    }
    /// Remove only known staging files from a held child directory after failed publication.
    pub(crate) fn discard_child(&self, child: &Self, names: &[&str]) {
        if self.revalidate().is_err()
            || child.revalidate().is_err()
            || child.path.parent() != Some(self.path.as_path())
        {
            return;
        }
        for name in names {
            if single_name(std::ffi::OsStr::new(name)).is_ok() {
                let _ = rustix::fs::unlinkat(&child.file, *name, rustix::fs::AtFlags::empty());
            }
        }
        if let Some(name) = child.path.file_name() {
            let _ = rustix::fs::unlinkat(&self.file, name, rustix::fs::AtFlags::REMOVEDIR);
            let _ = self.file.sync_all();
        }
    }
    pub(crate) fn write_new(
        &self,
        name: &std::ffi::OsStr,
        bytes: &[u8],
        private: bool,
    ) -> Result<()> {
        use rustix::fs::{Mode, OFlags};
        single_name(name)?;
        if bytes.is_empty() || bytes.len() > MAX_PUBLIC_BYTES {
            return Err(Error::InvalidInput);
        }
        self.revalidate()?;
        let mut file = File::from(
            rustix::fs::openat(
                &self.file,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(if private { 0o600 } else { 0o644 }),
            )
            .map_err(|_| Error::Io)?,
        );
        let before = file.metadata().map_err(|_| Error::Io)?;
        if before.uid() != rustix::process::geteuid().as_raw()
            || before.nlink() != 1
            || !before.is_file()
            || (private && before.mode() & 0o7777 != 0o600)
        {
            return Err(Error::InvalidCustody);
        }
        file.write_all(bytes).map_err(|_| Error::Io)?;
        file.sync_all().map_err(|_| Error::Io)?;
        let named = File::from(
            rustix::fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Io)?,
        );
        if !same_file(
            &file.metadata().map_err(|_| Error::Io)?,
            &named.metadata().map_err(|_| Error::Io)?,
        ) {
            return Err(Error::InvalidCustody);
        }
        self.file.sync_all().map_err(|_| Error::Io)?;
        self.revalidate()
    }
}
fn single_name(name: &std::ffi::OsStr) -> Result<()> {
    let mut parts = Path::new(name).components();
    if !matches!(parts.next(), Some(Component::Normal(_))) || parts.next().is_some() {
        return Err(Error::InvalidCustody);
    }
    Ok(())
}
fn validate_directory(m: &fs::Metadata) -> Result<()> {
    if !m.is_dir()
        || (m.uid() != 0 && m.uid() != rustix::process::geteuid().as_raw())
        || m.mode() & 0o022 != 0
    {
        return Err(Error::InvalidCustody);
    }
    Ok(())
}
fn read_public_bytes(path: &Path) -> Result<Vec<u8>> {
    read_public_bytes_bounded(path, MAX_PUBLIC_BYTES)
}
pub(crate) fn read_public_bytes_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    use rustix::fs::{Mode, OFlags};
    let parent = Directory::open(path.parent().ok_or(Error::InvalidCustody)?)?;
    let name = path.file_name().ok_or(Error::InvalidCustody)?;
    single_name(name)?;
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
    let mut file = File::from(
        rustix::fs::openat(&parent.file, name, flags, Mode::empty()).map_err(|_| Error::Io)?,
    );
    let before = file.metadata().map_err(|_| Error::Io)?;
    if !before.is_file()
        || before.nlink() != 1
        || before.mode() & 0o022 != 0
        || before.len() == 0
        || before.len() > maximum as u64
    {
        return Err(Error::InvalidInput);
    }
    let mut bytes = vec![0; before.len() as usize];
    file.read_exact(&mut bytes).map_err(|_| Error::Io)?;
    let mut end = [0];
    let named = File::from(
        rustix::fs::openat(&parent.file, name, flags, Mode::empty()).map_err(|_| Error::Io)?,
    );
    if file.read(&mut end).map_err(|_| Error::Io)? != 0
        || !same_file(&before, &file.metadata().map_err(|_| Error::Io)?)
        || !same_file(&before, &named.metadata().map_err(|_| Error::Io)?)
    {
        return Err(Error::InvalidInput);
    }
    parent.revalidate()?;
    Ok(bytes)
}
fn read_json<T: norito::json::JsonDeserialize>(path: &Path) -> Result<T> {
    norito::json::from_slice(&read_public_bytes(path)?).map_err(|_| Error::InvalidInput)
}
fn json_bytes<T: norito::json::JsonSerialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = norito::json::to_vec(value).map_err(|_| Error::InvalidInput)?;
    if bytes.is_empty() || bytes.len() > MAX_PUBLIC_BYTES {
        return Err(Error::InvalidInput);
    }
    Ok(bytes)
}
fn write_new(path: &Path, bytes: &[u8], private: bool) -> Result<()> {
    Directory::open(path.parent().ok_or(Error::InvalidCustody)?)?.write_new(
        path.file_name().ok_or(Error::InvalidCustody)?,
        bytes,
        private,
    )
}
fn read_exact_until(fd: BorrowedFd<'_>, deadline: Instant, bytes: &mut [u8]) -> Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        require_budget(deadline)?;
        let timeout =
            rustix::event::Timespec::try_from(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| Error::Deadline)?;
        let mut polls = [rustix::event::PollFd::new(
            &fd,
            rustix::event::PollFlags::IN,
        )];
        match rustix::event::poll(&mut polls, Some(&timeout)) {
            Ok(0) => return Err(Error::Deadline),
            Ok(_) if polls[0].revents().contains(rustix::event::PollFlags::NVAL) => {
                return Err(Error::Height);
            }
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return Err(Error::Io),
        }
        match rustix::io::read(fd, &mut bytes[offset..]) {
            Ok(0) => return Err(Error::Height),
            Ok(count) => offset += count,
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return Err(Error::Io),
        }
    }
    Ok(())
}

fn read_rotation_phase_height(
    fd: BorrowedFd<'_>,
    deadline: Instant,
    verifier: &mut BridgeFinalityVerifier,
    last_height: &mut u64,
    cutoff_height: u64,
) -> Result<u64> {
    // Each FIFO frame is a big-endian u32 byte length followed by one canonical
    // BridgeFinalityProof. The proof chain, never the controller's claimed height,
    // advances the DKG phase clock.
    let mut length = [0_u8; 4];
    read_exact_until(fd, deadline, &mut length)?;
    let length = usize::try_from(u32::from_be_bytes(length)).map_err(|_| Error::Height)?;
    if length == 0 || length > MAX_ROTATION_PHASE_PROOF_BYTES {
        return Err(Error::Height);
    }
    let mut encoded = vec![0_u8; length];
    read_exact_until(fd, deadline, &mut encoded)?;
    let proof: BridgeFinalityProof = norito::decode_canonical_with_limits(
        &encoded,
        norito::canonical_decode_limits(encoded.len()),
    )
    .map_err(|_| Error::Crypto)?;
    let height = proof.finality_artifact.height;
    check_rotation_phase_height(
        *last_height,
        height,
        proof.block_header.height().get(),
        cutoff_height,
    )?;
    verifier.verify(&proof).map_err(|_| Error::Crypto)?;
    *last_height = height;
    Ok(height)
}

fn check_rotation_phase_height(
    last_height: u64,
    proof_height: u64,
    header_height: u64,
    cutoff_height: u64,
) -> Result<()> {
    if proof_height != last_height.checked_add(1).ok_or(Error::Height)?
        || header_height != proof_height
        || proof_height >= cutoff_height
    {
        return Err(Error::Height);
    }
    Ok(())
}

fn rotation_phase_verifier(
    proof: &RotationProofArgs,
    evidence: &ValidatorCommitteeSelectionEvidenceV1,
) -> Result<BridgeFinalityVerifier> {
    let mut verifier = BridgeFinalityVerifier::with_context(
        proof.network_id,
        HeightContextId(HashOf::from_untyped_unchecked(proof.trusted_context_id)),
    );
    for artifact in &evidence.finality_chain {
        verifier.verify(artifact).map_err(|_| Error::Crypto)?;
    }
    Ok(verifier)
}

fn validate_rotation_phase_chain(
    proof: &RotationProofArgs,
    evidence: &ValidatorCommitteeSelectionEvidenceV1,
    start_height: u64,
    final_height: u64,
    cutoff_height: u64,
    chain: &[BridgeFinalityProof],
) -> Result<()> {
    let expected_count = usize::try_from(
        final_height
            .checked_sub(start_height)
            .ok_or(Error::Height)?,
    )
    .map_err(|_| Error::Height)?;
    if chain.len() != expected_count {
        return Err(Error::Height);
    }
    let mut verifier = rotation_phase_verifier(proof, evidence)?;
    let mut last_height = start_height;
    for phase in chain {
        let height = phase.finality_artifact.height;
        check_rotation_phase_height(
            last_height,
            height,
            phase.block_header.height().get(),
            cutoff_height,
        )?;
        verifier.verify(phase).map_err(|_| Error::Crypto)?;
        last_height = height;
    }
    if last_height != final_height {
        return Err(Error::Height);
    }
    Ok(())
}

fn draft_rotation_certificate(
    authorization_roster: &[PeerId],
    incumbent_session_id: [u8; 32],
    record: &FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    finalized_height: u64,
    effective_height: u64,
) -> Result<ThresholdKeyLifecycleCertificateV1> {
    let seats = authorization_roster.len();
    if effective_height <= finalized_height {
        return Err(Error::Height);
    }
    if !is_valid_committee_size(seats)
        || authorization_roster
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(Error::InvalidInput);
    }
    Ok(ThresholdKeyLifecycleCertificateV1 {
        version: THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
        action: ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
        expected_active_session_id: Some(incumbent_session_id),
        effective_height,
        network_id: record.session.network_id,
        roster_hash: global_threshold_beacon_roster_hash_v1(authorization_roster),
        committee_size: u16::try_from(seats).map_err(|_| Error::InvalidInput)?,
        quorum: u16::try_from(2 * ((seats - 1) / 3) + 1).map_err(|_| Error::InvalidInput)?,
        session_id: record.session.session_id,
        transcript_hash: record.session.transcript_hash,
        public_state: norito::encode_canonical(record).map_err(|_| Error::Crypto)?,
        signatures: Vec::new(),
    })
}

fn read_verified_rotation_selection(
    proof: &RotationProofArgs,
) -> Result<(
    ValidatorCommitteeSelectionEvidenceV1,
    VerifiedValidatorCommitteeSelectionV1,
)> {
    let bytes = read_public_bytes_bounded(
        &proof.selection_evidence,
        COMMITTEE_PROVISIONING_EVIDENCE_MAX_BYTES_V1,
    )?;
    let evidence: ValidatorCommitteeSelectionEvidenceV1 =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| Error::InvalidInput)?;
    let selected = verify_validator_committee_selection_evidence_v1(
        &evidence,
        proof.network_id,
        HeightContextId(HashOf::from_untyped_unchecked(proof.trusted_context_id)),
        proof.anchor_height,
        proof.target_epoch,
        proof.transition_id.into(),
    )
    .map_err(|_| Error::Crypto)?;
    Ok((evidence, selected))
}

fn validate_rotation_bundle(
    bundle: &RotationPublicBundle,
    proof: &RotationProofArgs,
    evidence: &ValidatorCommitteeSelectionEvidenceV1,
    selected: &VerifiedValidatorCommitteeSelectionV1,
) -> Result<Vec<PeerId>> {
    let preparation = selected.preparation();
    let target_roster = preparation
        .roster
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    let authorization_roster = selected
        .incumbent_authority()
        .validators
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    bundle.record.validate().map_err(|_| Error::Crypto)?;
    if bundle.schema != "iroha.validator-committee.rotation-dkg.v1"
        || bundle.preparation != *preparation
        || bundle.dkg_session.network_id != preparation.network_id
        || bundle.dkg_session.session_id
            != preparation
                .beacon_session_id()
                .map_err(|_| Error::InvalidInput)?
        || bundle.dkg_session.attempt_id
            != preparation
                .transition_id()
                .map_err(|_| Error::InvalidInput)?
        || bundle.dkg_session.authority_generation != preparation.authority_generation
        || bundle.dkg_session.roster_hash != global_threshold_beacon_roster_hash_v1(&target_roster)
        || usize::from(bundle.dkg_session.committee_size) != target_roster.len()
        || usize::from(bundle.dkg_session.threshold) != (target_roster.len() - 1) / 3 + 1
        || bundle.dkg_session.start_height != selected.observed_height()
        || bundle.dkg_session.commitments_end_height
            != bundle
                .dkg_session
                .start_height
                .checked_add(1)
                .ok_or(Error::Height)?
        || bundle.dkg_session.deliveries_end_height
            != bundle
                .dkg_session
                .start_height
                .checked_add(2)
                .ok_or(Error::Height)?
        || bundle.dkg_session.acceptances_end_height
            != bundle
                .dkg_session
                .start_height
                .checked_add(3)
                .ok_or(Error::Height)?
        || bundle.dkg_session.acceptances_end_height != bundle.finalized_observed_height
        || bundle.record.session.adaptive_dkg.session != bundle.dkg_session
        || bundle.record.session.network_id != preparation.network_id
        || bundle.record.session.session_id != bundle.dkg_session.session_id
        || bundle.record.session.roster_hash != bundle.dkg_session.roster_hash
        || bundle.record.session.adaptive_dkg.finalized_at_height
            != bundle.finalized_observed_height
        || bundle.record.activated_at_height.is_some()
        || bundle.record.retired_at_height.is_some()
        || selected.observed_height() >= bundle.finalized_observed_height
        || selected.observed_height() >= bundle.finalization_draft.effective_height
        || bundle.finalization_draft.effective_height >= preparation.first_height - 1
        || bundle.providers.len() != target_roster.len()
    {
        return Err(Error::InvalidInput);
    }
    validate_rotation_phase_chain(
        proof,
        evidence,
        selected.observed_height(),
        bundle.finalized_observed_height,
        preparation
            .first_height
            .checked_sub(1)
            .ok_or(Error::Height)?,
        &bundle.phase_proofs,
    )?;
    let mut handles = BTreeSet::new();
    let revision = bundle
        .providers
        .first()
        .ok_or(Error::InvalidInput)?
        .revision;
    for (offset, (provider, peer)) in bundle.providers.iter().zip(&target_roster).enumerate() {
        if provider.signer_index != u16::try_from(offset + 1).map_err(|_| Error::InvalidInput)?
            || provider.validator != *peer
            || provider.revision == 0
            || provider.revision != revision
            || !handles.insert(&provider.handle)
            || iroha_config::parameters::validate_production_runtime_handle(&provider.handle)
                .is_err()
            || provider.policy_digest
                != global_beacon_partial_signer_public_inventory_digest_v1(
                    bundle.record.session.network_id,
                    &[(bundle.record.session.clone(), provider.signer_index)],
                )
                .map_err(|_| Error::Crypto)?
        {
            return Err(Error::InvalidInput);
        }
    }
    let expected_certificate = draft_rotation_certificate(
        &authorization_roster,
        selected.incumbent_beacon().session_id,
        &bundle.record,
        bundle.finalized_observed_height,
        bundle.finalization_draft.effective_height,
    )?;
    if bundle.finalization_draft != expected_certificate {
        return Err(Error::InvalidInput);
    }
    Ok(authorization_roster)
}

fn load_lifecycle_key(file: File) -> Result<KeyPair> {
    use crate::taira_runtime_signer::{
        TairaRuntimeSignerErrorV1 as KeyError, load_private_record_from_file,
    };
    load_private_record_from_file(file, 71, |bytes| {
        let literal = std::str::from_utf8(bytes.strip_suffix(b"\n").ok_or(KeyError::InvalidKey)?)
            .map_err(|_| KeyError::InvalidKey)?;
        let exposed = literal
            .parse::<ExposedPrivateKey>()
            .map_err(|_| KeyError::InvalidKey)?;
        let canonical = Zeroizing::new(
            exposed
                .try_to_multihash_string()
                .map_err(|_| KeyError::InvalidKey)?,
        );
        if exposed.0.algorithm() != Algorithm::BlsNormal || canonical.as_str() != literal {
            return Err(KeyError::InvalidKey);
        }
        KeyPair::from_private_key(exposed.0).map_err(|_| KeyError::InvalidKey)
    })
    .map_err(|_| Error::InvalidCustody)
}
// A deliberately narrow native configuration view. Full Root parsing would
// reopen unrelated onboarding/faucet credentials and subsystem paths. Only this
// inline consensus identity participates in lifecycle signing; no environment,
// extends, file selector, or default can supply an authority here.
#[derive(iroha_config_base::ReadConfig)]
struct LifecycleConfigIdentity {
    chain: iroha_model_base::chain::ChainId,
    chain_discriminant: u16,
    public_key: PublicKey,
    private_key: iroha_crypto::PrivateKey,
    #[config(nested)]
    genesis: LifecycleConfigGenesis,
}
#[derive(iroha_config_base::ReadConfig)]
struct LifecycleConfigGenesis {
    expected_hash: iroha_data_model::NetworkId,
}
fn scrub_config_table(table: &mut toml::Table) {
    fn scrub(value: &mut toml::Value) {
        match value {
            toml::Value::String(value) => value.zeroize(),
            toml::Value::Array(values) => values.iter_mut().for_each(scrub),
            toml::Value::Table(table) => scrub_config_table(table),
            _ => {}
        }
    }
    table.iter_mut().for_each(|(_, value)| scrub(value));
}
fn lifecycle_key_from_config(
    bytes: &[u8],
    network: &iroha_data_model::NetworkId,
) -> Result<KeyPair> {
    use iroha_config_base::{read::ConfigReader, toml::TomlSource};
    let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidCustody)?;
    let table: toml::Table = toml::from_str(text).map_err(|_| Error::InvalidCustody)?;
    let origin = PathBuf::from("inherited-validator-config-fd198");
    let mut source = TomlSource::new_sensitive(origin.clone(), table, scrub_config_table);
    let root = source.table_mut();
    if root.contains_key("extends") || root.contains_key("private_key_file") {
        return Err(Error::InvalidCustody);
    }
    let genesis = root
        .get("genesis")
        .and_then(toml::Value::as_table)
        .ok_or(Error::InvalidCustody)?;
    if genesis.contains_key("expected_hash_file") {
        return Err(Error::InvalidCustody);
    }
    let expected = genesis
        .get("expected_hash")
        .ok_or(Error::InvalidCustody)?
        .clone();
    let mut projection = TomlSource::new_sensitive(origin, toml::Table::new(), scrub_config_table);
    for name in ["chain", "chain_discriminant", "public_key", "private_key"] {
        projection
            .table_mut()
            .insert(name.into(), root.remove(name).ok_or(Error::InvalidCustody)?);
    }
    let mut public_genesis = toml::Table::new();
    public_genesis.insert("expected_hash".into(), expected);
    projection
        .table_mut()
        .insert("genesis".into(), toml::Value::Table(public_genesis));
    let identity = ConfigReader::new()
        .without_env()
        .with_toml_source(projection)
        .read_and_complete::<LifecycleConfigIdentity>()
        .map_err(|_| Error::InvalidCustody)?;
    if identity.chain.to_string() != "fc56984b-2be7-431d-840e-21514d1883f0"
        || identity.chain_discriminant != 369
        || &identity.genesis.expected_hash != network
        || identity.public_key.try_algorithm() != Ok(Algorithm::BlsNormal)
        || identity.private_key.algorithm() != Algorithm::BlsNormal
    {
        return Err(Error::InvalidCustody);
    }
    KeyPair::new(identity.public_key, identity.private_key).map_err(|_| Error::InvalidCustody)
}
fn load_lifecycle_config(file: File, network: &iroha_data_model::NetworkId) -> Result<KeyPair> {
    use crate::taira_runtime_signer::{
        TairaRuntimeSignerErrorV1 as KeyError, load_private_record_from_file,
    };
    let length = file.metadata().map_err(|_| Error::InvalidCustody)?.len();
    if length == 0 || length > iroha_config_base::toml::MAX_TOML_SOURCE_BYTES {
        return Err(Error::InvalidCustody);
    }
    load_private_record_from_file(file, length, |bytes| {
        lifecycle_key_from_config(bytes, network).map_err(|_| KeyError::InvalidKey)
    })
    .map_err(|_| Error::InvalidCustody)
}
fn sign_rotation_command(
    proof: &RotationProofArgs,
    bundle_path: &Path,
    signer_index: u16,
    key_fd: Option<i32>,
    config_fd: Option<i32>,
    output: &Path,
) -> Result<()> {
    let (fd, config) = match (key_fd, config_fd) {
        (Some(198), None) => (198, false),
        (None, Some(198)) => (198, true),
        _ => return Err(Error::InvalidInput),
    };
    iroha_genesis::init_instruction_registry();
    let (evidence, selected) = read_verified_rotation_selection(proof)?;
    let bundle: RotationPublicBundle = read_json(bundle_path)?;
    let roster = validate_rotation_bundle(&bundle, proof, &evidence, &selected)?;
    let file = crate::taira_runtime_signer::take_inherited_private_file(fd)
        .map_err(|_| Error::InvalidCustody)?;
    let key = if config {
        load_lifecycle_config(file, &proof.network_id)?
    } else {
        load_lifecycle_key(file)?
    };
    let signed = sign_rotation_draft(&bundle.finalization_draft, &roster, signer_index, &key)?;
    write_new(output, &json_bytes(&signed)?, false)
}

fn sign_rotation_draft(
    draft: &ThresholdKeyLifecycleCertificateV1,
    roster: &[PeerId],
    signer_index: u16,
    key: &KeyPair,
) -> Result<ThresholdKeyLifecycleSignatureV1> {
    if !draft.signatures.is_empty() {
        return Err(Error::InvalidInput);
    }
    let peer = roster
        .get(usize::from(signer_index))
        .ok_or(Error::InvalidInput)?;
    if peer.public_key() != key.public_key() {
        return Err(Error::InvalidCustody);
    }
    let preimage =
        threshold_key_lifecycle_certificate_preimage_v1(draft).map_err(|_| Error::Crypto)?;
    let signature = Signature::try_new(key.private_key(), &preimage).map_err(|_| Error::Crypto)?;
    signature
        .verify(peer.public_key(), &preimage)
        .map_err(|_| Error::Crypto)?;
    Ok(ThresholdKeyLifecycleSignatureV1 {
        signer_index,
        signature,
    })
}

fn assemble_rotation_draft(
    draft: &ThresholdKeyLifecycleCertificateV1,
    roster: &[PeerId],
    signatures: Vec<ThresholdKeyLifecycleSignatureV1>,
) -> Result<ThresholdKeyLifecycleCertificateV1> {
    if !draft.signatures.is_empty() {
        return Err(Error::InvalidInput);
    }
    let mut certificate = draft.clone();
    certificate.signatures = signatures;
    verify_threshold_key_lifecycle_certificate_v1(
        &certificate,
        &certificate.network_id,
        certificate.effective_height,
        roster,
    )
    .map_err(|_| Error::Crypto)?;
    Ok(certificate)
}

fn assemble_rotation_command(
    proof: &RotationProofArgs,
    bundle_path: &Path,
    signatures: &[PathBuf],
    output: &Path,
) -> Result<()> {
    iroha_genesis::init_instruction_registry();
    let (evidence, selected) = read_verified_rotation_selection(proof)?;
    let bundle: RotationPublicBundle = read_json(bundle_path)?;
    let roster = validate_rotation_bundle(&bundle, proof, &evidence, &selected)?;
    let signatures = signatures
        .iter()
        .map(|path| read_json(path))
        .collect::<Result<Vec<_>>>()?;
    let certificate = assemble_rotation_draft(&bundle.finalization_draft, &roster, signatures)?;
    let instructions = vec![InstructionBox::from(
        ApplyThresholdKeyLifecycleCertificateV1 { certificate },
    )];
    write_new(output, &json_bytes(&instructions)?, false)
}

#[cfg(test)]
#[path = "beacon_bootstrap_tests.rs"]
mod tests;
