//! Canonical authorization and verified Musubi receipt fixtures for outbox tests.

use super::{
    FinalizedProviderIngestAuthorizationV1, FinalizedProviderIngestMusubiContextV1,
    ProviderIngestVerifiedMusubiBundleReceiptV1, cursor,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    musubi::{
        ArchiveId, MusubiArchiveCommitmentV1, MusubiContentDigestV1, MusubiSemanticReleaseDigestV1,
        MusubiVerificationLockDigestV1,
    },
    sorafs::pin_registry::{ChunkerProfileHandle, ManifestRootCid},
};

pub(super) fn authorization(order: u8, height: u64) -> FinalizedProviderIngestAuthorizationV1 {
    FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        height,
        cursor(height).block_hash,
        [0x11; 32],
        [order; 32],
        [order.wrapping_add(0x20); 32],
        ManifestRootCid::from_blake3_digest([order.wrapping_add(0x20); 32])
            .expect("canonical fixture manifest root CID")
            .as_bytes()
            .to_vec(),
        "sorafs.sf1@1.0.0".to_owned(),
        [order.wrapping_add(0x30); 32],
        [order.wrapping_add(0x40); 32],
        4_096,
    )
    .expect("authorization")
}
pub(super) fn authorization_with_musubi_context(
    generic: &FinalizedProviderIngestAuthorizationV1,
    context: FinalizedProviderIngestMusubiContextV1,
) -> FinalizedProviderIngestAuthorizationV1 {
    FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
        generic.finalized_height(),
        generic.finalized_block_hash(),
        generic.provider_id(),
        generic.order_id(),
        generic.manifest_digest(),
        generic.manifest_cid().to_vec(),
        generic.chunker_handle().to_owned(),
        generic.chunk_digest_sha3_256(),
        generic.por_root(),
        generic.content_length(),
        context,
    )
    .expect("Musubi authorization")
}
pub(super) fn musubi_commitment(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    seed: u8,
) -> MusubiArchiveCommitmentV1 {
    let commitment = MusubiArchiveCommitmentV1 {
        root_cid: ManifestRootCid::try_from_slice(authorization.manifest_cid())
            .expect("canonical manifest root CID"),
        chunker: ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1f,
        },
        chunk_plan_digest: MusubiContentDigestV1::new(authorization.chunk_digest_sha3_256()),
        por_root: MusubiContentDigestV1::new(authorization.por_root()),
        content_length: authorization.content_length(),
        car_digest: MusubiContentDigestV1::new([seed; 32]),
        car_size: authorization.content_length().saturating_add(1_024),
        bundle_digest: MusubiContentDigestV1::new([seed.wrapping_add(1); 32]),
        source_tree_digest: MusubiContentDigestV1::new([seed.wrapping_add(2); 32]),
        descriptor_digest: MusubiContentDigestV1::new([seed.wrapping_add(3); 32]),
        file_count: 1,
        chunk_count: 1,
    };
    commitment.validate().expect("valid Musubi commitment");
    commitment
}
fn verified_musubi_receipt(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    commitment: MusubiArchiveCommitmentV1,
) -> ProviderIngestVerifiedMusubiBundleReceiptV1 {
    ProviderIngestVerifiedMusubiBundleReceiptV1::new_for_test(
        authorization,
        commitment,
        MusubiSemanticReleaseDigestV1::new([0xC1; 32]),
        MusubiVerificationLockDigestV1::new([0xC2; 32]),
    )
}
pub(super) fn musubi_authorization_and_receipt(
    order: u8,
    height: u64,
    context_seed: u8,
) -> (
    FinalizedProviderIngestAuthorizationV1,
    ProviderIngestVerifiedMusubiBundleReceiptV1,
) {
    let generic = authorization(order, height);
    let commitment = musubi_commitment(&generic, context_seed);
    let context = FinalizedProviderIngestMusubiContextV1::new(
        network_id(context_seed.wrapping_add(0x40)),
        commitment.archive_id(),
    )
    .expect("Musubi context");
    let authorization = authorization_with_musubi_context(&generic, context);
    let receipt = verified_musubi_receipt(&authorization, commitment);
    (authorization, receipt)
}

pub(super) fn test_network_id() -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::new(b"provider-ingest-outbox-test"),
    ))
}
pub(super) fn network_id(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        [seed; 32],
    )))
}

pub(super) fn assert_musubi_context_rejects_unmarked_network(
    context: &FinalizedProviderIngestMusubiContextV1,
) {
    struct RawNetwork([u8; Hash::LENGTH]);
    impl norito::SerializePayload for RawNetwork {
        fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
            std::io::Write::write_all(writer, &self.0)?;
            Ok(())
        }
    }
    #[derive(norito::SerializePayload)]
    struct ContextPayload {
        network_id: RawNetwork,
        archive_id: ArchiveId,
    }
    fn payload(value: &impl norito::SerializePayload) -> Vec<u8> {
        let mut bytes = Vec::new();
        value
            .serialize(&mut norito::core::Encoder::for_buffer(&mut bytes))
            .expect("serialize context payload");
        bytes
    }
    for flags in (0..=norito::core::supported_header_flags())
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
    {
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        let mut raw = ContextPayload {
            network_id: RawNetwork(*context.network_id().as_bytes()),
            archive_id: context.archive_id(),
        };
        assert_eq!(
            payload(&raw),
            payload(context),
            "raw carrier must preserve the complete valid context in layout {flags:#04x}"
        );
        let valid = payload(&raw);
        let (decoded, used) =
            norito::core::decode_field_canonical::<FinalizedProviderIngestMusubiContextV1>(&valid)
                .expect("decode valid nested context in the selected layout");
        assert_eq!(decoded, *context);
        assert_eq!(used, valid.len());
        raw.network_id.0[Hash::LENGTH - 1] &= !1;
        let invalid = payload(&raw);
        assert!(matches!(
            norito::core::decode_field_canonical::<FinalizedProviderIngestMusubiContextV1>(&invalid),
            Err(norito::Error::Message(message)) if message == "invalid hash lsb"
        ));
    }
}
