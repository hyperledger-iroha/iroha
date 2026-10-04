//! Actual retained enrollment and refusal controls; no test observation claims provider readiness.

use super::*;
use crate::managed::{
    native_operation::test_support::{
        UnavailablePeers,
        native_fixture::{NativeFixture, quote_instructions},
    },
    service_bootstrap::ManagedServiceBootstrap,
};
use iroha_data_model::{
    asset::AssetDefinitionId,
    isi::{InstructionBox, Log},
    transaction::FeePaymentIntent,
};
use iroha_primitives::numeric::Quantity;
use iroha_wallet::operations::BoundedTransactionOptions;
use std::collections::BTreeMap;

fn enrolled(
    slot: usize,
) -> (
    tempfile::TempDir,
    PreparedLocalnet,
    RetainedCustodyEnrollment,
) {
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "discovery-activation",
        &temporary.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut parent = ManagedServiceBootstrap::open(&prepared).unwrap();
    let authorization = parent
        .authorize_generated_startup(
            deadline,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap()
        .unwrap();
    let aggregate = parent.selected_policies().unwrap();
    let provider = aggregate.providers[slot].provider_id;
    let policies = aggregate.provider(provider).unwrap();
    let enrollment = policies
        .initial_enrollment(
            crate::managed::native_operation::now_ms().unwrap(),
            authorization.test_terms().signing_deadline_unix_ms,
        )
        .unwrap();
    drop(parent);
    let authority =
        ServiceAuthority::open_network(&prepared, NetworkPurpose::BuildRegistry).unwrap();
    let mut native = NativeFixture::from_generated(&prepared, &authority);
    let log = quote_instructions(
        &native,
        &authority.config,
        [InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            "native enrollment selection prerequisites".into(),
        ))],
    );
    assert_eq!(native.chain.commit(vec![log]), vec![true]);
    drop(authority);
    drop(ports);
    let asset = AssetDefinitionId::parse_address_literal(
        crate::genesis::profile::TAIRA_XOR_ASSET_DEFINITION_ID,
    )
    .unwrap();
    let options = BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        max_total_fees: BTreeMap::from([(asset, Quantity::from(1_000u64))]),
        deadline,
    };
    let mut owner = ManagedStreamTokenCustody::open(&prepared, provider).unwrap();
    assert_eq!(
        owner
            .bootstrap_native_configure(
                &mut native,
                &policies.custody,
                enrollment.deadline_unix_ms,
                &options,
            )
            .finalized
            .unwrap()
            .height,
        3,
    );
    assert_eq!(
        owner
            .bootstrap_native_enroll(&mut native, &policies.custody, enrollment, &options,)
            .finalized
            .unwrap()
            .height,
        4,
    );
    let selected = owner
        .retained_initial_enrollment(&policies.custody, enrollment, deadline)
        .unwrap();
    (temporary, prepared, selected)
}

#[test]
fn activation_discovery_scope_deadline_and_carrier_mismatch_make_no_http() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared, enrollment) = enrolled(2);
    let provider = prepared
        .stream_token_authorities()
        .unwrap()
        .unwrap()
        .providers[2]
        .provider_id;
    let mut peers = UnavailablePeers::start(&prepared);
    let required = *enrollment.finalized();
    let deadline = Instant::now() + Duration::from_secs(30);
    let below = ManagedTransactionFinality {
        height: required.height - 1,
        ..required
    };
    assert!(observe_generated_service(&prepared, provider, &enrollment, below, deadline).is_err());
    let foreign_tx = ManagedTransactionFinality {
        transaction_hash: iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"comparison-only-foreign-tx",
        )),
        ..required
    };
    assert!(
        observe_generated_service(&prepared, provider, &enrollment, foreign_tx, deadline).is_err()
    );
    assert!(
        observe_generated_service(&prepared, provider, &enrollment, required, Instant::now())
            .is_err()
    );
    let mut foreign = prepared.clone();
    foreign.context.dataspace_id = 1;
    assert!(
        observe_generated_service(&foreign, provider, &enrollment, required, deadline).is_err()
    );
    let other = prepared
        .stream_token_authorities()
        .unwrap()
        .unwrap()
        .providers[0]
        .provider_id;
    assert!(observe_generated_service(&prepared, other, &enrollment, required, deadline).is_err());
    let mut unknown = *provider.as_bytes();
    unknown[0] ^= 1;
    assert!(
        observe_generated_service(
            &prepared,
            ProviderId::new(unknown),
            &enrollment,
            required,
            deadline
        )
        .is_err()
    );
    assert!(peers.requests.lock().unwrap().is_empty());
    peers.finish();
}

#[test]
fn unavailable_current_proof_preserves_exact_original_enrollment_and_parent() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared, enrollment) = enrolled(2);
    let provider = prepared
        .stream_token_authorities()
        .unwrap()
        .unwrap()
        .providers[2]
        .provider_id;
    let parent_path = prepared
        .context
        .client_config
        .parent()
        .unwrap()
        .join("runtime/service-operations/network/service-bootstrap/initial/original.nrt");
    let original = iroha_fs::read_private(&parent_path, 512 * 1024).unwrap();
    let mut peers = UnavailablePeers::start(&prepared);
    assert!(
        observe_generated_service(
            &prepared,
            provider,
            &enrollment,
            *enrollment.finalized(),
            Instant::now() + Duration::from_secs(5),
        )
        .is_err()
    );
    peers.finish();
    let requests = peers.requests.lock().unwrap();
    assert!(!requests.is_empty());
    assert!(requests.iter().all(|request| request.method == "GET"));
    assert_eq!(
        iroha_fs::read_private(&parent_path, 512 * 1024)
            .unwrap()
            .as_slice(),
        original.as_slice()
    );
    let parent = ManagedServiceBootstrap::open(&prepared).unwrap();
    let aggregate = parent.selected_policies().unwrap();
    let policies = aggregate.provider(provider).unwrap();
    drop(parent);
    let owner = ManagedStreamTokenCustody::open(&prepared, provider).unwrap();
    let interval = owner
        .inspect_local_initial_interval(&policies.custody)
        .unwrap();
    let replay = owner
        .retained_initial_enrollment(
            &policies.custody,
            interval,
            Instant::now() + Duration::from_secs(30),
        )
        .unwrap();
    assert_eq!(replay.bytes(), enrollment.bytes());
    assert_eq!(replay.finalized(), enrollment.finalized());
    assert_eq!(replay.record_digest(), enrollment.record_digest());
}
