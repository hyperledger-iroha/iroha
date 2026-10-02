//! Real Ed25519/BLS verification over explicitly synthetic complete-World fixture data.
//! These tests grant no issuer, installed-runtime or physical-device qualification.
use super::*;
use iroha_crypto::{KeyPair, SignatureOf};
use iroha_data_model::{
    account::{AccountDetails, MultisigMember, MultisigPolicy},
    common::Owned,
    sumeragi::SumeragiStatus,
    sumeragi_finality::{
        SumeragiFinalityAttestationBody, WorldStateElementKindV1, WorldStateSnapshotEntryV1,
        WorldStateSnapshotV1, test_fixtures::NativeFinalityFixture, world_state_value_hash_v1,
    },
};
use norito::codec::Encode as _;

// Reuse this actual four-statement Ed/BLS fixture without inventing a verified request.
#[path = "../participant_enrollment_dispatch_tests.rs"]
mod issuer_dispatch_retention_tests;

#[path = "../participant_enrollment_fi_read_tests.rs"]
mod fi_current_receiving_tests;

#[path = "../participant_enrollment_http_tests.rs"]
mod http_original_tests;

struct Fixture {
    signer: KeyPair,
    signatory: AccountId,
    wallet: AccountId,
    value: AccountValue,
    native: NativeFinalityFixture,
    world: WorldStateSnapshotV1,
}
impl Fixture {
    fn new() -> Self {
        let signer = KeyPair::from_seed(vec![13; 32], Algorithm::Ed25519);
        let signatory = AccountId::new(signer.public_key().clone());
        let wallet = AccountId::new_multisig(
            MultisigPolicy::new(
                1,
                vec![MultisigMember::new(signer.public_key().clone(), 1).unwrap()],
            )
            .unwrap(),
        );
        let value = Owned::new(AccountDetails::new(Default::default(), None, None, vec![]));
        let mut entries = vec![signatory.clone(), wallet.clone()]
            .into_iter()
            .map(|id| WorldStateSnapshotEntryV1 {
                field_id: "world.accounts".into(),
                kind: WorldStateElementKindV1::Table,
                key_hash: Some(world_state_value_hash_v1(&id).unwrap()),
                value_hash: world_state_value_hash_v1(&value).unwrap(),
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|v| v.key_hash);
        let world = WorldStateSnapshotV1 {
            schema_hash: Hash::new(b"explicit synthetic participant enrollment schema"),
            entries,
        };
        let mut native = NativeFinalityFixture::start("participant-request-tests");
        native.certify_with_world_root(
            native.block_with_submitted_work(native.next_header()),
            world.root().unwrap(),
        );
        Self {
            signer,
            signatory,
            wallet,
            value,
            native,
            world,
        }
    }
    fn nodes() -> [SelectedEnrollmentReadNodeV1; 4] {
        std::array::from_fn(|i| SelectedEnrollmentReadNodeV1 {
            peer_id: PeerId::new(
                KeyPair::from_seed(vec![i as u8 + 1; 32], Algorithm::BlsNormal)
                    .public_key()
                    .clone(),
            ),
            build_fingerprint: Hash::new(b"explicit selected fixture executable"),
            config_fingerprint: Hash::new(b"explicit selected fixture config"),
        })
    }
    fn statements(
        &self,
        challenge: [u8; 32],
        nodes: &[SelectedEnrollmentReadNodeV1; 4],
    ) -> [SumeragiFinalityAttestation; 4] {
        std::array::from_fn(|i| {
            let signer = KeyPair::from_seed(vec![i as u8 + 1; 32], Algorithm::BlsNormal);
            let node = &nodes[i];
            let body = SumeragiFinalityAttestationBody {
                observed_at_unix_ms: 1_000_000,
                challenge,
                network_id: self.native.network_id(),
                node_fingerprint: Hash::new(node.peer_id.encode()),
                node_id: node.peer_id.clone(),
                build_fingerprint: node.build_fingerprint,
                config_fingerprint: node.config_fingerprint,
                genesis_block_hash: self.native.genesis().hash(),
                genesis_finality_proof: self.native.genesis_proof().clone(),
                status: SumeragiStatus {
                    protocol_version: 1,
                    config_fingerprint: node.config_fingerprint,
                    beacon_horizon: None,
                    instance: self.native.verifier().instance().0,
                    height: 3,
                    view: 0,
                    stage: 0,
                    leader: None,
                    proxy_tail: None,
                    high_qc_view: None,
                    level: 0,
                    start_level: 0,
                    t_retx_ms: 100,
                    committed_height: 2,
                    applied_height: 2,
                    awaiting: false,
                    signer: Some(signer.public_key().clone()),
                    unanchored: false,
                    abstaining: false,
                    halted: None,
                    footprint: Default::default(),
                },
                finality_proof: self.native.latest().clone(),
            };
            SumeragiFinalityAttestation {
                signature: SignatureOf::try_from_hash(signer.private_key(), body.signing_hash())
                    .unwrap(),
                body,
            }
        })
    }
    fn admit(
        &self,
        challenge: EnrollmentWalletReadChallengeV1,
        nodes: &[SelectedEnrollmentReadNodeV1; 4],
        statements: &[SumeragiFinalityAttestation; 4],
    ) -> Result<VerifiedEnrollmentWalletSignatoryV1> {
        let verifier = self.native.verifier();
        let block = verifier.verify_retained_decision(self.native.latest())?;
        let world = self.world.authenticate(&block)?;
        VerifiedEnrollmentWalletSignatoryV1::authenticate(
            challenge,
            self.native.network_id(),
            nodes,
            &verifier,
            self.native.latest(),
            world,
            self.world.schema_hash,
            statements,
            self.signatory.clone(),
            &self.value,
            self.wallet.clone(),
            &self.value,
        )
    }
}
// Store the independently selected NetworkId as part of the fixture owner, rather than
// manufacturing an authenticated request reference from a temporary method result.
fn request<'a>(
    f: &'a Fixture,
    network: &'a NetworkId,
    target: &'a Url,
    body: &'a [u8],
) -> ParticipantEnrollmentRequestV1<'a> {
    ParticipantEnrollmentRequestV1 {
        network_id: network,
        authentication_namespace: "leumi.is2",
        actor_id: "fixture-retail-actor",
        session_sha256: [7; 32],
        signatory: &f.signatory,
        wallet: &f.wallet,
        request_id: "fixture-request",
        idempotency_key: "fixture-stable-attempt",
        operation: ParticipantEnrollmentOperationV1::Prepare,
        target,
        body,
        timestamp_ms: 1_000_000,
        nonce: "fixture-fresh-nonce-0001",
    }
}
fn valid_target() -> Url {
    Url::parse("https://fi.example.invalid/leumi.is2/v1/kagemusha/enrollment/ordinary/prepare")
        .unwrap()
}
#[test]
fn exact_ed_request_binds_fi_session_origin_body_route_and_idempotency() {
    let f = Fixture::new();
    let network = f.native.network_id();
    let target = valid_target();
    let base = request(&f, &network, &target, b"{\"attempt\":1}");
    let message = base.signing_message().unwrap();
    let signature = Signature::new(f.signer.private_key(), &message);
    signature.verify(f.signer.public_key(), &message).unwrap();
    let other_target = Url::parse(
        "https://other.example.invalid/leumi.is2/v1/kagemusha/enrollment/ordinary/prepare",
    )
    .unwrap();
    let mut variants = vec![];
    for field in 0..7 {
        let mut v = base;
        match field {
            0 => v.authentication_namespace = "hapoalim.is2",
            1 => v.actor_id = "foreign-actor",
            2 => v.session_sha256 = [8; 32],
            3 => v.target = &other_target,
            4 => v.body = b"{\"attempt\":2}",
            5 => v.idempotency_key = "different-attempt",
            _ => v.nonce = "fixture-fresh-nonce-0002",
        };
        variants.push(v.signing_message().unwrap());
    }
    for changed in variants {
        assert!(signature.verify(f.signer.public_key(), &changed).is_err());
    }
    let mut route = base;
    route.operation = ParticipantEnrollmentOperationV1::Certificate;
    assert!(route.signing_message().is_err());
    let mut invalid = base;
    invalid.timestamp_ms = 30_000;
    assert!(invalid.signing_message().is_err());
    let mut invalid = base;
    invalid.session_sha256 = [0; 32];
    assert!(invalid.signing_message().is_err());
}
#[test]
fn four_fresh_native_statements_and_original_s_w_admit_only_the_same_signed_body() {
    let f = Fixture::new();
    let network = f.native.network_id();
    let target = valid_target();
    let req = request(&f, &network, &target, b"original-exact-json");
    let challenge = EnrollmentWalletReadChallengeV1::for_request(&req).unwrap();
    let nodes = Fixture::nodes();
    let statements = f.statements(challenge.bytes(), &nodes);
    let wallet = f.admit(challenge, &nodes, &statements).unwrap();
    let signature = Signature::new(f.signer.private_key(), &req.signing_message().unwrap());
    let admitted = wallet.verify_request(&req, &signature).unwrap();
    admitted.verify_original_body(req.body).unwrap();
    assert!(admitted.verify_original_body(b"substituted-json").is_err());
    assert_eq!(
        admitted.operation(),
        ParticipantEnrollmentOperationV1::Prepare
    );
    assert_eq!(admitted.idempotency_key(), req.idempotency_key);
    let challenge = EnrollmentWalletReadChallengeV1::for_request(&req).unwrap();
    let statements = f.statements(challenge.bytes(), &nodes);
    let wallet = f.admit(challenge, &nodes, &statements).unwrap();
    let mut different = req;
    different.body = b"substituted-json";
    assert!(wallet.verify_request(&different, &signature).is_err());
}
#[test]
fn substituted_node_pin_challenge_and_signatory_membership_never_form_current_owner() {
    let f = Fixture::new();
    let network = f.native.network_id();
    let target = valid_target();
    let req = request(&f, &network, &target, b"original-json");
    let nodes = Fixture::nodes();
    let challenge = EnrollmentWalletReadChallengeV1::for_request(&req).unwrap();
    let statements = f.statements([99; 32], &nodes);
    assert!(f.admit(challenge, &nodes, &statements).is_err());
    let challenge = EnrollmentWalletReadChallengeV1::for_request(&req).unwrap();
    let statements = f.statements(challenge.bytes(), &nodes);
    let mut changed = nodes.clone();
    changed[3].build_fingerprint = Hash::new(b"foreign executable");
    assert!(f.admit(challenge, &changed, &statements).is_err());
    let challenge = EnrollmentWalletReadChallengeV1::for_request(&req).unwrap();
    let statements = f.statements(challenge.bytes(), &nodes);
    let mut changed = nodes.clone();
    changed[3] = changed[0].clone();
    assert!(f.admit(challenge, &changed, &statements).is_err());
    let foreign = AccountId::new(
        KeyPair::from_seed(vec![14; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    assert!(require_single_member_wallet(&foreign, &f.wallet).is_err());
    let challenge = EnrollmentWalletReadChallengeV1::for_request(&req).unwrap();
    // Exhaust this same process-bound suspend-inclusive Native reading; it has no
    // caller-supplied clock constructor or mutable local deadline.
    std::thread::sleep(READ_BUDGET);
    assert!(challenge.started.elapsed().unwrap() >= READ_BUDGET);
    assert!(challenge.remaining_native_budget().is_err());
    let statements = f.statements(challenge.bytes(), &nodes);
    assert!(f.admit(challenge, &nodes, &statements).is_err());
}

#[test]
fn actual_owner_rejects_another_network_schema_peer_config_and_retained_certified_root() {
    let f = Fixture::new();
    let network = f.native.network_id();
    let target = valid_target();
    let req = request(&f, &network, &target, b"original owner-bound request");
    let nodes = Fixture::nodes();
    let challenge = EnrollmentWalletReadChallengeV1::for_request(&req).unwrap();
    let statements = f.statements(challenge.bytes(), &nodes);
    let wallet = f.admit(challenge, &nodes, &statements).unwrap();
    let signature = Signature::new(f.signer.private_key(), &req.signing_message().unwrap());
    let admitted = wallet.verify_request(&req, &signature).unwrap();
    let verifier = f.native.verifier();
    let height = f.native.latest().height();
    admitted
        .verify_fi_owned_selection(&network, &nodes, f.world.schema_hash, &verifier, height)
        .unwrap();
    let foreign = NativeFinalityFixture::start_with_mode(
        "foreign-participant-owner",
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos,
    );
    assert_ne!(network, foreign.network_id());
    assert!(
        admitted
            .verify_fi_owned_selection(
                &foreign.network_id(),
                &nodes,
                f.world.schema_hash,
                &verifier,
                height
            )
            .is_err()
    );
    assert!(
        admitted
            .verify_fi_owned_selection(
                &network,
                &nodes,
                Hash::new(b"foreign compiled schema"),
                &verifier,
                height
            )
            .is_err()
    );
    assert!(
        admitted
            .verify_fi_owned_selection(&network, &nodes, f.world.schema_hash, &verifier, height + 1)
            .is_err()
    );
    for field in 0..3 {
        let mut changed = nodes.clone();
        match field {
            0 => changed[0].build_fingerprint = Hash::new(b"foreign installed build"),
            1 => changed[0].config_fingerprint = Hash::new(b"foreign installed config"),
            _ => {
                changed[0].peer_id = PeerId::new(
                    KeyPair::from_seed(vec![91; 32], Algorithm::BlsNormal)
                        .public_key()
                        .clone(),
                )
            }
        }
        assert!(
            admitted
                .verify_fi_owned_selection(
                    &network,
                    &changed,
                    f.world.schema_hash,
                    &verifier,
                    height
                )
                .is_err()
        );
    }
    // A genuine independently certified same-network H2 with another World root
    // cannot replace the FI owner's actual retained decision, even with the same S/W.
    let mut different_root = NativeFinalityFixture::start("participant-request-tests");
    different_root.certify_with_world_root(
        different_root.block_with_submitted_work(different_root.next_header()),
        Hash::new(b"another certified World root"),
    );
    assert_eq!(network, different_root.network_id());
    assert!(
        admitted
            .verify_fi_owned_selection(
                &network,
                &nodes,
                f.world.schema_hash,
                &different_root.verifier(),
                height
            )
            .is_err()
    );
    assert!(
        admitted
            .verify_fi_owned_selection(
                &network,
                &nodes,
                f.world.schema_hash,
                &foreign.verifier(),
                height
            )
            .is_err()
    );
}

pub(crate) struct NativeCustodyFixture(Fixture);
#[cfg(all(unix, feature = "kagemusha-ordinary-native"))]
impl NativeCustodyFixture {
    pub(crate) fn new() -> Self {
        Self(Fixture::new())
    }
    pub(crate) fn key(&self) -> &KeyPair {
        &self.0.signer
    }
    pub(crate) fn wallet(&self) -> &AccountId {
        &self.0.wallet
    }
    pub(crate) fn signatory(&self) -> &AccountId {
        &self.0.signatory
    }
    pub(crate) fn network(&self) -> NetworkId {
        self.0.native.network_id()
    }
    pub(crate) fn current(
        &self,
        request: &ParticipantEnrollmentRequestV1<'_>,
    ) -> VerifiedEnrollmentWalletSignatoryV1 {
        let challenge = EnrollmentWalletReadChallengeV1::for_request(request).unwrap();
        self.current_for_challenge(challenge)
    }
    // Reuse the same actual fixture statements/World/finality admission with a retained
    // production challenge; this creates no installed inventory or Native runtime authority.
    pub(crate) fn current_for_challenge(
        &self,
        challenge: EnrollmentWalletReadChallengeV1,
    ) -> VerifiedEnrollmentWalletSignatoryV1 {
        let nodes = Fixture::nodes();
        let statements = self.0.statements(challenge.bytes(), &nodes);
        self.0.admit(challenge, &nodes, &statements).unwrap()
    }
    pub(crate) fn wallet_original(
        &self,
        nonce: [u8; 32],
    ) -> iroha_torii_shared::ordinary_wallet_current::OrdinaryWalletCurrentOriginalV1 {
        let request = iroha_torii_shared::ordinary_wallet_current::OrdinaryWalletCurrentRequestV1 {
            version: 1,
            network_id: self.network(),
            height: self.0.native.latest().height(),
            request_nonce: nonce,
            signatory: self.signatory().clone(),
            wallet: self.wallet().clone(),
        };
        iroha_torii_shared::ordinary_wallet_current::OrdinaryWalletCurrentOriginalV1 {
            request,
            attestation: self.0.statements(nonce, &Fixture::nodes())[0].clone(),
            world_snapshot: self.0.world.clone(),
            signatory_value: self.0.value.clone(),
            wallet_value: self.0.value.clone(),
        }
    }
    pub(crate) fn clock(
        &self,
        root: &std::path::Path,
    ) -> iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryNativeClockOwnerV1 {
        use iroha_core_zk::kagemusha_v1_state::{
            KagemushaOrdinaryNativeClockNodeV1 as Node,
            KagemushaOrdinaryNativeClockOriginalsV1 as Originals,
            KagemushaOrdinaryNativeClockOwnerV1 as Owner,
            KagemushaOrdinaryNativeClockPolicyV1 as Policy,
        };
        let nodes = Fixture::nodes();
        let selected = Originals::from_selected_originals(
            self.0
                .native
                .verifier()
                .export_checkpoint(self.0.native.latest())
                .unwrap(),
            self.network(),
            "participant-request-tests".into(),
            std::array::from_fn(|i| Node {
                peer_id: nodes[i].peer_id.clone(),
                build_fingerprint: nodes[i].build_fingerprint,
                config_fingerprint: nodes[i].config_fingerprint,
            }),
            Policy {
                maximum_reply_age_ms: 10_000,
                maximum_node_skew_ms: 100,
                maximum_projection_age_ms: 120_000,
                maximum_persistence_age_ms: 1_000,
            },
        )
        .unwrap();
        let mut owner =
            Owner::create(&root.canonicalize().unwrap(), std::sync::Arc::new(selected)).unwrap();
        let read = owner.reserve_current_read().unwrap();
        let nonce = read.nonce();
        let statements = self.0.statements(nonce, &nodes);
        owner
            .admit_current_read(
                read,
                std::array::from_fn(|i| norito::encode_canonical(&statements[i]).unwrap()),
            )
            .unwrap();
        owner
    }
}

#[test]
fn enrollment_operation_routes_use_the_complete_canonical_product_prefix() {
    for (operation, path) in [
        (
            ParticipantEnrollmentOperationV1::Prepare,
            "/v1/kagemusha/enrollment/ordinary/prepare",
        ),
        (
            ParticipantEnrollmentOperationV1::RawAttestation,
            "/v1/kagemusha/enrollment/ordinary/raw-attestation",
        ),
        (
            ParticipantEnrollmentOperationV1::Certificate,
            "/v1/kagemusha/enrollment/ordinary/certificate",
        ),
    ] {
        assert_eq!(operation.path_suffix(), path);
    }
}
