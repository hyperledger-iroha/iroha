//! Actual BLS/checkpoint/WAL clock verification using explicitly synthetic certified fixture data.
use super::*;
use iroha_crypto::{KeyPair, SignatureOf};
use iroha_data_model::{
    sumeragi::SumeragiStatus,
    sumeragi_finality::{SumeragiFinalityAttestationBody, test_fixtures::NativeFinalityFixture},
};
use norito::codec::Encode as _;

/// Check the declared identity and complete current frame independently of Rust module paths.
fn canonical_clock_frame<T>(value: &T, identity: &str)
where
    T: norito::core::NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    assert_eq!(T::nominal_name(), identity);
    assert_eq!(T::frame_name(), identity);
    let original = norito::encode_canonical(value).unwrap();
    let header = norito::core::Header::read(std::io::Cursor::new(&original)).unwrap();
    assert_eq!(header.schema, norito::core::schema_hash_for_name(identity));
    let decoded: T = norito::decode_canonical(&original).unwrap();
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), original);

    let mut substituted = original.clone();
    substituted[6..22].copy_from_slice(&norito::schema::identity::frame_hash::<u8>());
    assert!(norito::decode_canonical::<T>(&substituted).is_err());
    let mut trailing = original.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    assert!(norito::decode_canonical::<T>(&original[..original.len() - 1]).is_err());
}

#[test]
fn clock_selection_and_private_wal_frames_advertise_their_exact_current_schema() {
    let policy = KagemushaOrdinaryNativeClockPolicyV1 {
        maximum_reply_age_ms: 10_000,
        maximum_node_skew_ms: 100,
        maximum_projection_age_ms: 120_000,
        maximum_persistence_age_ms: 1_000,
    };
    canonical_clock_frame(
        &policy,
        "iroha_core_zk::ordinary_native_clock::KagemushaOrdinaryNativeClockPolicyV1",
    );
    let node = KagemushaOrdinaryNativeClockNodeV1 {
        peer_id: PeerId::new(
            KeyPair::from_seed(vec![1; 32], Algorithm::BlsNormal)
                .public_key()
                .clone(),
        ),
        build_fingerprint: Hash::new(b"clock schema fixture executable"),
        config_fingerprint: Hash::new(b"clock schema fixture config"),
    };
    canonical_clock_frame(
        &node,
        "iroha_core_zk::ordinary_native_clock::KagemushaOrdinaryNativeClockNodeV1",
    );
    let initialize = Initialize {
        selection_digest: [3; 32],
    };
    canonical_clock_frame(
        &initialize,
        "iroha_core_zk::ordinary_native_clock::InitializeV1",
    );
    let observation = Observation {
        nonce: [4; 32],
        originals: std::array::from_fn(|index| vec![index as u8 + 5; index + 1]),
        median_ms: 1_000_003,
        high_water_ms: 1_001_003,
        certified_context_id: Hash::new(b"clock schema fixture certified context"),
    };
    canonical_clock_frame(
        &observation,
        "iroha_core_zk::ordinary_native_clock::ObservationV1",
    );
    let projected = Projected {
        observation_digest: [9; 32],
        sampled_at_ms: 1_000_004,
        high_water_ms: 1_001_004,
    };
    canonical_clock_frame(
        &projected,
        "iroha_core_zk::ordinary_native_clock::ProjectedV1",
    );
    // These are codec fixtures; signed observation admission remains covered separately.
    for record in [
        Record::Initialize(Box::new(initialize)),
        Record::Finality(Box::new(vec![10, 11, 12])),
        Record::Observation(Box::new(observation)),
        Record::Projected(Box::new(projected)),
    ] {
        canonical_clock_frame(&record, "iroha_core_zk::ordinary_native_clock::RecordV1");
        let original = encode(&record).unwrap();
        assert_eq!(encode(&decode(&original).unwrap()).unwrap(), original);
        let mut trailing = original;
        trailing.push(0);
        assert!(decode(&trailing).is_err());
    }
}

struct Fixture {
    native: NativeFinalityFixture,
    nodes: [KagemushaOrdinaryNativeClockNodeV1; 4],
    selected: Arc<KagemushaOrdinaryNativeClockOriginalsV1>,
}
impl Fixture {
    fn new() -> Self {
        Self::with_projection_age(120_000)
    }
    fn with_projection_age(maximum_projection_age_ms: u64) -> Self {
        let mut native = NativeFinalityFixture::start("ordinary-native-clock-tests");
        native.certify_with_world_root(
            native.block_with_submitted_work(native.next_header()),
            Hash::new(b"explicitly synthetic clock fixture World"),
        );
        let nodes = std::array::from_fn(|index| KagemushaOrdinaryNativeClockNodeV1 {
            peer_id: PeerId::new(
                KeyPair::from_seed(vec![index as u8 + 1; 32], Algorithm::BlsNormal)
                    .public_key()
                    .clone(),
            ),
            build_fingerprint: Hash::new(b"independently selected fixture node executable"),
            config_fingerprint: Hash::new(b"independently selected fixture effective config"),
        });
        let checkpoint = native
            .verifier()
            .export_checkpoint(native.latest())
            .unwrap();
        let selected = Arc::new(
            KagemushaOrdinaryNativeClockOriginalsV1::from_selected_originals(
                checkpoint,
                native.network_id(),
                "ordinary-native-clock-tests".into(),
                nodes.clone(),
                KagemushaOrdinaryNativeClockPolicyV1 {
                    maximum_reply_age_ms: 10_000,
                    maximum_node_skew_ms: 100,
                    maximum_projection_age_ms,
                    maximum_persistence_age_ms: 1_000,
                },
            )
            .unwrap(),
        );
        Self {
            native,
            nodes,
            selected,
        }
    }
    fn replies(&self, nonce: [u8; 32], times: [u64; 4]) -> [Vec<u8>; 4] {
        std::array::from_fn(|index| {
            let signer = KeyPair::from_seed(vec![index as u8 + 1; 32], Algorithm::BlsNormal);
            let selected = &self.nodes[index];
            let body = SumeragiFinalityAttestationBody {
                challenge: nonce,
                observed_at_unix_ms: times[index],
                network_id: self.native.network_id(),
                node_id: selected.peer_id.clone(),
                node_fingerprint: Hash::new(selected.peer_id.encode()),
                build_fingerprint: selected.build_fingerprint,
                config_fingerprint: selected.config_fingerprint,
                genesis_block_hash: self.native.genesis().hash(),
                genesis_finality_proof: self.native.genesis_proof().clone(),
                status: SumeragiStatus {
                    protocol_version: 1,
                    config_fingerprint: selected.config_fingerprint,
                    beacon_horizon: None,
                    instance: self.native.verifier().instance().0,
                    height: self.native.latest().height() + 1,
                    view: 0,
                    stage: 0,
                    leader: None,
                    proxy_tail: None,
                    high_qc_view: None,
                    level: 0,
                    start_level: 0,
                    t_retx_ms: 100,
                    committed_height: self.native.latest().height(),
                    applied_height: self.native.latest().height(),
                    awaiting: false,
                    signer: Some(signer.public_key().clone()),
                    unanchored: false,
                    abstaining: false,
                    halted: None,
                    footprint: Default::default(),
                },
                finality_proof: self.native.latest().clone(),
            };
            let reply = SumeragiFinalityAttestation {
                signature: SignatureOf::try_from_hash(signer.private_key(), body.signing_hash())
                    .unwrap(),
                body,
            };
            norito::encode_canonical(&reply).unwrap()
        })
    }
    fn resign_changed(
        &self,
        raw: &mut Vec<u8>,
        index: usize,
        mutate: impl FnOnce(&mut SumeragiFinalityAttestationBody),
    ) {
        let mut reply: SumeragiFinalityAttestation = norito::decode_canonical(raw).unwrap();
        mutate(&mut reply.body);
        let signer = KeyPair::from_seed(vec![index as u8 + 1; 32], Algorithm::BlsNormal);
        reply.signature =
            SignatureOf::try_from_hash(signer.private_key(), reply.body.signing_hash()).unwrap();
        *raw = norito::encode_canonical(&reply).unwrap();
    }
}

#[test]
fn actual_signed_clock_retains_median_and_requires_fresh_nonce_after_cold_recovery() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    assert!(owner.current_native_time_ms().is_err());
    let read = owner.reserve_current_read().unwrap();
    let nonce = read.nonce();
    owner
        .admit_current_read(
            read,
            fixture.replies(nonce, [1_000_000, 1_000_002, 1_000_004, 1_000_006]),
        )
        .unwrap();
    let before = owner.current_native_time_ms().unwrap();
    assert!(before >= 1_000_003 && before < 1_010_003);
    let prefix = owner.journal.recovery_prefix().unwrap();
    drop(owner);
    let mut recovered = KagemushaOrdinaryNativeClockOwnerV1::open_existing(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    assert_eq!(recovered.journal.recovery_prefix().unwrap(), prefix);
    assert!(
        recovered.current_native_time_ms().is_err(),
        "cold recovery cannot borrow elapsed time from another boot"
    );
    let read = recovered.reserve_current_read().unwrap();
    let fresh_nonce = read.nonce();
    assert_ne!(nonce, fresh_nonce);
    assert!(
        recovered
            .admit_current_read(read, fixture.replies(nonce, [1_030_000; 4]))
            .is_err(),
        "old valid node originals cannot answer a fresh Native read"
    );
    let read = recovered.reserve_current_read().unwrap();
    let originals = fixture.replies(read.nonce(), [1_030_000; 4]);
    recovered.admit_current_read(read, originals).unwrap();
    assert!(recovered.current_native_time_ms().unwrap() >= 1_030_000);
    let read = recovered.reserve_current_read().unwrap();
    let originals = fixture.replies(read.nonce(), [1_000_000; 4]);
    assert!(
        recovered.admit_current_read(read, originals).is_err(),
        "genuine signed clocks cannot regress the durable high-water"
    );
}

#[test]
fn signed_clock_rejects_current_node_runtime_nonce_skew_and_foreign_owner() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let foreign = KagemushaOrdinaryNativeClockOwnerV1::create(
        &other.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let read = foreign.reserve_current_read().unwrap();
    let originals = fixture.replies(read.nonce(), [1_000_000; 4]);
    assert!(owner.admit_current_read(read, originals).is_err());
    for mutation in 0..4 {
        let read = owner.reserve_current_read().unwrap();
        let mut originals = fixture.replies(read.nonce(), [1_000_000; 4]);
        fixture.resign_changed(&mut originals[0], 0, |body| match mutation {
            0 => body.build_fingerprint = Hash::new(b"other executable"),
            1 => body.config_fingerprint = Hash::new(b"other installed config"),
            2 => body.challenge = [99; 32],
            _ => body.observed_at_unix_ms += 101,
        });
        assert!(
            owner.admit_current_read(read, originals).is_err(),
            "signed mutation {mutation}"
        );
    }
    assert!(owner.current_native_time_ms().is_err());
    assert_eq!(
        owner.rows, 1,
        "rejected replies cannot append a clock authority"
    );
}

#[test]
fn clock_never_recreates_original_wal_or_lends_expired_native_reference() {
    let fixture = Fixture::with_projection_age(10_000);
    let temporary = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    assert!(
        KagemushaOrdinaryNativeClockOwnerV1::create(
            &temporary.path().canonicalize().unwrap(),
            fixture.selected.clone()
        )
        .is_err()
    );
    let read = owner.reserve_current_read().unwrap();
    let originals = fixture.replies(read.nonce(), [1_000_000; 4]);
    owner.admit_current_read(read, originals).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10_000));
    assert!(
        owner.current_native_time_ms().is_err(),
        "actual elapsed expiry has no handset-clock repair"
    );
    drop(owner);
    let recovered = KagemushaOrdinaryNativeClockOwnerV1::open_existing(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    assert!(recovered.reference.is_none());
}

#[test]
fn retained_nonce_history_refuses_nonadjacent_signed_replay() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let read = owner.reserve_current_read().unwrap();
    let first_nonce = read.nonce();
    owner
        .admit_current_read(read, fixture.replies(first_nonce, [1_000_000; 4]))
        .unwrap();
    let read = owner.reserve_current_read().unwrap();
    let originals = fixture.replies(read.nonce(), [1_030_000; 4]);
    owner.admit_current_read(read, originals).unwrap();
    // Deliberately append a fully signed historical nonce with a later time, using the private
    // test-only journal access. High-water alone cannot distinguish this replay from a fresh read.
    let originals = fixture.replies(first_nonce, [1_060_000; 4]);
    let (_, context) = owner.verify_observation(first_nonce, &originals).unwrap();
    owner
        .append(&Record::Observation(Box::new(Observation {
            nonce: first_nonce,
            originals,
            median_ms: 1_060_000,
            high_water_ms: 1_060_000,
            certified_context_id: context,
        })))
        .unwrap();
    drop(owner);
    assert!(
        KagemushaOrdinaryNativeClockOwnerV1::open_existing(
            &temporary.path().canonicalize().unwrap(),
            fixture.selected.clone()
        )
        .is_err()
    );
}

#[test]
fn durable_ceiling_never_satisfies_a_future_not_before_interval() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let read = owner.reserve_current_read().unwrap();
    let originals = fixture.replies(read.nonce(), [1_000_000; 4]);
    owner.admit_current_read(read, originals).unwrap();
    let actual = owner.current_native_time_ms().unwrap();
    let future_not_before = owner.high_water_ms;
    assert!(actual < future_not_before);
    assert!(
        !(future_not_before..future_not_before + 10_000).contains(&actual),
        "only the post-fsync sample, never the reserved upper bound, may satisfy not-before"
    );
    let again = owner.current_native_time_ms().unwrap();
    assert!(again >= actual && again < future_not_before);
}

#[test]
fn slow_publication_cannot_lend_a_pre_fsync_clock_sample() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let read = owner.reserve_current_read().unwrap();
    let originals = fixture.replies(read.nonce(), [1_000_000; 4]);
    owner.admit_current_read(read, originals).unwrap();
    // This hook delays only the test owner after the real WAL append/fsync. It supplies no
    // timestamp and does not exist in production; the actual Native elapsed clock keeps running.
    owner.persistence_delay = Duration::from_millis(1_001);
    assert!(owner.current_native_time_ms().is_err());
    assert_eq!(
        owner.rows, 3,
        "the uncertain loan still retains its durable ceiling"
    );
    assert!(owner.reference.as_ref().unwrap().last_lent_upper_ms < owner.high_water_ms);
}

#[test]
fn fresh_low_latency_signed_read_must_reach_the_durable_ceiling() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let read = owner.reserve_current_read().unwrap();
    let originals = fixture.replies(read.nonce(), [1_000_000; 4]);
    std::thread::sleep(Duration::from_millis(60));
    owner.admit_current_read(read, originals).unwrap();
    owner.current_native_time_ms().unwrap();
    let prefix = owner.journal.recovery_prefix().unwrap();
    let read = owner.reserve_current_read().unwrap();
    let originals = fixture.replies(read.nonce(), [1_000_000; 4]);
    assert!(
        owner.admit_current_read(read, originals).is_err(),
        "a genuine fresh nonce cannot repair a lower signed reading with the old ceiling"
    );
    assert_eq!(owner.journal.recovery_prefix().unwrap(), prefix);
}

#[test]
fn recovery_refuses_a_projected_ceiling_outside_the_original_window() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let read = owner.reserve_current_read().unwrap();
    let originals = fixture.replies(read.nonce(), [1_000_000; 4]);
    owner.admit_current_read(read, originals).unwrap();
    let end = 1_000_000 + fixture.selected.policy.maximum_projection_age_ms;
    owner
        .append(&Record::Projected(Box::new(Projected {
            observation_digest: owner.observation_digest,
            sampled_at_ms: end - fixture.selected.policy.maximum_persistence_age_ms,
            high_water_ms: end,
        })))
        .unwrap();
    drop(owner);
    assert!(
        KagemushaOrdinaryNativeClockOwnerV1::open_existing(
            &temporary.path().canonicalize().unwrap(),
            fixture.selected.clone(),
        )
        .is_err()
    );
}

#[test]
fn nonzero_request_latency_cannot_admit_a_future_not_before_and_does_not_extend_expiry() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let read = owner.reserve_current_read().unwrap();
    // Real request latency occurs before the validators make their signed sample. Counting it
    // again may conservatively overestimate expiry, but cannot advance activation.
    std::thread::sleep(Duration::from_millis(80));
    let originals = fixture.replies(read.nonce(), [1_000_000; 4]);
    owner.admit_current_read(read, originals).unwrap();
    let interval = owner.current_native_time_interval().unwrap();
    assert!(interval.upper_ms() - interval.lower_ms() >= 80);
    let future = interval.lower_ms() + 30;
    assert!(future <= interval.upper_ms());
    assert!(
        interval
            .require_validity(future, interval.upper_ms() + 10_000)
            .is_err()
    );
    assert!(
        interval
            .require_validity(interval.lower_ms(), interval.upper_ms())
            .is_err()
    );
    assert!(
        interval
            .require_validity(interval.lower_ms(), interval.upper_ms() + 1)
            .is_ok()
    );
}

#[test]
fn retained_clock_originals_survive_fresh_observation_and_cold_recovery_without_time_grant() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let mut owner =
        KagemushaOrdinaryNativeClockOwnerV1::create(&root, fixture.selected.clone()).unwrap();
    let read = owner.reserve_current_read().unwrap();
    let first_originals =
        fixture.replies(read.nonce(), [1_000_000, 1_000_002, 1_000_004, 1_000_006]);
    owner
        .admit_current_read(read, first_originals.clone())
        .unwrap();
    let context = owner.current_cash_clock_context().unwrap();
    let loan = owner.retained_cash_clock_originals(&context).unwrap();
    let first_frame = loan.canonical_original().to_vec();
    let decoded =
        KagemushaOrdinaryNativeSignedClockOriginalV1::decode_original(&first_frame).unwrap();
    assert_eq!(decoded.signed_observations(), &first_originals);
    let read = owner.reserve_current_read().unwrap();
    let nonce = read.nonce();
    owner
        .admit_current_read(
            read,
            fixture.replies(nonce, [1_010_000, 1_010_002, 1_010_004, 1_010_006]),
        )
        .unwrap();
    loan.recheck(&owner).unwrap();
    assert_eq!(
        owner
            .retained_cash_clock_originals(&context)
            .unwrap()
            .canonical_original(),
        first_frame
    );
    drop(owner);
    let mut recovered =
        KagemushaOrdinaryNativeClockOwnerV1::open_existing(&root, fixture.selected.clone())
            .unwrap();
    assert!(
        loan.recheck(&recovered).is_err(),
        "process-bound loan does not cross owner identity"
    );
    assert!(recovered.current_native_time_interval().is_err());
    assert_eq!(
        recovered
            .retained_cash_clock_originals(&context)
            .unwrap()
            .canonical_original(),
        first_frame
    );
    assert!(
        recovered.current_native_time_interval().is_err(),
        "borrowing old originals cannot restore elapsed-clock freshness"
    );
}

#[test]
fn retained_clock_original_context_requires_exact_nonce_digest_and_finite_bounds() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let read = owner.reserve_current_read().unwrap();
    let nonce = read.nonce();
    owner
        .admit_current_read(
            read,
            fixture.replies(nonce, [1_000_000, 1_000_002, 1_000_004, 1_000_006]),
        )
        .unwrap();
    let context = owner.current_cash_clock_context().unwrap();
    for field in 0..4 {
        let mut changed = context.clone();
        match field {
            0 => changed.request_nonce[0] ^= 1,
            1 => changed.signed_observations_original_digest[0] ^= 1,
            2 => changed.lower_at_ms = 1_000_002,
            _ => changed.upper_at_ms = 1_120_003,
        }
        assert!(owner.retained_cash_clock_originals(&changed).is_err());
    }
}

#[test]
fn signed_clock_public_frame_authenticates_complete_originals_and_refuses_codec_substitution() {
    let fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let mut owner = KagemushaOrdinaryNativeClockOwnerV1::create(
        &temporary.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let read = owner.reserve_current_read().unwrap();
    let nonce = read.nonce();
    owner
        .admit_current_read(
            read,
            fixture.replies(nonce, [1_000_000, 1_000_002, 1_000_004, 1_000_006]),
        )
        .unwrap();
    let context = owner.current_cash_clock_context().unwrap();
    let loan = owner.retained_cash_clock_originals(&context).unwrap();
    let raw = loan.canonical_original();
    let original = KagemushaOrdinaryNativeSignedClockOriginalV1::decode_original(raw).unwrap();
    canonical_clock_frame(
        &original,
        "iroha_core_zk::ordinary_native_clock::KagemushaOrdinaryNativeSignedClockOriginalV1",
    );
    let verified =
        verify_ordinary_native_signed_clock_original_v1(&fixture.selected, &owner.verifier, raw)
            .unwrap();
    verified.recheck_cash_context(&context).unwrap();
    assert_eq!(verified.original(), raw);
    let mut trailing = raw.to_vec();
    trailing.push(0);
    assert!(KagemushaOrdinaryNativeSignedClockOriginalV1::decode_original(&trailing).is_err());
    let mut changed = original.clone();
    fixture.resign_changed(&mut changed.originals[0], 0, |body| {
        body.build_fingerprint = Hash::new(b"foreign node executable")
    });
    assert!(
        verify_ordinary_native_signed_clock_original_v1(
            &fixture.selected,
            &owner.verifier,
            &changed.canonical_original().unwrap()
        )
        .is_err()
    );
    let mut changed = original;
    changed.certified_context_id = Hash::new(b"foreign certified context");
    assert!(
        verify_ordinary_native_signed_clock_original_v1(
            &fixture.selected,
            &owner.verifier,
            &changed.canonical_original().unwrap()
        )
        .is_err()
    );
}

#[test]
fn historical_clock_original_lookup_retains_earlier_authenticated_finality_decision() {
    let mut fixture = Fixture::new();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let mut owner =
        KagemushaOrdinaryNativeClockOwnerV1::create(&root, fixture.selected.clone()).unwrap();
    let read = owner.reserve_current_read().unwrap();
    let nonce = read.nonce();
    owner
        .admit_current_read(
            read,
            fixture.replies(nonce, [1_000_000, 1_000_002, 1_000_004, 1_000_006]),
        )
        .unwrap();
    let context = owner.current_cash_clock_context().unwrap();
    let raw = owner
        .retained_cash_clock_originals(&context)
        .unwrap()
        .canonical_original()
        .to_vec();
    fixture.native.certify_with_world_root(
        fixture
            .native
            .block_with_submitted_work(fixture.native.next_header()),
        Hash::new(b"second explicit synthetic World"),
    );
    let successor = norito::encode_canonical(fixture.native.latest()).unwrap();
    owner.advance_certified_prefix(&successor).unwrap();
    assert!(owner.current_native_time_interval().is_err());
    assert_eq!(
        owner
            .retained_cash_clock_originals(&context)
            .unwrap()
            .canonical_original(),
        raw
    );
    drop(owner);
    let mut recovered =
        KagemushaOrdinaryNativeClockOwnerV1::open_existing(&root, fixture.selected.clone())
            .unwrap();
    assert_eq!(
        recovered
            .retained_cash_clock_originals(&context)
            .unwrap()
            .canonical_original(),
        raw
    );
    assert!(recovered.current_native_time_interval().is_err());
}

#[test]
fn received_historical_signed_clock_authenticates_sender_originals_without_native_time_grant() {
    let fixture = Fixture::new();
    let sender_root = tempfile::tempdir().unwrap();
    let mut sender = KagemushaOrdinaryNativeClockOwnerV1::create(
        &sender_root.path().canonicalize().unwrap(),
        fixture.selected.clone(),
    )
    .unwrap();
    let read = sender.reserve_current_read().unwrap();
    let nonce = read.nonce();
    sender
        .admit_current_read(
            read,
            fixture.replies(nonce, [1_000_000, 1_000_002, 1_000_004, 1_000_006]),
        )
        .unwrap();
    let context = sender.current_cash_clock_context().unwrap();
    let raw = sender
        .retained_cash_clock_originals(&context)
        .unwrap()
        .canonical_original()
        .to_vec();
    let receiver_root = tempfile::tempdir().unwrap();
    let root = receiver_root.path().canonicalize().unwrap();
    let receiver =
        KagemushaOrdinaryNativeClockOwnerV1::create(&root, fixture.selected.clone()).unwrap();
    let prefix = receiver.journal.recovery_prefix().unwrap();
    let admitted = receiver
        .authenticate_received_historical_signed_original(&raw)
        .unwrap();
    admitted.recheck_cash_context(&context).unwrap();
    assert_eq!(admitted.original(), raw);
    assert_eq!(
        admitted.certified_height().unwrap(),
        fixture.native.latest().height()
    );
    assert!(receiver.retained_cash_clock_originals(&context).is_err());
    let mut altered = KagemushaOrdinaryNativeSignedClockOriginalV1::decode_original(&raw).unwrap();
    fixture.resign_changed(&mut altered.originals[0], 0, |body| {
        body.build_fingerprint = Hash::new(b"foreign received clock executable")
    });
    assert!(
        receiver
            .authenticate_received_historical_signed_original(
                &altered.canonical_original().unwrap()
            )
            .is_err()
    );
    let mut trailing = raw.clone();
    trailing.push(0);
    assert!(
        receiver
            .authenticate_received_historical_signed_original(&trailing)
            .is_err()
    );
    assert_eq!(receiver.journal.recovery_prefix().unwrap(), prefix);
    drop(receiver);
    let mut recovered =
        KagemushaOrdinaryNativeClockOwnerV1::open_existing(&root, fixture.selected.clone())
            .unwrap();
    recovered
        .authenticate_received_historical_signed_original(&raw)
        .unwrap()
        .recheck_cash_context(&context)
        .unwrap();
    assert!(recovered.current_native_time_interval().is_err());
    assert!(recovered.retained_cash_clock_originals(&context).is_err());
    assert_eq!(recovered.journal.recovery_prefix().unwrap(), prefix);
}
