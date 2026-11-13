//! Body-independent checkpoint ancestry from original signed systematic RS16 rows.
//!
//! A row prefix proves the proposal's original header, never an executed body. The next
//! proposal authenticates its parent's executed header hash; its quorum-certified pre-state
//! root authenticates that parent's published State. Native full-body custody is not created.

use super::*;
use crate::block::{MAX_PROPOSAL_PREFIX_BYTES, parse_proposal_prefix};
use crate::da::{commitment::DaCommitmentBundle, pin_intent::DaPinIntentBundle};
use iroha_primitives::erasure::rs16::compact::CompactShape;
use iroha_sumeragi::{
    availability::{
        AvailableBody, MAX_AVAILABILITY_FRAME_BYTES, MAX_DA_CHUNK_COUNT, MAX_DA_CHUNK_SIZE_BYTES,
        verify_availability,
    },
    types::HeightConfig,
};
use std::sync::Arc;

/// One bounded original DA projection plus certificate/table and minimal prefix rows.
pub const MAX_PORTABLE_PREFIX_FRAME_BYTES: usize = MAX_FINALITY_BLOCK_BYTES + 1024 * 1024;

/// One actual systematic row and its complete original manifest coordinates.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema, norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::PortablePrefixRowV1")]
pub struct PortablePrefixRowV1 {
    /// Exact signed consensus instance.
    pub instance: Hash32,
    /// Exact manifest height.
    pub height: u64,
    /// Exact core-header hash bound by the original row signature.
    pub block_hash: Hash32,
    /// Canonical codeword coordinate; parity rows are never prefix evidence.
    pub index: u32,
    /// Actual original row, including canonical terminal zero padding if present.
    pub bytes: Vec<u8>,
}

/// Original certificate and only the systematic rows covering its proposal-header prefix.
/// These are untrusted DATA until a genesis-rooted [`PortablePrefixVerifierV1`] accepts them.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema, norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::PortablePrefixFrameV1")]
pub struct PortablePrefixFrameV1 {
    /// Original exact-quorum certificate components, with no block body.
    pub certificate: SumeragiCommitCertificateV1,
    /// Canonical original `AvailabilityFrame` envelope.
    pub availability: Vec<u8>,
    /// Executed parent header, authenticated only by this proposal's signed `prev_block_hash`.
    pub executed_parent_header: BlockHeader,
    /// Exact parent's DA commitments, authenticated by the executed header's Merkle commitment.
    pub parent_da_commitments: Option<DaCommitmentBundle>,
    /// Exact parent's DA pin intents, authenticated by the executed header's Merkle commitment.
    pub parent_da_pin_intents: Option<DaPinIntentBundle>,
    /// Exact minimal ordered systematic rows, starting at payload offset zero.
    pub rows: Vec<PortablePrefixRowV1>,
}

impl PortablePrefixFrameV1 {
    /// Attach original parent DA projections; this does not authenticate the supplied values.
    #[must_use]
    pub fn with_parent_da_bundles(
        mut self,
        commitments: Option<DaCommitmentBundle>,
        pin_intents: Option<DaPinIntentBundle>,
    ) -> Self {
        self.parent_da_commitments = commitments;
        self.parent_da_pin_intents = pin_intents;
        self
    }

    fn shape(&self) -> Result<(), FinalityError> {
        self.certificate.validate_shape()?;
        need(
            !self.availability.is_empty()
                && self.availability.len() <= MAX_AVAILABILITY_FRAME_BYTES + 128
                && self.rows.len() <= MAX_DA_CHUNK_COUNT as usize,
            "portable prefix table or row count exceeds its bound",
        )?;
        let mut total = 0usize;
        for row in &self.rows {
            need(
                !row.bytes.is_empty() && row.bytes.len() <= MAX_DA_CHUNK_SIZE_BYTES as usize,
                "portable prefix row length exceeds its bound",
            )?;
            total = total
                .checked_add(row.bytes.len())
                .ok_or_else(|| FinalityError("portable prefix row lengths overflow".into()))?;
        }
        need(
            total <= MAX_PROPOSAL_PREFIX_BYTES + MAX_DA_CHUNK_SIZE_BYTES as usize,
            "portable prefix carries bytes beyond its bounded prefix",
        )?;
        need(
            self.parent_da_commitments
                .as_ref()
                .is_none_or(|bundle| !bundle.is_empty())
                && self
                    .parent_da_pin_intents
                    .as_ref()
                    .is_none_or(|bundle| !bundle.is_empty())
                && da_projection_bytes(self)? <= MAX_FINALITY_BLOCK_BYTES,
            "portable parent DA projection is empty or exceeds the canonical block bound",
        )
    }

    /// Encode one bounded proof frame without claiming that it has been authenticated.
    ///
    /// # Errors
    /// Oversized components, invalid row geometry or a transport allocation refusal.
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, FinalityReadError> {
        self.shape()?;
        need(
            norito::canonical_frame_len(self).map_err(malformed)?
                <= MAX_PORTABLE_PREFIX_FRAME_BYTES,
            "portable prefix frame exceeds transport bound",
        )?;
        norito::encode_canonical(self).map_err(|error| malformed(error).into())
    }

    /// Decode one bounded canonical frame; this creates no verified prefix or body custody.
    ///
    /// # Errors
    /// Noncanonical or oversized bytes and original decoder resource refusals.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, FinalityReadError> {
        need(
            !bytes.is_empty() && bytes.len() <= MAX_PORTABLE_PREFIX_FRAME_BYTES,
            "portable prefix frame exceeds transport bound",
        )?;
        let frame: Self = norito::decode_canonical_for_admission(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(FinalityReadError::from)?;
        frame.shape()?;
        Ok(frame)
    }
}

/// A parent execution authenticated by a contiguous successor certificate and signed rows.
/// This owner exposes no transactions, output graph, full-block verdict or native source join.
#[derive(Debug, Clone)]
pub struct PortableCertifiedBlockV1 {
    header: BlockHeader,
    tip: Arc<Tip>,
    root_scope: crate::block::consensus::SumeragiRootScope,
    da: Arc<ParentDaProjection>,
}

#[derive(Debug)]
struct ParentDaProjection {
    commitments: Option<DaCommitmentBundle>,
    pin_intents: Option<DaPinIntentBundle>,
}

impl PortableCertifiedBlockV1 {
    /// Exact executed header whose hash the certified successor's original proposal names.
    #[must_use]
    pub const fn header(&self) -> &BlockHeader {
        &self.header
    }
    /// Exact certified core header, absent only for signed genesis.
    #[must_use]
    pub fn core_header(&self) -> Option<&CoreHeader> {
        self.tip.core_header.as_ref()
    }
    /// Certified consensus identity (the independently selected Iroha hash for genesis).
    #[must_use]
    pub fn core_hash(&self) -> Hash32 {
        self.tip.core_hash
    }
    /// Exact predecessor result hash bound by the successor.
    #[must_use]
    pub fn result(&self) -> Hash32 {
        self.tip.result
    }
    /// Exact certified execution and historical schedule, without its executed body.
    #[must_use]
    pub fn certified_result(&self) -> &CertifiedResultV1 {
        &self.tip.certified_result
    }
    /// Root scope selected by the original authenticated signed genesis.
    #[must_use]
    pub const fn root_scope(&self) -> crate::block::consensus::SumeragiRootScope {
        self.root_scope
    }
    /// Original DA commitments bound by this exact executed header, without body custody.
    #[must_use]
    pub fn da_commitments(&self) -> Option<&DaCommitmentBundle> {
        self.da.commitments.as_ref()
    }
    /// Original DA pin intents bound by this exact executed header, without body custody.
    #[must_use]
    pub fn da_pin_intents(&self) -> Option<&DaPinIntentBundle> {
        self.da.pin_intents.as_ref()
    }
}

/// Sealed checkpoint H and undo H-1 context, certified through H+1 without pre-H bodies.
#[derive(Debug, Clone)]
pub struct VerifiedPortableCheckpointV1 {
    network: NetworkId,
    chain: Arc<str>,
    checkpoint: PortableCertifiedBlockV1,
    predecessor: Option<PortableCertifiedBlockV1>,
    published_root: KeyedStateRoot,
    successor: Arc<Tip>,
}

impl VerifiedPortableCheckpointV1 {
    /// Independently selected genesis-derived network.
    #[must_use]
    pub const fn network_id(&self) -> NetworkId {
        self.network
    }
    /// Exact selected chain label.
    #[must_use]
    pub fn chain_id(&self) -> &str {
        &self.chain
    }
    /// Checkpoint H's authenticated executed-header and schedule context.
    #[must_use]
    pub const fn checkpoint(&self) -> &PortableCertifiedBlockV1 {
        &self.checkpoint
    }
    /// Undo H-1's authenticated executed-header and schedule context, absent only at H=1.
    #[must_use]
    pub const fn predecessor(&self) -> Option<&PortableCertifiedBlockV1> {
        self.predecessor.as_ref()
    }
    /// Complete published State P_H, from H+1's certified pre-state, never H's execution root.
    #[must_use]
    pub const fn published_root(&self) -> KeyedStateRoot {
        self.published_root
    }
    /// Actual successor consensus header whose QC anchors this checkpoint.
    #[must_use]
    pub fn successor_header(&self) -> &CoreHeader {
        self.successor
            .core_header
            .as_ref()
            .expect("non-genesis successor")
    }
    /// Exact H+1 result and historical schedule, for a separately checked native resume join.
    #[must_use]
    pub fn successor_result(&self) -> &CertifiedResultV1 {
        &self.successor.certified_result
    }
    /// Exact successor consensus-header identity already authenticated by its quorum.
    #[must_use]
    pub fn successor_core_hash(&self) -> Hash32 {
        self.successor.core_hash
    }
    /// Exact successor result identity already authenticated by its quorum.
    #[must_use]
    pub fn successor_result_hash(&self) -> Hash32 {
        self.successor.result
    }
}

#[derive(Debug)]
struct Tip {
    core_header: Option<CoreHeader>,
    core_hash: Hash32,
    result: Hash32,
    certified_result: CertifiedResultV1,
}

/// Constant-history portable verifier; only the selected genesis and completed prefix govern it.
/// A failed verification leaves its tip and historical authority unchanged.
pub struct PortablePrefixVerifierV1 {
    network: NetworkId,
    chain: Arc<str>,
    instance: Hash32,
    scope: crate::block::consensus::SumeragiRootScope,
    genesis_hash: HashOf<BlockHeader>,
    schedule: ConsensusSchedule,
    tip: Arc<Tip>,
    last_parent: Option<PortableCertifiedBlockV1>,
}

impl PortablePrefixVerifierV1 {
    /// Start at an independently authenticated signed genesis and a candidate genesis result.
    /// The result grants no execution authority until height two certifies its exact hash.
    /// Signed genesis alone supplies the initial committee and both lag-two parameter slots.
    ///
    /// # Errors
    /// Malformed result, changed signed authority/parameters, height or genesis timestamp.
    pub fn new(
        selected: &SumeragiFinalityVerifier,
        genesis_result_preimage: &[u8],
    ) -> Result<Self, FinalityReadError> {
        let scope = selected.root_scope();
        let certified_result = CertifiedResultV1::decode(genesis_result_preimage, scope.into())
            .map_err(FinalityReadError::from)?;
        certified_result
            .check_block(1, selected.genesis.header().creation_time_ms)
            .map_err(malformed)?;
        need(
            certified_result.commitment().schedule.current == *selected.initial_epoch(),
            "portable genesis result changes signed initial authority",
        )?;
        let schedule =
            ConsensusSchedule::from_genesis_outcome(&certified_result.commitment().schedule)
                .map_err(malformed)?;
        let initial = selected
            .initial_chain_parameters()
            .map_err(FinalityReadError::from)?;
        need(
            schedule.ready(2).map_err(malformed)?.params == initial
                && schedule.ready(3).map_err(malformed)?.params == initial,
            "portable genesis result changes signed initial parameters",
        )?;
        let result = result_of_preimage(genesis_result_preimage)
            .ok_or_else(|| FinalityError("invalid portable genesis result".into()))?;
        norito::core::classify_decode_attempt(|| {
            norito::core::reserve_decode_arc_allocation::<Tip>()?;
            norito::core::reserve_decode_allocation(selected.chain_id().len())
        })
        .map_err(FinalityReadError::from)?;
        Ok(Self {
            network: NetworkId::from_genesis_hash(selected.genesis.hash()),
            chain: Arc::from(selected.chain_id()),
            instance: selected.instance(),
            scope,
            genesis_hash: selected.genesis.hash(),
            schedule,
            tip: Arc::new(Tip {
                core_header: None,
                core_hash: Hash32(Hash::from(selected.genesis.hash()).into()),
                result,
                certified_result,
            }),
            last_parent: None,
        })
    }

    /// Highest original certificate accepted; height one alone is not an execution anchor.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.tip.certified_result.commitment().height
    }

    fn prepare(&self, frame: &PortablePrefixFrameV1) -> Result<Prepared, FinalityReadError> {
        frame.shape()?;
        let header: CoreHeader = decode_component(&frame.certificate.consensus_header)?;
        let qc: Qc = decode_component(&frame.certificate.commit_qc)?;
        let result =
            CertifiedResultV1::decode(&frame.certificate.result_preimage, self.scope.into())
                .map_err(FinalityReadError::from)?;
        let expected_height = self
            .height()
            .checked_add(1)
            .ok_or_else(|| FinalityError("portable prefix height exhausted".into()))?;
        need(
            header.height == expected_height,
            "portable prefix is not the next height",
        )?;
        let selected = self.schedule.ready(expected_height).map_err(malformed)?;
        let config = selected.height_config().map_err(malformed)?;
        need(
            result.commitment().height == expected_height
                && result.commitment().schedule.current == selected.epoch
                && (self.scope == crate::block::consensus::SumeragiRootScope::Global
                    || !result.header().is_active()),
            "portable result changes its selected height, scope or committee",
        )?;
        let schedule = self
            .schedule
            .advanced(&result.commitment().schedule)
            .map_err(malformed)?;
        let validators = selected
            .epoch
            .committee
            .iter()
            .map(|member| FinalityValidator {
                public_key: member.validator.public_key().clone(),
                proof_of_possession: member.proof_of_possession.clone(),
            })
            .collect::<Vec<_>>();
        let (crypto, committee) = ProofCrypto::new(&validators)?;
        let core_hash = header.hash(&crypto);
        let result_hash = result_of_preimage(&frame.certificate.result_preimage)
            .ok_or_else(|| FinalityError("invalid portable result preimage".into()))?;
        need(
            header.instance == self.instance
                && header.epoch == config.epoch.id
                && header.parent_hash == self.tip.core_hash
                && header.parent_result == self.tip.result
                && qc.instance == self.instance
                && qc.epoch == config.epoch.id
                && qc.kind == VoteKind::Commit
                && qc.height == header.height
                && qc.block_hash == core_hash
                && qc.result == result_hash,
            "portable certificate changes instance, parent, result or selected epoch",
        )?;
        iroha_sumeragi::crypto::Verifier::new(
            &crypto,
            &self.instance,
            &config.epoch.id,
            &committee,
        )
        .verify_qc(&qc)
        .map_err(|error| FinalityError(format!("portable commit certificate: {error:?}")))?;
        let availability: AvailabilityFrame = decode_component(&frame.availability)?;
        let verified = verify_availability(
            self.instance,
            &config,
            &header,
            availability.as_slice(),
            &crypto,
        )
        .map_err(|error| FinalityError(format!("portable availability: {error:?}")))?;
        let shape = verified.shape();
        let mut row_commitments = Vec::new();
        let bytes = shape.chunk_count() * std::mem::size_of::<Hash32>();
        norito::core::classify_decode_attempt(|| {
            norito::core::reserve_decode_allocation(bytes)?;
            row_commitments
                .try_reserve_exact(shape.chunk_count())
                .map_err(|_| norito::Error::AllocationFailed {
                    bytes: bytes as u64,
                })?;
            Ok(())
        })
        .map_err(FinalityReadError::from)?;
        for index in 0..shape.chunk_count() {
            row_commitments.push(
                verified
                    .row_commitment(index)
                    .expect("verified bounded index"),
            );
        }
        Ok(Prepared {
            header,
            core_hash,
            result_hash,
            result,
            schedule,
            config,
            crypto,
            availability,
            shape,
            row_commitments,
        })
    }

    /// Verify exactly the next certificate and minimal row prefix, then publish its parent.
    ///
    /// # Errors
    /// Missing/extra/corrupt/source-substituted rows, invalid certificates, changed historical
    /// authority or any broken parent/schedule/boundary binding; decoder refusals stay typed.
    pub fn verify(
        &mut self,
        frame: &PortablePrefixFrameV1,
    ) -> Result<VerifiedPortableCheckpointV1, FinalityReadError> {
        let prepared = self.prepare(frame)?;
        let proposal = verify_rows(&prepared, &frame.rows)?;
        let parent = &frame.executed_parent_header;
        need(
            proposal.height().get() == prepared.header.height
                && proposal.prev_block_hash() == Some(parent.hash())
                && parent.height().get() == self.height()
                && (self.height() != 1 || parent.hash() == self.genesis_hash),
            "portable successor does not bind the exact executed parent header",
        )?;
        self.tip
            .certified_result
            .check_block(parent.height().get(), parent.creation_time_ms)
            .map_err(malformed)?;
        prepared
            .result
            .check_block(proposal.height().get(), proposal.creation_time_ms)
            .map_err(malformed)?;
        check_beacon_and_boundary(&prepared, &proposal, parent, &self.tip.certified_result)?;
        let da = authenticate_parent_da(frame)?;
        norito::core::classify_decode_attempt(norito::core::reserve_decode_arc_allocation::<Tip>)
            .map_err(FinalityReadError::from)?;
        let parent = PortableCertifiedBlockV1 {
            header: *parent,
            tip: Arc::clone(&self.tip),
            root_scope: self.scope,
            da,
        };
        let published_root = prepared
            .result
            .commitment()
            .execution
            .parent_world_state_root;
        let next = Arc::new(Tip {
            core_header: Some(prepared.header),
            core_hash: prepared.core_hash,
            result: prepared.result_hash,
            certified_result: prepared.result,
        });
        let output = VerifiedPortableCheckpointV1 {
            network: self.network,
            chain: self.chain.clone(),
            checkpoint: parent.clone(),
            predecessor: self.last_parent.clone(),
            published_root,
            successor: Arc::clone(&next),
        };
        self.tip = next;
        self.schedule = prepared.schedule;
        self.last_parent = Some(parent);
        Ok(output)
    }

    /// Prepare immutable original coordinates for row-by-row acquisition under this prefix.
    /// The acquisition retains accepted rows and scratch across source or resource refusals.
    ///
    /// # Errors
    /// The original certificate/table fails the same historical authority and source checks.
    pub fn acquisition(
        &self,
        certificate: SumeragiCommitCertificateV1,
        availability: Vec<u8>,
        executed_parent_header: BlockHeader,
    ) -> Result<PortablePrefixAcquisitionV1, FinalityReadError> {
        let frame = PortablePrefixFrameV1 {
            certificate,
            availability,
            executed_parent_header,
            parent_da_commitments: None,
            parent_da_pin_intents: None,
            rows: Vec::new(),
        };
        let prepared = self.prepare(&frame)?;
        Ok(PortablePrefixAcquisitionV1 {
            frame,
            prepared,
            prefix: Vec::new(),
            complete: false,
        })
    }
}

struct Prepared {
    header: CoreHeader,
    core_hash: Hash32,
    result_hash: Hash32,
    result: CertifiedResultV1,
    schedule: ConsensusSchedule,
    config: HeightConfig,
    crypto: ProofCrypto,
    availability: AvailabilityFrame,
    shape: CompactShape,
    row_commitments: Vec<Hash32>,
}

fn da_projection_bytes(frame: &PortablePrefixFrameV1) -> Result<usize, FinalityError> {
    let commitments =
        norito::canonical_frame_len(&frame.parent_da_commitments).map_err(malformed)?;
    let pins = norito::canonical_frame_len(&frame.parent_da_pin_intents).map_err(malformed)?;
    commitments
        .checked_add(pins)
        .ok_or_else(|| malformed("parent DA projection size overflows"))
}

fn clone_da_bundle<T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>>(
    bundle: &Option<T>,
) -> Result<Option<T>, FinalityReadError> {
    let Some(bundle) = bundle else {
        return Ok(None);
    };
    // The output must outlive the caller's untrusted DTO. Clone through the original bounded
    // codec so nested allocations retain its inherited refusal instead of an unchecked Clone.
    let bytes = norito::core::classify_decode_attempt(|| {
        norito::core::reserve_decode_allocation(norito::canonical_frame_len(bundle)?)?;
        norito::encode_canonical(bundle)
    })
    .map_err(FinalityReadError::from)?;
    decode_component(&bytes).map(Some)
}

fn authenticate_parent_da(
    frame: &PortablePrefixFrameV1,
) -> Result<Arc<ParentDaProjection>, FinalityReadError> {
    let count = frame
        .parent_da_commitments
        .as_ref()
        .map_or(0, |bundle| bundle.commitments().len())
        .checked_add(
            frame
                .parent_da_pin_intents
                .as_ref()
                .map_or(0, |bundle| bundle.intents.len()),
        )
        .ok_or_else(|| malformed("parent DA leaf count overflows"))?;
    // Existing Merkle functions own canonical leaf encodings, domain preimages and geometric
    // hash layers. Charge their full conservative simultaneous backing before calling them.
    let scratch = da_projection_bytes(frame)?
        .checked_mul(4)
        .and_then(|bytes| {
            count
                .checked_mul(128)
                .and_then(|nodes| bytes.checked_add(nodes))
        })
        .ok_or_else(|| malformed("parent DA Merkle scratch overflows"))?;
    norito::core::classify_decode_attempt(|| norito::core::reserve_decode_allocation(scratch))
        .map_err(FinalityReadError::from)?;
    need(
        frame.executed_parent_header.da_commitments_hash()
            == frame
                .parent_da_commitments
                .as_ref()
                .and_then(DaCommitmentBundle::merkle_commitment)
            && frame.executed_parent_header.da_pin_intents_hash()
                == frame
                    .parent_da_pin_intents
                    .as_ref()
                    .and_then(DaPinIntentBundle::merkle_commitment),
        "portable parent DA projections differ from the authenticated executed header",
    )?;
    let commitments = clone_da_bundle(&frame.parent_da_commitments)?;
    let pin_intents = clone_da_bundle(&frame.parent_da_pin_intents)?;
    norito::core::classify_decode_attempt(
        norito::core::reserve_decode_arc_allocation::<ParentDaProjection>,
    )
    .map_err(FinalityReadError::from)?;
    Ok(Arc::new(ParentDaProjection {
        commitments,
        pin_intents,
    }))
}

fn decode_component<T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>>(
    bytes: &[u8],
) -> Result<T, FinalityReadError> {
    norito::decode_canonical_for_admission(bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(FinalityReadError::from)
}

fn check_beacon_and_boundary(
    prepared: &Prepared,
    proposal: &BlockHeader,
    parent: &BlockHeader,
    parent_result: &CertifiedResultV1,
) -> Result<(), FinalityError> {
    let commitment = prepared.result.commitment();
    need(
        commitment.beacon.as_ref().is_none_or(|pulse| {
            pulse.finalized_chain_anchor.block_hash == parent.hash()
                && pulse.context.instance == prepared.header.instance.0
                && pulse.context.epoch == commitment.schedule.current.authorization.epoch
                && pulse.context.epoch_context_id == prepared.header.epoch.context.0
                && pulse.context.parent_consensus_hash == prepared.header.parent_hash.0
                && pulse.context.parent_result == prepared.header.parent_result.0
        }),
        "portable beacon names another certified parent or context",
    )?;
    if let Some(boundary) = &commitment.schedule.boundary {
        need(
            Some(boundary.selection_anchor) == proposal.prev_block_hash(),
            "portable boundary changes predecessor selection anchor",
        )?;
        let pulse = parent_result.commitment().beacon.as_ref().ok_or_else(|| {
            FinalityError("portable boundary omits predecessor selection pulse".into())
        })?;
        need(
            boundary.next.leader_seed
                == global_threshold_beacon_npos_successor_seed_v1(
                    pulse,
                    prepared.header.height,
                    boundary.next.authorization.epoch,
                ),
            "portable boundary changes fresh leader seed",
        )?;
        if let Some(preparation) = &boundary.preparation {
            need(
                preparation.election_seed
                    == election_seed(
                        commitment.schedule.current.network_id,
                        commitment.schedule.current.authorization.epoch,
                        pulse,
                    )
                    .map_err(malformed)?,
                "portable boundary changes election seed",
            )?;
        }
    }
    Ok(())
}

fn next_index(prepared: &Prepared, rows: usize) -> Result<usize, FinalityError> {
    let k = usize::from(prepared.config.epoch.da_layout.data_shards);
    let n = k + usize::from(prepared.config.epoch.da_layout.parity_shards);
    (rows / k)
        .checked_mul(n)
        .and_then(|base| base.checked_add(rows % k))
        .ok_or_else(|| FinalityError("portable row index overflow".into()))
}

fn append_row(
    prepared: &Prepared,
    rows: usize,
    prefix: &mut Vec<u8>,
    row: &PortablePrefixRowV1,
) -> Result<Option<BlockHeader>, FinalityReadError> {
    let index = next_index(prepared, rows)?;
    need(
        row.instance == prepared.header.instance
            && row.height == prepared.header.height
            && row.block_hash == prepared.core_hash
            && usize::try_from(row.index).ok() == Some(index),
        "portable row substitutes its original source or ordered systematic coordinate",
    )?;
    let shape = prepared.shape;
    need(
        shape
            .chunk_range(index)
            .is_some_and(|range| range.len() == row.bytes.len())
            && prepared.row_commitments.get(index)
                == Some(&iroha_sumeragi::availability::row_digest(
                    &prepared.crypto,
                    &row.bytes,
                )),
        "portable systematic row differs from signed manifest",
    )?;
    let k = usize::from(prepared.config.epoch.da_layout.data_shards);
    let n = k + usize::from(prepared.config.epoch.da_layout.parity_shards);
    let offset = (index / n) * k * prepared.config.epoch.da_layout.chunk_size_bytes as usize
        + (index % n) * row.bytes.len();
    let actual = shape
        .payload_bytes()
        .saturating_sub(offset)
        .min(row.bytes.len());
    need(
        row.bytes[actual..].iter().all(|byte| *byte == 0),
        "portable terminal systematic padding is nonzero",
    )?;
    let keep = actual.min(MAX_PROPOSAL_PREFIX_BYTES.saturating_sub(prefix.len()));
    need(keep > 0, "portable prefix exceeds independent parser bound")?;
    norito::core::classify_decode_attempt(|| {
        norito::core::reserve_decode_allocation(keep)?;
        prefix
            .try_reserve_exact(keep)
            .map_err(|_| norito::Error::AllocationFailed { bytes: keep as u64 })?;
        Ok(())
    })
    .map_err(FinalityReadError::from)?;
    let original_len = prefix.len();
    prefix.extend_from_slice(&row.bytes[..keep]);
    match parse_proposal_prefix(prefix, shape.payload_bytes()) {
        Ok(Some(parsed)) => Ok(Some(*parsed.header())),
        Ok(None) => {
            if prefix.len() == MAX_PROPOSAL_PREFIX_BYTES || actual < row.bytes.len() {
                prefix.truncate(original_len);
                Err(
                    FinalityError("signed rows do not contain a bounded proposal header".into())
                        .into(),
                )
            } else {
                Ok(None)
            }
        }
        Err(error) => {
            prefix.truncate(original_len);
            Err(error.into())
        }
    }
}

fn verify_rows(
    prepared: &Prepared,
    rows: &[PortablePrefixRowV1],
) -> Result<BlockHeader, FinalityReadError> {
    let mut prefix = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if let Some(header) = append_row(prepared, index, &mut prefix, row)? {
            need(
                index + 1 == rows.len(),
                "portable proof includes rows after its complete prefix",
            )?;
            return Ok(header);
        }
    }
    Err(FinalityError("portable proof omits required systematic prefix rows".into()).into())
}

/// Retained source-bound acquisition of exactly the rows needed for one portable prefix.
/// Callers obtain each requested row from original custody or a peer; missing rows do not
/// trigger whole-body reconstruction or a decoded-block fallback.
pub struct PortablePrefixAcquisitionV1 {
    frame: PortablePrefixFrameV1,
    prepared: Prepared,
    prefix: Vec<u8>,
    complete: bool,
}

impl PortablePrefixAcquisitionV1 {
    /// Next exact instance, height, core hash and systematic coordinate, or None when complete.
    ///
    /// # Errors
    /// A protocol-bounded coordinate overflows the host index.
    pub fn requested_row(&self) -> Result<Option<(Hash32, u64, Hash32, u32)>, FinalityError> {
        if self.complete {
            return Ok(None);
        }
        let index =
            u32::try_from(next_index(&self.prepared, self.frame.rows.len())?).map_err(malformed)?;
        Ok(Some((
            self.prepared.header.instance,
            self.prepared.header.height,
            self.prepared.core_hash,
            index,
        )))
    }

    /// Admit one actual requested row. Failure returns that original row and preserves progress.
    ///
    /// # Errors
    /// A wrong source, coordinate, signed content, padding, canonical prefix or local refusal.
    #[expect(
        clippy::result_large_err,
        reason = "return original row ownership on refusal"
    )]
    pub fn push(
        &mut self,
        row: PortablePrefixRowV1,
    ) -> Result<(), (PortablePrefixRowV1, FinalityReadError)> {
        if self.complete {
            return Err((
                row,
                FinalityError("portable prefix is already complete".into()).into(),
            ));
        }
        let reserve = norito::core::classify_decode_attempt(|| {
            norito::core::reserve_decode_allocation(std::mem::size_of::<PortablePrefixRowV1>())?;
            self.frame
                .rows
                .try_reserve_exact(1)
                .map_err(|_| norito::Error::AllocationFailed {
                    bytes: std::mem::size_of::<PortablePrefixRowV1>() as u64,
                })?;
            Ok(())
        });
        if let Err(error) = reserve {
            return Err((row, error.into()));
        }
        match append_row(
            &self.prepared,
            self.frame.rows.len(),
            &mut self.prefix,
            &row,
        ) {
            Ok(header) => {
                self.complete = header.is_some();
                self.frame.rows.push(row);
                Ok(())
            }
            Err(error) => Err((row, error)),
        }
    }

    /// Borrow the completed original proof. Its receiving verifier still authenticates it.
    ///
    /// # Errors
    /// At least one required systematic row is missing.
    pub fn proof(&self) -> Result<&PortablePrefixFrameV1, FinalityError> {
        need(self.complete, "portable prefix acquisition is incomplete")?;
        Ok(&self.frame)
    }

    /// Move a completed proof, returning the original acquisition if rows are still absent.
    ///
    /// # Errors
    /// Returns the unchanged acquisition when at least one required row is missing.
    #[expect(
        clippy::result_large_err,
        reason = "return original acquisition and backing"
    )]
    pub fn into_proof(self) -> Result<PortablePrefixFrameV1, Self> {
        if self.complete {
            Ok(self.frame)
        } else {
            Err(self)
        }
    }

    /// Acquire requested systematic rows directly from already authenticated immutable payload
    /// custody. No body is decoded, reencoded, reconstructed or accepted by the portable reader.
    ///
    /// # Errors
    /// Foreign original custody, signature/content mismatch or a typed allocation refusal.
    pub fn acquire_from_body(&mut self, body: &AvailableBody) -> Result<(), FinalityReadError> {
        need(
            body.header() == &self.prepared.header
                && body.source().instance() == self.prepared.header.instance
                && body.source().block_hash() == self.prepared.core_hash
                && body.source().height() == self.prepared.header.height
                && body.source().config() == &self.prepared.config
                && body.availability().as_slice() == self.prepared.availability.as_slice(),
            "portable acquisition received another original body",
        )?;
        let shape = self
            .prepared
            .config
            .epoch
            .da_layout
            .shape(u64::from(self.prepared.header.payload_len))
            .map_err(malformed)?;
        let k = usize::from(self.prepared.config.epoch.da_layout.data_shards);
        let n = k + usize::from(self.prepared.config.epoch.da_layout.parity_shards);
        while let Some((instance, height, block_hash, index)) = self.requested_row()? {
            let index_usize = index as usize;
            let length = shape
                .chunk_range(index_usize)
                .ok_or_else(|| FinalityError("portable row is outside original codeword".into()))?
                .len();
            let offset = (index_usize / n)
                * k
                * self.prepared.config.epoch.da_layout.chunk_size_bytes as usize
                + (index_usize % n) * length;
            let actual = shape.payload_bytes().saturating_sub(offset).min(length);
            let source = body
                .payload()
                .as_slice()
                .get(offset..offset + actual)
                .ok_or_else(|| {
                    FinalityError("original body omits requested systematic bytes".into())
                })?;
            let mut bytes = Vec::new();
            norito::core::classify_decode_attempt(|| {
                norito::core::reserve_decode_allocation(length)?;
                bytes
                    .try_reserve_exact(length)
                    .map_err(|_| norito::Error::AllocationFailed {
                        bytes: length as u64,
                    })?;
                Ok(())
            })
            .map_err(FinalityReadError::from)?;
            bytes.extend_from_slice(source);
            bytes.resize(length, 0);
            self.push(PortablePrefixRowV1 {
                instance,
                height,
                block_hash,
                index,
                bytes,
            })
            .map_err(|(_, error)| error)?;
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "transparent_api"))]
mod tests;
