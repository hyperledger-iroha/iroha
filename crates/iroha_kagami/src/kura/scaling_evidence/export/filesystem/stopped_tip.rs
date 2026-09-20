//! Retained stopped-store height observation, with no later execution or finality authority.

use super::*;
use iroha_core::kura::{
    CanonicalKuraEvidenceComplete, CanonicalKuraEvidenceError, CanonicalKuraEvidenceReader,
};
use iroha_data_model::NetworkId;
use std::cell::Cell;

/// The original genesis lease and actual Core completion survive the reply callback.
///
/// The launcher must already own successful termination and an independently authenticated
/// generated genesis. This observation checks the entire journal shape and merge-log framing;
/// it authenticates only the requested genesis carrier. Later carriers still require collection
/// and full anchored facts verification through the observed height.
pub(crate) struct RetainedStoppedTip {
    input: InputPublicationLease,
    complete: CanonicalKuraEvidenceComplete,
    identity: StoppedTipIdentity,
    poisoned: Cell<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Boundary {
    Original(Phase),
    BeforeCoreOpen,
    AfterCoreOpen,
    BeforeCarrier,
    AfterCarrier,
    BeforeMergeScan,
    AfterMergeScan,
    BeforeCoreFinish,
    AfterCoreFinish,
    BeforeIdentity,
    AfterIdentity,
}

impl RetainedStoppedTip {
    fn check(&self) -> Result<()> {
        self.input.check()?;
        self.complete.recheck_sources()?;
        let original = self
            .input
            .files
            .first()
            .ok_or_else(|| eyre!("stopped-tip original genesis is absent"))?;
        let ancestry = original
            .parent
            .directories
            .iter()
            .map(|directory| (directory.identity.dev, directory.identity.ino))
            .collect::<Vec<_>>();
        self.complete.ensure_publication_ancestry(&ancestry)?;
        ensure!(
            self.complete.carrier_count() == 1
                && self.complete.committed_height() == self.identity.committed_height
                && original.digest == self.identity.genesis.raw_sha256
                && original.state.size == self.identity.genesis.byte_length,
            "stopped-tip retained scope changed"
        );
        self.input.check()?;
        self.complete.recheck_sources()?;
        Ok(())
    }

    fn identity_with_hook(
        &self,
        mut hook: impl FnMut(Boundary) -> Result<()>,
    ) -> Result<StoppedTipIdentity> {
        ensure!(
            !self.poisoned.replace(true),
            "stopped-tip observation is permanently failed"
        );
        self.check()?;
        hook(Boundary::BeforeIdentity)?;
        self.check()?;
        let identity = self.identity;
        hook(Boundary::AfterIdentity)?;
        self.check()?;
        self.poisoned.set(false);
        Ok(identity)
    }

    /// Consume all source custody only after the actual bounded reply is written and flushed.
    ///
    /// A callback failure or unwind destroys this owner without a success result. Any failed
    /// retained check is permanent; replacing a damaged file cannot repair an old observation.
    pub(crate) fn finish_reply(
        self,
        write_and_flush: impl FnOnce(StoppedTipIdentity) -> Result<()>,
    ) -> Result<StoppedTipIdentity> {
        ensure!(
            !self.poisoned.replace(true),
            "stopped-tip observation is permanently failed"
        );
        self.check()?;
        let identity = self.identity;
        write_and_flush(identity)?;
        self.check()?;
        Ok(identity)
    }
}

fn admit(
    genesis: &ProofInputBinding,
    block_store: &Path,
    merge_log: &Path,
    limits: CanonicalKuraEvidenceLimits,
) -> Result<()> {
    // Core's bounds are private; this smaller explicit [1,1] contract is checked before
    // original file admission and Core repeats its own full validation when opened.
    ensure!(
        limits.first_height == 1
            && limits.last_height == 1
            && (1..=1_000_000).contains(&limits.max_committed_blocks)
            && limits.owner_uid == rustix::process::geteuid().as_raw(),
        "invalid stopped-tip genesis interval or owner"
    );
    ensure!(
        (1..=2 * 1024 * 1024 * 1024).contains(&limits.max_store_data_bytes)
            && (1..=32 * 1024 * 1024).contains(&limits.max_carrier_bytes)
            && limits.max_merge_log_bytes <= MAX_INPUT_BYTES
            && limits.max_merge_frames <= limits.max_committed_blocks
            && (1..=MAX_INPUT_BYTES).contains(&limits.max_output_bytes)
            && (1..=512 * 1024 * 1024).contains(&limits.max_decode_allocation_bytes)
            && (1..=32 * 1024 * 1024).contains(&genesis.max_bytes),
        "invalid stopped-tip file, frame or decode limits"
    );
    for path in [genesis.path.as_path(), block_store, merge_log] {
        ensure!(
            path.is_absolute() && path.as_os_str().as_bytes().len() <= MAX_PATH_BYTES,
            "stopped-tip paths must be bounded and absolute"
        );
        let mut normalized = PathBuf::new();
        let mut components = 0_usize;
        for component in path.components() {
            match component {
                Component::RootDir => normalized.push(component.as_os_str()),
                Component::Normal(name) => {
                    normalized.push(name);
                    components += 1;
                }
                _ => return Err(eyre!("stopped-tip path is not lexical and normalized")),
            }
        }
        ensure!(
            normalized.as_os_str() == path.as_os_str()
                && (1..=MAX_COMPONENTS).contains(&components),
            "stopped-tip path components are invalid"
        );
    }
    let core_paths = [
        block_store.join("blocks.data"),
        block_store.join("blocks.index"),
        block_store.join("blocks.hashes"),
        block_store.join("blocks.count.norito"),
        merge_log.to_owned(),
    ];
    ensure!(
        !core_paths.contains(&genesis.path),
        "original genesis overlaps a stopped Core input"
    );
    Ok(())
}

/// Observe an original stopped store using the actual Core reader's explicit genesis interval.
///
/// The raw-pinned genesis must come from the launcher's authenticated generation. The returned
/// count is the whole published marker, not the number of requested carriers. It is never a
/// substitute for verifying every later carrier, merge effect and finality certificate.
pub(crate) fn observe_stopped_tip(
    genesis: ProofInputBinding,
    expected_network_id: NetworkId,
    block_store: &Path,
    merge_log: &Path,
    reader: CanonicalKuraEvidenceLimits,
) -> Result<RetainedStoppedTip> {
    observe_with_hook(
        genesis,
        expected_network_id,
        block_store,
        merge_log,
        reader,
        |_| Ok(()),
    )
}

fn observe_with_hook(
    genesis: ProofInputBinding,
    expected_network_id: NetworkId,
    block_store: &Path,
    merge_log: &Path,
    limits: CanonicalKuraEvidenceLimits,
    mut hook: impl FnMut(Boundary) -> Result<()>,
) -> Result<RetainedStoppedTip> {
    admit(&genesis, block_store, merge_log, limits)?;
    let maximum = genesis.max_bytes;
    let mut inputs = Inputs::open(vec![genesis], maximum, &mut |phase| {
        hook(Boundary::Original(phase))
    })?;
    let bytes = inputs
        .read_all(&mut |phase| hook(Boundary::Original(phase)))?
        .pop()
        .ok_or_else(|| eyre!("stopped-tip original genesis is absent"))?;
    let input = inputs.finish_lease(&mut |phase| hook(Boundary::Original(phase)))?;
    let decode = norito::DecodeLimits::new(
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        512 * 1024 * 1024,
        limits.max_decode_allocation_bytes,
        64,
    );
    // One cumulative Norito allocation budget covers genesis and the entire merge scan.
    // This does not claim to meter every dependency allocation or kernel I/O latency.
    let complete = norito::with_decode_limits_scope(decode, || -> Result<_> {
        input.check()?;
        let original = iroha_genesis::decode_signed_genesis(&bytes)?;
        ensure!(
            original.header().height().get() == 1
                && original.header().prev_block_hash().is_none()
                && NetworkId::from_genesis_hash(original.hash()) == expected_network_id
                && original
                    .execution_context()
                    .and_then(|context| context.merge_entry.as_ref())
                    .is_none(),
            "stopped-tip original is not the expected unmerged genesis"
        );
        drop(original);
        let mut boundary = |point| {
            input.check()?;
            hook(point)?;
            input.check()
        };
        boundary(Boundary::BeforeCoreOpen)?;
        let mut reader = CanonicalKuraEvidenceReader::open(block_store, merge_log, limits)?;
        boundary(Boundary::AfterCoreOpen)?;
        boundary(Boundary::BeforeCarrier)?;
        let carrier = reader.read_carrier(1)?;
        ensure!(
            carrier == bytes,
            "stopped store differs from original signed genesis"
        );
        drop(carrier);
        boundary(Boundary::AfterCarrier)?;
        boundary(Boundary::BeforeMergeScan)?;
        reader.scan_merge_entries(&[], |_, _, _| {
            Err(CanonicalKuraEvidenceError::Invalid(
                "stopped-tip genesis interval cannot contain a merge entry",
            ))
        })?;
        boundary(Boundary::AfterMergeScan)?;
        boundary(Boundary::BeforeCoreFinish)?;
        let complete = reader.finish()?;
        boundary(Boundary::AfterCoreFinish)?;
        complete.recheck_sources()?;
        Ok(complete)
    })?;
    let identity = StoppedTipIdentity {
        genesis: PreparedTransportIdentity {
            raw_sha256: input.files[0].digest,
            byte_length: u64::try_from(bytes.len())?,
        },
        network_id: expected_network_id,
        committed_height: complete.committed_height(),
    };
    drop(bytes);
    let owner = RetainedStoppedTip {
        input,
        complete,
        identity,
        poisoned: Cell::new(false),
    };
    owner.identity_with_hook(hook)?;
    Ok(owner)
}

#[cfg(test)]
#[path = "stopped_tip_tests.rs"]
mod tests;
