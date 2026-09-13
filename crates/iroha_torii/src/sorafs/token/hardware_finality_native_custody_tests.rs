//! Local native custody and durable finality integration, using software-signed fixtures.
//!
//! This is not replicated consensus, state-root certification, or physical HSM qualification.
//! The existing test-only world commit stages real typed instructions; the independent SCCP
//! fixture binds their exact block bodies to real three-of-four BLS Commit certificates.

use super::{
    StreamTokenHardwarePinsV1, StreamTokenIssuerError,
    hardware_finality::{
        CoreFinalityV1, FinalityFloorV1, HardwareFinalityV1, HistoricalFinalityV1,
    },
    hardware_test_support::{NOW_MS, PROVIDER, storage_config},
};
use iroha_core::{
    kura::Kura,
    query::{
        store::LiveQueryStore,
        stream_token_custody::{
            StreamTokenCustodyControlSnapshotV1, read_stream_token_custody_control_at_v1,
        },
    },
    smartcontracts::Execute,
    state::{State, StateReadOnly, World},
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    block::{BlockHeader, builder::BlockBuilder},
    isi::sorafs::MutateSorafsStreamTokenCustody,
    permission::{Permission, Permissions},
    sorafs::{capacity::ProviderId, stream_token_custody::SorafsStreamTokenCustodyActionV1},
    transaction::{
        DataTriggerSequence, FeePaymentIntent, TransactionBuilder, TransactionResultInner,
    },
};
use iroha_executor_data_model::permission::sorafs::CanManageSorafsStreamTokenCustody;
use iroha_sccp::{
    SCCP_TAIRA_CHAIN_ID_V1, SccpFinalizedBlockTestFixtureV1,
    sccp_finalize_taira_block_test_fixture_v1, sccp_taira_finality_network_id_v1,
};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAnchorV1,
        SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    stream_token::stream_token_binding_digest_v1,
    stream_token_custody_control::StreamTokenCustodyPolicyV1,
    stream_token_evidence::{
        SignerStreamTokenObservationExpectedV1, SignerStreamTokenObservationPhaseV1,
        SignerStreamTokenStateObservationBodyV1, SignerStreamTokenStateObservationV1,
        SignerStreamTokenStateSubjectV1, verify_stream_token_signer_current_evidence_v1,
    },
};
use std::{num::NonZeroUsize, sync::Arc, time::Duration};

fn fixture_key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("checked local fixture key")
}

struct NativeCustodyFixture {
    state: Arc<State>,
    kura: Arc<Kura>,
    pins: StreamTokenHardwarePinsV1,
    approval: SignerCustodyAnchorV1,
    current: StreamTokenCustodyControlSnapshotV1,
    enrollment: Vec<u8>,
    finalized: [SccpFinalizedBlockTestFixtureV1; 2],
}

// Execute the actual permission-checked mutation, then bind the exact corresponding signed
// instruction and successful result into the fixture block. The existing world-only helper is
// deliberately not the production consensus commit path or a proof of the resulting WSV root.
fn execute_fixture_block(
    state: &mut State,
    kura: &Kura,
    key: &KeyPair,
    now: u64,
    instruction: MutateSorafsStreamTokenCustody,
    parent: Option<&SccpFinalizedBlockTestFixtureV1>,
) -> SccpFinalizedBlockTestFixtureV1 {
    let height = u64::try_from(state.view().block_hashes().len()).expect("fixture height") + 1;
    let header = BlockHeader::new(
        height.try_into().expect("positive height"),
        state.view().latest_block_hash(),
        None,
        None,
        now,
        0,
    );
    let authority = AccountId::new(key.public_key().clone());
    let mut builder = TransactionBuilder::new(
        *state.network_id_ref(),
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(now));
    let signed_transaction = builder
        .with_instructions([instruction.clone()])
        .try_sign(key.private_key())
        .expect("sign exact native mutation fixture transaction");
    let mut block = state.block(header.clone());
    let mut tx = block.transaction();
    instruction
        .execute(&authority, &mut tx)
        .expect("actual authorized native mutation");
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit isolated native world fixture");
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(signed_transaction);
    builder.push_result(TransactionResultInner::Ok(DataTriggerSequence::default()));
    let signed = builder
        .try_build_with_signature(0, key.private_key())
        .expect("sign complete result-bearing fixture block");
    let finalized = sccp_finalize_taira_block_test_fixture_v1(&signed, parent);
    let artifact = &finalized.proof().finality_artifact;
    artifact
        .verify()
        .expect("actual four-validator BLS verification");
    assert_eq!(
        artifact.height_context.protocol_version,
        iroha_data_model::block::consensus_v2::PROTOCOL_VERSION
    );
    assert_eq!(
        artifact.height_context.da_layout.encoding,
        iroha_data_model::block::consensus_v2::PayloadEncoding::ReedSolomon16
    );
    assert_eq!(artifact.height_context.roster.len(), 4);
    assert_eq!(artifact.commit_qc.signers.len(), 3);
    let hash = signed.hash();
    let header = signed.header();
    kura.store_block(Arc::new(signed))
        .expect("persist exact fixture block");
    state.push_block_hash_for_testing(hash);
    state.update_latest_block_header_cache_for_tests(header);
    finalized
}

impl NativeCustodyFixture {
    fn new() -> Self {
        let key = fixture_key(0xA1);
        let authority = AccountId::new(key.public_key().clone());
        let provider = ProviderId::new(PROVIDER);
        let mut world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
        world
            .provider_owners_mut_for_testing()
            .insert(provider, authority.clone());
        let mut permissions = Permissions::new();
        permissions.insert(Permission::from(CanManageSorafsStreamTokenCustody {
            provider_id: provider,
        }));
        world
            .account_permissions_mut_for_testing()
            .insert(authority, permissions);
        let kura = Kura::blank_kura_for_testing();
        let mut state = State::new_with_chain_and_network_id_for_testing(
            world,
            kura.clone(),
            LiveQueryStore::start_test(),
            SCCP_TAIRA_CHAIN_ID_V1.parse().expect("fixture chain"),
            sccp_taira_finality_network_id_v1(),
        );
        let pins = StreamTokenHardwarePinsV1::from_config(
            &storage_config(1),
            SCCP_TAIRA_CHAIN_ID_V1,
            *sccp_taira_finality_network_id_v1().as_bytes(),
        )
        .expect("independent public fixture pins")
        .expect("enabled hardware profile");
        let trust = pins.custody_trust();
        assert_eq!(&trust.public_key, fixture_key(0x44).public_key());
        assert_eq!(
            &pins.observer_trust().public_key,
            fixture_key(0x55).public_key()
        );
        let policy = StreamTokenCustodyPolicyV1 {
            binding: pins.binding().clone(),
            attester_authority: trust.authority.clone(),
            attester_public_key: trust.public_key.clone(),
            active_from_unix_ms: trust.active_from_unix_ms,
            active_until_unix_ms: trust.active_until_unix_ms,
            max_validity_ms: trust.max_validity_ms,
            max_anchor_age_ms: trust.max_anchor_age_ms,
        };
        let first = execute_fixture_block(
            &mut state,
            &kura,
            &key,
            NOW_MS - 500,
            MutateSorafsStreamTokenCustody {
                provider_id: provider,
                expected_revision: 0,
                expected_digest: [0; 32],
                action: SorafsStreamTokenCustodyActionV1::Configure(
                    norito::encode_canonical(&policy).expect("canonical policy"),
                ),
            },
            None,
        );
        let approval = read_stream_token_custody_control_at_v1(&state.view(), pins.binding(), 1)
            .expect("real native approval lookup")
            .expect("configured native control");
        assert!(approval.state.active_head.is_none());
        let statement = SignerCustodyStatementV1 {
            magic: SIGNER_CUSTODY_MAGIC_V1,
            version: SIGNER_CUSTODY_VERSION_V1,
            binding: pins.binding().clone(),
            authority: trust.authority.clone(),
            anchor: approval.anchor,
            sequence: approval.state.next_sequence,
            predecessor_digest: approval.state.predecessor_digest,
            issued_at_unix_ms: NOW_MS,
            expires_at_unix_ms: NOW_MS + 5_000,
            hardware_identity_digest: [0x87; 32],
            evidence_digest: [0x88; 32],
            generated_in_hardware: true,
            exportable: false,
            ever_exported: false,
            revoked: false,
        };
        // These signed hardware assertions are trusted software fixture claims, not device evidence.
        let signature = Signature::try_new(
            fixture_key(0x44).private_key(),
            &statement
                .signing_payload()
                .expect("canonical custody preimage"),
        )
        .expect("software fixture attestation");
        let enrollment = norito::encode_canonical(&SignerCustodyRecordV1 {
            statement,
            attestation: signature.payload().try_into().expect("Ed25519 signature"),
        })
        .expect("canonical full enrollment");
        let second = execute_fixture_block(
            &mut state,
            &kura,
            &key,
            NOW_MS,
            MutateSorafsStreamTokenCustody {
                provider_id: provider,
                expected_revision: 1,
                expected_digest: approval.anchor.state_digest,
                action: SorafsStreamTokenCustodyActionV1::Enroll(enrollment.clone()),
            },
            Some(&first),
        );
        let current = read_stream_token_custody_control_at_v1(&state.view(), pins.binding(), 2)
            .expect("real native current lookup")
            .expect("enrolled native control");
        assert_eq!(
            current
                .state
                .active_head
                .expect("native enrolled head")
                .approved_anchor,
            approval.anchor
        );
        Self {
            state: Arc::new(state),
            kura,
            pins,
            approval: approval.anchor,
            current,
            enrollment,
            finalized: [first, second],
        }
    }

    fn persist_finality(&self, index: usize) {
        let artifact = &self.finalized[index].proof().finality_artifact;
        let receipt = self
            .kura
            .store_v2_finality_artifact(artifact)
            .expect("durably persist verified complete artifact");
        assert_eq!(receipt.height(), artifact.height);
        assert_eq!(receipt.block_hash(), artifact.block_hash);
        assert_eq!(receipt.context_id(), artifact.context_id());
        assert_eq!(receipt.certificate(), artifact.commit_qc.as_ref());
        let reloaded = self
            .kura
            .v2_finality_artifact(artifact.height)
            .expect("verified Kura artifact read")
            .expect("durable artifact exists");
        assert_eq!(reloaded, *artifact);
        assert_eq!(
            self.kura.get_durable_block_hash(
                NonZeroUsize::new(usize::try_from(artifact.height).expect("small height")).unwrap()
            ),
            Some(artifact.block_hash)
        );
    }

    fn guard(&self) -> CoreFinalityV1 {
        CoreFinalityV1::new(self.state.clone(), self.pins.clone())
    }

    fn verified_observation_at(
        &self,
        current_anchor: SignerCustodyAnchorV1,
    ) -> SignerStreamTokenStateObservationBodyV1 {
        let expected = SignerStreamTokenObservationExpectedV1::current(
            self.pins.binding(),
            SignerStreamTokenObservationPhaseV1::BeforeAdmission,
            [0x93; 32],
            self.approval,
            NOW_MS,
        )
        .expect("exact independent observation request");
        let body = SignerStreamTokenStateObservationBodyV1 {
            magic: SignerStreamTokenStateObservationBodyV1::magic(),
            request_digest: expected
                .request()
                .digest()
                .expect("canonical request digest"),
            phase: SignerStreamTokenObservationPhaseV1::BeforeAdmission,
            subject: SignerStreamTokenStateSubjectV1::CurrentCustody {
                binding_digest: stream_token_binding_digest_v1(self.pins.binding())
                    .expect("exact binding digest"),
            },
            authority: self.pins.observer_trust().authority.clone(),
            chain_id: self.pins.binding().chain_id.clone(),
            network_id: self.pins.binding().network_id,
            observed_at_unix_ms: NOW_MS,
            expires_at_unix_ms: NOW_MS + 1_000,
            current_anchor,
            active_head: self.current.state.active_head.expect("native active head"),
            signer_revoked: self.current.state.signer_revoked,
            attester_revoked: self.current.state.attester_revoked,
        };
        let signature = Signature::try_new(
            fixture_key(0x55).private_key(),
            &body
                .signing_payload()
                .expect("exact state observation preimage"),
        )
        .expect("software fixture observer signature");
        let bytes = SignerStreamTokenStateObservationV1 {
            body: body.clone(),
            signature: signature.payload().try_into().expect("Ed25519 signature"),
        }
        .encode_canonical()
        .expect("canonical signed observation");
        verify_stream_token_signer_current_evidence_v1(
            &self.enrollment,
            &bytes,
            self.pins.binding(),
            self.pins.custody_trust(),
            self.pins.observer_trust(),
            expected,
            NOW_MS,
        )
        .expect("full independent custody and observation verification");
        body
    }

    fn validate(
        &self,
        guard: &CoreFinalityV1,
        floor: FinalityFloorV1,
        candidate: SignerCustodyAnchorV1,
        observation: &SignerStreamTokenStateObservationBodyV1,
    ) -> Result<(), StreamTokenIssuerError> {
        guard.validate(
            self.approval,
            candidate,
            floor,
            &[HistoricalFinalityV1::Custody(self.approval)],
            observation,
        )
    }
}

#[test]
fn actual_native_custody_requires_both_durable_artifacts_then_accepts_signed_observation() {
    let fixture = NativeCustodyFixture::new();
    let guard = fixture.guard();
    let observation = fixture.verified_observation_at(fixture.current.anchor);
    assert!(matches!(
        guard.capture(fixture.approval),
        Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
    ));
    fixture.persist_finality(0);
    assert!(matches!(
        guard.capture(fixture.approval),
        Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
    ));
    fixture.persist_finality(1);
    let floor = guard
        .capture(fixture.approval)
        .expect("real native and durable current floor");
    assert_eq!(floor.height, 2);
    assert_eq!(floor.block_hash, fixture.current.anchor.block_hash);
    fixture
        .validate(&guard, floor, fixture.current.anchor, &observation)
        .expect("exact native custody, historical approval and durable Commit QCs");
}

#[test]
fn actual_native_custody_rejects_same_height_forged_control_digests() {
    let fixture = NativeCustodyFixture::new();
    fixture.persist_finality(0);
    fixture.persist_finality(1);
    let guard = fixture.guard();
    let observation = fixture.verified_observation_at(fixture.current.anchor);
    let floor = guard
        .capture(fixture.approval)
        .expect("positive floor control");
    fixture
        .validate(&guard, floor, fixture.current.anchor, &observation)
        .expect("positive validation control");
    let mut forged = fixture.current.anchor;
    forged.state_digest[0] ^= 1;
    // Re-sign the substituted current anchor and pass the actual shared evidence verifier.
    // Core must reject its native digest even though the signed observation agrees with it.
    let forged_observation = fixture.verified_observation_at(forged);
    assert_eq!(forged_observation.current_anchor, forged);
    assert!(matches!(
        fixture.validate(&guard, floor, forged, &forged_observation),
        Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
    ));
    let mut forged_approval = fixture.approval;
    forged_approval.state_digest[0] ^= 1;
    assert!(matches!(
        guard.capture(forged_approval),
        Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
    ));
    guard
        .validate(
            fixture.approval,
            fixture.current.anchor,
            floor,
            &[
                HistoricalFinalityV1::Custody(fixture.approval),
                HistoricalFinalityV1::Custody(fixture.approval),
            ],
            &observation,
        )
        .expect("matching two-item historical custody control");
    assert!(matches!(
        guard.validate(
            fixture.approval,
            fixture.current.anchor,
            floor,
            &[
                HistoricalFinalityV1::Custody(fixture.approval),
                HistoricalFinalityV1::Custody(forged_approval),
            ],
            &observation
        ),
        Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
    ));
}

#[test]
fn actual_native_custody_retains_history_but_fences_removed_current_provider() {
    let fixture = NativeCustodyFixture::new();
    fixture.persist_finality(0);
    fixture.persist_finality(1);
    let guard = fixture.guard();
    let observation = fixture.verified_observation_at(fixture.current.anchor);
    let floor = guard
        .capture(fixture.approval)
        .expect("positive registered-provider control");
    fixture
        .validate(&guard, floor, fixture.current.anchor, &observation)
        .expect("positive validation control");
    // Isolate the current registry predicate with the existing world-only fixture commit. This
    // does not append or claim a consensus-certified provider-removal block; both already-durable
    // finality artifacts and the retained native custody anchors remain exactly the same.
    let header = BlockHeader::new(
        3.try_into().unwrap(),
        fixture.state.view().latest_block_hash(),
        None,
        None,
        NOW_MS + 1,
        0,
    );
    let mut block = fixture.state.block(header);
    let mut tx = block.transaction();
    assert_eq!(
        tx.world
            .remove_provider_owner_for_testing(ProviderId::new(PROVIDER)),
        Some(AccountId::new(fixture_key(0xA1).public_key().clone()))
    );
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("isolated current registry removal");
    assert_eq!(
        read_stream_token_custody_control_at_v1(&fixture.state.view(), fixture.pins.binding(), 2)
            .expect("historical custody remains readable")
            .expect("retained control"),
        fixture.current
    );
    assert_eq!(fixture.state.view().block_hashes().len(), 2);
    for finalized in &fixture.finalized {
        assert_eq!(
            fixture
                .kura
                .v2_finality_artifact(finalized.proof().finality_artifact.height)
                .expect("unchanged durable artifact")
                .as_ref(),
            Some(&finalized.proof().finality_artifact)
        );
    }
    assert!(matches!(
        guard.capture(fixture.approval),
        Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
    ));
    assert!(matches!(
        fixture.validate(&guard, floor, fixture.current.anchor, &observation),
        Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
    ));
}
