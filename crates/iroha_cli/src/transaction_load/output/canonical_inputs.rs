// Retained original context and two typed transport outputs. This owner grants no finality proof.
use super::*;
use iroha_core::kura::CanonicalKuraEvidenceComplete;
use iroha_data_model::{bridge::BridgeFinalityProof, query::CommittedTransaction};
use sha2::{Digest as _, Sha256};
use std::{cell::Cell, io::Write as _, os::unix::fs::FileExt as _};

const MAX_CONTEXT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TRANSPORT_BYTES: u64 = 256 * 1024 * 1024;
const READ_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);

/// Independent raw digest and bounded admission for one original private file.
pub(crate) struct OriginalInputBinding {
    pub(crate) path: PathBuf,
    pub(crate) raw_sha256: [u8; 32],
    pub(crate) max_bytes: u64,
}
/// A raw file identity, distinct from Iroha's typed hash and from proof authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RawFileIdentity {
    pub(crate) raw_sha256: [u8; 32],
    pub(crate) byte_length: u64,
}
/// Independently reserved transport output allocations, including the context reservation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CanonicalInputCaps {
    pub(crate) finality_bytes: u64,
    pub(crate) query_bytes: u64,
    pub(crate) total_bytes: u64,
}
/// Raw identities rechecked while all original input and published output handles remain held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalInputsIdentity {
    pub(crate) context: RawFileIdentity,
    pub(crate) finality: RawFileIdentity,
    pub(crate) queries: RawFileIdentity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    ContextParent(Phase),
    ContextAdmitted,
    ContextOpened,
    BeforeContextRead,
    AfterContextRead,
    BeforeContextCheck,
    AfterContextCheck,
    OutputParent(usize, Phase),
    PairAdmitted,
    BeforeCount,
    AfterCount,
    AfterEncode,
    Output(usize, Phase),
    BeforeWrite(usize),
    AfterWrite(usize),
    BeforeReadback(usize),
    Readback(usize),
    BothStaged,
    BeforeIdentity,
    AfterIdentity,
}

fn read_at(file: &File, mut bytes: &mut [u8], mut offset: u64) -> Result<()> {
    while !bytes.is_empty() {
        let n = file.read_at(bytes, offset)?;
        ensure!(n > 0, "retained file became short");
        offset = offset
            .checked_add(u64::try_from(n)?)
            .ok_or_else(|| eyre!("read offset overflow"))?;
        bytes = &mut bytes[n..];
    }
    Ok(())
}
pub(super) fn raw_digest(file: &File, length: u64) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    while offset < length {
        let count = usize::try_from((length - offset).min(buffer.len() as u64))?;
        read_at(file, &mut buffer[..count], offset)?;
        hasher.update(&buffer[..count]);
        offset += count as u64;
    }
    ensure!(
        file.read_at(&mut buffer[..1], length)? == 0,
        "retained file gained trailing bytes"
    );
    Ok(hasher.finalize().into())
}

/// Owns exact original private bytes, their read-only descriptor and ancestor chain.
/// Any failed operation or caught callback panic permanently poisons this owner.
pub(crate) struct RetainedOriginalInput {
    parent: Parent,
    name: OsString,
    file: File,
    state: FileState,
    bytes: zeroize::Zeroizing<Vec<u8>>,
    identity: RawFileIdentity,
    maximum: u64,
    poisoned: Cell<bool>,
}
impl RetainedOriginalInput {
    pub(crate) fn open(binding: OriginalInputBinding) -> Result<Self> {
        Self::open_with_hook(binding, |_| Ok(()))
    }
    fn open_with_hook(
        binding: OriginalInputBinding,
        mut hook: impl FnMut(Event) -> Result<()>,
    ) -> Result<Self> {
        ensure!(
            (1..=MAX_CONTEXT_BYTES).contains(&binding.max_bytes),
            "invalid context allocation"
        );
        let (parent, name) =
            Parent::capture(&binding.path, &mut |p| hook(Event::ContextParent(p)))?;
        let state = named(parent.file(), &name)?;
        ensure!(
            state.owned_regular(parent.uid) && state.size > 0 && state.size <= binding.max_bytes,
            "context must be exact private singly linked bounded regular file"
        );
        hook(Event::ContextAdmitted)?;
        parent.check()?;
        ensure!(
            named(parent.file(), &name)? == state,
            "context changed before open"
        );
        let file = File::from(rustix::fs::openat(
            parent.file(),
            &name,
            READ_FLAGS,
            Mode::empty(),
        )?);
        hook(Event::ContextOpened)?;
        ensure!(
            parent.check_file(&file, &name, state.identity, binding.max_bytes)? == state,
            "context changed during open"
        );
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        bytes.try_reserve_exact(usize::try_from(state.size)?)?;
        bytes.resize(usize::try_from(state.size)?, 0);
        hook(Event::BeforeContextRead)?;
        ensure!(
            parent.check_file(&file, &name, state.identity, binding.max_bytes)? == state,
            "context changed before read"
        );
        read_at(&file, &mut bytes, 0)?;
        hook(Event::AfterContextRead)?;
        let owner = Self {
            parent,
            name,
            file,
            state,
            bytes,
            identity: RawFileIdentity {
                raw_sha256: binding.raw_sha256,
                byte_length: state.size,
            },
            maximum: binding.max_bytes,
            poisoned: Cell::new(true),
        };
        ensure!(
            iroha_crypto::sha256(&owner.bytes) == owner.identity.raw_sha256,
            "context raw digest mismatch"
        );
        owner.check_inner()?;
        owner.poisoned.set(false);
        Ok(owner)
    }
    /// Bind the inherited client descriptor to this exact retained original inode and content.
    pub(crate) fn require_descriptor(&self, fd: u32) -> Result<()> {
        self.with_bytes(|bytes| {
            let original = crate::client_config::duplicate_inherited_descriptor(fd)?;
            ensure!(
                rustix::fs::fcntl_getfl(&original)? & OFlags::ACCMODE == OFlags::RDONLY
                    && held(&original)? == self.state,
                "inherited client descriptor differs from retained original"
            );
            let inherited = crate::client_config::read_inherited_private_file(
                fd,
                self.maximum,
                "original scaling client config",
            )?;
            let after = crate::client_config::duplicate_inherited_descriptor(fd)?;
            ensure!(
                inherited.as_slice() == bytes
                    && held(&original)? == self.state
                    && held(&after)? == self.state
                    && rustix::fs::fcntl_getfl(&after)? & OFlags::ACCMODE == OFlags::RDONLY,
                "inherited client bytes differ from retained original"
            );
            Ok(())
        })
    }
    fn check_inner(&self) -> Result<()> {
        ensure!(
            self.parent
                .check_file(&self.file, &self.name, self.state.identity, self.maximum)?
                == self.state,
            "retained context full file state changed"
        );
        ensure!(
            raw_digest(&self.file, self.state.size)? == self.identity.raw_sha256,
            "retained context raw digest changed"
        );
        ensure!(
            self.parent
                .check_file(&self.file, &self.name, self.state.identity, self.maximum)?
                == self.state,
            "retained context changed during digest read"
        );
        Ok(())
    }
    fn ensure_live(&self) -> Result<()> {
        ensure!(!self.poisoned.get(), "context owner is poisoned");
        self.check_inner()
    }
    /// Read bounded immutable bytes while the original source is rechecked on both sides.
    pub(crate) fn with_bytes<T>(&self, consume: impl FnOnce(&[u8]) -> Result<T>) -> Result<T> {
        self.with_bytes_hook(consume, |_| Ok(()))
    }
    fn with_bytes_hook<T>(
        &self,
        consume: impl FnOnce(&[u8]) -> Result<T>,
        mut hook: impl FnMut(Event) -> Result<()>,
    ) -> Result<T> {
        ensure!(!self.poisoned.replace(true), "context owner is poisoned");
        hook(Event::BeforeContextCheck)?;
        self.check_inner()?;
        let value = consume(&self.bytes)?;
        hook(Event::AfterContextCheck)?;
        self.check_inner()?;
        self.poisoned.set(false);
        Ok(value)
    }
    pub(crate) fn identity(&self) -> Result<RawFileIdentity> {
        self.with_bytes(|_| Ok(self.identity))
    }
}

struct Destination {
    parent: Parent,
    name: OsString,
    stage: OsString,
    maximum: u64,
}
impl Destination {
    fn admit(
        path: &Path,
        maximum: u64,
        index: usize,
        hook: &mut impl FnMut(Event) -> Result<()>,
    ) -> Result<Self> {
        let (parent, name) = Parent::capture(path, &mut |p| hook(Event::OutputParent(index, p)))?;
        let leaf = name
            .to_str()
            .ok_or_else(|| eyre!("canonical transport name must be UTF-8"))?;
        ensure!(
            path.as_os_str()
                .as_bytes()
                .len()
                .checked_add(".collecting".len())
                .is_some_and(|n| n <= MAX_PATH_BYTES),
            "canonical stage path exceeds bound"
        );
        let stage = OsString::from(format!("{leaf}.collecting"));
        parent.require_absent(&name)?;
        parent.require_absent(&stage)?;
        Ok(Self {
            parent,
            name,
            stage,
            maximum,
        })
    }
    fn location(&self) -> Identity {
        self.parent
            .directories
            .last()
            .expect("retained root")
            .identity
    }
    fn aliases(&self, other: &Self) -> bool {
        self.location().dev == other.location().dev
            && self.location().ino == other.location().ino
            && [self.name.as_os_str(), self.stage.as_os_str()]
                .iter()
                .any(|a| [other.name.as_os_str(), other.stage.as_os_str()].contains(a))
    }
    fn check_context(&self, context: &RetainedOriginalInput) -> Result<()> {
        let held = context
            .parent
            .directories
            .last()
            .expect("retained context root")
            .identity;
        ensure!(
            self.location().dev != held.dev
                || self.location().ino != held.ino
                || (self.name != context.name && self.stage != context.name),
            "context and canonical output alias"
        );
        Ok(())
    }
    fn check_ancestry(&self, complete: &CanonicalKuraEvidenceComplete) -> Result<()> {
        self.parent.check()?;
        let ancestry: Vec<_> = self
            .parent
            .directories
            .iter()
            .map(|d| (d.identity.dev, d.identity.ino))
            .collect();
        complete.ensure_publication_ancestry(&ancestry)?;
        self.parent.check()
    }
}
/// Owns both absent destinations before typed encoding or stage creation.
pub(crate) struct CanonicalInputPair {
    finality: Destination,
    queries: Destination,
    caps: CanonicalInputCaps,
}
impl CanonicalInputPair {
    pub(crate) fn admit(finality: &Path, queries: &Path, caps: CanonicalInputCaps) -> Result<Self> {
        Self::admit_with_hook(finality, queries, caps, |_| Ok(()))
    }
    fn admit_with_hook(
        finality: &Path,
        queries: &Path,
        caps: CanonicalInputCaps,
        mut hook: impl FnMut(Event) -> Result<()>,
    ) -> Result<Self> {
        ensure!(
            (1..=MAX_TRANSPORT_BYTES).contains(&caps.finality_bytes)
                && (1..=MAX_TRANSPORT_BYTES).contains(&caps.query_bytes)
                && (1..=MAX_TRANSPORT_BYTES).contains(&caps.total_bytes)
                && caps
                    .finality_bytes
                    .checked_add(caps.query_bytes)
                    .is_some_and(|n| n <= caps.total_bytes),
            "invalid canonical pair allocations"
        );
        let finality = Destination::admit(finality, caps.finality_bytes, 0, &mut hook)?;
        let queries = Destination::admit(queries, caps.query_bytes, 1, &mut hook)?;
        ensure!(!finality.aliases(&queries), "canonical output names alias");
        hook(Event::PairAdmitted)?;
        for target in [&finality, &queries] {
            target.parent.require_absent(&target.name)?;
            target.parent.require_absent(&target.stage)?;
        }
        Ok(Self {
            finality,
            queries,
            caps,
        })
    }
    /// Publish only the two concrete canonical transport vectors, retaining every original owner.
    /// This operation creates no canonical finality/useful-effect proof authority.
    pub(crate) fn publish(
        self,
        context: RetainedOriginalInput,
        complete: CanonicalKuraEvidenceComplete,
        finality: &Vec<BridgeFinalityProof>,
        queries: &Vec<CommittedTransaction>,
        verify: &impl Fn() -> Result<()>,
    ) -> Result<PublishedCanonicalInputs> {
        self.publish_with_hook(context, complete, finality, queries, |_| verify())
    }
    fn publish_with_hook(
        self,
        context: RetainedOriginalInput,
        complete: CanonicalKuraEvidenceComplete,
        finality: &Vec<BridgeFinalityProof>,
        queries: &Vec<CommittedTransaction>,
        mut hook: impl FnMut(Event) -> Result<()>,
    ) -> Result<PublishedCanonicalInputs> {
        ensure!(
            context
                .maximum
                .checked_add(self.caps.finality_bytes)
                .and_then(|n| n.checked_add(self.caps.query_bytes))
                .is_some_and(|n| n <= self.caps.total_bytes),
            "context and output reservations exceed total allocation"
        );
        let check_inputs = || -> Result<()> {
            context.ensure_live()?;
            complete.recheck_sources()?;
            Ok(())
        };
        check_inputs()?;
        for target in [&self.finality, &self.queries] {
            target.check_context(&context)?;
            target.check_ancestry(&complete)?;
            target.parent.require_absent(&target.name)?;
            target.parent.require_absent(&target.stage)?;
        }
        hook(Event::BeforeCount)?;
        check_inputs()?;
        let finality_size = u64::try_from(norito::canonical_frame_len(finality)?)?;
        let query_size = u64::try_from(norito::canonical_frame_len(queries)?)?;
        ensure!(
            finality_size > 0
                && finality_size <= self.caps.finality_bytes
                && query_size > 0
                && query_size <= self.caps.query_bytes,
            "canonical vector exceeds reserved output allocation"
        );
        hook(Event::AfterCount)?;
        check_inputs()?;
        let (finality_bytes, query_bytes) = {
            // Match encode_canonical's exact layout while enforcing an actual bounded,
            // non-growing destination allocation for both concrete transport roots.
            let _canonical_flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            (
                norito::core::to_bytes_bounded(
                    finality,
                    usize::try_from(self.caps.finality_bytes)?,
                )?,
                norito::core::to_bytes_bounded(queries, usize::try_from(self.caps.query_bytes)?)?,
            )
        };
        ensure!(
            finality_bytes.len() as u64 == finality_size && query_bytes.len() as u64 == query_size,
            "canonical vector frame count changed"
        );
        hook(Event::AfterEncode)?;
        check_inputs()?;
        // Recheck both namespaces together before either first-stage create.
        for target in [&self.finality, &self.queries] {
            target.check_context(&context)?;
            target.check_ancestry(&complete)?;
            target.parent.require_absent(&target.name)?;
            target.parent.require_absent(&target.stage)?;
        }
        let mut guarded = |event| {
            check_inputs()?;
            hook(event)?;
            check_inputs()
        };
        let mut first = Stage::create(self.finality, &finality_bytes, 0, &mut guarded)?;
        let mut second = Stage::create(self.queries, &query_bytes, 1, &mut |event| {
            first.check()?;
            guarded(event)?;
            first.check()
        })?;
        first.check()?;
        second.check()?;
        guarded(Event::BothStaged)?;
        first.check()?;
        second.check()?;
        first.publish(0, &mut |event| {
            second.check()?;
            guarded(event)?;
            second.check()
        })?;
        first.check()?;
        second.check()?;
        second.publish(1, &mut |event| {
            first.check()?;
            guarded(event)?;
            first.check()
        })?;
        first.check()?;
        second.check()?;
        check_inputs()?;
        let published = PublishedCanonicalInputs {
            context,
            complete,
            finality: first,
            queries: second,
            poisoned: Cell::new(false),
        };
        published.identity_with_hook(hook)?;
        Ok(published)
    }
}
struct Stage {
    destination: Destination,
    file: File,
    state: FileState,
    identity: RawFileIdentity,
    published: bool,
}
impl Stage {
    fn create(
        destination: Destination,
        bytes: &[u8],
        index: usize,
        hook: &mut impl FnMut(Event) -> Result<()>,
    ) -> Result<Self> {
        let (mut file, identity) =
            destination
                .parent
                .create_with_access(&destination.stage, true, &mut |p| {
                    hook(Event::Output(index, p))
                })?;
        let empty = destination
            .parent
            .check_file(&file, &destination.stage, identity, 0)?;
        hook(Event::BeforeWrite(index))?;
        ensure!(
            destination
                .parent
                .check_file(&file, &destination.stage, identity, 0)?
                == empty,
            "canonical stage changed before write"
        );
        file.write_all(bytes)?;
        let state = destination.parent.check_file(
            &file,
            &destination.stage,
            identity,
            destination.maximum,
        )?;
        ensure!(
            state.size == bytes.len() as u64,
            "canonical stage byte count changed"
        );
        hook(Event::AfterWrite(index))?;
        let stage = Self {
            destination,
            file,
            state,
            identity: RawFileIdentity {
                raw_sha256: iroha_crypto::sha256(bytes),
                byte_length: bytes.len() as u64,
            },
            published: false,
        };
        stage.check()?;
        hook(Event::Output(index, Phase::BeforeFileSync))?;
        stage.check()?;
        stage.file.sync_all()?;
        hook(Event::Output(index, Phase::AfterFileSync))?;
        stage.check()?;
        hook(Event::BeforeReadback(index))?;
        stage.check()?;
        hook(Event::Readback(index))?;
        stage.check()?;
        Ok(stage)
    }
    fn name(&self) -> &std::ffi::OsStr {
        if self.published {
            &self.destination.name
        } else {
            &self.destination.stage
        }
    }
    fn check(&self) -> Result<()> {
        ensure!(
            self.destination.parent.check_file(
                &self.file,
                self.name(),
                self.state.identity,
                self.destination.maximum
            )? == self.state,
            "canonical output full file state changed"
        );
        ensure!(
            raw_digest(&self.file, self.identity.byte_length)? == self.identity.raw_sha256,
            "canonical output raw digest changed"
        );
        ensure!(
            self.destination.parent.check_file(
                &self.file,
                self.name(),
                self.state.identity,
                self.destination.maximum
            )? == self.state,
            "canonical output changed during readback"
        );
        if self.published {
            self.destination
                .parent
                .require_absent(&self.destination.stage)?;
        }
        Ok(())
    }
    fn publish(&mut self, index: usize, hook: &mut impl FnMut(Event) -> Result<()>) -> Result<()> {
        self.check()?;
        hook(Event::Output(index, Phase::BeforeRename))?;
        self.check()?;
        self.destination
            .parent
            .require_absent(&self.destination.name)?;
        hook(Event::Output(index, Phase::RenameReady))?;
        rustix::fs::renameat_with(
            self.destination.parent.file(),
            &self.destination.stage,
            self.destination.parent.file(),
            &self.destination.name,
            RenameFlags::NOREPLACE,
        )?;
        let state = self.destination.parent.check_file(
            &self.file,
            &self.destination.name,
            self.state.identity,
            self.destination.maximum,
        )?;
        ensure!(
            state.size == self.state.size && state.modified == self.state.modified,
            "canonical output changed during rename"
        );
        self.state = state;
        self.published = true;
        self.check()?;
        hook(Event::Output(index, Phase::AfterRename))?;
        self.check()?;
        self.destination.parent.sync(&mut |p| {
            self.check()?;
            hook(Event::Output(index, p))?;
            self.check()
        })?;
        self.check()
    }
}
/// Retains input, Core completion and both published descriptors through the caller's reply flush.
/// Every identity call checks full file state and raw content; any error or panic is permanent.
pub(crate) struct PublishedCanonicalInputs {
    context: RetainedOriginalInput,
    complete: CanonicalKuraEvidenceComplete,
    finality: Stage,
    queries: Stage,
    poisoned: Cell<bool>,
}
impl PublishedCanonicalInputs {
    fn check(&self) -> Result<()> {
        self.check_with_midpoint(|| Ok(()))
    }
    fn check_with_midpoint(&self, mut midpoint: impl FnMut() -> Result<()>) -> Result<()> {
        // The whole scan must observe one stable pair of parent namespaces, including an
        // earlier output name while a later output's potentially large digest is read.
        let first_parent = held(self.finality.destination.parent.file())?;
        let second_parent = held(self.queries.destination.parent.file())?;
        self.context.ensure_live()?;
        self.complete.recheck_sources()?;
        self.finality.destination.check_ancestry(&self.complete)?;
        self.queries.destination.check_ancestry(&self.complete)?;
        self.finality.check()?;
        midpoint()?;
        self.queries.check()?;
        self.context.ensure_live()?;
        self.complete.recheck_sources()?;
        ensure!(
            held(self.finality.destination.parent.file())? == first_parent
                && held(self.queries.destination.parent.file())? == second_parent,
            "published input namespace changed during complete pair check"
        );
        self.finality.destination.parent.check()?;
        self.queries.destination.parent.check()?;
        Ok(())
    }
    pub(crate) fn identity(&self) -> Result<CanonicalInputsIdentity> {
        self.identity_with_hook(|_| Ok(()))
    }
    fn identity_with_hook(
        &self,
        mut hook: impl FnMut(Event) -> Result<()>,
    ) -> Result<CanonicalInputsIdentity> {
        ensure!(
            !self.poisoned.replace(true),
            "canonical input publication is poisoned"
        );
        hook(Event::BeforeIdentity)?;
        self.check()?;
        let identity = CanonicalInputsIdentity {
            context: self.context.identity,
            finality: self.finality.identity,
            queries: self.queries.identity,
        };
        hook(Event::AfterIdentity)?;
        self.check()?;
        self.poisoned.set(false);
        Ok(identity)
    }
}

#[cfg(test)]
mod tests {
    include!("canonical_inputs/tests.rs");
}
