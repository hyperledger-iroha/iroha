//! Canonical metadata for assignment-bound publisher source staging.
//!
//! This transport carries no finalized authority. Receivers must independently authenticate the
//! publisher and current assignment before reserving staging space or accepting any chunk.

use crate::{CarBuildPlan, CarChunk, FilePlan};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::ManifestV1;

/// Maximum encoded metadata for one publisher source session.
pub const PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
/// Maximum canonical staging request: a metadata reservation or one bound chunk.
pub const PUBLISHER_SOURCE_REQUEST_MAX_BYTES_V1: usize =
    PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1 + 4096;
/// Maximum assigned-source response: one metadata header or one chunk, never both.
pub const PROVIDER_SOURCE_RESPONSE_MAX_BYTES_V1: usize =
    PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1 + 4096;

/// Sole canonical response to an assignment-authorized source request.
///
/// Metadata is authenticated once before fetching chunks. Each chunk must then match that
/// retained metadata's exact ordinal, length and digest; it carries no independent authority.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_car::publisher::ProviderSourceResponseV1")]
pub enum ProviderSourceResponseV1 {
    /// Complete metadata returned only when the request has no chunk selector.
    Metadata(PublisherSourceHeaderV1),
    /// One chunk returned only for the corresponding exact ordinal selector.
    Chunk(PublisherSourceUploadV1),
}

impl ProviderSourceResponseV1 {
    /// Decode the bounded canonical response and validate its structural variant.
    /// The caller must separately verify the requested variant and exact chunk commitment.
    pub fn decode(bytes: &[u8]) -> Result<Self, PublisherSourceMetadataErrorV1> {
        let rejected = PublisherSourceMetadataErrorV1;
        let maximum = PROVIDER_SOURCE_RESPONSE_MAX_BYTES_V1;
        if bytes.len() > maximum {
            return Err(rejected);
        }
        let response: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(maximum, maximum, maximum, maximum * 4, 32),
        )
        .map_err(|_| rejected)?;
        match &response {
            Self::Metadata(header) => {
                header.verify()?;
            }
            Self::Chunk(upload) => {
                if upload.bytes.is_empty()
                    || upload.bytes.len() > crate::CHUNK_STORE_MAX_CHUNK_BYTES as usize
                {
                    return Err(rejected);
                }
            }
        }
        Ok(response)
    }
}

/// One complete content-addressed chunk uploaded by the manifest's publisher.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_car::publisher::PublisherSourceUploadV1")]
pub struct PublisherSourceUploadV1 {
    /// Ordinal in the exact header's ordered chunk inventory.
    pub index: u32,
    /// Exact chunk bytes, including neither framing nor trailing bytes.
    pub bytes: Vec<u8>,
}

/// One publisher-authenticated chunk bound to an already reserved exact session.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_car::publisher::PublisherSourceChunkRequestV1")]
pub struct PublisherSourceChunkRequestV1 {
    /// Exact assigned receiving provider.
    pub provider_id: [u8; 32],
    /// Exact finalized replication order.
    pub order_id: [u8; 32],
    /// Exact current assignment revision.
    pub assignment_revision: u64,
    /// Canonical manifest digest authenticated by native pin state.
    pub manifest_digest: [u8; 32],
    /// BLAKE3-256 of the canonical metadata header admitted for this session.
    pub header_digest: [u8; 32],
    /// Exact ordinal and bytes checked against retained session metadata.
    pub upload: PublisherSourceUploadV1,
}

/// Sole canonical publisher request: reserve metadata once, then upload bound chunks.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_car::publisher::PublisherSourceRequestV1")]
pub enum PublisherSourceRequestV1 {
    /// Admit or idempotently confirm one full exact metadata reservation.
    Metadata(PublisherSourceHeaderV1),
    /// Upload one chunk for an existing exact reservation without repeating its plan.
    Chunk(PublisherSourceChunkRequestV1),
}

impl PublisherSourceRequestV1 {
    /// Decode one bounded canonical request and verify its structural bindings.
    /// Chunk commitments require the receiver's independently authenticated reserved header.
    pub fn decode(bytes: &[u8]) -> Result<Self, PublisherSourceMetadataErrorV1> {
        let rejected = PublisherSourceMetadataErrorV1;
        let maximum = PUBLISHER_SOURCE_REQUEST_MAX_BYTES_V1;
        if bytes.len() > maximum {
            return Err(rejected);
        }
        let request: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(maximum, maximum, maximum, maximum * 4, 32),
        )
        .map_err(|_| rejected)?;
        match &request {
            Self::Metadata(header) => {
                header.verify()?;
            }
            Self::Chunk(chunk) => {
                if chunk.provider_id == [0; 32]
                    || chunk.order_id == [0; 32]
                    || chunk.assignment_revision == 0
                    || chunk.manifest_digest == [0; 32]
                    || chunk.header_digest == [0; 32]
                    || chunk.upload.bytes.is_empty()
                    || chunk.upload.bytes.len() > crate::CHUNK_STORE_MAX_CHUNK_BYTES as usize
                {
                    return Err(rejected);
                }
            }
        }
        Ok(request)
    }
}

/// Exact canonical chunk geometry; each upload contains one complete chunk.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_car::publisher::PublisherSourceChunkV1")]
pub struct PublisherSourceChunkV1 {
    /// Offset in the unchunked payload.
    pub offset: u64,
    /// Exact upload length.
    pub length: u32,
    /// Expected BLAKE3-256 digest.
    pub digest: [u8; 32],
}

/// Exact native file inventory committed by the manifest's root CID.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_car::publisher::PublisherSourceFileV1")]
pub struct PublisherSourceFileV1 {
    /// Canonical relative path components.
    pub path: Vec<String>,
    /// First contiguous chunk ordinal.
    pub first_chunk: u32,
    /// Number of chunks in this file.
    pub chunk_count: u32,
    /// Exact file length.
    pub size: u64,
}

/// Signed-request body selecting one exact publisher staging session.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_car::publisher::PublisherSourceHeaderV1")]
pub struct PublisherSourceHeaderV1 {
    /// First-release layout, exactly one.
    pub version: u8,
    /// Assigned provider receiving these bytes.
    pub provider_id: [u8; 32],
    /// Exact finalized replication order.
    pub order_id: [u8; 32],
    /// Exact current assignment revision.
    pub assignment_revision: u64,
    /// Canonical encoded `ManifestV1`.
    pub manifest_bytes: Vec<u8>,
    /// BLAKE3-256 digest of the concatenated payload.
    pub payload_digest: [u8; 32],
    /// Ordered chunk inventory.
    pub chunks: Vec<PublisherSourceChunkV1>,
    /// Ordered native file inventory.
    pub files: Vec<PublisherSourceFileV1>,
}

/// Fixed, payload-free publisher metadata rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("publisher source metadata does not match the canonical manifest")]
pub struct PublisherSourceMetadataErrorV1;

impl PublisherSourceHeaderV1 {
    /// BLAKE3-256 identity of the sole canonical metadata frame, computed once per session.
    pub fn canonical_digest(&self) -> Result<[u8; 32], PublisherSourceMetadataErrorV1> {
        let encoded = norito::encode_canonical(self).map_err(|_| PublisherSourceMetadataErrorV1)?;
        Ok(*blake3::hash(&encoded).as_bytes())
    }

    /// Build canonical session metadata from an already derived plan.
    pub fn new(
        provider_id: [u8; 32],
        order_id: [u8; 32],
        assignment_revision: u64,
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
    ) -> Result<Self, PublisherSourceMetadataErrorV1> {
        let rejected = PublisherSourceMetadataErrorV1;
        let header = Self {
            version: 1,
            provider_id,
            order_id,
            assignment_revision,
            manifest_bytes: manifest.encode().map_err(|_| rejected)?,
            payload_digest: *plan.payload_digest.as_bytes(),
            chunks: plan
                .chunks
                .iter()
                .map(|chunk| PublisherSourceChunkV1 {
                    offset: chunk.offset,
                    length: chunk.length,
                    digest: chunk.digest,
                })
                .collect(),
            files: plan
                .files
                .iter()
                .map(|file| {
                    Ok(PublisherSourceFileV1 {
                        path: file.path.clone(),
                        first_chunk: u32::try_from(file.first_chunk).map_err(|_| rejected)?,
                        chunk_count: u32::try_from(file.chunk_count).map_err(|_| rejected)?,
                        size: file.size,
                    })
                })
                .collect::<Result<_, PublisherSourceMetadataErrorV1>>()?,
        };
        header.verify()?;
        Ok(header)
    }

    /// Reconstruct and verify exact manifest, registered profile, native paths and CAR geometry.
    /// Payload digests, PoR and CAR byte digests still require streaming payload verification.
    pub fn verify(&self) -> Result<(ManifestV1, CarBuildPlan), PublisherSourceMetadataErrorV1> {
        let rejected = PublisherSourceMetadataErrorV1;
        if self.version != 1
            || self.provider_id == [0; 32]
            || self.order_id == [0; 32]
            || self.assignment_revision == 0
            || self.payload_digest == [0; 32]
            || self.manifest_bytes.len() > sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES
            || self.chunks.len() > crate::CAR_PLAN_MAX_CHUNKS
        {
            return Err(rejected);
        }
        let manifest = sorafs_manifest::decode_manifest_v1_canonical(&self.manifest_bytes)
            .map_err(|_| rejected)?;
        let descriptor = sorafs_manifest::validate_registered_chunker_profile(&manifest.chunking)
            .map_err(|_| rejected)?;
        let plan = CarBuildPlan {
            chunk_profile: descriptor.profile,
            payload_digest: blake3::Hash::from_bytes(self.payload_digest),
            content_length: manifest.content_length,
            chunks: self
                .chunks
                .iter()
                .map(|chunk| CarChunk {
                    offset: chunk.offset,
                    length: chunk.length,
                    digest: chunk.digest,
                })
                .collect(),
            files: self
                .files
                .iter()
                .map(|file| {
                    Ok(FilePlan {
                        path: file.path.clone(),
                        first_chunk: usize::try_from(file.first_chunk).map_err(|_| rejected)?,
                        chunk_count: usize::try_from(file.chunk_count).map_err(|_| rejected)?,
                        size: file.size,
                    })
                })
                .collect::<Result<_, PublisherSourceMetadataErrorV1>>()?,
        };
        plan.verify_manifest_metadata(&manifest)
            .map_err(|_| rejected)?;
        if norito::core::encoded_frame_len(self).map_err(|_| rejected)?
            > PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1
        {
            return Err(rejected);
        }
        Ok((manifest, plan))
    }

    /// Decode the sole canonical Norito representation with allocation and recursion bounds.
    pub fn decode(bytes: &[u8]) -> Result<Self, PublisherSourceMetadataErrorV1> {
        let rejected = PublisherSourceMetadataErrorV1;
        let maximum = PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1;
        if bytes.len() > maximum {
            return Err(rejected);
        }
        let header: Self = norito::decode_from_bytes_with_limits(
            bytes,
            norito::DecodeLimits::new(maximum, maximum, maximum, maximum * 4, 32),
        )
        .map_err(|_| rejected)?;
        if norito::to_bytes(&header).map_err(|_| rejected)? != bytes {
            return Err(rejected);
        }
        header.verify()?;
        Ok(header)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigned_source_metadata_and_chunk_have_distinct_canonical_frames() {
        let payload = b"canonical source response without repeated metadata";
        let plan = CarBuildPlan::single_file(payload).unwrap();
        let car = crate::CarWriter::new(&plan, payload)
            .unwrap()
            .write_to(std::io::sink())
            .unwrap();
        let manifest = sorafs_manifest::ManifestBuilder::new()
            .root_cid(car.root_cids[0].clone())
            .dag_codec(sorafs_manifest::DagCodecId(car.dag_codec))
            .chunking_from_profile(
                plan.chunk_profile,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .chunk_digest_sha3_256(crate::compute_chunk_plan_digest_sha3(&plan.chunks))
            .por_root(crate::compute_por_root(payload, &plan).unwrap())
            .content_length(plan.content_length)
            .car_digest(*car.car_archive_digest.as_bytes())
            .car_size(car.car_size)
            .pin_policy(sorafs_manifest::PinPolicy::default())
            .build()
            .unwrap();
        let header = PublisherSourceHeaderV1::new([1; 32], [2; 32], 1, &manifest, &plan).unwrap();
        for response in [
            ProviderSourceResponseV1::Metadata(header),
            ProviderSourceResponseV1::Chunk(PublisherSourceUploadV1 {
                index: 0,
                bytes: payload.to_vec(),
            }),
        ] {
            let bytes = norito::encode_canonical(&response).unwrap();
            assert_eq!(ProviderSourceResponseV1::decode(&bytes).unwrap(), response);
            if matches!(response, ProviderSourceResponseV1::Chunk(_)) {
                assert!(
                    bytes.len() < payload.len() + 256,
                    "chunks must not repeat the manifest plan"
                );
            }
            let mut trailing = bytes;
            trailing.push(0);
            assert!(ProviderSourceResponseV1::decode(&trailing).is_err());
        }
    }

    #[test]
    fn assigned_source_response_rejects_empty_and_oversized_chunks() {
        for bytes in [
            Vec::new(),
            vec![0; crate::CHUNK_STORE_MAX_CHUNK_BYTES as usize + 1],
        ] {
            let response =
                ProviderSourceResponseV1::Chunk(PublisherSourceUploadV1 { index: 0, bytes });
            assert!(
                ProviderSourceResponseV1::decode(&norito::encode_canonical(&response).unwrap())
                    .is_err()
            );
        }
    }
}
