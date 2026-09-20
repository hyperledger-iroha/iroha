//! Retained original-facts owner for the two canonical preparation transports.
//!
//! Both staging files finish before either destination is published. Publication
//! is two NOREPLACE operations, not one atomic pair; failure preserves every
//! surviving artifact and yields no successful owner. This capability never
//! claims complete canonical execution proof authority.

use super::*;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PrepareRole {
    Facts,
    Request,
    Bundle,
    Pair,
}

/// Two absent canonical transport destinations and their retained namespaces.
pub(crate) struct PreparedOutputPair {
    request: TransportOutput,
    bundle: TransportOutput,
    caps: PrepareOutputCaps,
}
struct TransportOutput {
    path: PathBuf,
    stage_path: PathBuf,
    parent: Parent,
    stage: OsString,
    destination: OsString,
    maximum: u64,
}
impl TransportOutput {
    fn admit(
        path: &Path,
        maximum: u64,
        hook: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<Self> {
        ensure!(
            path.as_os_str()
                .as_bytes()
                .len()
                .checked_add(".publishing".len())
                .is_some_and(|n| n <= MAX_PATH_BYTES),
            "prepare stage path exceeds bound"
        );
        let (parent, destination) = Parent::capture(path, hook)?;
        let mut stage = destination.clone();
        stage.push(".publishing");
        let output = Self {
            path: path.to_owned(),
            stage_path: path.with_file_name(&stage),
            parent,
            stage,
            destination,
            maximum,
        };
        output.check_absent()?;
        Ok(output)
    }
    fn check_absent(&self) -> Result<()> {
        self.parent.require_absent(&self.destination)?;
        self.parent.require_absent(&self.stage)
    }
    fn names(&self) -> [&std::ffi::OsStr; 2] {
        [&self.destination, &self.stage]
    }
    fn paths(&self) -> [&Path; 2] {
        [&self.path, &self.stage_path]
    }
}
impl PreparedOutputPair {
    /// Reserve two distinct absent destinations and explicit aggregate byte caps.
    pub(crate) fn admit(request: &Path, bundle: &Path, caps: PrepareOutputCaps) -> Result<Self> {
        Self::admit_with_hook(request, bundle, caps, |_, _| Ok(()))
    }
    pub(super) fn admit_with_hook(
        request: &Path,
        bundle: &Path,
        caps: PrepareOutputCaps,
        mut hook: impl FnMut(PrepareRole, Phase) -> Result<()>,
    ) -> Result<Self> {
        ensure!(
            [caps.request_bytes, caps.bundle_bytes, caps.total_bytes]
                .iter()
                .all(|n| (1..=MAX_INPUT_BYTES).contains(n))
                && caps
                    .request_bytes
                    .checked_add(caps.bundle_bytes)
                    .is_some_and(|n| n < caps.total_bytes),
            "invalid prepare output reservations"
        );
        let request = TransportOutput::admit(request, caps.request_bytes, &mut |p| {
            hook(PrepareRole::Request, p)
        })?;
        let bundle = TransportOutput::admit(bundle, caps.bundle_bytes, &mut |p| {
            hook(PrepareRole::Bundle, p)?;
            request.check_absent()
        })?;
        let pair = Self {
            request,
            bundle,
            caps,
        };
        pair.check_absent()?;
        let request_parent = held_identity(pair.request.parent.file())?;
        let bundle_parent = held_identity(pair.bundle.parent.file())?;
        for (path, name) in pair.request.paths().into_iter().zip(pair.request.names()) {
            for (other_path, other_name) in pair.bundle.paths().into_iter().zip(pair.bundle.names())
            {
                ensure!(
                    path != other_path
                        && ((request_parent.dev, request_parent.ino)
                            != (bundle_parent.dev, bundle_parent.ino)
                            || name != other_name),
                    "prepare output and stage roles share a path or namespace"
                );
            }
        }
        Ok(pair)
    }
    fn check_absent(&self) -> Result<()> {
        self.request.check_absent()?;
        self.bundle.check_absent()
    }
    fn admit_facts(&self, facts: &ProofInputBinding) -> Result<()> {
        ensure!(
            (1..=MAX_INPUT_BYTES).contains(&facts.max_bytes)
                && facts
                    .max_bytes
                    .checked_add(self.caps.request_bytes)
                    .and_then(|n| n.checked_add(self.caps.bundle_bytes))
                    .is_some_and(|n| n <= self.caps.total_bytes),
            "prepare facts and output reservations exceed total cap"
        );
        ensure!(
            self.request
                .paths()
                .into_iter()
                .chain(self.bundle.paths())
                .all(|p| p != facts.path),
            "prepare facts share an output or stage path"
        );
        self.check_absent()
    }
}

struct RetainedTransport {
    output: TransportOutput,
    file: File,
    state: FileState,
    digest: [u8; 32],
    published: bool,
}
enum OtherTransport<'a> {
    Absent(&'a TransportOutput),
    Retained(&'a RetainedTransport),
}
impl OtherTransport<'_> {
    fn check(&self) -> Result<()> {
        match self {
            Self::Absent(output) => output.check_absent(),
            Self::Retained(output) => output.check(),
        }
    }
    fn reject_inode(&self, identity: Identity) -> Result<()> {
        if let Self::Retained(other) = self {
            ensure!(
                (identity.dev, identity.ino)
                    != (other.state.identity.dev, other.state.identity.ino),
                "prepare outputs share an inode"
            );
        }
        Ok(())
    }
}
fn check_inputs_and_other(
    lease: &InputPublicationLease,
    other: &OtherTransport<'_>,
    parent: &Parent,
) -> Result<()> {
    lease.check()?;
    other.check()?;
    parent.check()?;
    lease.check()
}
impl RetainedTransport {
    fn name(&self) -> &std::ffi::OsStr {
        if self.published {
            &self.output.destination
        } else {
            &self.output.stage
        }
    }
    fn check_namespace(&self) -> Result<()> {
        ensure!(
            self.output.parent.check_file(
                &self.file,
                self.name(),
                self.state.identity,
                self.state.size
            )? == self.state,
            "retained prepare output changed"
        );
        if self.published {
            self.output.parent.require_absent(&self.output.stage)?;
        } else {
            self.output
                .parent
                .require_absent(&self.output.destination)?;
        }
        Ok(())
    }
    fn check(&self) -> Result<()> {
        self.check_namespace()?;
        let (digest, size) = iroha_crypto::sha256_reader_bounded(
            ContentReader {
                file: &self.file,
                offset: 0,
            }
            .take(self.state.size + 1),
            self.state.size,
        )?;
        ensure!(
            size == self.state.size && digest == self.digest,
            "prepare output readback mismatch"
        );
        self.check_namespace()
    }
    fn stage(
        output: TransportOutput,
        bytes: &[u8],
        lease: &InputPublicationLease,
        other: OtherTransport<'_>,
        role: PrepareRole,
        hook: &mut impl FnMut(PrepareRole, Phase) -> Result<()>,
    ) -> Result<Self> {
        let length = u64::try_from(bytes.len())?;
        ensure!(
            length > 0 && length <= output.maximum,
            "prepare output exceeds cap"
        );
        output.check_absent()?;
        check_inputs_and_other(lease, &other, &output.parent)?;
        let mut retained_hook = |phase| {
            hook(role, phase)?;
            check_inputs_and_other(lease, &other, &output.parent)
        };
        let (mut file, identity) = output.parent.create(&output.stage, &mut retained_hook)?;
        other.reject_inode(identity)?;
        ensure!(
            lease.files.iter().all(|f| {
                (f.state.identity.dev, f.state.identity.ino) != (identity.dev, identity.ino)
            }),
            "prepare output aliases original facts inode"
        );
        retained_hook(Phase::BeforeWrite)?;
        output
            .parent
            .check_file(&file, &output.stage, identity, 0)?;
        file.write_all(bytes)?;
        let state = output
            .parent
            .check_file(&file, &output.stage, identity, length)?;
        ensure!(state.size == length, "prepare output write incomplete");
        retained_hook(Phase::AfterWrite)?;
        ensure!(
            output
                .parent
                .check_file(&file, &output.stage, identity, length)?
                == state,
            "prepare output changed after write"
        );
        retained_hook(Phase::BeforeFileSync)?;
        ensure!(
            output
                .parent
                .check_file(&file, &output.stage, identity, length)?
                == state,
            "prepare output changed before sync"
        );
        file.sync_all()?;
        retained_hook(Phase::AfterFileSync)?;
        retained_hook(Phase::BeforeReadback)?;
        drop(retained_hook);
        let staged = Self {
            output,
            file,
            state,
            digest: iroha_crypto::sha256(bytes),
            published: false,
        };
        staged.check()?;
        hook(role, Phase::AfterReadback)?;
        check_inputs_and_other(lease, &other, &staged.output.parent)?;
        staged.check()?;
        Ok(staged)
    }
    fn publish(
        &mut self,
        lease: &InputPublicationLease,
        other: &RetainedTransport,
        role: PrepareRole,
        hook: &mut impl FnMut(PrepareRole, Phase) -> Result<()>,
    ) -> Result<()> {
        ensure!(!self.published, "prepare transport already published");
        let other = OtherTransport::Retained(other);
        self.check()?;
        check_inputs_and_other(lease, &other, &self.output.parent)?;
        for phase in [Phase::BeforeRename, Phase::RenameReady] {
            hook(role, phase)?;
            check_inputs_and_other(lease, &other, &self.output.parent)?;
            self.check()?;
        }
        let before = self.state;
        rustix::fs::renameat_with(
            self.output.parent.file(),
            &self.output.stage,
            self.output.parent.file(),
            &self.output.destination,
            RenameFlags::NOREPLACE,
        )?;
        self.published = true;
        self.state = self.output.parent.check_file(
            &self.file,
            &self.output.destination,
            before.identity,
            before.size,
        )?;
        ensure!(
            self.state.size == before.size && self.state.modified == before.modified,
            "prepare output bytes changed during publication"
        );
        self.check()?;
        hook(role, Phase::AfterRename)?;
        check_inputs_and_other(lease, &other, &self.output.parent)?;
        self.check()?;
        self.output.parent.sync(&mut |phase| {
            hook(role, phase)?;
            check_inputs_and_other(lease, &other, &self.output.parent)?;
            self.check()
        })?;
        self.check()?;
        check_inputs_and_other(lease, &other, &self.output.parent)
    }
    fn identity(&self) -> PreparedTransportIdentity {
        PreparedTransportIdentity {
            raw_sha256: self.digest,
            byte_length: self.state.size,
        }
    }
}

/// Both published transports, retained original facts, and both output descriptors.
///
/// Keep this owner alive through reply writing, writer flush and the final
/// identity/census check. A failed or panicking check permanently poisons it.
/// These files carry admitted launch facts; they are not a completed proof.
pub(crate) struct PreparedLaunch {
    facts: InputPublicationLease,
    request: RetainedTransport,
    bundle: RetainedTransport,
    poisoned: Cell<bool>,
}
impl PreparedLaunch {
    fn check(&self) -> Result<()> {
        self.facts.check()?;
        self.request.check()?;
        self.bundle.check()?;
        self.facts.check()?;
        self.request.check_namespace()?;
        self.bundle.check_namespace()
    }
    /// Recheck original facts and both exact outputs before returning raw identities.
    pub(crate) fn identity(&self) -> Result<PreparedLaunchIdentity> {
        self.identity_with_hook(|_, _| Ok(()))
    }
    pub(super) fn identity_with_hook(
        &self,
        mut hook: impl FnMut(PrepareRole, Phase) -> Result<()>,
    ) -> Result<PreparedLaunchIdentity> {
        ensure!(!self.poisoned.replace(true), "prepared launch is poisoned");
        self.check()?;
        hook(PrepareRole::Pair, Phase::BeforeIdentity)?;
        self.check()?;
        let facts = self
            .facts
            .files
            .first()
            .ok_or_else(|| eyre!("missing retained preparation facts"))?;
        let identity = PreparedLaunchIdentity {
            facts: PreparedTransportIdentity {
                raw_sha256: facts.digest,
                byte_length: facts.state.size,
            },
            request: self.request.identity(),
            bundle: self.bundle.identity(),
        };
        hook(PrepareRole::Pair, Phase::AfterIdentity)?;
        self.check()?;
        self.poisoned.set(false);
        Ok(identity)
    }
}

/// Admit one original facts file, prepare both transports, and retain the published pair.
pub(crate) fn prepare_bound(
    facts: ProofInputBinding,
    outputs: PreparedOutputPair,
) -> Result<PreparedLaunch> {
    prepare_with_hook(facts, outputs, |_, _| Ok(()))
}
pub(super) fn prepare_with_hook(
    facts: ProofInputBinding,
    outputs: PreparedOutputPair,
    mut hook: impl FnMut(PrepareRole, Phase) -> Result<()>,
) -> Result<PreparedLaunch> {
    outputs.admit_facts(&facts)?;
    let expected = facts.sha256;
    let maximum = facts.max_bytes;
    let mut retained_hook = |phase| {
        hook(PrepareRole::Facts, phase)?;
        outputs.check_absent()
    };
    let mut inputs = Inputs::open(vec![facts], outputs.caps.total_bytes, &mut retained_hook)?;
    let bytes = inputs
        .read_all(&mut retained_hook)?
        .pop()
        .ok_or_else(|| eyre!("missing retained preparation facts"))?;
    drop(retained_hook);
    hook(PrepareRole::Pair, Phase::BeforeVerification)?;
    for file in &inputs.files {
        file.recheck_content()?;
    }
    outputs.check_absent()?;
    let transports = crate::kura::scaling_evidence::export::launcher::prepare::prepare(
        &bytes,
        expected,
        maximum,
        outputs.caps,
    )?;
    drop(bytes);
    hook(PrepareRole::Pair, Phase::AfterVerification)?;
    for file in &inputs.files {
        file.recheck_content()?;
    }
    outputs.check_absent()?;
    let facts = inputs.finish_lease(&mut |phase| {
        hook(PrepareRole::Facts, phase)?;
        outputs.check_absent()
    })?;
    let (request_bytes, bundle_bytes) = transports.into_buffers();
    let PreparedOutputPair {
        request, bundle, ..
    } = outputs;
    let mut request = RetainedTransport::stage(
        request,
        &request_bytes,
        &facts,
        OtherTransport::Absent(&bundle),
        PrepareRole::Request,
        &mut hook,
    )?;
    drop(request_bytes);
    let mut bundle = RetainedTransport::stage(
        bundle,
        &bundle_bytes,
        &facts,
        OtherTransport::Retained(&request),
        PrepareRole::Bundle,
        &mut hook,
    )?;
    drop(bundle_bytes);
    request.publish(&facts, &bundle, PrepareRole::Request, &mut hook)?;
    bundle.publish(&facts, &request, PrepareRole::Bundle, &mut hook)?;
    let launch = PreparedLaunch {
        facts,
        request,
        bundle,
        poisoned: Cell::new(false),
    };
    launch.identity_with_hook(&mut hook)?;
    Ok(launch)
}
