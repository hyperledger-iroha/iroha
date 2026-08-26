use iroha_data_model::musubi::{MusubiArchiveCommitmentV1, MusubiContentDigestV1};
fn bind_musubi_archive(archived: &mut ProviderIngestFinalizedArchivedOrderV1, seed: u8) {
    let commitment = MusubiArchiveCommitmentV1 {
        root_cid: archived.pin_manifest.root_cid.clone(),
        chunker: archived.pin_manifest.chunker.clone(),
        chunk_plan_digest: MusubiContentDigestV1::new(archived.pin_manifest.chunk_digest_sha3_256),
        por_root: MusubiContentDigestV1::new(archived.pin_manifest.por_root),
        content_length: archived.pin_manifest.content_length,
        car_digest: MusubiContentDigestV1::new([seed; 32]),
        car_size: archived.pin_manifest.content_length.saturating_add(1_024),
        bundle_digest: MusubiContentDigestV1::new([seed.wrapping_add(1); 32]),
        source_tree_digest: MusubiContentDigestV1::new([seed.wrapping_add(2); 32]),
        descriptor_digest: MusubiContentDigestV1::new([seed.wrapping_add(3); 32]),
        file_count: 1,
        chunk_count: 1,
    };
    archived.replication_order.musubi_archive = Some(commitment.archive_id());
    archived.musubi_archive = Some(MusubiReplicationOrderArchiveBindingV1::new(
        archived.replication_order.order_id,
        commitment.archive_id(),
        commitment,
    ));
}
#[test]
fn first_projection_rejects_conflicting_provider_archive_bindings_for_one_order() {
    let mut projection = projection(7);
    let shared_order = ReplicationOrderId::new([0x21; 32]);
    for provider in &mut projection.providers {
        if let Some(archived) = provider
            .orders
            .iter_mut()
            .find(|archived| archived.order_id() == shared_order)
        {
            let seed = if provider.provider_id == PROVIDER_A {
                0x81
            } else {
                0x82
            };
            bind_musubi_archive(archived, seed);
        }
    }
    assert!(matches!(
        projection.validate(bounds()),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidProjection {
            reason: "one order has conflicting provider projections"
        })
    ));
}
#[test]
fn archive_preserves_musubi_binding_and_rejects_late_or_substituted_binding() {
    let directory = physical_tempdir().expect("archive tempdir");
    let archive = ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds())
        .expect("open archive");
    let mut first = projection(7);
    for provider in &mut first.providers {
        for archived in &mut provider.orders {
            if archived.order_id() == ReplicationOrderId::new([0x21; 32]) {
                bind_musubi_archive(archived, 0x81);
            }
        }
    }
    archive
        .insert(first.clone())
        .expect("insert bound projection");
    let page = archive
        .read_provider_page(&first.key, PROVIDER_A, None, 1)
        .expect("read exact provider page");
    let binding = page.rows[0]
        .musubi_archive
        .as_ref()
        .expect("preserve finalized Musubi binding");
    assert_eq!(
        binding.replication_order,
        page.rows[0].replication_order.order_id
    );
    let mut removed = advance_projection(&first, 8);
    for provider in &mut removed.providers {
        for archived in &mut provider.orders {
            if archived.order_id() == ReplicationOrderId::new([0x21; 32]) {
                archived.musubi_archive = None;
                archived.replication_order.musubi_archive = None;
            }
        }
    }
    assert!(matches!(
        archive.insert(removed),
        Err(ProviderIngestFinalizedArchiveErrorV1::OrderSubstitution { .. })
    ));
    let mut substituted = advance_projection(&first, 8);
    for provider in &mut substituted.providers {
        for archived in &mut provider.orders {
            if archived.order_id() == ReplicationOrderId::new([0x21; 32]) {
                let binding = archived.musubi_archive.as_mut().expect("bound order");
                binding.commitment.car_digest = MusubiContentDigestV1::new([0xF1; 32]);
                binding.archive_id = binding.commitment.archive_id();
                archived.replication_order.musubi_archive = Some(binding.archive_id);
            }
        }
    }
    assert!(matches!(
        archive.insert(substituted),
        Err(ProviderIngestFinalizedArchiveErrorV1::OrderSubstitution { .. })
    ));
    let mut mismatched = projection(9);
    for provider in &mut mismatched.providers {
        for archived in &mut provider.orders {
            if archived.order_id() == ReplicationOrderId::new([0x21; 32]) {
                bind_musubi_archive(archived, 0x82);
                let binding = archived.musubi_archive.as_mut().expect("bound order");
                binding.commitment.content_length += 1;
                binding.archive_id = binding.commitment.archive_id();
            }
        }
    }
    assert!(matches!(
        mismatched.validate(bounds()),
        Err(ProviderIngestFinalizedArchiveErrorV1::InvalidProjection { .. })
    ));
}
