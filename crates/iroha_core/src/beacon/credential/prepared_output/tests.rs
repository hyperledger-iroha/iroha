//! Exact prepared backing, canonical-byte equality, borrowed retry and safe scrub controls.

use super::*;
use crate::{
    beacon::fixtures::*,
    test_allocations::{allocations_during, refuse_one_layout_during},
};
use std::{
    alloc::Layout,
    cell::Cell,
    task::{Context, Waker},
};

const HANDLE: &str = "software://iroha/consensus-threshold/prepared";
thread_local! { static SCRUBBED: Cell<Option<(usize, bool)>> = const { Cell::new(None) }; }
pub(super) fn observe_scrubbed(bytes: &[u8]) {
    let _ = SCRUBBED.try_with(|slot| {
        if slot.get().is_some() {
            slot.set(Some((bytes.len(), bytes.iter().all(|byte| *byte == 0))));
        }
    });
}
fn fixture(n: u16, budget: &AllocationBudget) -> AdaptiveBeaconFixture {
    let mut session = adaptive_dkg_session_fixture();
    let keys = adaptive_fixture_signing_keys(n);
    let roster = keys
        .iter()
        .map(|key| iroha_model_base::peer::PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    session.committee_size = n;
    session.threshold = (n - 1) / 3 + 1;
    session.roster_hash = crate::beacon::global_threshold_beacon_roster_hash_v1(&roster);
    adaptive_beacon_fixture_for_session_and_keys(session, &keys, budget)
}
fn share(fixture: &AdaptiveBeaconFixture) -> RuntimeGlobalBeaconShareProvisioningV1 {
    use iroha_crypto::threshold_bls::AdaptiveThresholdBlsSecretShare;
    let parts = fixture
        .dealer_secrets
        .iter()
        .zip(&fixture.dealer_commitments)
        .map(|(secret, dealer)| {
            secret
                .private_share(&fixture.parameters, dealer, 1)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let components =
        AdaptiveThresholdBlsSecretShare::from_dealer_shares(fixture.session.transcript(), &parts)
            .unwrap()
            .into_components_for_runtime_custody();
    RuntimeGlobalBeaconShareProvisioningV1::new(fixture.session.clone(), 1, components)
}
fn plan(
    fixture: &AdaptiveBeaconFixture,
    budget: &AllocationBudget,
) -> Result<PreparedGlobalBeaconCredentialV1, GlobalBeaconCredentialEncodeErrorV1> {
    let network = fixture.session.record().network_id;
    let digest = global_beacon_partial_signer_public_inventory_digest_v1(
        network,
        &[(fixture.session.record(), 1)],
    )
    .unwrap();
    PreparedGlobalBeaconCredentialV1::new(
        network,
        HANDLE,
        7,
        digest,
        [(&fixture.session, 1)],
        budget,
    )
}

#[test]
fn prepared_credential_uses_exact_original_output_without_late_growth_at_four_and_thirty_one() {
    for n in [4, 31] {
        let budget = fixture_budget();
        let fixture = fixture(n, &budget);
        let sources = [share(&fixture)];
        let mut prepared = plan(&fixture, &budget).unwrap();
        let pointer = prepared.output.bytes.as_slice().as_ptr();
        let capacity = prepared.output.bytes.capacity();
        let baseline = budget.reserved_bytes();
        let block = budget
            .try_reserve_bytes(budget.limit_bytes() - baseline)
            .unwrap();
        assert_eq!(
            allocations_during(|| {
                encode_global_beacon_partial_signer_credential_v1(&mut prepared, &sources).unwrap();
            }),
            0,
            "canonical encoding and exact import primitive must not allocate after extraction"
        );
        assert_eq!(prepared.output.bytes.as_slice().as_ptr(), pointer);
        assert_eq!(prepared.output.bytes.as_slice().len(), capacity);
        assert!(prepared.belongs_to(&budget));
        let wire = RuntimeGlobalBeaconSignerCredentialWireV1 {
            header: ConsensusThresholdCredentialHeaderV1::new(
                GLOBAL_BEACON_PARTIAL_SIGNER_SLOT_WIRE_ID_V1,
                fixture.session.record().network_id,
                HANDLE.to_owned(),
                7,
                prepared.digest,
            ),
            sessions: vec![RuntimeGlobalBeaconShareCredentialWireV1 {
                public_session: fixture.session.record().clone(),
                signer_index: 1,
                components: ConsensusThresholdSecretScalarTripleV1(sources[0].components.0),
            }],
        };
        assert!(
            prepared.output.as_slice() == norito::encode_canonical(&wire).unwrap(),
            "prepared credential must exactly match the canonical wire bytes"
        );
        assert!(matches!(
            encode_global_beacon_partial_signer_credential_v1(&mut prepared, &sources),
            Err(GlobalBeaconCredentialEncodeErrorV1::PlanChanged)
        ));
        let output = prepared.into_credential().ok().unwrap();
        assert_eq!(output.as_slice().as_ptr(), pointer);
        assert!(output.belongs_to(&budget));
        SCRUBBED.with(|slot| slot.set(Some((0, false))));
        drop(output);
        SCRUBBED.with(|slot| {
            assert_eq!(slot.replace(None), Some((capacity, true)));
        });
        drop(block);
        drop(wire);
        drop(sources);
        drop(fixture);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn prepared_credential_capacity_and_physical_refusals_retain_original_public_owner() {
    let budget = fixture_budget();
    let fixture = fixture(4, &budget);
    let mut registration = crate::unit_test_support::release_registration(&budget);
    let baseline = budget.reserved_bytes();
    let occupied = budget
        .try_reserve_bytes(budget.limit_bytes() - baseline)
        .unwrap();
    let error = plan(&fixture, &budget).err().unwrap();
    let GlobalBeaconCredentialEncodeErrorV1::Admission(AllocationRefusal::Capacity {
        release, ..
    }) = error
    else {
        panic!("original capacity source")
    };
    let mut cx = Context::from_waker(Waker::noop());
    assert!(registration.poll_wait(&release, &mut cx).is_pending());
    let foreign = AllocationBudget::new(1);
    drop(foreign.try_reserve_bytes(1).unwrap());
    assert!(registration.poll_wait(&release, &mut cx).is_pending());
    drop(occupied);
    assert!(registration.poll_wait(&release, &mut cx).is_ready());
    registration.cancel();
    let prepared = plan(&fixture, &budget).unwrap();
    let output_layout = Layout::array::<u8>(prepared.output.bytes.capacity()).unwrap();
    drop(prepared);
    for layout in [Layout::array::<u8>(HANDLE.len()).unwrap(), output_layout] {
        let (result, refused) = refuse_one_layout_during(layout, || plan(&fixture, &budget));
        assert!(refused);
        assert!(matches!(
            result,
            Err(GlobalBeaconCredentialEncodeErrorV1::Buffer(
                PrepaidBufferError::Allocation(_)
            ))
        ));
        assert_eq!(budget.reserved_bytes(), baseline);
    }
    assert!(plan(&fixture, &foreign).is_err());
    let ready = plan(&fixture, &budget).unwrap();
    assert!(ready.bindings[0].session.ptr_eq(&fixture.session));
    drop(ready);
    drop(registration);
    drop(fixture);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn invalid_share_and_replaced_public_owner_leave_prepared_output_empty_for_exact_retry() {
    let budget = fixture_budget();
    let fixture = fixture(4, &budget);
    let valid = [share(&fixture)];
    let invalid = [RuntimeGlobalBeaconShareProvisioningV1::new(
        fixture.session.clone(),
        1,
        Zeroizing::new([[0; 32]; 3]),
    )];
    let mut prepared = plan(&fixture, &budget).unwrap();
    let pointer = prepared.output.bytes.as_slice().as_ptr();
    assert!(matches!(
        encode_global_beacon_partial_signer_credential_v1(&mut prepared, &invalid),
        Err(GlobalBeaconCredentialEncodeErrorV1::Share(_))
    ));
    assert!(prepared.output.as_slice().is_empty());
    let resealed = validate_global_threshold_beacon_session_v1(
        fixture.session.record(),
        &global_beacon_session_binding_v1(fixture.session.record()),
        &budget,
    )
    .unwrap();
    let replaced = [RuntimeGlobalBeaconShareProvisioningV1::new(
        resealed,
        1,
        Zeroizing::new(valid[0].components.0),
    )];
    assert!(matches!(
        encode_global_beacon_partial_signer_credential_v1(&mut prepared, &replaced),
        Err(GlobalBeaconCredentialEncodeErrorV1::PlanChanged)
    ));
    assert_eq!(prepared.output.bytes.as_slice().as_ptr(), pointer);
    encode_global_beacon_partial_signer_credential_v1(&mut prepared, &valid).unwrap();
    assert_eq!(prepared.output.bytes.as_slice().as_ptr(), pointer);
}

#[test]
fn streamed_public_inventory_matches_former_canonical_digest_without_output_allocation() {
    use std::io::Read as _;
    for n in [4, 31] {
        let budget = fixture_budget();
        let fixture = fixture(n, &budget);
        let network = fixture.session.record().network_id;
        let inventory =
            global_beacon_public_inventory_wire_v1(network, [(fixture.session.record(), 1)])
                .unwrap();
        let encoded = norito::encode_canonical(&inventory).unwrap();
        let length = u64::try_from(encoded.len()).unwrap().to_be_bytes();
        let limit =
            u64::try_from(CONSENSUS_THRESHOLD_PUBLIC_INVENTORY_DOMAIN_V1.len() + 8 + encoded.len())
                .unwrap();
        let (expected, used) = iroha_crypto::sha256_reader_bounded(
            CONSENSUS_THRESHOLD_PUBLIC_INVENTORY_DOMAIN_V1
                .chain(length.as_slice())
                .chain(encoded.as_slice()),
            limit,
        )
        .unwrap();
        assert_eq!(used, limit);
        let mut actual = [0; 32];
        assert_eq!(
            allocations_during(
                || actual = consensus_threshold_public_inventory_digest_v1(&inventory).unwrap()
            ),
            0
        );
        assert_eq!(actual, expected);
    }
}

#[test]
fn secret_output_unwind_scrubs_initialized_bytes_before_original_refund() {
    let budget = fixture_budget();
    let fixture = fixture(4, &budget);
    let sources = [share(&fixture)];
    let floor = budget.reserved_bytes();
    let mut prepared = plan(&fixture, &budget).unwrap();
    encode_global_beacon_partial_signer_credential_v1(&mut prepared, &sources).unwrap();
    let output = prepared.into_credential().ok().unwrap();
    let bytes = output.as_slice().len();
    SCRUBBED.with(|slot| slot.set(Some((0, false))));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _original = output;
        panic!("controlled export unwind");
    }));
    assert!(result.is_err());
    SCRUBBED.with(|slot| assert_eq!(slot.replace(None), Some((bytes, true))));
    assert_eq!(budget.reserved_bytes(), floor);
}
