//! A current native cut whose full decoded bodies retire before the next stage.
//!
//! Each frame still passes the original canonical decoder and C1–C5 reader.
//! Only closed metadata and exact charged certificate/payload bytes survive a
//! stage. Inherited logical limits remain cumulative across the entire operation.

use super::*;
use crate::sumeragi::finality::NativeFinalityProofBudget;
use iroha_allocation::AllocationBudget;
use iroha_data_model::query::error::QueryExecutionFail;

/// Source work bounds for a staged current-cut reader.
/// Physical allocation corridors are separate caller-owned pools; this type
/// carries no cumulative logical decoder allowance.
#[derive(Clone, Copy, Debug)]
pub struct CurrentCutSourceLimits {
    block_bytes: usize,
    journal_bytes: usize,
    block_count: usize,
}
impl CurrentCutSourceLimits {
    /// Validate finite source bounds before any original body is read.
    /// # Errors
    /// Rejects zero, contradictory or above-protocol block, journal or count bounds.
    pub fn new(
        block_bytes: usize,
        journal_bytes: usize,
        block_count: usize,
    ) -> Result<Self, String> {
        use iroha_data_model::sumeragi::finality::{
            NATIVE_FINALITY_MAX_BLOCK_BYTES, NATIVE_FINALITY_MAX_BLOCK_COUNT,
            NATIVE_FINALITY_MAX_JOURNAL_BYTES,
        };
        if block_bytes == 0
            || block_bytes > NATIVE_FINALITY_MAX_BLOCK_BYTES
            || journal_bytes < block_bytes
            || journal_bytes > NATIVE_FINALITY_MAX_JOURNAL_BYTES
            || block_count == 0
            || block_count > NATIVE_FINALITY_MAX_BLOCK_COUNT
        {
            return Err("native current-cut source limits are invalid".into());
        }
        Ok(Self {
            block_bytes,
            journal_bytes,
            block_count,
        })
    }
}

/// Immutable scalar cut returned only after the original native certificate checks.
/// It cannot be decoded, imported or constructed by an external caller.
#[derive(Clone, Copy, Debug)]
pub struct CurrentCertifiedCut {
    height: u64,
    block_hash: HashOf<IrohaHeader>,
    core_hash: Hash32,
    result: Hash32,
    block_time_ms: u64,
    root_scope: SumeragiRootScope,
    network: NetworkId,
    world_state_root: iroha_data_model::sumeragi_finality::KeyedStateRoot,
}
impl CurrentCertifiedCut {
    fn from_committed(original: &CommittedBlock) -> Option<Self> {
        (original.height() >= 2
            && original.header().is_some()
            && original.commitment().height == original.height())
        .then(|| Self {
            height: original.height(),
            block_hash: original.block_hash(),
            core_hash: original.core_hash(),
            result: original.result(),
            block_time_ms: original.block_time_ms(),
            root_scope: original.root_scope(),
            network: original.commitment().schedule.current.network_id,
            world_state_root: original.commitment().execution.world_state_root,
        })
    }
    /// One-based certified height.
    pub const fn height(&self) -> u64 {
        self.height
    }
    /// Exact original Iroha header identity.
    pub const fn block_hash(&self) -> HashOf<IrohaHeader> {
        self.block_hash
    }
    /// Certified consensus header identity.
    pub const fn core_hash(&self) -> Hash32 {
        self.core_hash
    }
    /// Certified execution result.
    pub const fn result(&self) -> Hash32 {
        self.result
    }
    /// Original creation timestamp in milliseconds.
    pub const fn block_time_ms(&self) -> u64 {
        self.block_time_ms
    }
    /// Root selected by the authenticated original genesis.
    pub const fn root_scope(&self) -> SumeragiRootScope {
        self.root_scope
    }
    /// Certified context identity.
    pub fn id(&self) -> HeightContextId {
        certified_block_id(&self.core_hash, &self.result)
    }
    pub(crate) const fn network(&self) -> NetworkId {
        self.network
    }
    /// Complete original certified World state root.
    pub const fn world_state_root(&self) -> iroha_data_model::sumeragi_finality::KeyedStateRoot {
        self.world_state_root
    }
}

fn query_error(
    error: ExecutionAttemptError<ChainReadError>,
) -> ExecutionAttemptError<QueryExecutionFail> {
    error.map_rejection(|error| QueryExecutionFail::Conversion(error.to_string()))
}
fn invalid(message: &str) -> ExecutionAttemptError<QueryExecutionFail> {
    QueryExecutionFail::Conversion(message.into()).into()
}

/// Authenticate the current cut with a finite physical stage and original source pool.
///
/// `frames` and `graphs` must belong to disjoint corridors of the request's already
/// retained admission. All inherited codec layers still accumulate their original
/// charges. This function creates no replacement for an inherited logical owner.
///
/// # Errors
/// Refuses a non-current cut, source-policy exhaustion, changed or malformed original
/// frames, a foreign root, invalid native certificates, or physical/codec exhaustion.
pub fn read_current_certified_cut(
    view: &impl StateReadOnly,
    height: u64,
    scope: SumeragiRootScope,
    limits: CurrentCutSourceLimits,
    frames: &AllocationBudget,
    graphs: &AllocationBudget,
) -> Result<CurrentCertifiedCut, ExecutionAttemptError<QueryExecutionFail>> {
    if height < 2 || u64::try_from(view.height()).ok() != Some(height) {
        return Err(invalid(
            "native cut must be the original non-genesis current height",
        ));
    }
    let index = usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| invalid("native cut height exceeds the host index"))?;
    let parent = NonZeroUsize::new(index.get() - 1).expect("non-genesis cut");
    let mut frames_left = limits.block_count as u64;
    let mut bytes_left = limits.journal_bytes as u64;
    let mut admit = |count: u64, bytes: u64| {
        if bytes > limits.block_bytes as u64 {
            return Err(invalid(
                "native current cut exceeds its finite source policy",
            ));
        }
        let next_frames = frames_left
            .checked_sub(count)
            .ok_or_else(|| invalid("native current cut exceeds its finite source policy"))?;
        let next_bytes = bytes_left
            .checked_sub(bytes)
            .ok_or_else(|| invalid("native current cut exceeds its finite source policy"))?;
        frames_left = next_frames;
        bytes_left = next_bytes;
        Ok(())
    };
    // Retained uncharged metadata is at most two checked result schedules and
    // the reverse walk's two validated epoch generations. The existing workspace
    // prepays five complete intrinsic result envelopes, so these fit even while
    // the next stage decodes. Header, certificate, payload and verifier authority
    // custody is separate in `frames`; no complete transaction graph escapes.
    let owner = NativeFinalityProofBudget::new(
        frames.clone(),
        graphs.clone(),
        norito::DecodeLimits::new(
            limits.block_bytes,
            limits.block_bytes,
            limits.journal_bytes,
            graphs.limit_bytes(),
            128,
        ),
    );
    let workspace = owner.workspace().map_err(query_error)?;
    let history = view.canonical_history().with_frame_pool(frames);
    let context = workspace
        .stage(|| {
            let original = history.block_with_admission(NonZeroUsize::new(1).unwrap(), &mut admit);
            Ok(original.and_then(|original| {
                let (_, instance, root_scope) =
                    authenticate_genesis(&original, view.network_id(), view.chain_id())
                        .map_err(query_error)?;
                if root_scope != scope {
                    return Err(invalid("native current cut belongs to another root"));
                }
                Ok(PrefixVerifierContext {
                    instance,
                    root_scope,
                })
            }))
        })
        .map_err(query_error)??;
    let mut target = None;
    let mut predecessor = None;
    history.visit_executed_backwards_staged(
        parent,
        index,
        &mut admit,
        |receipt| {
            let original = receipt.into_owned();
            if original.height() == height {
                let cut = CurrentCertifiedCut::from_committed(&original)
                    .ok_or_else(|| invalid("native current cut is inconsistent"))?;
                let wire = original
                    .block
                    .canonical_wire_identity()
                    .map_err(|error| invalid(&error.to_string()))?;
                let length = original
                    .header()
                    .ok_or_else(|| invalid("native current cut has no header"))?
                    .payload_len as usize;
                let payload = query_reader::capture_payload(
                    &original.block,
                    length,
                    limits.block_bytes,
                    frames,
                    height,
                )
                .map_err(query_error)?;
                // Unlike the interval reader, this adapter preserves the original
                // decoded header to avoid repeating its logical charges. Fund its
                // entire intrinsic decode envelope before it leaves this stage;
                // arbitrary skipped-leader vectors cannot hide in fixed metadata.
                let header_bytes = norito::canonical_decode_limits(
                    original
                        .block
                        .commit_certificate()
                        .expect("checked certificate")
                        .consensus_header()
                        .len(),
                )
                .max_total_allocated_bytes();
                let header_custody = frames
                    .try_reserve_bytes(header_bytes)
                    .map_err(|error| ExecutionAttemptError::Deferred(error.into()))?;
                let (frame, header) =
                    query_projection::CheckedFrameProjection::from_current_original(
                        original, frames,
                    )
                    .map_err(query_error)?;
                target = Some((cut, frame, header, payload, wire, header_custody));
            } else {
                let (frame, header) =
                    query_projection::CheckedFrameProjection::from_current_original(
                        original, frames,
                    )
                    .map_err(query_error)?;
                drop(header);
                predecessor = Some(frame);
            }
            Ok(core::ops::ControlFlow::Continue(()))
        },
        &workspace,
    )?;
    let target = target.ok_or_else(|| invalid("native current cut is absent"))?;
    // Tuple field order destroys the original header before its physical charge.
    let (cut, frame, header, payload, wire, _) = &target;
    let predecessor = predecessor.ok_or_else(|| invalid("native current parent is absent"))?;
    workspace
        .stage(|| {
            Ok((|| {
                let parent = predecessor.borrowed(None);
                let current = frame.borrowed(header.as_ref());
                check_projected_result_scope(&parent, context.root_scope)?;
                check_projected_result_scope(&current, context.root_scope)?;
                let authority = state_certificate::executed_projected_successor_authority(
                    parent, current, frames,
                )?;
                let source = ProjectedCertificateSource {
                    frame,
                    header: header.as_ref(),
                    payload: Some(payload),
                    wire: *wire,
                    admit_payload: true,
                };
                let certificate = context.prepare_certificate_source_with_admission(
                    CertificateSource::Projected(&source),
                    authority.projection(),
                    Some(authority.config()),
                    None,
                    &mut state_certificate::query_scratch_admission,
                    None,
                )?;
                verify_projected_boundary_source(&current, &parent.parent, authority.projection())?;
                if certificate.verification != QcVerification::Verified {
                    return Err(ChainReadError::MissingCertificate { height }.into());
                }
                Ok::<_, VerificationReadError>(*cut)
            })()
            .map_err(state_certificate::verification_attempt_failure))
        })
        .map_err(query_error)?
}

#[cfg(test)]
mod tests;
