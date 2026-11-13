//! Export minimal signed rows from original native body custody in a pinned State cut.

use super::*;
use std::{num::NonZeroUsize, sync::Arc};

use iroha_allocation::{ChargedBuffer, ChargedBufferError};
use iroha_data_model::{block::SharedSignedBlock, sumeragi_finality::SumeragiCommitCertificateV1};

use crate::{
    state::State,
    sumeragi::{
        block_store::{KuraBlockStore, Staging},
        crypto::BlsCrypto,
        runtime_availability::NativeGlobalAvailability,
    },
};

/// Original native source for one transferable checkpoint, retaining only bounded working state.
/// The receiver needs the resulting prefix rows, genesis and H+1, not bodies 2..=H.
pub struct CheckpointPrefixExport {
    state: Arc<State>,
    hashes: ChargedBuffer<HashOf<BlockHeader>>,
    genesis: SharedSignedBlock,
    successor: SharedSignedBlock,
    reader: Reader,
}

struct Reader {
    state: Arc<State>,
    source: KuraBlockStore,
    verifier: PortablePrefixVerifierV1,
}

fn buffer_error(error: ChargedBufferError) -> ExecutionAttemptError<Error> {
    ExecutionAttemptError::Deferred(match error {
        ChargedBufferError::Admission(original) => original.into(),
        original @ ChargedBufferError::Allocator { .. } => {
            iroha_allocation::AllocatorRefusal::try_from(original)
                .expect("matched original allocator refusal")
                .into()
        }
    })
}

fn read_block(state: &State, height: u64) -> Attempt<SharedSignedBlock> {
    let index = usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| invalid("checkpoint export height exceeds host capacity"))?;
    state
        .kura()
        .get_block(index, &state.ivm_execution_budget())
        .map_err(|error| error.map_rejection(|original| Error::Storage(Box::new(original))))?
        .ok_or_else(|| {
            invalid(format!(
                "original checkpoint export body {height} is unavailable"
            ))
        })
}

impl CheckpointPrefixExport {
    /// Pin the exact history through H+1 and acquire its original genesis/native authority.
    /// This creates no checkpoint, signs nothing and changes no State or original store.
    ///
    /// # Errors
    /// Missing original bodies, insufficient history, a foreign root or original resource refusal.
    pub fn new(state: Arc<State>, height: u64) -> Attempt<Self> {
        let through = height
            .checked_add(1)
            .ok_or_else(|| invalid("checkpoint export height overflows"))?;
        let count = usize::try_from(through)
            .map_err(|_| invalid("checkpoint export journal exceeds host capacity"))?;
        if height < 2 || state.committed_height() < count {
            return Err(invalid(
                "checkpoint export requires actual history through H+1",
            ));
        }
        if state
            .kura()
            .history_boundary()
            .is_some_and(|boundary| height < boundary.height)
        {
            return Err(invalid(
                "checkpoint export is below the authenticated startup floor",
            ));
        }
        let mut hashes =
            ChargedBuffer::new(count, &state.ivm_execution_budget()).map_err(buffer_error)?;
        {
            let view = state.try_view_once().map_err(|error| match error {
                crate::state::StateViewError::Runtime(
                    crate::state::LaneLifecycleError::NposPolicy(ExecutionAttemptError::Deferred(
                        original,
                    )),
                ) => ExecutionAttemptError::Deferred(original),
                error => {
                    let kind = if matches!(error, crate::state::StateViewError::Busy(_)) {
                        std::io::ErrorKind::WouldBlock
                    } else {
                        std::io::ErrorKind::InvalidData
                    };
                    storage_error(crate::kura::Error::IO(
                        std::io::Error::new(kind, error),
                        state.kura().store_root(),
                    ))
                }
            })?;
            for hash in view.block_hashes().iter().take(count) {
                hashes.push_reserved(*hash);
            }
        }
        if hashes.as_slice().len() != count {
            return Err(invalid("checkpoint export State cut changed"));
        }
        let genesis = read_block(&state, 1)?;
        let successor = read_block(&state, through)?;
        if genesis.hash() != hashes.as_slice()[0]
            || successor.hash() != hashes.as_slice()[count - 1]
        {
            return Err(invalid(
                "checkpoint export original source differs from pinned history",
            ));
        }
        let verifier = verifier(&genesis, state.chain_id_ref(), *state.network_id_ref())?;
        let instance =
            crate::sumeragi::node::root_instance(&genesis, &state.chain_id_ref().to_string())
                .map_err(|error| error.map_rejection(Error::Invalid))?;
        let crypto = Arc::new(BlsCrypto::new());
        let availability = Arc::new(
            NativeGlobalAvailability::new(Arc::clone(&state), instance, Arc::clone(&crypto))
                .map_err(|error| {
                    error.map_rejection(|error| {
                        Error::Storage(Box::new(crate::kura::Error::IO(
                            error,
                            state.kura().store_root(),
                        )))
                    })
                })?,
        );
        let source = KuraBlockStore::new(
            state.kura_handle(),
            crypto,
            1,
            state
                .authenticated_root_scope()
                .ok_or_else(|| invalid("checkpoint export has no authenticated root scope"))?,
            Staging::new(),
            state.ivm_execution_budget(),
            availability,
        );
        Ok(Self {
            reader: Reader {
                state: Arc::clone(&state),
                source,
                verifier,
            },
            state,
            hashes,
            genesis,
            successor,
        })
    }

    /// The exact checkpoint height, excluding its retained native successor.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.successor.header().height().get() - 1
    }

    /// Export only into a fresh destination through Kura's recoverable staged bootstrap.
    /// The destination's snapshot must separately pass complete-State authentication on startup.
    ///
    /// # Errors
    /// Existing destination, altered original history, missing native custody, failed proof
    /// verification, storage failure or the original local resource refusal.
    pub fn write_kura(
        &mut self,
        config: &iroha_config::parameters::actual::Kura,
    ) -> Attempt<VerifiedPortableCheckpointV1> {
        let Self {
            state,
            hashes,
            genesis,
            successor,
            reader,
        } = self;
        let parents = &hashes.as_slice()[..hashes.as_slice().len() - 1];
        let configured_catalog = state
            .kura()
            .configured_lane_catalog_baseline()
            .map_err(storage_error)?
            .ok_or_else(|| invalid("checkpoint source lacks its configured lane catalog"))?;
        let verified = crate::kura::Kura::prepare_portable_checkpoint_store(
            config,
            configured_catalog,
            state.chain_id_ref(),
            *state.network_id_ref(),
            genesis,
            parents,
            successor,
            &mut |height| reader.frame(height, parents),
        )?;
        state
            .kura()
            .verify_configured_lane_catalog_baseline(configured_catalog)
            .map_err(storage_error)?;
        Ok(verified)
    }
}

impl Reader {
    fn frame(
        &mut self,
        height: u64,
        hashes: &[HashOf<BlockHeader>],
    ) -> Attempt<PortablePrefixFrameV1> {
        if self.verifier.height().checked_add(1) != Some(height) {
            return Err(invalid(
                "checkpoint export requested a noncontiguous original height",
            ));
        }
        if let Some(frame) = self.state.kura().checkpoint_prefix_frame(height)? {
            // Previously transferred rows remain untrusted DATA until this new walk verifies
            // their original signatures and the caller binds every parent to its pinned cut.
            if hashes.get((height - 2) as usize) != Some(&frame.executed_parent_header.hash()) {
                return Err(invalid(
                    "retained prefix differs from the pinned export journal",
                ));
            }
            self.verifier.verify(&frame).map_err(finality_error)?;
            return Ok(frame);
        }
        let block = read_block(&self.state, height)?;
        let parent = read_block(&self.state, height - 1)?;
        if hashes.get((height - 2) as usize) != Some(&parent.hash())
            || block.header().prev_block_hash() != Some(parent.hash())
        {
            return Err(invalid(
                "checkpoint export parent changed from the pinned journal",
            ));
        }
        let certificate = block
            .commit_certificate()
            .ok_or_else(|| invalid("checkpoint export original has no certificate"))?;
        let mut acquisition = self
            .verifier
            .acquisition(
                SumeragiCommitCertificateV1 {
                    consensus_header: certificate.consensus_header().to_vec(),
                    commit_qc: certificate.commit_qc().to_vec(),
                    result_preimage: certificate.result_preimage().to_vec(),
                },
                certificate.availability().to_vec(),
                parent.header(),
            )
            .map_err(finality_error)?;
        let (body, _) = self
            .source
            .committed_body(height)
            .map_err(|error| {
                error.map_rejection(|error| {
                    Error::Storage(Box::new(crate::kura::Error::IO(
                        error,
                        self.state.kura().store_root(),
                    )))
                })
            })?
            .ok_or_else(|| invalid("checkpoint export original availability is missing"))?;
        acquisition
            .acquire_from_body(&body)
            .map_err(finality_error)?;
        let frame = acquisition
            .into_proof()
            .map_err(|_| invalid("original custody did not complete its prefix"))?
            .with_parent_da_bundles(
                parent.da_commitments().cloned(),
                parent.da_pin_intents().cloned(),
            );
        self.verifier.verify(&frame).map_err(finality_error)?;
        Ok(frame)
    }
}
