//! Global-beacon seat credential codec tests over real adaptive DKG shares.

use iroha_crypto::{Hash, HashOf, threshold_bls::AdaptiveThresholdBlsSecretShare};
use iroha_data_model::{
    block::BlockHeader,
    consensus::{GLOBAL_THRESHOLD_BEACON_VERSION_V1, GlobalThresholdBeaconDkgSessionV1},
};
use norito::NoritoSchema as _;

use super::*;
use crate::beacon::{
    GlobalThresholdBeaconChainAnchorV1, GlobalThresholdBeaconPartialSignerV1 as _,
    GlobalThresholdBeaconPulseAggregatorV1, ValidatedGlobalThresholdBeaconSessionV1,
    fixtures::{adaptive_beacon_fixture_for_session, adaptive_fixture_signing_keys},
    global_threshold_beacon_roster_hash_v1,
};
use iroha_model_base::peer::PeerId;

const HANDLE: &str = "software://iroha/consensus-threshold/primary";
const REVISION: u64 = 7;
const POLICY_DIGEST: [u8; 32] = [0xA7; 32];

struct SeatFixture {
    record: GlobalThresholdBeaconKeySessionV1,
    session: ValidatedGlobalThresholdBeaconSessionV1,
    components: Vec<Zeroizing<[[u8; 32]; 3]>>,
}

fn network_id(marker: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([marker; 32]),
    ))
}

fn seat_fixture(network_id: NetworkId, session_byte: u8, committee_size: u16) -> SeatFixture {
    let roster = adaptive_fixture_signing_keys(committee_size)
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let fixture = adaptive_beacon_fixture_for_session(GlobalThresholdBeaconDkgSessionV1 {
        version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        network_id,
        session_id: [session_byte; 32],
        attempt_id: [session_byte; 32],
        authority_generation: 0,
        roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
        committee_size,
        threshold: (committee_size - 1) / 3 + 1,
        start_height: 1,
        commitments_end_height: 2,
        deliveries_end_height: 3,
        acceptances_end_height: 4,
    });
    let components = (1..=committee_size)
        .map(|recipient| {
            let shares = fixture
                .dealer_secrets
                .iter()
                .zip(&fixture.dealer_commitments)
                .map(|(secret, dealer)| {
                    secret
                        .private_share(&fixture.parameters, dealer, recipient)
                        .expect("verified private contribution")
                })
                .collect::<Vec<_>>();
            AdaptiveThresholdBlsSecretShare::from_dealer_shares(
                &fixture.session.transcript,
                &shares,
            )
            .expect("aggregate seat share")
            .into_components_for_runtime_custody()
        })
        .collect();
    SeatFixture {
        record: fixture.session.record().clone(),
        session: fixture.session,
        components,
    }
}

fn provisioning(
    fixture: &SeatFixture,
    signer_index: u16,
) -> Vec<RuntimeGlobalBeaconShareProvisioningV1> {
    vec![RuntimeGlobalBeaconShareProvisioningV1::new(
        fixture.record.clone(),
        signer_index,
        Zeroizing::new(*fixture.components[usize::from(signer_index - 1)]),
    )]
}

fn decode_wire(bytes: &[u8]) -> RuntimeGlobalBeaconSignerCredentialWireV1 {
    decode_consensus_threshold_credential_v1(bytes).expect("decode canonical beacon credential")
}

fn pulse_aggregator(
    session: &ValidatedGlobalThresholdBeaconSessionV1,
) -> GlobalThresholdBeaconPulseAggregatorV1 {
    let anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: 40,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x81; 32])),
    };
    GlobalThresholdBeaconPulseAggregatorV1::new(
        session.clone(),
        41,
        anchor,
        crate::beacon::pulse_context_fixture_v1(),
    )
    .expect("canonical pulse")
}

#[test]
fn every_seat_credential_roundtrips_and_signs_verified_partials() {
    let network = network_id(0xC1);
    for committee_size in [4_u16, 7] {
        let fixture = seat_fixture(network, 0x71, committee_size);
        let mut aggregator = pulse_aggregator(&fixture.session);
        for signer_index in 1..=committee_size {
            let inventory = provisioning(&fixture, signer_index);
            let policy_digest =
                global_beacon_partial_signer_inventory_digest_v1(network, &inventory)
                    .expect("inventory digest");
            assert_eq!(
                policy_digest,
                global_beacon_partial_signer_public_inventory_digest_v1(
                    network,
                    &[(fixture.record.clone(), signer_index)]
                )
                .expect("public inventory digest")
            );
            let credential = encode_global_beacon_partial_signer_credential_v1(
                network,
                HANDLE,
                REVISION,
                policy_digest,
                inventory,
            )
            .expect("encode seat credential");
            for (handle, revision, digest, network) in [
                (
                    "software://iroha/consensus-threshold/other",
                    REVISION,
                    policy_digest,
                    network,
                ),
                (HANDLE, REVISION + 1, policy_digest, network),
                (HANDLE, REVISION, POLICY_DIGEST, network),
                (HANDLE, REVISION, policy_digest, network_id(0x91)),
            ] {
                assert_eq!(
                    decode_global_beacon_partial_signer_credential_v1(
                        &credential,
                        &network,
                        handle,
                        revision,
                        digest
                    )
                    .err(),
                    Some(ConsensusThresholdCredentialErrorV1::Rejected)
                );
            }
            let custody = decode_global_beacon_partial_signer_credential_v1(
                &credential,
                &network,
                HANDLE,
                REVISION,
                policy_digest,
            )
            .expect("decode exact seat credential");
            assert!(
                custody
                    .attest_partial_signing_capability(&fixture.session, signer_index)
                    .expect("owned seat")
                    .matches(&fixture.session, signer_index)
            );
            let other = if signer_index == committee_size {
                1
            } else {
                signer_index + 1
            };
            assert!(
                custody
                    .attest_partial_signing_capability(&fixture.session, other)
                    .is_err()
            );
            let partial = custody
                .sign_partial(&fixture.session, aggregator.payload())
                .expect("sign pulse partial");
            assert!(
                aggregator
                    .accept_partial(partial)
                    .expect("verified partial")
            );
        }
        aggregator.finalize().expect("threshold pulse");
    }
}

#[test]
fn credential_frames_and_schema_hashes_are_golden() {
    for (nominal, frame, expected_hash_hex, actual_hash) in [
        (
            RuntimeGlobalBeaconSignerCredentialWireV1::nominal_name(),
            RuntimeGlobalBeaconSignerCredentialWireV1::frame_name(),
            "0b311f1a10d971b693860f8fb160ed1c",
            norito::schema::identity::frame_hash::<RuntimeGlobalBeaconSignerCredentialWireV1>(),
        ),
        (
            RuntimeGlobalBeaconPublicInventoryWireV1::nominal_name(),
            RuntimeGlobalBeaconPublicInventoryWireV1::frame_name(),
            "ea71fde9b50685c39f6977c4f472ac39",
            norito::schema::identity::frame_hash::<RuntimeGlobalBeaconPublicInventoryWireV1>(),
        ),
    ] {
        assert!(nominal.starts_with("iroha_core::beacon::credential::"));
        assert!(frame.starts_with("iroha.runtime_provider_broker.v1.consensus_threshold."));
        assert_eq!(hex::encode(actual_hash), expected_hash_hex);
        assert_eq!(actual_hash, norito::core::schema_hash_for_name(&frame));
    }
    assert_eq!(
        std::any::type_name::<RuntimeGlobalBeaconSignerCredentialWireV1>(),
        RuntimeGlobalBeaconSignerCredentialWireV1::nominal_name()
    );
    assert_eq!(
        std::any::type_name::<RuntimeGlobalBeaconPublicInventoryWireV1>(),
        RuntimeGlobalBeaconPublicInventoryWireV1::nominal_name()
    );
    let wire = RuntimeGlobalBeaconSignerCredentialWireV1 {
        header: ConsensusThresholdCredentialHeaderV1::new(
            GLOBAL_BEACON_PARTIAL_SIGNER_SLOT_WIRE_ID_V1,
            network_id(0xC1),
            HANDLE.to_owned(),
            REVISION,
            POLICY_DIGEST,
        ),
        sessions: Vec::new(),
    };
    let encoded = norito::encode_canonical(&wire).expect("encode frame");
    let view = norito::core::from_bytes_view(&encoded).expect("valid frame header");
    assert_eq!(
        view.schema(),
        norito::schema::identity::frame_hash::<RuntimeGlobalBeaconSignerCredentialWireV1>()
    );
    let mut substituted = encoded.clone();
    substituted[6] ^= 1;
    assert!(matches!(
        norito::decode_canonical::<RuntimeGlobalBeaconSignerCredentialWireV1>(&substituted),
        Err(norito::Error::SchemaMismatch)
    ));
}

#[test]
fn secret_credential_encoding_ignores_ambient_layout_flags() {
    let wire = RuntimeGlobalBeaconSignerCredentialWireV1 {
        header: ConsensusThresholdCredentialHeaderV1::new(
            GLOBAL_BEACON_PARTIAL_SIGNER_SLOT_WIRE_ID_V1,
            network_id(0xC1),
            HANDLE.to_owned(),
            REVISION,
            POLICY_DIGEST,
        ),
        sessions: Vec::new(),
    };
    let canonical = norito::encode_canonical(&wire).expect("canonical fixture");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::core::to_bytes(&wire).expect("alternate-layout fixture")
    };
    assert_ne!(alternate, canonical, "fixture must exercise layout drift");
    let encoded = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        encode_consensus_threshold_secret_credential_v1(&wire).expect("canonical secret encoding")
    };
    assert_eq!(encoded.as_slice(), canonical.as_slice());
}

#[test]
fn public_inventory_digest_is_order_stable_seat_bound_and_wire_ordered() {
    let network = network_id(0xC1);
    let first = seat_fixture(network, 0x81, 4);
    let second = seat_fixture(network, 0x82, 4);
    let mut inventory = vec![
        provisioning(&first, 1).remove(0),
        provisioning(&second, 1).remove(0),
    ];
    let forward = global_beacon_partial_signer_inventory_digest_v1(network, &inventory)
        .expect("forward digest");
    inventory.reverse();
    let reverse = global_beacon_partial_signer_inventory_digest_v1(network, &inventory)
        .expect("reverse digest");
    assert_eq!(forward, reverse);
    let reverse_credential = encode_global_beacon_partial_signer_credential_v1(
        network, HANDLE, REVISION, reverse, inventory,
    )
    .expect("reverse-ordered inventory");
    let forward_credential = encode_global_beacon_partial_signer_credential_v1(
        network,
        HANDLE,
        REVISION,
        forward,
        vec![
            provisioning(&first, 1).remove(0),
            provisioning(&second, 1).remove(0),
        ],
    )
    .expect("forward-ordered inventory");
    assert_eq!(&*forward_credential, &*reverse_credential);
    decode_global_beacon_partial_signer_credential_v1(
        &forward_credential,
        &network,
        HANDLE,
        REVISION,
        forward,
    )
    .expect("two-session inventory");
    let mut reordered = decode_wire(&forward_credential);
    reordered.sessions.reverse();
    let reordered = encode_consensus_threshold_secret_credential_v1(&reordered)
        .expect("noncanonical session order");
    assert_eq!(
        decode_global_beacon_partial_signer_credential_v1(
            &reordered, &network, HANDLE, REVISION, forward
        )
        .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
    assert_ne!(
        global_beacon_partial_signer_inventory_digest_v1(network, &provisioning(&first, 1))
            .expect("seat one"),
        global_beacon_partial_signer_inventory_digest_v1(network, &provisioning(&first, 2))
            .expect("seat two")
    );
    // A share encoded under another seat's index fails Core share import.
    let mut swapped = provisioning(&first, 1);
    swapped[0] = RuntimeGlobalBeaconShareProvisioningV1::new(
        first.record.clone(),
        2,
        Zeroizing::new(*first.components[0]),
    );
    let digest = global_beacon_partial_signer_inventory_digest_v1(network, &swapped)
        .expect("seat-two digest");
    assert_eq!(
        encode_global_beacon_partial_signer_credential_v1(
            network, HANDLE, REVISION, digest, swapped
        )
        .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
}

#[test]
fn credential_header_substitution_and_noncanonical_bytes_fail_closed() {
    let network = network_id(0xC1);
    let fixture = seat_fixture(network, 0x73, 4);
    let inventory = provisioning(&fixture, 1);
    let policy_digest =
        global_beacon_partial_signer_inventory_digest_v1(network, &inventory).expect("digest");
    let credential = encode_global_beacon_partial_signer_credential_v1(
        network,
        HANDLE,
        REVISION,
        policy_digest,
        inventory,
    )
    .expect("valid credential");
    for substitution in 0..7 {
        let mut wire = decode_wire(&credential);
        match substitution {
            0 => wire.header.network_id = network_id(0x91),
            1 => wire.header.handle = "software://iroha/consensus-threshold/other".to_owned(),
            2 => wire.header.revision += 1,
            3 => wire.header.policy_digest = [0xA8; 32],
            4 => wire.header.slot = GLOBAL_BEACON_PARTIAL_SIGNER_SLOT_WIRE_ID_V1 + 1,
            5 => wire.header.magic[0] ^= 1,
            _ => wire.header.version += 1,
        }
        let substituted = encode_consensus_threshold_secret_credential_v1(&wire)
            .expect("structurally canonical substitution");
        assert_eq!(
            decode_global_beacon_partial_signer_credential_v1(
                &substituted,
                &network,
                HANDLE,
                REVISION,
                policy_digest
            )
            .err(),
            Some(ConsensusThresholdCredentialErrorV1::Rejected)
        );
    }
    let mut noncanonical = Zeroizing::new(credential.to_vec());
    noncanonical.push(0);
    assert_eq!(
        decode_global_beacon_partial_signer_credential_v1(
            &noncanonical,
            &network,
            HANDLE,
            REVISION,
            policy_digest
        )
        .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
}

#[test]
fn credential_header_reads_the_public_binding_of_canonical_bytes_only() {
    let network = network_id(0xC1);
    let fixture = seat_fixture(network, 0x75, 4);
    let inventory = provisioning(&fixture, 2);
    let policy_digest =
        global_beacon_partial_signer_inventory_digest_v1(network, &inventory).expect("digest");
    let credential = encode_global_beacon_partial_signer_credential_v1(
        network,
        HANDLE,
        REVISION,
        policy_digest,
        inventory,
    )
    .expect("valid credential");
    let header =
        global_beacon_partial_signer_credential_header_v1(&credential).expect("canonical header");
    assert_eq!(
        header,
        ConsensusThresholdCredentialHeaderV1::new(
            GLOBAL_BEACON_PARTIAL_SIGNER_SLOT_WIRE_ID_V1,
            network,
            HANDLE.to_owned(),
            REVISION,
            policy_digest,
        )
    );
    // The derived binding imports exactly the credential it was read from.
    decode_global_beacon_partial_signer_credential_v1(
        &credential,
        &header.network_id,
        &header.handle,
        header.revision,
        header.policy_digest,
    )
    .expect("header-derived binding imports the credential");
    let mut trailing = Zeroizing::new(credential.to_vec());
    trailing.push(0);
    assert_eq!(
        global_beacon_partial_signer_credential_header_v1(&trailing).err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
    assert_eq!(
        global_beacon_partial_signer_credential_header_v1(&credential[..credential.len() - 1])
            .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
    assert_eq!(
        global_beacon_partial_signer_credential_header_v1(&[]).err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
}

#[test]
fn decoded_shares_reencode_the_exact_credential_and_only_after_validation() {
    let network = network_id(0xC1);
    let first = seat_fixture(network, 0x91, 4);
    let second = seat_fixture(network, 0x92, 4);
    let mut inventory = provisioning(&first, 2);
    inventory.extend(provisioning(&second, 3));
    let digest =
        global_beacon_partial_signer_inventory_digest_v1(network, &inventory).expect("digest");
    let credential = encode_global_beacon_partial_signer_credential_v1(
        network, HANDLE, REVISION, digest, inventory,
    )
    .expect("two-session credential");
    let shares = decode_global_beacon_partial_signer_credential_shares_v1(
        &credential,
        &network,
        HANDLE,
        REVISION,
        digest,
    )
    .expect("validated shares");
    let mut seats = shares
        .iter()
        .map(|share| (share.public_session().session_id, share.signer_index()))
        .collect::<Vec<_>>();
    seats.sort_unstable();
    assert_eq!(seats, [([0x91; 32], 2), ([0x92; 32], 3)]);
    let reencoded = encode_global_beacon_partial_signer_credential_v1(
        network, HANDLE, REVISION, digest, shares,
    )
    .expect("re-encode decoded shares");
    assert_eq!(reencoded.as_slice(), credential.as_slice());
    assert_eq!(
        decode_global_beacon_partial_signer_credential_shares_v1(
            &credential,
            &network,
            HANDLE,
            REVISION + 1,
            digest,
        )
        .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
}

#[test]
fn same_revision_public_inventory_substitution_fails_closed() {
    let network = network_id(0xC1);
    let expected = seat_fixture(network, 0x87, 4);
    let expected_digest =
        global_beacon_partial_signer_inventory_digest_v1(network, &provisioning(&expected, 1))
            .expect("expected digest");
    let substituted = seat_fixture(network, 0x88, 4);
    let inventory = provisioning(&substituted, 1);
    let substituted_digest =
        global_beacon_partial_signer_inventory_digest_v1(network, &inventory).expect("digest");
    assert_ne!(expected_digest, substituted_digest);
    let credential = encode_global_beacon_partial_signer_credential_v1(
        network,
        HANDLE,
        REVISION,
        substituted_digest,
        inventory,
    )
    .expect("substituted inventory");
    let mut wire = decode_wire(&credential);
    wire.header.policy_digest = expected_digest;
    let rebound = encode_consensus_threshold_secret_credential_v1(&wire).expect("rebound header");
    assert_eq!(
        decode_global_beacon_partial_signer_credential_v1(
            &rebound,
            &network,
            HANDLE,
            REVISION,
            expected_digest
        )
        .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
}

#[test]
fn duplicate_empty_cross_network_and_invalid_inventories_fail_closed() {
    let network = network_id(0xC1);
    assert_eq!(
        encode_global_beacon_partial_signer_credential_v1(
            network,
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            Vec::new()
        )
        .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
    let fixture = seat_fixture(network, 0x74, 4);
    let duplicate = vec![
        provisioning(&fixture, 1).remove(0),
        provisioning(&fixture, 1).remove(0),
    ];
    let digest =
        global_beacon_partial_signer_inventory_digest_v1(network, &duplicate).expect("digest");
    assert_eq!(
        encode_global_beacon_partial_signer_credential_v1(
            network, HANDLE, REVISION, digest, duplicate
        )
        .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
    assert_eq!(
        global_beacon_partial_signer_inventory_digest_v1(
            network_id(0x91),
            &provisioning(&fixture, 1)
        )
        .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
    let invalid = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
        fixture.record.clone(),
        1,
        Zeroizing::new([[0; 32]; 3]),
    )];
    let digest =
        global_beacon_partial_signer_inventory_digest_v1(network, &invalid).expect("digest");
    assert_eq!(
        encode_global_beacon_partial_signer_credential_v1(
            network, HANDLE, REVISION, digest, invalid
        )
        .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected)
    );
    let inventory = provisioning(&fixture, 1);
    let digest =
        global_beacon_partial_signer_inventory_digest_v1(network, &inventory).expect("digest");
    assert_eq!(
        encode_global_beacon_partial_signer_credential_v1(
            network,
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            inventory
        )
        .err(),
        Some(ConsensusThresholdCredentialErrorV1::Rejected),
        "a supplied digest must match the inventory"
    );
    for (handle, revision) in [("software://iroha/test/primary", REVISION), (HANDLE, 0)] {
        assert_eq!(
            encode_global_beacon_partial_signer_credential_v1(
                network,
                handle,
                revision,
                digest,
                provisioning(&fixture, 1)
            )
            .err(),
            Some(ConsensusThresholdCredentialErrorV1::Rejected)
        );
    }
}

#[test]
fn shared_framing_validators_reject_malformed_qualification() {
    let network = network_id(0xC1);
    assert!(validate_consensus_threshold_session_count_v1(1).is_ok());
    assert!(
        validate_consensus_threshold_session_count_v1(
            MAX_CONSENSUS_THRESHOLD_CREDENTIAL_SESSIONS_V1
        )
        .is_ok()
    );
    for count in [0, MAX_CONSENSUS_THRESHOLD_CREDENTIAL_SESSIONS_V1 + 1] {
        assert_eq!(
            validate_consensus_threshold_session_count_v1(count),
            Err(ConsensusThresholdCredentialErrorV1::Rejected)
        );
    }
    assert!(
        validate_consensus_threshold_provisioning_v1(&network, HANDLE, REVISION, POLICY_DIGEST)
            .is_ok()
    );
    for (network, handle, revision, digest) in [
        (
            network,
            "software://iroha/mock/primary",
            REVISION,
            POLICY_DIGEST,
        ),
        (network, "", REVISION, POLICY_DIGEST),
        (network, HANDLE, 0, POLICY_DIGEST),
        (network, HANDLE, REVISION, [0; 32]),
    ] {
        assert_eq!(
            validate_consensus_threshold_provisioning_v1(&network, handle, revision, digest),
            Err(ConsensusThresholdCredentialErrorV1::Rejected)
        );
    }
    let header = ConsensusThresholdCredentialHeaderV1::new(
        60,
        network,
        HANDLE.to_owned(),
        REVISION,
        POLICY_DIGEST,
    );
    assert!(
        header
            .validate(60, &network, HANDLE, REVISION, POLICY_DIGEST)
            .is_ok()
    );
    assert!(
        header
            .validate(
                GLOBAL_BEACON_PARTIAL_SIGNER_SLOT_WIRE_ID_V1,
                &network,
                HANDLE,
                REVISION,
                POLICY_DIGEST
            )
            .is_err()
    );
    let zero_revision =
        ConsensusThresholdCredentialHeaderV1::new(60, network, HANDLE.to_owned(), 0, POLICY_DIGEST);
    assert!(
        zero_revision
            .validate(60, &network, HANDLE, 0, POLICY_DIGEST)
            .is_err()
    );
    let triple =
        ConsensusThresholdSecretScalarTripleV1::from_zeroizing(Zeroizing::new([[7; 32]; 3]));
    assert_eq!(*triple.into_zeroizing(), [[7; 32]; 3]);
    let digest = consensus_threshold_public_inventory_digest_v1(&header.handle)
        .expect("digest of a public value");
    assert_ne!(digest, [0; 32]);
    assert_eq!(
        digest,
        consensus_threshold_public_inventory_digest_v1(&HANDLE.to_owned()).expect("stable digest")
    );
}

#[test]
fn decode_budget_scales_with_the_frame_and_rejects_empty_or_oversized_bytes() {
    let small = consensus_threshold_credential_decode_limits_v1(4096);
    let largest = consensus_threshold_credential_decode_limits_v1(
        MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1,
    );
    for limits in [small, largest] {
        assert_eq!(limits.max_sequence_elements(), 16_384);
        assert_eq!(limits.max_nesting_depth(), 64);
    }
    assert_eq!(small.max_field_bytes(), 4096);
    assert!(small.max_total_allocated_bytes() < largest.max_total_allocated_bytes());
    // Signed all-edge transcripts decode to about 16 times their encoded size.
    assert!(
        largest.max_total_allocated_bytes() >= 16 * MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1
    );
    for bytes in [
        Vec::new(),
        vec![0; MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1 + 1],
    ] {
        assert_eq!(
            decode_consensus_threshold_credential_v1::<RuntimeGlobalBeaconSignerCredentialWireV1>(
                &bytes
            )
            .err(),
            Some(ConsensusThresholdCredentialErrorV1::Rejected)
        );
    }
}
