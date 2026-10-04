//! Native executed cuts and source ownership; no injected archive or completion records.
use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    isi::Log,
    musubi::ArchiveId,
    sorafs::{capacity::ProviderId, pin_registry::ReplicationOrderId},
};

fn fixture() -> (CertifiedTestChain, KeyPair) {
    let key = KeyPair::from_seed(vec![0xB9; 32], Algorithm::Ed25519);
    let mut config = TestChainConfig::new(World::new(), 1_000);
    config.genesis_key = key.clone();
    (CertifiedTestChain::start(config).unwrap(), key)
}
fn log(chain: &mut CertifiedTestChain, key: &KeyPair) {
    let signed = chain.sign(
        key,
        [Log::new(
            iroha_logger::Level::INFO,
            "inventory native source".to_owned(),
        )
        .into()],
        chain.committed(chain.height()).block_time_ms() + 1,
    );
    assert!(chain.commit(vec![signed])[0]);
}
fn key() -> MusubiProviderBundleAttestationKeyV1 {
    MusubiProviderBundleAttestationKeyV1 {
        archive_id: ArchiveId::new([1; 32]),
        replication_order: ReplicationOrderId::new([2; 32]),
        provider_id: ProviderId::new([3; 32]),
    }
}
#[test]
fn native_inventory_admission_requires_original_view_and_current_receipt() {
    let (mut chain, signer) = fixture();
    log(&mut chain, &signer);
    log(&mut chain, &signer);
    let view = chain.state().view();
    let same_bytes_foreign_view = chain.state().view();
    with_native_check_read_limits(|| {
        let reader = SignerCertifiedWalkV1::new(&view).unwrap();
        let current = reader.walk(3, 3).next().unwrap().unwrap();
        assert!(
            read_provider_admission_at_native_current_v1(&view, &current, key().provider_id, 1)
                .unwrap()
                .is_none()
        );
        assert!(
            read_provider_admission_at_native_current_v1(
                &same_bytes_foreign_view,
                &current,
                key().provider_id,
                1
            )
            .is_err()
        );
        let older = reader.walk(2, 2).next().unwrap().unwrap();
        assert!(
            read_provider_admission_at_native_current_v1(&view, &older, key().provider_id, 1)
                .is_err()
        );
    });
}
#[test]
fn native_inventory_genesis_missing_archive_and_zero_resources_never_authorize() {
    use ProviderAttestationInventoryReadErrorV1::{Rejected, Unavailable};
    let (mut chain, signer) = fixture();
    let manager = AccountId::new(signer.public_key().clone());
    assert_eq!(
        authorize_provider_attestation_inventory_read_v1(
            &chain.state().view(),
            &manager,
            key(),
            None,
            1
        ),
        Err(Rejected)
    );
    log(&mut chain, &signer);
    assert_eq!(
        authorize_provider_attestation_inventory_read_v1(
            &chain.state().view(),
            &manager,
            key(),
            None,
            1
        ),
        Err(Rejected)
    );
    let zero =
        norito::DecodeLimits::new(64 * 1024 * 1024, 64 * 1024 * 1024, 64 * 1024 * 1024, 0, 128);
    assert_eq!(
        norito::with_decode_limits_scope(
            zero,
            || authorize_provider_attestation_inventory_read_v1(
                &chain.state().view(),
                &manager,
                key(),
                None,
                1
            )
        ),
        Err(Unavailable)
    );
    assert_eq!(
        authorize_provider_attestation_inventory_read_v1(
            &chain.state().view(),
            &manager,
            key(),
            None,
            1
        ),
        Err(Rejected),
        "fresh attempt still proves no archive; budget refusal created no authority"
    );
}
