//! One immutable native input body beside its actual instance/key safety WAL.
//!
//! Physical durability is separate from source finality, current membership and
//! Ready. A receipt binds an exact origin manifest, but no view/lock state lives
//! here. Every valid origin reconstructs the same payload and RS16 codeword.
//! TODO: connect receipts to the process-lived reducer's issued storage jobs;
//! retire only after authenticated global terminal settlement and owner drain.

use std::sync::Arc;

use iroha_crypto::Hash;
use iroha_data_model::block::{
    consensus_v2 as wire,
    lane_consensus::{LaneManifestV1, LaneValueKindV1, LaneValueRefV1},
    lane_input::LaneInputPayloadV1,
};
use norito::{Decode, Encode};

use super::{
    safety_wal::SafetyWalNativeBodyStoreAuthority, v2_lane_payload::verify_lane_input_manifest,
    v2_lane_wire::LaneAuthenticator,
};
use crate::state::{VerifiedLaneContext, VerifiedLaneInputBodyV1};

const FORMAT: u16 = 1;

/// Exact canonical physical snapshot, never an authenticated source token.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::sumeragi::v2_lane_body_store::LaneBodyRecordV1")]
struct LaneBodyRecordV1 {
    version: u16,
    frozen_context_hash: Hash,
    consensus_key_hash: Hash,
    /// First stored origin; later origins must reconstruct the same bytes.
    manifest: LaneManifestV1,
    canonical_payload: Vec<u8>,
}

/// A native physical storage or exact-readback error.
#[derive(Debug, thiserror::Error)]
#[error("native lane input storage: {0}")]
pub(crate) struct LaneBodyStoreError(String);
fn bad(error: impl ToString) -> LaneBodyStoreError {
    LaneBodyStoreError(error.to_string())
}
type Result<T> = std::result::Result<T, LaneBodyStoreError>;

/// Private physical receipt for one exact requested origin and opened store.
/// It grants no source-finality, current-set, validation or signing authority.
#[derive(Debug)]
pub(crate) struct DurableLaneBodyReceipt {
    store: Arc<()>,
    frame_hash: Hash,
    manifest: LaneManifestV1,
}
impl DurableLaneBodyReceipt {
    /// Exact immutable origin whose body was read back after durability.
    pub(crate) fn manifest(&self) -> &LaneManifestV1 {
        &self.manifest
    }
}

/// Readback bytes and a physical receipt; source/live authority stays separate.
#[derive(Debug)]
pub(crate) struct DurableLaneBodyRead {
    payload: LaneInputPayloadV1,
    canonical_bytes: Vec<u8>,
    receipt: DurableLaneBodyReceipt,
}
impl DurableLaneBodyRead {
    /// Canonical payload with structural checks, not finality authentication.
    pub(crate) fn payload(&self) -> &LaneInputPayloadV1 {
        &self.payload
    }
    /// Exact immutable bytes that deterministically reconstruct the codeword.
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Physical readback receipt, still requiring the exact issued job gate.
    pub(crate) fn receipt(&self) -> &DurableLaneBodyReceipt {
        &self.receipt
    }
}

/// Sole mutable physical owner of a fixed bounded snapshot; no origin catalog.
pub(crate) struct LaneBodyStore {
    authority: SafetyWalNativeBodyStoreAuthority,
    lane: VerifiedLaneContext,
    identity: Arc<()>,
    frozen_context_hash: Hash,
    consensus_key_hash: Hash,
    maximum_frame_bytes: u64,
    publication_failed: bool,
}
impl LaneBodyStore {
    /// Construction is limited to the actual lane WAL's once-minted authority.
    pub(super) fn open(
        authority: SafetyWalNativeBodyStoreAuthority,
        lane: VerifiedLaneContext,
        signer: u32,
    ) -> Result<Self> {
        let wal_identity = LaneAuthenticator::new(&lane)
            .wal_identity(signer)
            .map_err(bad)?;
        if !authority.matches_identity(wal_identity) {
            return Err(bad(
                "native input authority belongs to another WAL identity",
            ));
        }
        let frozen_context_hash = lane.frozen().canonical_hash().map_err(bad)?;
        let consensus_key_hash = Hash::prehashed(wal_identity.consensus_key_hash());
        let maximum_frame_bytes = maximum_frame_bytes(&lane)?;
        let store = Self {
            authority,
            lane,
            identity: Arc::new(()),
            frozen_context_hash,
            consensus_key_hash,
            maximum_frame_bytes,
            publication_failed: false,
        };
        // Reopening a corrupt/foreign snapshot is not an absent-body retry.
        let _ = store.read_record()?;
        Ok(store)
    }

    /// Store only a fully joined authenticated input and its exact RS16 origin.
    /// An existing body is immutable; a later valid origin changes no file bytes.
    pub(crate) fn persist(
        &mut self,
        body: &VerifiedLaneInputBodyV1,
        manifest: &LaneManifestV1,
    ) -> Result<DurableLaneBodyRead> {
        self.publish_before_readback(body, manifest)?;
        self.finish_readback(body, manifest)
    }

    fn publish_before_readback(
        &mut self,
        body: &VerifiedLaneInputBodyV1,
        manifest: &LaneManifestV1,
    ) -> Result<()> {
        self.check_open()?;
        verify_lane_input_manifest(&self.lane, body, manifest, body.canonical_bytes())
            .map_err(bad)?;
        if let Some((record, _, _)) = self.read_record()? {
            if record.canonical_payload != body.canonical_bytes() {
                return Err(bad("an instance's durable input cannot be replaced"));
            }
        } else {
            let record = LaneBodyRecordV1 {
                version: FORMAT,
                frozen_context_hash: self.frozen_context_hash,
                consensus_key_hash: self.consensus_key_hash,
                manifest: *manifest,
                canonical_payload: body.canonical_bytes().to_vec(),
            };
            let bytes = norito::encode_canonical(&record).map_err(bad)?;
            // From the first possible write through exact successful readback,
            // an error may leave published bytes. Only reopen resolves failure.
            self.publication_failed = true;
            self.authority
                .publish_atomic(&bytes, self.maximum_frame_bytes)
                .map_err(bad)?;
        }
        Ok(())
    }

    fn finish_readback(
        &mut self,
        body: &VerifiedLaneInputBodyV1,
        manifest: &LaneManifestV1,
    ) -> Result<DurableLaneBodyRead> {
        // This sole private continuation may read while publication is fenced.
        // Every error/absence leaves that fence intact. Public reads cannot.
        let read = self
            .read_for_manifest_inner(manifest)?
            .ok_or_else(|| bad("native input persistence has no exact durable readback"))?;
        if read.canonical_bytes() != body.canonical_bytes() {
            return Err(bad("native input changed across publication/readback"));
        }
        self.publication_failed = false;
        Ok(read)
    }

    /// Recover an exact origin from the one stored body. Missing is distinct from
    /// corrupt/foreign storage. The caller owns any required first-source fetch.
    pub(crate) fn read_for_manifest(
        &self,
        manifest: &LaneManifestV1,
    ) -> Result<Option<DurableLaneBodyRead>> {
        self.check_open()?;
        self.read_for_manifest_inner(manifest)
    }

    fn read_for_manifest_inner(
        &self,
        manifest: &LaneManifestV1,
    ) -> Result<Option<DurableLaneBodyRead>> {
        let Some((record, payload, frame_hash)) = self.read_record()? else {
            return Ok(None);
        };
        validate_manifest(&self.lane, &payload, &record.canonical_payload, manifest)?;
        Ok(Some(DurableLaneBodyRead {
            payload,
            canonical_bytes: record.canonical_payload,
            receipt: DurableLaneBodyReceipt {
                store: Arc::clone(&self.identity),
                frame_hash,
                manifest: *manifest,
            },
        }))
    }

    /// Recheck the actual physical owner and exact bytes before consuming a receipt.
    /// A receipt from another owner/reopen is not interchangeable, even for equal bytes.
    pub(crate) fn validate_receipt(&self, receipt: &DurableLaneBodyReceipt) -> Result<()> {
        self.check_open()?;
        if !Arc::ptr_eq(&self.identity, &receipt.store) {
            return Err(bad("receipt belongs to another opened native input owner"));
        }
        let read = self
            .read_for_manifest(&receipt.manifest)?
            .ok_or_else(|| bad("the exact durable native input disappeared"))?;
        if read.receipt.frame_hash != receipt.frame_hash {
            return Err(bad("the exact durable native snapshot was replaced"));
        }
        Ok(())
    }

    fn check_open(&self) -> Result<()> {
        if self.publication_failed {
            return Err(bad(
                "uncertain native input publication requires physical reopen",
            ));
        }
        Ok(())
    }

    fn read_record(&self) -> Result<Option<(LaneBodyRecordV1, LaneInputPayloadV1, Hash)>> {
        let Some(bytes) = self
            .authority
            .read_bounded(self.maximum_frame_bytes)
            .map_err(bad)?
        else {
            return Ok(None);
        };
        let record: LaneBodyRecordV1 = norito::decode_canonical(&bytes).map_err(bad)?;
        if record.version != FORMAT
            || record.frozen_context_hash != self.frozen_context_hash
            || record.consensus_key_hash != self.consensus_key_hash
            || record.canonical_payload.is_empty()
            || record.canonical_payload.len() as u64
                > self.lane.frozen().da_layout.max_payload_size_bytes
        {
            return Err(bad("native snapshot differs from its frozen instance/size"));
        }
        let payload: LaneInputPayloadV1 =
            norito::decode_canonical(&record.canonical_payload).map_err(bad)?;
        validate_manifest(
            &self.lane,
            &payload,
            &record.canonical_payload,
            &record.manifest,
        )?;
        Ok(Some((record, payload, Hash::new(&bytes))))
    }

    /// Stop after the real fsynced publication, before its private readback.
    #[cfg(test)]
    pub(crate) fn publish_before_readback_for_test(
        &mut self,
        body: &VerifiedLaneInputBodyV1,
        manifest: &LaneManifestV1,
    ) -> Result<()> {
        self.publish_before_readback(body, manifest)
    }

    /// Exercise the exact production readback after an adverse filesystem cut.
    #[cfg(test)]
    pub(crate) fn finish_readback_for_test(
        &mut self,
        body: &VerifiedLaneInputBodyV1,
        manifest: &LaneManifestV1,
    ) -> Result<DurableLaneBodyRead> {
        self.finish_readback(body, manifest)
    }

    /// Exact physical frame bound for storage-boundary controls.
    #[cfg(test)]
    pub(crate) fn maximum_frame_bytes_for_test(&self) -> u64 {
        self.maximum_frame_bytes
    }

    /// Actual fixture path only; production has no pathname-based constructor.
    #[cfg(test)]
    pub(crate) fn path_for_test(&self) -> &std::path::Path {
        self.authority.path_for_test()
    }
}

/// Validate storage integrity and exact codeword, not first-carrier finality or
/// the other routes' current membership. Those owners stay in State/the driver.
fn validate_manifest(
    lane: &VerifiedLaneContext,
    payload: &LaneInputPayloadV1,
    bytes: &[u8],
    manifest: &LaneManifestV1,
) -> Result<()> {
    let frozen = lane.frozen();
    let kind = payload.validate_structure().map_err(bad)?;
    let slot = payload
        .descriptor
        .slots
        .iter()
        .find(|slot| {
            slot.route.lane_id == frozen.lane_id && slot.route.dataspace_id == frozen.dataspace_id
        })
        .ok_or_else(|| bad("stored input omits its exact route"))?;
    let instance = Hash::from(lane.instance_id().0);
    let context = lane.reducer_context();
    let origin = context
        .roster()
        .get(manifest.value.origin_producer as usize)
        .ok_or_else(|| bad("stored origin is outside the frozen committee"))?;
    if slot.instance_id != instance
        || slot.lane_incarnation != frozen.lane_incarnation
        || slot.lane_height != frozen.next_lane_height
        || payload.descriptor.admission_priority != frozen.admission_priority
        || payload.input.certificate.binding.canonical_hash() != frozen.admitted_binding_hash
        || manifest.value.instance_id != instance
        || manifest.value.admitted_binding_hash != frozen.admitted_binding_hash
        || manifest.value.kind != kind
        || manifest.value.descriptor_hash != payload.descriptor.canonical_hash().map_err(bad)?
        || manifest.value.payload_hash != Hash::new(bytes)
        || manifest.layout != frozen.da_layout
        || manifest.byte_len != bytes.len() as u64
        || origin.id() != context.leader(manifest.value.origin_view)
    {
        return Err(bad(
            "stored input/manifest differs from the exact frozen native value",
        ));
    }
    manifest.validate_availability().map_err(bad)?;
    let chunks = wire::encode_payload_chunks(frozen.da_layout, bytes).map_err(bad)?;
    let hashes = chunks.iter().map(Hash::new).collect::<Vec<_>>();
    if manifest.chunk_count as usize != chunks.len()
        || wire::payload_chunk_root(&hashes) != Some(manifest.chunk_root)
    {
        return Err(bad(
            "stored native input differs from its exact RS16 codeword",
        ));
    }
    Ok(())
}

/// Count the real canonical storage DTO at its signed maximum body size.
/// All non-payload fields have fixed widths except the two fixed enum tags;
/// use the maximum scalar/tag encodings. These private sizing values are never
/// persisted, returned or authenticated. No made-up global block is encoded.
fn maximum_frame_bytes(lane: &VerifiedLaneContext) -> Result<u64> {
    let maximum = usize::try_from(lane.frozen().da_layout.max_payload_size_bytes).map_err(bad)?;
    if maximum == 0 || maximum as u64 > wire::MAX_DA_PAYLOAD_SIZE_BYTES {
        return Err(bad(
            "signed native payload limit is outside protocol bounds",
        ));
    }
    // TODO: replace this bounded temporary with a shared codec counting view if
    // profiling warrants it; never replace exact sizing by guessed headroom.
    let mut payload = Vec::new();
    payload.try_reserve_exact(maximum).map_err(bad)?;
    payload.resize(maximum, 0);
    let hash = Hash::prehashed([u8::MAX; Hash::LENGTH]);
    let mut record = LaneBodyRecordV1 {
        version: u16::MAX,
        frozen_context_hash: hash,
        consensus_key_hash: hash,
        manifest: LaneManifestV1 {
            value: LaneValueRefV1 {
                instance_id: hash,
                admitted_binding_hash: hash,
                kind: LaneValueKindV1::Execution,
                origin_view: u64::MAX,
                origin_producer: u32::MAX,
                descriptor_hash: hash,
                payload_hash: hash,
                availability_hash: hash,
            },
            layout: lane.frozen().da_layout,
            chunk_root: hash,
            byte_len: u64::MAX,
            chunk_count: u32::MAX,
        },
        canonical_payload: payload,
    };
    let first = norito::canonical_frame_len(&record).map_err(bad)?;
    record.manifest.value.kind = LaneValueKindV1::AtomicGroup;
    let second = norito::canonical_frame_len(&record).map_err(bad)?;
    u64::try_from(first.max(second)).map_err(bad)
}
