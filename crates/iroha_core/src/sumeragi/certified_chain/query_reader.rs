//! Bounded original-owner consumer of the sole signed-genesis native verifier.

use super::*;
use crate::sumeragi::finality::FinalityDecodeWorkspace;
use iroha_data_model::sumeragi_finality::ScheduledSlot;

#[cfg(test)]
#[path = "query_reader_retention_tests.rs"]
mod retention_tests;

/// Cursor/acquisition controls and two legal native certificate sets for one query.
/// This sizes these byte/control families only. Prepared DA policy/commitment,
/// parent-service, signature and penalty-action graph children still charge this
/// same finite frame pool; their complete source-dependent high water is not
/// established by this helper. Malformed leaf extents can also refuse locally.
pub fn native_finality_source_control_bytes() -> Option<usize> {
    use iroha_allocation::{ChargedBuffer, ChargedShared};
    let certificate = (iroha_sumeragi::pacemaker::FRAME_OVERHEAD as usize)
        .checked_mul(2)?
        .checked_add(iroha_data_model::sumeragi_finality::MAX_RESULT_PREIMAGE_BYTES)?
        .checked_add(iroha_sumeragi::availability::MAX_AVAILABILITY_FRAME_BYTES)?
        .checked_add(128)?
        .checked_add(ChargedShared::<[ChargedBuffer<u8>; 4]>::allocation_layout().size())?;
    let decode_controls = norito::core::PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .try_fold(0usize, |sum, layout| sum.checked_add(layout.size()))?;
    let source = iroha_data_model::block::SharedSignedBlock::allocation_layout()
        .size()
        .checked_add(decode_controls)?
        .checked_add(std::mem::size_of::<NativeCarrierAcquisition<'_>>())?
        .checked_add(ChargedShared::<ChargedBuffer<u8>>::allocation_layout().size())?
        .checked_add(certificate)?;
    source
        .checked_mul(2)?
        .checked_add(std::mem::size_of::<QueryCertifiedChain<'_>>())
}

fn query_committee_admission(
    bytes: usize,
) -> Result<(), crate::execution_attempt::ExecutionDeferred> {
    state_certificate::query_scratch_admission(bytes)
}

fn epoch_clone_bytes(epoch: &ValidatorEpochContextV1) -> Result<usize, norito::Error> {
    let ValidatorEpochContextV1 {
        version: _,
        network_id: _,
        mode: _,
        authorization: _,
        committee,
        leader_seed: _,
        da_layout: _,
    } = epoch;
    let mut bytes = committee
        .len()
        .checked_mul(std::mem::size_of::<
            iroha_data_model::sumeragi::epoch::ValidatorCommitteeMemberV1,
        >())
        .ok_or(norito::Error::LengthMismatch)?;
    for member in committee {
        bytes = bytes
            .checked_add(
                member
                    .validator
                    .public_key()
                    .retained_allocation_layout()
                    .size(),
            )
            .and_then(|bytes| bytes.checked_add(member.proof_of_possession.len()))
            .ok_or(norito::Error::LengthMismatch)?;
    }
    Ok(bytes)
}

// The same closed native constructors retain exactly these deep families. Their
// original request admissions remain unchanged; this independent prepayment is
// held through the final persistent owner. Parsed points and canonical PoP bytes
// are inline in the credential Arc layout measured by the crypto owner.
pub(super) fn authority_retention_bytes(
    epoch: &ValidatorEpochContextV1,
) -> Result<usize, ExecutionDeferred> {
    use crate::sumeragi::crypto::QueryCommitteeError;
    let crypto = crate::sumeragi::crypto::query_committee_allocation_bytes(&epoch.committee)
        .map_err(|error| match error {
            QueryCommitteeError::Resource(original) => original,
            QueryCommitteeError::Member { .. } => retained_query_authority::invariant(),
        })?;
    epoch_clone_bytes(epoch)
        .ok()
        .and_then(|epoch_bytes| {
            core_keys_bytes(epoch.committee.len()).and_then(|keys| epoch_bytes.checked_add(keys))
        })
        .and_then(|bytes| bytes.checked_add(crypto))
        .ok_or_else(|| iroha_allocation::AllocationRefusal::DemandOverflow.into())
}

/// Maximum retained backing of one successfully validated native query authority.
///
/// The closed owner has only epoch material, core keys and the crypto constructor's
/// backing; its inline fields live in the Arc shell. Geometric Vec allowances
/// include spare `PoP` capacity. The post-validation gate checks actual material
/// capacities before escape; this bound grants no protocol or allocation authority.
pub fn native_finality_authority_bytes() -> Option<usize> {
    use iroha_crypto::bls::consensus;
    use iroha_data_model::sumeragi::epoch::{MAX_VALIDATORS, ValidatorCommitteeMemberV1};
    let committee = vector_capacity_series(MAX_VALIDATORS)?
        .checked_mul(std::mem::size_of::<ValidatorCommitteeMemberV1>())?;
    let member = consensus::PUBLIC_KEY_LEN
        .checked_add(std::mem::size_of::<u8>())?
        .checked_add(vector_capacity_series(consensus::SIGNATURE_LEN)?)?;
    committee
        .checked_add(MAX_VALIDATORS.checked_mul(member)?)?
        .checked_add(core_keys_bytes(MAX_VALIDATORS)?)?
        .checked_add(crate::sumeragi::crypto::maximum_query_committee_allocation_bytes()?)?
        .checked_add(norito::core::owned_arc_allocation_bytes::<VerifiedAuthority>().ok()?)
}

fn epoch_owned_bytes(epoch: &ValidatorEpochContextV1) -> Option<usize> {
    let ValidatorEpochContextV1 {
        version: _,
        network_id: _,
        mode: _,
        authorization: _,
        committee,
        leader_seed: _,
        da_layout: _,
    } = epoch;
    committee.iter().try_fold(
        committee.capacity().checked_mul(std::mem::size_of::<
            iroha_data_model::sumeragi::epoch::ValidatorCommitteeMemberV1,
        >())?,
        |bytes, member| {
            bytes
                .checked_add(
                    member
                        .validator
                        .public_key()
                        .retained_allocation_layout()
                        .size(),
                )?
                .checked_add(member.proof_of_possession.capacity())
        },
    )
}

fn checked_authority_retention(
    material: &ValidatorEpochContextV1,
    maximum: usize,
) -> Result<(), ExecutionAttemptError<ChainReadError>> {
    if authority_owned_bytes(material).is_none_or(|bytes| bytes > maximum) {
        return Err(ExecutionAttemptError::Deferred(
            retained_query_authority::invariant(),
        ));
    }
    Ok(())
}

fn authority_owned_bytes(material: &ValidatorEpochContextV1) -> Option<usize> {
    authority_retention_bytes(material)
        .ok()
        .and_then(|bytes| bytes.checked_sub(epoch_clone_bytes(material).ok()?))
        .and_then(|bytes| bytes.checked_add(epoch_owned_bytes(material)?))
        .and_then(|bytes| {
            bytes.checked_add(norito::core::owned_arc_allocation_bytes::<VerifiedAuthority>().ok()?)
        })
}

pub(super) fn admit_epoch_clone(
    epoch: &ValidatorEpochContextV1,
    height: u64,
) -> Result<(), ExecutionAttemptError<ChainReadError>> {
    epoch_clone_bytes(epoch)
        .and_then(norito::core::reserve_decode_allocation)
        .map_err(|error| read_decode_error(height, error))
}

pub(super) fn admit_schedule(
    outcome: &schedule::ScheduleOutcome,
    height: u64,
) -> Result<(), ExecutionAttemptError<ChainReadError>> {
    norito::core::reserve_decode_allocation(3 * std::mem::size_of::<ScheduledSlot>())
        .map_err(|error| read_decode_error(height, error))?;
    admit_epoch_clone(&outcome.current, height)?;
    for slot in [&outcome.next, &outcome.after_next] {
        if let ScheduledSlot::Ready(config) = slot {
            admit_epoch_clone(&config.epoch, height)?;
        }
    }
    Ok(())
}

pub(super) fn admit_height_config(
    epoch: &ValidatorEpochContextV1,
    height: u64,
) -> Result<(), ExecutionAttemptError<ChainReadError>> {
    let bytes = core_keys_bytes(epoch.committee.len())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<iroha_sumeragi::types::EpochConfig>())
        })
        .ok_or_else(|| read_decode_error(height, norito::Error::LengthMismatch))?;
    norito::core::reserve_decode_allocation(bytes).map_err(|error| read_decode_error(height, error))
}

// Result-collect does not promise an exact-size Vec allocation. Cover the
// complete geometric capacity series, including the minimum nonzero capacity.
fn core_keys_bytes(count: usize) -> Option<usize> {
    vector_capacity_series(count)?
        .checked_mul(std::mem::size_of::<iroha_sumeragi::types::PublicKey>())?
        .checked_add(count.checked_mul(iroha_crypto::bls::consensus::PUBLIC_KEY_LEN)?)
}

fn vector_capacity_series(count: usize) -> Option<usize> {
    count.max(4).checked_next_power_of_two()?.checked_mul(2)
}

fn admit_genesis_root(
    genesis: &SignedBlock,
    chain_id: &ChainId,
) -> Result<(), ExecutionAttemptError<ChainReadError>> {
    use iroha_data_model::{isi::RegisterBox, transaction::Executable};
    use iroha_model_base::peer::PeerId;
    let mut count = 0usize;
    let mut bytes = 0usize;
    for transaction in genesis.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        for instruction in instructions {
            let Some(RegisterBox::Peer(register)) =
                instruction.as_any().downcast_ref::<RegisterBox>()
            else {
                continue;
            };
            count = count
                .checked_add(1)
                .ok_or_else(|| read_decode_error(1, norito::Error::LengthMismatch))?;
            bytes = bytes
                .checked_add(
                    register
                        .peer
                        .public_key()
                        .retained_allocation_layout()
                        .size(),
                )
                .and_then(|bytes| bytes.checked_add(register.pop.len()))
                .ok_or_else(|| read_decode_error(1, norito::Error::LengthMismatch))?;
        }
    }
    // BTreeMap owns copied keys/proofs. Its exact-size consuming iterator can
    // collect directly; this also covers a full geometric Vec growth series.
    let tree = norito::core::owned_btree_allocation_bytes::<PeerId, Vec<u8>>(count)
        .map_err(|error| read_decode_error(1, error))?;
    let committee = count
        .checked_mul(4)
        .and_then(|count| {
            count.max(4).checked_mul(std::mem::size_of::<
                iroha_data_model::sumeragi::epoch::ValidatorCommitteeMemberV1,
            >())
        })
        .ok_or_else(|| read_decode_error(1, norito::Error::LengthMismatch))?;
    let chain_bytes = chain_id.as_ref().len();
    let instance = iroha_sumeragi::preimage::TAG_INSTANCE
        .len()
        .checked_add(32 + 5)
        .and_then(|bytes| bytes.checked_add(chain_bytes))
        .ok_or_else(|| read_decode_error(1, norito::Error::LengthMismatch))?;
    let bytes = bytes
        .checked_add(tree)
        .and_then(|bytes| bytes.checked_add(committee))
        .and_then(|bytes| bytes.checked_add(chain_bytes))
        .and_then(|bytes| bytes.checked_add(instance))
        .ok_or_else(|| read_decode_error(1, norito::Error::LengthMismatch))?;
    norito::core::reserve_decode_allocation(bytes).map_err(|error| read_decode_error(1, error))
}

/// One bounded query lifetime over the existing native verifier. Large decoded
/// bodies never become persistent prefix authority; the original source fence and
/// checked successor fields are retained instead.
pub struct QueryCertifiedChain<'v> {
    pub(super) source: ChainSource<'v, StateView<'v>>,
    pub(super) genesis_epoch: ValidatorEpochContextV1,
    pub(super) instance: Hash32,
    pub(super) root_scope: SumeragiRootScope,
    pub(super) prefix: parking_lot::Mutex<Option<QueryPrefix>>,
    pub(super) proof_source_genesis: Option<(u64, Hash)>,
    pub(super) query_cache: parking_lot::Mutex<Option<QueryCacheOwner>>,
}

pub(super) struct QueryPrefix {
    pub(super) tip: ParentProjection,
    pub(super) wire: (u64, Hash),
    pub(super) schedule: schedule::ConsensusSchedule,
    pub(super) authority: PrefixAuthority,
    pub(super) validation: EpochValidationScope,
    pub(super) proof_source: Option<Hash>,
}
impl QueryPrefix {
    fn borrowed(&mut self) -> PrefixRef<'_> {
        PrefixRef {
            tip: self.tip,
            schedule: &self.schedule,
            authority: &self.authority,
            validation: &mut self.validation,
            proof_source: self.proof_source,
        }
    }
}

impl<'v> QueryCertifiedChain<'v> {
    pub(crate) fn new(
        chain_id: &'v ChainId,
        network: &'v NetworkId,
        hashes: &'v dyn crate::state::BlockHashRead,
        kura: &'v Kura,
        budget: &iroha_allocation::AllocationBudget,
        maximum_wire_bytes: usize,
    ) -> Result<Self, ExecutionAttemptError<ChainReadError>> {
        let source = ChainSource::Pinned {
            chain_id,
            network,
            hashes: PinnedHashes::Captured(hashes),
            kura,
            budget: budget.clone(),
            maximum_wire_bytes,
        };
        let genesis = source.block(GENESIS_HEIGHT)?;
        admit_genesis_root(&genesis, chain_id)?;
        let (genesis_epoch, instance, root_scope) =
            authenticate_genesis(&genesis, network, chain_id)?;
        let proof_source_genesis = genesis.canonical_wire_identity().ok();
        // The authenticated epoch and source identity are sufficient for all later
        // reads. No decoded transaction body survives this constructor stage.
        drop(genesis);
        Ok(Self {
            source,
            genesis_epoch,
            instance,
            root_scope,
            prefix: parking_lot::Mutex::new(None),
            proof_source_genesis,
            query_cache: parking_lot::Mutex::new(None),
        })
    }

    fn original(
        &self,
        height: u64,
    ) -> Result<
        (
            iroha_data_model::block::SharedSignedBlock,
            crate::kura::NativeFrameBytes,
        ),
        ExecutionAttemptError<ChainReadError>,
    > {
        let ChainSource::Pinned {
            hashes,
            kura,
            budget,
            maximum_wire_bytes,
            ..
        } = &self.source
        else {
            unreachable!("closed query source")
        };
        let index = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(ChainReadError::NotCommitted { height })?;
        let expected = hashes
            .get(index.get() - 1)
            .ok_or(ChainReadError::NotCommitted { height })?;
        let mut original = NativeCarrierAcquisition::new(
            kura,
            index,
            *expected,
            budget.clone(),
            *maximum_wire_bytes,
        );
        let block = original.complete()?;
        let bytes = original.take_delivered_bytes().ok_or_else(|| {
            ExecutionAttemptError::Deferred(retained_query_authority::invariant())
        })?;
        Ok((block, bytes))
    }

    fn budget(&self) -> &iroha_allocation::AllocationBudget {
        let ChainSource::Pinned { budget, .. } = &self.source else {
            unreachable!("closed query source")
        };
        budget
    }

    fn context(&self) -> PrefixVerifierContext {
        PrefixVerifierContext {
            instance: self.instance,
            root_scope: self.root_scope,
        }
    }

    pub(crate) fn proof_source_cut(&self) -> Option<(u64, Hash)> {
        let cursor = self.prefix.lock();
        let prefix = cursor.as_ref()?;
        Some((prefix.tip.height, prefix.proof_source?))
    }
}

impl VerifiedAuthority {
    pub(super) fn new_query(
        material: ValidatorEpochContextV1,
        height: u64,
        validation: &mut EpochValidationScope,
    ) -> Result<Self, ExecutionAttemptError<ChainReadError>> {
        use super::super::crypto::QueryCommitteeError;
        let retained =
            authority_retention_bytes(&material).map_err(ExecutionAttemptError::Deferred)?;
        let shell = norito::core::owned_arc_allocation_bytes::<VerifiedAuthority>()
            .map_err(|error| read_decode_error(height, error))?;
        if retained
            .checked_add(shell)
            .is_none_or(|bytes| bytes > crate::sumeragi::finality::metadata_generation_bytes())
        {
            return Err(ExecutionAttemptError::Deferred(
                retained_query_authority::invariant(),
            ));
        }

        let crypto =
            BlsCrypto::for_query_committee(&material.committee, &mut query_committee_admission)
                .map_err(|error| match error {
                    QueryCommitteeError::Resource(error) => ExecutionAttemptError::Deferred(error),
                    QueryCommitteeError::Member { index, error } => {
                        use super::super::crypto::KeyError;
                        let detail = match &error {
                            KeyError::Malformed(text) | KeyError::BadPop(text) => text.len(),
                            KeyError::NotBlsNormal(_) => 0,
                        };
                        let bytes = detail
                            .checked_add(128)
                            .and_then(|bytes| bytes.checked_mul(2));
                        match bytes
                            .ok_or(norito::Error::LengthMismatch)
                            .and_then(norito::core::reserve_decode_allocation)
                        {
                            Ok(()) => ChainReadError::Committee {
                                height,
                                reason: format!("member {index}: {error}"),
                            }
                            .into(),
                            Err(error) => read_decode_error(height, error),
                        }
                    }
                })?;
        // The canonical committee owns one Vec of core keys and each key's bytes.
        let bytes = core_keys_bytes(material.committee.len())
            .ok_or_else(|| read_decode_error(height, norito::Error::LengthMismatch))?;
        norito::core::reserve_decode_allocation(bytes)
            .map_err(|error| read_decode_error(height, error))?;
        let malformed = |reason: String| ChainReadError::Committee { height, reason };
        let epoch = validation
            .core_epoch(&material)
            .map_err(|error| malformed(error.to_string()))?
            .id;
        let keys = material
            .committee
            .iter()
            .map(|member| core_key(member.validator.public_key()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| malformed(error.to_string()))?;
        let committee =
            schedule::global_committee(keys).map_err(|error| malformed(error.to_string()))?;
        // Preserve all original key, PoP, epoch and committee failure ordering.
        // Only this successfully validated closed graph may use the smaller
        // retained-authority allowance; malformed material never gains it.
        checked_authority_retention(
            &material,
            native_finality_authority_bytes().ok_or_else(|| {
                ExecutionAttemptError::Deferred(retained_query_authority::invariant())
            })?,
        )?;
        Ok(Self {
            material,
            epoch,
            committee,
            crypto,
        })
    }
}

impl QueryCertifiedChain<'_> {
    /// Stage original checked DATA, retire its decoded body, then run the shared
    /// parent/QC/availability kernel. The callback receives no unchecked authority.
    pub(crate) fn with_verified_source<R, E>(
        &self,
        height: u64,
        progress: &mut impl FnMut() -> Result<(), E>,
        workspace: &FinalityDecodeWorkspace,
        finish: impl FnOnce(
            crate::kura::NativeFrameBytes,
            IrohaHeader,
            &[iroha_data_model::sumeragi::epoch::ValidatorCommitteeMemberV1],
            QcVerification,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<ExecutionAttemptError<ChainReadError>>,
    {
        progress()?;
        let target = workspace.target_context()?;
        let (block, raw) = target.with(|| self.original(height))?;
        progress()?;
        let mut cursor = self.prefix.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC210"))]
        {
            *cursor = None;
        }
        if cursor
            .as_ref()
            .is_none_or(|prefix| prefix.tip.height >= height)
        {
            *cursor = None;
            // Preserve the original target-source -> fresh genesis -> target C1-C5
            // order. Its actual already consumed target corridor stays reserved.
            *cursor = Some(workspace.bootstrap(&target, || self.genesis_prefix_query())?);
        }
        let prefix = cursor
            .as_mut()
            .expect("authenticated native genesis prefix");
        let (committed, payload) = target.with(|| {
            read_frame_capturing_payload(
                block,
                height,
                self.root_scope,
                &mut prefix.validation,
                None,
                Some((self.budget(), raw.len())),
            )
        })?;
        #[cfg(test)]
        workspace.note_target_cost(&target);
        progress()?;
        let wire = (raw.len() as u64, Hash::new(raw.as_slice()));
        let mut staged = StagedQueryFrame::new(committed, payload, Some(raw), wire, self.budget())?;
        // Decoded target graph is gone; only explicitly bounded metadata and exact
        // frame-pool DATA remain. This counter is never reset or granted to a peer.
        drop(target);
        let context = self.context();
        while prefix
            .tip
            .height
            .checked_add(1)
            .is_some_and(|next| next < height)
        {
            progress()?;
            let next_height = prefix.tip.height + 1;
            let next = workspace.stage(|| {
                let (body, raw) = self.original(next_height)?;
                let wire = (raw.len() as u64, Hash::new(raw.as_slice()));
                let maximum_payload = raw.len();
                // This gap is not an output. Its original source was canonically
                // decoded and remains joined at the end; raw backing can retire
                // before the exact resultless payload destination is allocated.
                drop(raw);
                progress()?;
                let (checked, payload) = read_frame_capturing_payload(
                    body,
                    next_height,
                    self.root_scope,
                    &mut prefix.validation,
                    None,
                    Some((self.budget(), maximum_payload)),
                )?;
                progress()?;
                StagedQueryFrame::new(checked, payload, None, wire, self.budget()).map_err(E::from)
            })?;
            workspace.stage(|| {
                let header = next.frame.reconstruct_header()?;
                let prepared = context.prepare_advance_source(
                    prefix.borrowed(),
                    next.source(header.as_ref()),
                    None,
                    (true, self.query_cache.lock().as_mut()),
                    None,
                    None,
                )?;
                prefix.tip = next.frame.parent();
                prefix.wire = next.wire;
                prefix.schedule = prepared.schedule;
                prefix.authority = prepared.authority;
                prefix.proof_source = prepared.proof_source;
                Ok::<_, E>(())
            })?;
        }
        progress()?;
        workspace.stage(|| {
            let header = staged.frame.reconstruct_header()?;
            let source = staged.source(header.as_ref());
            let prepared;
            let certificate = if height == GENESIS_HEIGHT {
                let frame = staged.frame.parent();
                if frame.core_hash != prefix.tip.core_hash || frame.result != prefix.tip.result {
                    return Err(ExecutionAttemptError::from(ChainReadError::ForeignGenesis).into());
                }
                context.prepare_certificate_source(source, &prefix.authority, None, None, None)?
            } else {
                prepared = context.prepare_advance_source(
                    prefix.borrowed(),
                    source,
                    None,
                    (true, self.query_cache.lock().as_mut()),
                    None,
                    None,
                )?;
                let verdict = prepared.certificate.verification;
                progress()?;
                let result = finish(
                    staged.raw.take().expect("original target frame"),
                    staged.frame.source_header(),
                    &staged.frame.schedule().current.committee,
                    verdict,
                )?;
                prefix.tip = staged.frame.parent();
                prefix.wire = staged.wire;
                prefix.schedule = prepared.schedule;
                prefix.authority = prepared.authority;
                prefix.proof_source = prepared.proof_source;
                return Ok(result);
            };
            progress()?;
            finish(
                staged.raw.take().expect("original target frame"),
                staged.frame.source_header(),
                &staged.frame.schedule().current.committee,
                certificate.verification,
            )
        })
    }

    fn genesis_prefix_query(&self) -> Result<QueryPrefix, ExecutionAttemptError<ChainReadError>> {
        let mut validation = EpochValidationScope::new();
        let (genesis, raw) = self.original(GENESIS_HEIGHT)?;
        let tip =
            read_frame_with_validation(genesis, GENESIS_HEIGHT, self.root_scope, &mut validation)?;
        admit_epoch_clone(&self.genesis_epoch, GENESIS_HEIGHT)?;
        let (schedule, authority) = {
            let mut cache = self.query_cache.lock();
            let reservation = cache
                .as_mut()
                .and_then(|owner| owner.reserve_authority(&self.genesis_epoch));
            genesis_prefix_authority_query(
                &tip,
                self.genesis_epoch.clone(),
                &mut validation,
                cache.as_mut(),
                reservation,
            )?
        };
        let wire = (raw.len() as u64, Hash::new(raw.as_slice()));
        let proof_source = self
            .proof_source_genesis
            .filter(|expected| *expected == wire)
            .map(|(length, wire)| {
                proof_source_append(proof_source_start(), GENESIS_HEIGHT, length, wire)
            });
        Ok(QueryPrefix {
            tip: ParentProjection::from_original(&tip),
            wire,
            schedule,
            authority,
            validation,
            proof_source,
        })
    }
}

/// Unauthenticated DATA only. It cannot construct a prefix or a returned proof;
/// only `with_verified_source` can lend it to the shared verification kernel.
struct StagedQueryFrame {
    frame: query_projection::CheckedFrameProjection,
    payload: Option<iroha_sumeragi::availability::PayloadBytes>,
    raw: Option<crate::kura::NativeFrameBytes>,
    wire: (u64, Hash),
}
impl StagedQueryFrame {
    fn new(
        original: CommittedBlock,
        payload: Option<iroha_sumeragi::availability::PayloadBytes>,
        raw: Option<crate::kura::NativeFrameBytes>,
        wire: (u64, Hash),
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<Self, ExecutionAttemptError<ChainReadError>> {
        let frame = query_projection::CheckedFrameProjection::from_original(original, budget)?;
        Ok(Self {
            frame,
            payload,
            raw,
            wire,
        })
    }
    fn source<'a>(&'a self, header: Option<&'a PrefixHeader>) -> CertificateSource<'a> {
        CertificateSource::Projected {
            frame: &self.frame,
            header,
            payload: self.payload.as_ref(),
            wire: self.wire,
        }
    }
}

/// Capture the existing C1 resultless writer into exact original-pool bytes once.
/// Header/QC/availability authority is still checked by the same later kernels.
pub(super) fn capture_payload(
    block: &SignedBlock,
    length: usize,
    maximum: usize,
    budget: &iroha_allocation::AllocationBudget,
    height: u64,
) -> Result<iroha_sumeragi::availability::PayloadBytes, ExecutionAttemptError<ChainReadError>> {
    use crate::sumeragi::driver::payload_build::{PayloadBuild, PayloadBuildError};
    let build = PayloadBuild::new(block, budget.clone(), maximum.min(MAX_PROPOSAL_BYTES));
    build
        .finish(
            |_| Ok(length),
            |block, writer| block.write_resultless_proposal_wire(writer),
        )
        .map(|(_, payload)| payload)
        .map_err(|(original, error)| {
            drop(original);
            match error {
                PayloadBuildError::Admission(error) => byte_refusal(error),
                PayloadBuildError::Encoding(error) => read_decode_error(height, error),
                PayloadBuildError::TooLarge | PayloadBuildError::Poisoned => {
                    ChainReadError::Malformed {
                        height,
                        reason: "original payload exceeds its enclosing native frame".into(),
                    }
                    .into()
                }
            }
        })
}

fn byte_refusal(
    error: iroha_sumeragi::message::ByteAdmissionError,
) -> ExecutionAttemptError<ChainReadError> {
    use iroha_sumeragi::message::ByteAdmissionError;
    let deferred = match error {
        ByteAdmissionError::Buffer(error) => retained_query_authority::buffer_refusal(error),
        ByteAdmissionError::ControlAdmission(error) => error.into(),
        ByteAdmissionError::ControlAllocation(error) => {
            retained_query_authority::shared_refusal(error)
        }
        ByteAdmissionError::Length { .. } | ByteAdmissionError::ForeignBudget => {
            retained_query_authority::invariant()
        }
    };
    ExecutionAttemptError::Deferred(deferred)
}
