#[cfg(test)]
mod signed_transaction_fixture_tests {
    use std::time::Duration;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId, account::AccountId, block::BlockHeader, transaction::TransactionBuilder,
    };
    use iroha_version::codec::EncodeVersioned as _;
    use super::decode_signed_transaction;
    // Matches account::address::DEFAULT_CHAIN_DISCRIMINANT (i105 discriminant).
    const FIXTURE_CHAIN_DISCRIMINANT: u16 = 0x02F1;
    fn fixture_key_pair() -> KeyPair {
        KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair")
    }
    fn fixture_network_id() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"connect-norito-signed-transaction-fixture-genesis",
        )))
    }
    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(fixture_key_pair().algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }
    #[test]
    fn signed_transaction_decoder_accepts_only_versioned_bytes() {
        let _scope = super::test_support::ChainDiscriminantScope::enter(FIXTURE_CHAIN_DISCRIMINANT);
        let keypair = fixture_key_pair();
        let authority = AccountId::new(keypair.public_key().clone());
        let mut builder = TransactionBuilder::new(
            fixture_network_id(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(1));
        let tx = builder.sign(keypair.private_key());
        let versioned = tx.encode_versioned();
        decode_signed_transaction(&versioned).expect("decode versioned signed tx");
        let bytes = norito::codec::encode_adaptive(&tx);
        assert!(decode_signed_transaction(&bytes).is_err());
        let framed = norito::to_bytes(&tx).expect("encode framed signed tx");
        assert!(decode_signed_transaction(&framed).is_err());
    }
    #[test]
    fn signed_transaction_versioned_reencode_match() {
        let _scope = super::test_support::ChainDiscriminantScope::enter(FIXTURE_CHAIN_DISCRIMINANT);
        let keypair = fixture_key_pair();
        let authority = AccountId::new(keypair.public_key().clone());
        let mut builder = TransactionBuilder::new(
            fixture_network_id(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(1));
        let tx = builder.sign(keypair.private_key());
        let bytes = tx.encode_versioned();
        let signed = decode_signed_transaction(&bytes).expect("decode versioned signed tx");
        assert_eq!(signed.encode_versioned(), bytes);
    }
    #[test]
    fn generated_signed_transaction_versioned_bytes_prefix_bare_payload() {
        let _scope = super::test_support::ChainDiscriminantScope::enter(FIXTURE_CHAIN_DISCRIMINANT);
        let keypair = fixture_key_pair();
        let authority = AccountId::new(keypair.public_key().clone());
        let mut builder = TransactionBuilder::new(
            fixture_network_id(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(1));
        let tx = builder.sign(keypair.private_key());
        let versioned = tx.encode_versioned();
        let bare = norito::codec::encode_adaptive(&tx);
        assert_eq!(versioned.first().copied(), Some(1));
        assert_eq!(&versioned[1..], bare.as_slice());
    }
}
#[cfg(test)]
mod da_proof_summary_tests {
    use iroha_data_model::{
        da::{
            manifest::{ChunkCommitment, ChunkRole},
            types::{
                BlobClass, BlobCodec, BlobDigest, ChunkDigest, DaRentQuote, ErasureProfile,
                ExtraMetadata, GovernanceTag, MetadataEntry, MetadataVisibility, RetentionPolicy,
                StorageTicketId,
            },
        },
        nexus::LaneId,
        sorafs::pin_registry::StorageClass,
    };
    use sorafs_car::ChunkStore;
    use super::*;
    #[test]
    fn da_proof_summary_via_ffi() {
        let (manifest_bytes, payload) = sample_manifest_bytes();
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let status = unsafe {
            connect_norito_da_proof_summary(
                manifest_bytes.as_ptr(),
                manifest_bytes.len() as c_ulong,
                payload.as_ptr(),
                payload.len() as c_ulong,
                2,
                0,
                ptr::null(),
                0,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, 0, "da proof summary call failed");
        assert!(!out_ptr.is_null());
        let summary_bytes = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
        connect_norito_free(out_ptr);
        let value: JsonValue = norito::json::from_slice(&summary_bytes).expect("json summary");
        assert!(value.get("proofs").is_some(), "missing proofs array");
        assert!(
            value.get("blob_hash_hex").is_some(),
            "missing blob hash field"
        );
    }
    fn sample_manifest_bytes() -> (Vec<u8>, Vec<u8>) {
        let payload: Vec<u8> = (0..64).map(|idx| idx as u8).collect();
        let mut store = ChunkStore::new();
        store.ingest_bytes(&payload).expect("ingest sample payload");
        let data_shards = 2usize;
        let chunk_commitments = store
            .chunks()
            .iter()
            .enumerate()
            .map(|(idx, chunk)| {
                let stripe_id = u32::try_from(idx / data_shards).unwrap_or(u32::MAX);
                ChunkCommitment::new_with_role(
                    idx as u32,
                    chunk.offset,
                    chunk.length,
                    ChunkDigest::new(chunk.blake3),
                    ChunkRole::Data,
                    stripe_id,
                )
            })
            .collect::<Vec<_>>();
        let chunk_size = chunk_commitments
            .first()
            .map(|commitment| commitment.length)
            .unwrap_or(payload.len() as u32);
        let metadata = ExtraMetadata {
            items: vec![
                MetadataEntry::new(
                    "taikai.event_id",
                    b"demo-event".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.stream_id",
                    b"primary-stream".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.rendition_id",
                    b"main-1080p".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.segment.sequence",
                    b"42".to_vec(),
                    MetadataVisibility::Public,
                ),
            ],
        };
        let chunk_root = BlobDigest::new(*store.por_tree().root());
        let manifest = DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::new([0x11; 32]),
            lane_id: LaneId::new(7),
            epoch: 1,
            blob_class: BlobClass::TaikaiSegment,
            codec: BlobCodec::new(String::from("custom.binary")),
            blob_hash: BlobDigest::new(*store.payload_digest().as_bytes()),
            chunk_root,
            storage_ticket: StorageTicketId::new([0x44; 32]),
            total_size: payload.len() as u64,
            chunk_size,
            total_stripes: chunk_commitments.len().div_ceil(2).try_into().unwrap_or(0),
            shards_per_stripe: 3,
            erasure_profile: ErasureProfile {
                data_shards: 2,
                parity_shards: 1,
                row_parity_stripes: 0,
                chunk_alignment: 1,
                fec_scheme: iroha_data_model::da::types::FecScheme::Rs12_10,
            },
            retention_policy: RetentionPolicy {
                hot_retention_secs: 10,
                cold_retention_secs: 20,
                required_replicas: 3,
                storage_class: StorageClass::Warm,
                governance_tag: GovernanceTag::new(String::from("da.test")),
            },
            rent_quote: DaRentQuote::default(),
            chunks: chunk_commitments,
            ipa_commitment: chunk_root,
            metadata,
            issued_at_unix: 123,
        };
        let manifest_bytes = norito::to_bytes(&manifest).expect("manifest encode");
        (manifest_bytes, payload)
    }
}
