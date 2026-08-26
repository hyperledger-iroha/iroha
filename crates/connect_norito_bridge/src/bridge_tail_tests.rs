#[cfg(test)]
mod signed_transaction_fixture_tests {
    use super::decode_signed_transaction;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        Level, NetworkId,
        account::AccountId,
        block::BlockHeader,
        isi::Log,
        transaction::{SignedTransaction, TransactionBuilder},
    };
    use iroha_version::codec::{DecodeVersioned as _, EncodeVersioned as _};
    use std::time::Duration;
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
    fn read_compact_length(bytes: &[u8], offset: &mut usize) -> usize {
        let mut value = 0_u64;
        let mut shift = 0_u32;
        loop {
            let byte = *bytes.get(*offset).expect("compact length byte");
            *offset += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return usize::try_from(value).expect("compact length fits usize");
            }
            shift += 7;
            assert!(shift < 64, "compact length overflow");
        }
    }
    fn split_compact_fields(bytes: &[u8], count: usize) -> Vec<Vec<u8>> {
        let mut offset = 0;
        let mut fields = Vec::with_capacity(count);
        for _ in 0..count {
            let len = read_compact_length(bytes, &mut offset);
            let end = offset.checked_add(len).expect("field end");
            fields.push(bytes.get(offset..end).expect("complete field").to_vec());
            offset = end;
        }
        assert_eq!(offset, bytes.len(), "unexpected compact field tail");
        fields
    }
    fn push_compact_length(bytes: &mut Vec<u8>, mut value: usize) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            bytes.push(byte);
            if value == 0 {
                break;
            }
        }
    }
    fn compact_fields(fields: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for field in fields {
            push_compact_length(&mut bytes, field.len());
            bytes.extend_from_slice(field);
        }
        bytes
    }
    fn signed_transaction_with_type_name_instruction_pair(
        canonical: &[u8],
        concrete_type_name: &str,
    ) -> Vec<u8> {
        assert_eq!(canonical.first(), Some(&1));
        let mut signed = split_compact_fields(&canonical[1..], 3);
        let mut payload = split_compact_fields(&signed[1], 10);

        assert_eq!(&payload[3][..4], &0_u32.to_le_bytes());
        let executable_fields = split_compact_fields(&payload[3][4..], 1);
        let sequence = &executable_fields[0];
        assert_eq!(&sequence[..8], &1_u64.to_le_bytes());
        let sequence_fields = split_compact_fields(&sequence[8..], 1);
        let mut instruction = split_compact_fields(&sequence_fields[0], 2);
        let wire_id = split_compact_fields(&instruction[0], 1);
        assert_eq!(wire_id[0], b"iroha.log");

        instruction[0] = compact_fields(&[concrete_type_name.as_bytes().to_vec()]);
        let instruction = compact_fields(&instruction);
        let mut sequence = 1_u64.to_le_bytes().to_vec();
        sequence.extend_from_slice(&compact_fields(&[instruction]));
        let mut executable = 0_u32.to_le_bytes().to_vec();
        executable.extend_from_slice(&compact_fields(&[sequence]));
        payload[3] = executable;
        signed[1] = compact_fields(&payload);

        let mut alternate = vec![1];
        alternate.extend_from_slice(&compact_fields(&signed));
        alternate
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
    fn signed_transaction_decoder_rejects_registered_type_name_alias() {
        let _scope = super::test_support::ChainDiscriminantScope::enter(FIXTURE_CHAIN_DISCRIMINANT);
        let keypair = fixture_key_pair();
        let authority = AccountId::new(keypair.public_key().clone());
        let mut builder = TransactionBuilder::new(
            fixture_network_id(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "noncanonical instruction alias".to_owned(),
        )]);
        builder.set_creation_time(Duration::from_millis(1));
        let transaction = builder.sign(keypair.private_key());
        let canonical = transaction
            .encode_wire_v1()
            .expect("encode canonical signed transaction");
        let alternate = signed_transaction_with_type_name_instruction_pair(
            &canonical,
            std::any::type_name::<Log>(),
        );

        assert_ne!(alternate, canonical);
        let error = SignedTransaction::decode_all_versioned(&alternate)
            .expect_err("canonical V1 decoding must reject the concrete type-name alias");
        assert!(
            matches!(error, iroha_version::error::Error::NoritoCodec(_)),
            "alias rejection must remain a codec failure: {error}"
        );
        assert!(
            decode_signed_transaction(&alternate).is_err(),
            "the bridge must reject every noncanonical instruction alias"
        );
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
    use super::*;
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
