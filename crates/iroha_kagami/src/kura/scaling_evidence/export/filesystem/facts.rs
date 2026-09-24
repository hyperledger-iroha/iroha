//! Ten-original custody and retained publication of semantically assembled facts.

use super::*;
use crate::kura::scaling_evidence::export::launcher::{
    journal::{JournalExpectations, admit_expectations},
    prepare::assemble::{
        AssembledFacts, FactsAssemblyCaps, FactsOriginals, GenesisExpectations, OriginalFact,
        admit_work, assemble,
    },
};
use std::cell::Cell;
use zeroize::Zeroizing;

impl BoundInput {
    fn read_sensitive(
        &mut self,
        hook: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<Zeroizing<Vec<u8>>> {
        self.check()?;
        hook(Phase::BeforeRead)?;
        self.check()?;
        self.file.seek(SeekFrom::Start(0))?;
        let size = usize::try_from(self.state.size)?;
        // The allocation belongs to its erasing owner before any descriptor
        // read, including short reads, later hash failures and unwinding hooks.
        let mut bytes = Zeroizing::new(Vec::new());
        bytes.try_reserve_exact(size)?;
        bytes.resize(size, 0);
        self.file.read_exact(&mut bytes)?;
        let mut extra = Zeroizing::new([0_u8; 1]);
        ensure!(
            self.file.read(&mut *extra)? == 0,
            "input grew beyond admitted length"
        );
        hook(Phase::AfterRead)?;
        self.check()?;
        ensure!(
            iroha_crypto::sha256(&bytes) == self.digest,
            "input SHA-256 mismatch"
        );
        Ok(bytes)
    }
}

impl Inputs {
    fn read_sensitive_all(
        &mut self,
        hook: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<Vec<Zeroizing<Vec<u8>>>> {
        ensure!(
            !self.poisoned && !self.read,
            "input owner poisoned or already consumed"
        );
        self.poisoned = true;
        let mut result = Vec::with_capacity(self.files.len());
        for file in &mut self.files {
            result.push(file.read_sensitive(hook)?);
        }
        for file in &self.files {
            file.check()?;
        }
        self.read = true;
        self.poisoned = false;
        Ok(result)
    }
}

/// Only the original role index, output or assembly crosses a test boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FactsRole {
    Original(usize),
    Output,
    Assembly,
}

impl FactsInputBindings {
    fn into_ordered(self) -> [ProofInputBinding; 10] {
        let [peer0, peer1, peer2, peer3] = self.peer_configs;
        [
            self.manifest,
            self.signed_genesis,
            peer0,
            peer1,
            peer2,
            peer3,
            self.context,
            self.journal,
            self.finality,
            self.queries,
        ]
    }
}

fn admit_caps(bindings: &[ProofInputBinding; 10], caps: &FactsAssemblyCaps) -> Result<()> {
    ensure!(
        bindings[6].max_bytes <= 8 * 1024 * 1024,
        "facts genesis context reservation exceeds 8 MiB"
    );
    ensure!(
        [caps.input_bytes, caps.facts_bytes, caps.total_bytes]
            .iter()
            .all(|n| (1..=MAX_INPUT_BYTES).contains(n))
            && (1..=512 * 1024 * 1024).contains(&caps.decode_bytes),
        "invalid facts source, output or decode allocation"
    );
    ensure!(
        caps.input_bytes
            .checked_add(caps.facts_bytes)
            .is_some_and(|n| n <= caps.total_bytes),
        "facts original and output reservations exceed the total"
    );
    let mut total = 0_u64;
    for binding in bindings {
        ensure!(
            (1..=caps.input_bytes).contains(&binding.max_bytes),
            "invalid original facts file reservation"
        );
        total = total
            .checked_add(binding.max_bytes)
            .ok_or_else(|| eyre!("facts original reservation overflow"))?;
    }
    ensure!(
        total <= caps.input_bytes,
        "facts original reservations exceed the input cap"
    );
    Ok(())
}

struct FactsOutput {
    path: PathBuf,
    stage_path: PathBuf,
    parent: Parent,
    destination: OsString,
    stage: OsString,
    maximum: u64,
}
impl FactsOutput {
    fn admit(
        path: &Path,
        maximum: u64,
        bindings: &[ProofInputBinding; 10],
        block_store: &Path,
        merge_log: &Path,
        hook: &mut impl FnMut(FactsRole, Phase) -> Result<()>,
    ) -> Result<Self> {
        ensure!(
            (1..=MAX_INPUT_BYTES).contains(&maximum),
            "invalid facts output cap"
        );
        ensure!(
            [block_store, merge_log]
                .iter()
                .all(|path| path.as_os_str().as_bytes().len() <= MAX_PATH_BYTES),
            "facts Core source path exceeds the bound"
        );
        ensure!(
            path.as_os_str()
                .as_bytes()
                .len()
                .checked_add(".publishing".len())
                .is_some_and(|n| n <= MAX_PATH_BYTES),
            "facts stage path exceeds the bound"
        );
        let mut stage_path = path.as_os_str().to_owned();
        stage_path.push(".publishing");
        let stage_path = PathBuf::from(stage_path);
        let core = [
            block_store.join("blocks.data"),
            block_store.join("blocks.index"),
            block_store.join("blocks.hashes"),
            block_store.join("blocks.count.norito"),
            merge_log.to_owned(),
        ];
        let mut paths = std::collections::BTreeSet::new();
        for binding in bindings {
            ensure!(
                paths.insert(&binding.path)
                    && binding.path != path
                    && binding.path != stage_path
                    && !core.contains(&binding.path),
                "facts original, output, stage or Core roles overlap"
            );
        }
        ensure!(
            !core.contains(&path.to_owned()) && !core.contains(&stage_path),
            "facts output or stage overlaps a Core input"
        );
        let (parent, destination) =
            Parent::capture(path, &mut |phase| hook(FactsRole::Output, phase))?;
        let mut stage = destination.clone();
        stage.push(".publishing");
        let output = Self {
            path: path.to_owned(),
            stage_path,
            parent,
            destination,
            stage,
            maximum,
        };
        output.check_absent()?;
        Ok(output)
    }
    fn check_absent(&self) -> Result<()> {
        self.parent.require_absent(&self.destination)?;
        self.parent.require_absent(&self.stage)
    }
    fn check_sources(&self, inputs: &InputPublicationLease, facts: &AssembledFacts) -> Result<()> {
        self.parent.check()?;
        inputs.check()?;
        facts.recheck_sources()?;
        let ancestry = self
            .parent
            .directories
            .iter()
            .map(|d| (d.identity.dev, d.identity.ino))
            .collect::<Vec<_>>();
        facts.ensure_publication_ancestry(&ancestry)?;
        for input in &inputs.files {
            ensure!(
                input.path != self.path && input.path != self.stage_path,
                "facts role path changed"
            );
            let ancestry = input
                .parent
                .directories
                .iter()
                .map(|d| (d.identity.dev, d.identity.ino))
                .collect::<Vec<_>>();
            facts.ensure_publication_ancestry(&ancestry)?;
        }
        inputs.check()?;
        self.parent.check()
    }
}

/// The original files, completed Core owner and actual RDWR output outlive the reply.
pub(in crate::kura::scaling_evidence::export) struct PublishedFacts {
    inputs: InputPublicationLease,
    facts: AssembledFacts,
    output: FactsOutput,
    file: File,
    state: FileState,
    digest: [u8; 32],
    published: bool,
    poisoned: Cell<bool>,
}
impl PublishedFacts {
    fn check_namespace(&self) -> Result<()> {
        let name = if self.published {
            &self.output.destination
        } else {
            &self.output.stage
        };
        ensure!(
            self.output.parent.check_file(
                &self.file,
                name,
                self.state.identity,
                self.state.size
            )? == self.state,
            "facts held or named output changed"
        );
        self.output.parent.require_absent(if self.published {
            &self.output.stage
        } else {
            &self.output.destination
        })
    }
    fn check(&self) -> Result<()> {
        self.output.check_sources(&self.inputs, &self.facts)?;
        self.check_namespace()?;
        let (digest, length) = iroha_crypto::sha256_reader_bounded(
            ContentReader {
                file: &self.file,
                offset: 0,
            }
            .take(self.state.size + 1),
            self.state.size,
        )?;
        ensure!(
            digest == self.digest && length == self.state.size,
            "facts output bytes changed"
        );
        self.output.check_sources(&self.inputs, &self.facts)?;
        self.check_namespace()
    }
    fn unchecked_identity(&self) -> PreparedTransportIdentity {
        PreparedTransportIdentity {
            raw_sha256: self.digest,
            byte_length: self.state.size,
        }
    }
    fn identity_with_hook(
        &self,
        mut hook: impl FnMut(FactsRole, Phase) -> Result<()>,
    ) -> Result<PreparedTransportIdentity> {
        ensure!(
            !self.poisoned.replace(true),
            "facts publication is permanently failed"
        );
        ensure!(self.published, "facts publication has not completed");
        self.check()?;
        hook(FactsRole::Assembly, Phase::BeforeIdentity)?;
        self.check()?;
        let identity = self.unchecked_identity();
        hook(FactsRole::Assembly, Phase::AfterIdentity)?;
        self.check()?;
        self.poisoned.set(false);
        Ok(identity)
    }
    /// Recheck every retained source and the exact facts descriptor before returning scalars.
    #[cfg(test)]
    pub(in crate::kura::scaling_evidence::export) fn identity(
        &self,
    ) -> Result<PreparedTransportIdentity> {
        self.identity_with_hook(|_, _| Ok(()))
    }
    /// Consume custody only after the scalar reply is written, flushed and rechecked.
    /// Any callback error or unwind destroys this non-clone owner without a success result.
    pub(in crate::kura::scaling_evidence::export) fn finish_reply(
        self,
        write_and_flush: impl FnOnce(PreparedTransportIdentity) -> Result<()>,
    ) -> Result<PreparedTransportIdentity> {
        ensure!(
            !self.poisoned.replace(true),
            "facts publication is permanently failed"
        );
        ensure!(self.published, "facts publication has not completed");
        self.check()?;
        let identity = self.unchecked_identity();
        write_and_flush(identity)?;
        self.check()?;
        ensure!(
            identity == self.unchecked_identity(),
            "facts identity changed across reply"
        );
        Ok(identity)
    }
    fn publish(
        output: FactsOutput,
        inputs: InputPublicationLease,
        facts: AssembledFacts,
        hook: &mut impl FnMut(FactsRole, Phase) -> Result<()>,
    ) -> Result<Self> {
        output.check_sources(&inputs, &facts)?;
        let length = u64::try_from(facts.canonical_bytes().len())?;
        ensure!(
            length > 0 && length <= output.maximum,
            "assembled facts exceed the output reservation"
        );
        let digest = iroha_crypto::sha256(facts.canonical_bytes());
        output.check_absent()?;
        let mut checked_hook = |phase| {
            output.check_sources(&inputs, &facts)?;
            hook(FactsRole::Output, phase)?;
            output.check_sources(&inputs, &facts)
        };
        let (mut file, identity) = output.parent.create(&output.stage, &mut checked_hook)?;
        ensure!(
            inputs
                .files
                .iter()
                .all(|i| (i.state.identity.dev, i.state.identity.ino)
                    != (identity.dev, identity.ino)),
            "facts output aliases an original input inode"
        );
        checked_hook(Phase::BeforeWrite)?;
        ensure!(
            output
                .parent
                .check_file(&file, &output.stage, identity, length)?
                .size
                == 0,
            "facts stage changed before write"
        );
        output.parent.require_absent(&output.destination)?;
        file.write_all(facts.canonical_bytes())?;
        let state = output
            .parent
            .check_file(&file, &output.stage, identity, length)?;
        ensure!(state.size == length, "facts stage write is incomplete");
        let owner = Self {
            inputs,
            facts,
            output,
            file,
            state,
            digest,
            published: false,
            poisoned: Cell::new(true),
        };
        for phase in [Phase::AfterWrite, Phase::BeforeFileSync] {
            owner.check()?;
            hook(FactsRole::Output, phase)?;
            owner.check()?;
        }
        owner.file.sync_all()?;
        for phase in [
            Phase::AfterFileSync,
            Phase::BeforeReadback,
            Phase::AfterReadback,
            Phase::BeforeRename,
            Phase::RenameReady,
        ] {
            owner.check()?;
            hook(FactsRole::Output, phase)?;
            owner.check()?;
        }
        rustix::fs::renameat_with(
            owner.output.parent.file(),
            &owner.output.stage,
            owner.output.parent.file(),
            &owner.output.destination,
            RenameFlags::NOREPLACE,
        )?;
        let mut owner = owner;
        owner.published = true;
        let state = owner.output.parent.check_file(
            &owner.file,
            &owner.output.destination,
            owner.state.identity,
            length,
        )?;
        ensure!(
            state.size == owner.state.size && state.modified == owner.state.modified,
            "facts changed during publication"
        );
        owner.state = state;
        owner.check()?;
        hook(FactsRole::Output, Phase::AfterRename)?;
        owner.check()?;
        owner.output.parent.sync(&mut |phase| {
            owner.check()?;
            hook(FactsRole::Output, phase)?;
            owner.check()
        })?;
        owner.check()?;
        owner.poisoned.set(false);
        owner.identity_with_hook(hook)?;
        Ok(owner)
    }
}

/// Assemble the exact ten retained originals and publish only the opaque successful owner.
#[expect(
    clippy::too_many_arguments,
    reason = "independent typed launch inputs retain separate owners"
)]
pub(in crate::kura::scaling_evidence::export) fn produce_facts(
    bindings: FactsInputBindings,
    output_path: &Path,
    genesis: GenesisExpectations,
    journal: JournalExpectations,
    verification_limits: VerificationLimits,
    block_store: &Path,
    merge_log: &Path,
    reader_limits: CanonicalKuraEvidenceLimits,
    caps: FactsAssemblyCaps,
) -> Result<PublishedFacts> {
    produce_with_hook(
        bindings,
        output_path,
        genesis,
        journal,
        verification_limits,
        block_store,
        merge_log,
        reader_limits,
        caps,
        |_, _| Ok(()),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the production inputs remain exact while tests observe retained boundaries"
)]
fn produce_with_hook(
    bindings: FactsInputBindings,
    output_path: &Path,
    genesis: GenesisExpectations,
    journal: JournalExpectations,
    verification_limits: VerificationLimits,
    block_store: &Path,
    merge_log: &Path,
    reader_limits: CanonicalKuraEvidenceLimits,
    caps: FactsAssemblyCaps,
    mut hook: impl FnMut(FactsRole, Phase) -> Result<()>,
) -> Result<PublishedFacts> {
    let bindings = bindings.into_ordered();
    admit_caps(&bindings, &caps)?;
    admit_expectations(&journal)?;
    admit_work(verification_limits, reader_limits)?;
    ensure!(
        reader_limits.owner_uid == rustix::process::geteuid().as_raw(),
        "facts and Core owners differ"
    );
    let output = FactsOutput::admit(
        output_path,
        caps.facts_bytes,
        &bindings,
        block_store,
        merge_log,
        &mut hook,
    )?;
    let source_caps = bindings.each_ref().map(|b| b.max_bytes);
    let mut index = 0;
    let mut inputs = Inputs::open(Vec::from(bindings), caps.input_bytes, &mut |phase| {
        output.check_absent()?;
        hook(FactsRole::Original(index), phase)?;
        output.check_absent()?;
        if phase == Phase::InputRetained {
            index += 1;
        }
        Ok(())
    })?;
    // These four original configurations carry private validator and transport
    // keys. The generic public-frame reader remains permissive; facts require
    // owner-only access before the first original content read.
    for config in &inputs.files[2..6] {
        config.check()?;
        ensure!(
            config.state.identity.mode & 0o077 == 0,
            "facts peer configuration must be owner-only"
        );
    }
    let mut index = 0;
    let raw = inputs.read_sensitive_all(&mut |phase| {
        output.check_absent()?;
        hook(FactsRole::Original(index), phase)?;
        output.check_absent()?;
        if phase == Phase::AfterRead {
            index += 1;
        }
        Ok(())
    })?;
    let lease = inputs.finish_lease(&mut |phase| {
        output.check_absent()?;
        hook(FactsRole::Assembly, phase)?;
        output.check_absent()
    })?;
    let original = |index: usize| OriginalFact {
        path: &lease.files[index].path,
        bytes: &raw[index],
        expected_raw_sha256: lease.files[index].digest,
        max_bytes: source_caps[index],
    };
    lease.check()?;
    output.check_absent()?;
    hook(FactsRole::Assembly, Phase::BeforeVerification)?;
    lease.check()?;
    output.check_absent()?;
    let facts = assemble(
        FactsOriginals {
            manifest: original(0),
            signed_genesis: original(1),
            peer_configs: [original(2), original(3), original(4), original(5)],
            context: original(6),
            journal: original(7),
            finality: original(8),
            queries: original(9),
        },
        genesis,
        journal,
        verification_limits,
        block_store,
        merge_log,
        reader_limits,
        caps,
    )?;
    drop(raw);
    lease.check()?;
    output.check_absent()?;
    facts.recheck_sources()?;
    hook(FactsRole::Assembly, Phase::AfterVerification)?;
    output.check_sources(&lease, &facts)?;
    output.check_absent()?;
    PublishedFacts::publish(output, lease, facts, &mut hook)
}

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
