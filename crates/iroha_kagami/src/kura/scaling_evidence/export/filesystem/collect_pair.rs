//! Native collection publication through the retained two-file filesystem primitive.
//!
//! Original genesis, epoch context and the completed Kura reader survive every
//! write, readback, NOREPLACE publication and reply flush. The pair has no atomic
//! filesystem transaction; failures preserve any surviving files and return no success.

use super::*;
use crate::kura::scaling_evidence::export::collector::{
    CollectedNativeInputs, CollectionLimits, collect_native_inputs,
};
use iroha_data_model::{NetworkId, sumeragi::epoch::ValidatorEpochContextV1};
use iroha_model_base::chain::ChainId;

/// Two absent canonical native-vector destinations admitted before collection.
pub(crate) struct CollectedOutputPair(PreparedOutputPair);
impl CollectedOutputPair {
    /// Retain disjoint destinations and bounded output reservations.
    pub(crate) fn admit(carrier: &Path, queries: &Path, limits: CollectionLimits) -> Result<Self> {
        Ok(Self(PreparedOutputPair::admit(
            carrier,
            queries,
            PrepareOutputCaps {
                request_bytes: limits.carrier_bytes,
                bundle_bytes: limits.query_bytes,
                total_bytes: limits.total_bytes,
            },
        )?))
    }
}

/// Fully published native vectors with their original input and output custody.
pub(crate) struct CollectedLaunch {
    original: InputPublicationLease,
    collected: CollectedNativeInputs,
    carrier: RetainedTransport,
    queries: RetainedTransport,
}
impl CollectedLaunch {
    fn check(&self) -> Result<()> {
        self.original.check()?;
        self.collected.recheck_sources()?;
        for output in [&self.carrier, &self.queries] {
            let ancestry = output
                .output
                .parent
                .directories
                .iter()
                .map(|directory| (directory.identity.dev, directory.identity.ino))
                .collect::<Vec<_>>();
            self.collected.disk.ensure_publication_ancestry(&ancestry)?;
            output.check()?;
        }
        self.original.check()?;
        self.collected.recheck_sources()
    }
    /// Consume source custody only after an exact bounded reply has been written and flushed.
    pub(crate) fn finish_reply(
        self,
        write_and_flush: impl FnOnce(NativeCollectionIdentity) -> Result<()>,
    ) -> Result<NativeCollectionIdentity> {
        self.check()?;
        let identity = NativeCollectionIdentity {
            genesis: PreparedTransportIdentity {
                raw_sha256: self.original.files[0].digest,
                byte_length: self.original.files[0].state.size,
            },
            context: PreparedTransportIdentity {
                raw_sha256: self.original.files[1].digest,
                byte_length: self.original.files[1].state.size,
            },
            carrier: self.carrier.identity(),
            queries: self.queries.identity(),
            committed_height: self.collected.disk.committed_height(),
            carrier_count: self.collected.height_count(),
            query_count: u64::try_from(self.collected.query_count())?,
        };
        write_and_flush(identity)?;
        self.check()?;
        Ok(identity)
    }
}

/// Authenticate original native inputs and retain both exact published output files.
#[allow(clippy::too_many_arguments)]
pub(crate) fn collect_bound(
    genesis: ProofInputBinding,
    context: ProofInputBinding,
    chain_id: ChainId,
    network: NetworkId,
    genesis_epoch_context_id: [u8; 32],
    block_store: &Path,
    reader: CanonicalKuraEvidenceLimits,
    limits: CollectionLimits,
    outputs: CollectedOutputPair,
) -> Result<CollectedLaunch> {
    collect_with_hook(
        genesis,
        context,
        chain_id,
        network,
        genesis_epoch_context_id,
        block_store,
        reader,
        limits,
        outputs,
        |_, _| Ok(()),
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_with_hook(
    genesis: ProofInputBinding,
    context: ProofInputBinding,
    chain_id: ChainId,
    network: NetworkId,
    genesis_epoch_context_id: [u8; 32],
    block_store: &Path,
    reader: CanonicalKuraEvidenceLimits,
    limits: CollectionLimits,
    outputs: CollectedOutputPair,
    mut hook: impl FnMut(PrepareRole, Phase) -> Result<()>,
) -> Result<CollectedLaunch> {
    let outputs = outputs.0;
    ensure!(
        genesis.max_bytes <= 32 * 1024 * 1024
            && context.max_bytes <= 8 * 1024 * 1024
            && genesis
                .max_bytes
                .checked_add(context.max_bytes)
                .and_then(|n| n.checked_add(limits.carrier_bytes))
                .and_then(|n| n.checked_add(limits.query_bytes))
                .is_some_and(|n| n <= limits.total_bytes),
        "native collection original and output reservations exceed admission"
    );
    outputs.admit_facts(&genesis)?;
    outputs.admit_facts(&context)?;
    let mut inputs = Inputs::open(vec![genesis, context], limits.total_bytes, &mut |phase| {
        hook(PrepareRole::Facts, phase)?;
        outputs.check_absent()
    })?;
    let mut bytes = inputs.read_all(&mut |phase| {
        hook(PrepareRole::Facts, phase)?;
        outputs.check_absent()
    })?;
    let context_bytes = bytes
        .pop()
        .ok_or_else(|| eyre!("missing original epoch context"))?;
    let genesis_bytes = bytes
        .pop()
        .ok_or_else(|| eyre!("missing original signed genesis"))?;
    let original = inputs.finish_lease(&mut |phase| {
        hook(PrepareRole::Facts, phase)?;
        outputs.check_absent()
    })?;
    let decode = norito::DecodeLimits::new(
        reader.max_carrier_bytes.max(limits.context_bytes),
        reader.max_carrier_bytes.max(limits.context_bytes),
        reader.max_decode_allocation_bytes,
        reader.max_decode_allocation_bytes,
        64,
    );
    // The enclosing scope cannot be expanded by canonical decoders inside the
    // complete original-input and certificate verification pipeline.
    let collected = norito::with_decode_limits_scope(decode, || -> Result<_> {
        let epoch: ValidatorEpochContextV1 = norito::decode_canonical_with_limits(
            &context_bytes,
            norito::canonical_decode_limits(context_bytes.len()),
        )?;
        ensure!(
            epoch.context_id().map_err(|error| eyre!(error))? == genesis_epoch_context_id,
            "original epoch context differs from independent launch identity"
        );
        let genesis = iroha_genesis::decode_signed_genesis(&genesis_bytes)?;
        ensure!(
            iroha_data_model::sumeragi_finality::genesis_epoch(&genesis)
                .map_err(|error| eyre!(error))?
                == epoch,
            "original signed genesis differs from original epoch context"
        );
        drop(genesis);
        drop(epoch);
        drop(context_bytes);
        original.check()?;
        let collected = collect_native_inputs(
            chain_id,
            network,
            genesis_epoch_context_id,
            &genesis_bytes,
            block_store,
            reader,
            limits,
        )?;
        Ok(collected)
    })?;
    drop(genesis_bytes);
    original.check()?;
    outputs.check_absent()?;
    for output in [&outputs.request, &outputs.bundle] {
        let ancestry = output
            .parent
            .directories
            .iter()
            .map(|directory| (directory.identity.dev, directory.identity.ino))
            .collect::<Vec<_>>();
        collected.disk.ensure_publication_ancestry(&ancestry)?;
    }
    // The existing primitive checks original-file digests and both retained output
    // roles at each boundary. This hook adds the completed native store/archive owner.
    let mut retained_hook = |role, phase| {
        collected.recheck_sources()?;
        hook(role, phase)?;
        collected.recheck_sources()
    };
    let PreparedOutputPair {
        request, bundle, ..
    } = outputs;
    let mut carrier = RetainedTransport::stage(
        request,
        collected.carrier_bytes(),
        &original,
        OtherTransport::Absent(&bundle),
        PrepareRole::Request,
        &mut retained_hook,
    )?;
    let mut queries = RetainedTransport::stage(
        bundle,
        collected.query_bytes(),
        &original,
        OtherTransport::Retained(&carrier),
        PrepareRole::Bundle,
        &mut retained_hook,
    )?;
    carrier.publish(
        &original,
        &queries,
        PrepareRole::Request,
        &mut retained_hook,
    )?;
    queries.publish(&original, &carrier, PrepareRole::Bundle, &mut retained_hook)?;
    drop(retained_hook);
    let complete = CollectedLaunch {
        original,
        collected,
        carrier,
        queries,
    };
    complete.check()?;
    Ok(complete)
}

#[cfg(test)]
#[path = "collect_pair_tests.rs"]
mod tests;
