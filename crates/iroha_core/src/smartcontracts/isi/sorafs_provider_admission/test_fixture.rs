//! Native provider-state setup with a real four-validator test block/QC chain.
//!
//! The crate-private callback executes actual governed effects but deliberately bypasses
//! Parliament certification for setup/adversarial State tests. Empty signed test blocks and
//! three-of-four BLS/RS16 proofs authenticate the retained block cut, not the callback or a
//! production application-state root. This helper grants no production enactment authority.

use super::{Action, apply, native};
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, StateReadOnly, StateTransaction, World},
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::{Account, AccountId},
    block::{BlockHeader, builder::BlockBuilder},
    sorafs::{
        capacity::ProviderId,
        provider_admission::{
            PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1, ProviderAdmissionCouncilPolicyV1,
        },
    },
};
use iroha_sccp::{
    SCCP_TAIRA_CHAIN_ID_V1, SccpFinalizedBlockTestFixtureV1,
    sccp_finalize_taira_block_test_fixture_v1, sccp_taira_finality_network_id_v1,
};
use sorafs_manifest::{
    CouncilSignature, ProviderAdmissionEnvelopeV1, ProviderAdmissionRevocationV1, ProviderAdvertV1,
    provider_admission::{
        PROVIDER_ADMISSION_ENVELOPE_VERSION_V1, PROVIDER_ADMISSION_REVOCATION_VERSION_V1,
        compute_advert_body_digest, compute_envelope_authorization_digest, compute_envelope_digest,
        compute_proposal_digest,
    },
};
use std::sync::Arc;

pub(crate) const NOW: u64 = 1_700_000_000;

pub(crate) fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture key")
}

pub(crate) fn raw(key: &KeyPair) -> [u8; 32] {
    key.public_key()
        .to_bytes()
        .1
        .try_into()
        .expect("Ed25519 public key")
}

pub(crate) fn sign_envelope(
    envelope: &mut ProviderAdmissionEnvelopeV1,
    policy: &ProviderAdmissionCouncilPolicyV1,
    signer: &KeyPair,
) {
    envelope.network_id = policy.network_id;
    envelope.policy_id = policy.policy_id;
    envelope.policy_revision = policy.revision;
    envelope.policy_digest = policy.canonical_digest().expect("actual policy digest");
    envelope.proposal_digest = compute_proposal_digest(&envelope.proposal).unwrap();
    envelope.advert_body_digest = compute_advert_body_digest(&envelope.advert_body).unwrap();
    envelope.council_signatures.clear();
    let digest = compute_envelope_authorization_digest(envelope).unwrap();
    envelope.council_signatures.push(CouncilSignature {
        signer: raw(signer),
        signature: Signature::try_new(signer.private_key(), &digest)
            .unwrap()
            .payload()
            .to_vec(),
    });
    policy
        .verify_envelope_policy_claim(envelope, envelope.issued_at)
        .expect("actual council signature");
}

/// Apply the actual native effect solely for crate-local State fixture setup.
pub(crate) fn enact_for_test(action: Action, transaction: &mut StateTransaction<'_, '_>) {
    assert!(apply(action, transaction).expect("native governed effect"));
}

/// Owns one provider's test setup and an independently retained exact signed block chain.
pub struct ProviderAdmissionTestFixtureV1 {
    pub(crate) state: Arc<State>,
    pub(crate) policy: ProviderAdmissionCouncilPolicyV1,
    pub(crate) envelope: ProviderAdmissionEnvelopeV1,
    pub(crate) signer: KeyPair,
    pub(crate) parent: Option<SccpFinalizedBlockTestFixtureV1>,
    now: u64,
}

impl ProviderAdmissionTestFixtureV1 {
    /// Create the empty-history test State at the fixed test epoch.
    /// # Panics
    /// Panics on malformed checked-in fixture material or unavailable test storage.
    #[must_use]
    pub fn new() -> Self {
        Self::new_at(NOW)
    }

    /// Create account/provider setup and genuinely signed admission material at `now` seconds.
    /// No policy or admission is installed until `admit` executes its native effects.
    /// # Panics
    /// Panics on invalid time, fixture material, signing or test storage errors.
    #[must_use]
    pub fn new_at(now: u64) -> Self {
        assert!(now > 0 && now < u64::MAX / 1000 - 3600);
        let proposal = norito::decode_from_bytes(include_bytes!(
            "../../../../../../fixtures/sorafs_manifest/provider_admission/proposal_v1.to"
        ))
        .expect("original proposal fixture");
        let advert: ProviderAdvertV1 = norito::decode_from_bytes(include_bytes!(
            "../../../../../../fixtures/sorafs_manifest/provider_admission/advert_v1.to"
        ))
        .expect("original advert fixture");
        let signer = key(31);
        let policy = ProviderAdmissionCouncilPolicyV1 {
            network_id: *sccp_taira_finality_network_id_v1().as_bytes(),
            policy_id: [0x41; 32],
            version: PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            trusted_signers: vec![raw(&signer)],
            signature_threshold: 1,
            paused: false,
        };
        let mut envelope = ProviderAdmissionEnvelopeV1 {
            version: PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
            network_id: policy.network_id,
            policy_id: policy.policy_id,
            policy_revision: 1,
            policy_digest: policy.canonical_digest().unwrap(),
            admission_revision: 1,
            expected_current_event_digest: None,
            proposal,
            proposal_digest: [0; 32],
            advert_body: advert.body,
            advert_body_digest: [0; 32],
            issued_at: now,
            retention_epoch: now + 3600,
            council_signatures: Vec::new(),
            notes: None,
        };
        sign_envelope(&mut envelope, &policy, &signer);
        let owner = AccountId::new(key(1).public_key().clone());
        let mut world = World::new();
        let (id, account) = Account::new(owner.clone()).build(&owner).into_key_value();
        world.accounts.insert(id, account);
        world
            .provider_owners
            .insert(ProviderId::new(envelope.proposal.provider_id), owner);
        let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            SCCP_TAIRA_CHAIN_ID_V1.parse().unwrap(),
            sccp_taira_finality_network_id_v1(),
        ));
        Self {
            state,
            policy,
            envelope,
            signer,
            parent: None,
            now,
        }
    }

    /// Borrow the State used by the native admission reader.
    #[must_use]
    pub fn state(&self) -> &Arc<State> {
        &self.state
    }

    /// Borrow the actual council-signed test envelope.
    #[must_use]
    pub fn envelope(&self) -> &ProviderAdmissionEnvelopeV1 {
        &self.envelope
    }

    /// Return the exact provider scope of this fixture.
    #[must_use]
    pub fn provider(&self) -> ProviderId {
        ProviderId::new(self.envelope.proposal.provider_id)
    }

    /// Execute actual ConfigureCouncil/Admit effects and retain durable test finality.
    /// # Panics
    /// Panics if native policy/signature/CAS checks or fixture storage reject the transition.
    pub fn admit(&mut self) {
        let policy = native::encode(&self.policy).unwrap();
        let envelope = native::encode(&self.envelope).unwrap();
        self.commit(
            |tx| {
                enact_for_test(Action::ConfigureCouncil(policy), tx);
                enact_for_test(Action::Admit(envelope), tx);
            },
            true,
        );
    }

    /// Construct a genuinely signed exact next revocation of this fixture's admitted envelope.
    /// # Panics
    /// Panics on malformed canonical material or signing errors.
    #[must_use]
    pub fn revocation(&self) -> ProviderAdmissionRevocationV1 {
        let digest = compute_envelope_digest(&self.envelope).unwrap();
        let mut revocation = ProviderAdmissionRevocationV1 {
            version: PROVIDER_ADMISSION_REVOCATION_VERSION_V1,
            network_id: self.policy.network_id,
            policy_id: self.policy.policy_id,
            policy_revision: self.policy.revision,
            policy_digest: self.policy.canonical_digest().unwrap(),
            transition_revision: self.envelope.admission_revision + 1,
            expected_current_event_digest: digest,
            provider_id: *self.provider().as_bytes(),
            envelope_digest: digest,
            revoked_at: self.now + 1,
            reason: "native test revocation".into(),
            council_signatures: Vec::new(),
            notes: None,
        };
        revocation.council_signatures.push(CouncilSignature {
            signer: raw(&self.signer),
            signature: Signature::try_new(self.signer.private_key(), &revocation.digest().unwrap())
                .unwrap()
                .payload()
                .to_vec(),
        });
        revocation
    }

    /// Execute the signed revocation's actual native effect and retain durable test finality.
    /// # Panics
    /// Panics if native signature/CAS checks or fixture storage reject the transition.
    pub fn revoke(&mut self) {
        let bytes = native::encode(&self.revocation()).unwrap();
        self.commit(|tx| enact_for_test(Action::Revoke(bytes), tx), true);
    }

    // This callback is deliberately crate-private and separate from signed native execution.
    // Tests can omit durable QC or alter setup state to exercise fail-closed readers. Neither
    // this callback nor the resulting block claims a Parliament certificate or state-root proof.
    pub(crate) fn commit(
        &mut self,
        call: impl FnOnce(&mut StateTransaction<'_, '_>),
        finality: bool,
    ) {
        let height = self
            .parent
            .as_ref()
            .map_or(1, |p| p.proof().finality_artifact.height + 1);
        assert!(height < 10, "fixed fixture epoch exhausted");
        {
            let view = self.state.view();
            assert_eq!(view.height() as u64, height - 1, "fixture frontier changed");
            assert_eq!(
                view.latest_block_hash(),
                self.parent.as_ref().map(|p| p.block().hash()),
                "fixture parent changed"
            );
            assert_eq!(view.network_id(), &sccp_taira_finality_network_id_v1());
        }
        let now = self.now.checked_add(1).expect("fixture clock");
        let header = BlockHeader::new(
            height.try_into().unwrap(),
            self.state.view().latest_block_hash(),
            None,
            now * 1000,
            0,
        );
        let mut block = self.state.block(header.clone());
        let mut tx = block.transaction();
        call(&mut tx);
        tx.apply();
        let fragments = block.committed_fragment_count().try_into().unwrap();
        let mut signed = BlockBuilder::new(header)
            .try_build_with_signature(0, key(0xfe).private_key())
            .unwrap();
        signed
            .set_execution_outputs(
                Vec::new(),
                fragments,
                Default::default(),
                Vec::new(),
                Default::default(),
                Default::default(),
                Vec::new(),
                &iroha_data_model::parameter::ExecutionOutputPolicyV1::bootstrap().limits(),
            )
            .unwrap();
        let finalized = sccp_finalize_taira_block_test_fixture_v1(&signed, self.parent.as_ref());
        let proof = &finalized.proof().finality_artifact;
        assert_eq!(proof.height_context.roster.len(), 4);
        assert_eq!(proof.commit_qc.signers.len(), 3);
        assert_eq!(
            proof.height_context.da_layout.encoding,
            iroha_data_model::block::consensus_v2::PayloadEncoding::ReedSolomon16
        );
        proof.verify().unwrap();
        block.commit_world_overlay_for_testing().unwrap();
        self.state
            .kura()
            .store_block(Arc::new(signed.clone()))
            .unwrap();
        self.state
            .append_committed_block_header_for_tests(signed.header());
        if finality {
            self.state.kura().store_v2_finality_artifact(proof).unwrap();
        }
        self.parent = Some(finalized);
        self.now = now;
    }
}

impl Default for ProviderAdmissionTestFixtureV1 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restored_provider_fixture_installs_actual_signed_claim_and_durable_quorum() {
        let mut fixture = ProviderAdmissionTestFixtureV1::new();
        assert_eq!(fixture.state.view().height(), 0);
        fixture.admit();
        let actual = native::read_finalized_provider_admission_v1(
            &fixture.state.view(),
            fixture.provider(),
            NOW + 1,
        )
        .unwrap()
        .unwrap();
        assert_eq!(actual.envelope(), fixture.envelope());
        assert!(actual.is_council_verified());
        fixture.revoke();
        assert!(
            native::read_finalized_provider_admission_v1(
                &fixture.state.view(),
                fixture.provider(),
                NOW + 2
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn restored_provider_fixture_rejects_substituted_signed_material_without_history_write() {
        let mut fixture = ProviderAdmissionTestFixtureV1::new();
        let mut changed = fixture.envelope.clone();
        changed.retention_epoch += 1;
        let policy = native::encode(&fixture.policy).unwrap();
        let provider = fixture.provider();
        fixture.commit(
            |tx| {
                enact_for_test(Action::ConfigureCouncil(policy), tx);
                assert!(apply(Action::Admit(native::encode(&changed).unwrap()), tx).is_err());
                assert!(
                    native::read_head(tx.world(), Some(provider))
                        .unwrap()
                        .is_none()
                );
            },
            true,
        );
    }
}
