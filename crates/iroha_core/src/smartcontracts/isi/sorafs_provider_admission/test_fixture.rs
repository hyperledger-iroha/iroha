//! Test-only native admission execution with actual software-signed four-validator finality.
use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, StateReadOnly, World},
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::{Account, AccountId},
    block::{BlockHeader, builder::BlockBuilder},
    sorafs::capacity::ProviderId,
};
use iroha_sccp::{
    SCCP_TAIRA_CHAIN_ID_V1, SccpFinalizedBlockTestFixtureV1,
    sccp_finalize_taira_block_test_fixture_v1, sccp_taira_finality_network_id_v1,
};
use sorafs_manifest::{
    CouncilSignature,
    provider_admission::{compute_envelope_authorization_digest, compute_envelope_digest},
};
use std::sync::Arc;
#[cfg(test)]
pub(crate) const NOW: u64 = 1_700_000_000;
pub(crate) fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}
pub(crate) fn raw(key: &KeyPair) -> [u8; 32] {
    key.public_key().to_bytes().1.try_into().unwrap()
}
/// Owns native admission execution and genuine fixed-roster test finality.
/// This fixture bypasses Parliament certificate admission and is not production evidence.
pub struct ProviderAdmissionTestFixtureV1 {
    pub(crate) state: Arc<State>,
    pub(crate) policy: ProviderAdmissionCouncilPolicyV1,
    pub(crate) envelope: ProviderAdmissionEnvelopeV1,
    pub(crate) signer: KeyPair,
    pub(crate) parent: Option<SccpFinalizedBlockTestFixtureV1>,
    now: u64,
}
impl ProviderAdmissionTestFixtureV1 {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::new_at(NOW)
    }
    /// Start with provider-owner fixture setup and no admission policy or history.
    #[must_use]
    pub fn new_at(now: u64) -> Self {
        let signer = key(31);
        let mut envelope: ProviderAdmissionEnvelopeV1 = norito::decode_from_bytes(include_bytes!(
            "../../../../../../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
        ))
        .unwrap();
        let provider = ProviderId::new(envelope.proposal.provider_id);
        let owner = AccountId::new(key(1).public_key().clone());
        let mut world = World::new();
        let (id, account) = Account::new(owner.clone()).build(&owner).into_key_value();
        world.accounts.insert(id, account);
        world.provider_owners.insert(provider, owner);
        let state = State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            SCCP_TAIRA_CHAIN_ID_V1.parse().unwrap(),
            sccp_taira_finality_network_id_v1(),
        );
        let policy = ProviderAdmissionCouncilPolicyV1 {
            network_id: *state.view().network_id().as_bytes(),
            policy_id: [0xc1; 32],
            version: 1,
            revision: 1,
            predecessor_policy_digest: None,
            trusted_signers: vec![raw(&signer)],
            signature_threshold: 1,
            paused: false,
        };
        envelope.issued_at = now;
        envelope.retention_epoch = now + 3600;
        sign_envelope(&mut envelope, &policy, &signer);
        Self {
            state: Arc::new(state),
            policy,
            envelope,
            signer,
            parent: None,
            now,
        }
    }
    /// Provider governed by this fixture.
    #[must_use]
    pub fn provider(&self) -> ProviderId {
        ProviderId::new(self.envelope.proposal.provider_id)
    }
    pub(crate) fn commit(
        &mut self,
        call: impl FnOnce(&mut StateTransaction<'_, '_>),
        durable_qc: bool,
    ) {
        let height = self.state.view().block_hashes().len() as u64 + 1;
        let header = BlockHeader::new(
            height.try_into().unwrap(),
            self.state.view().latest_block_hash(),
            None,
            (self.now + height) * 1000,
            0,
        );
        let mut block = self.state.block(header.clone());
        let mut tx = block.transaction();
        call(&mut tx);
        tx.apply();
        block.commit_world_overlay_for_testing().unwrap();
        let mut signed = BlockBuilder::new(header)
            .try_build_with_signature(0, key(0xfe).private_key())
            .unwrap();
        signed
            .set_execution_outputs(
                Vec::new(),
                0,
                Default::default(),
                Vec::new(),
                Default::default(),
                Default::default(),
                Vec::new(),
                &iroha_data_model::parameter::ExecutionOutputPolicyV1::bootstrap().limits(),
            )
            .unwrap();
        let finalized = sccp_finalize_taira_block_test_fixture_v1(&signed, self.parent.as_ref());
        self.state
            .kura()
            .store_block(Arc::new(finalized.block().clone()))
            .unwrap();
        self.state
            .append_committed_block_header_for_tests(finalized.block().header());
        if durable_qc {
            let _receipt = self
                .state
                .kura()
                .store_v2_finality_artifact(&finalized.proof().finality_artifact)
                .unwrap();
        }
        self.parent = Some(finalized);
    }
    /// Execute the initial policy and signed admission with durable native finality.
    pub fn admit(&mut self) {
        let policy = self.policy.clone();
        let envelope = self.envelope.clone();
        self.commit(
            |tx| {
                assert!(
                    apply(
                        Action::ConfigureCouncil(native::encode(&policy).unwrap()),
                        tx
                    )
                    .unwrap()
                );
                assert!(apply(Action::Admit(native::encode(&envelope).unwrap()), tx).unwrap());
            },
            true,
        );
    }

    /// The exact State shared with the production registry under test.
    #[must_use]
    pub fn state(&self) -> &Arc<State> {
        &self.state
    }
    /// Signed current fixture envelope for constructing independently signed adverts.
    #[must_use]
    pub fn envelope(&self) -> &ProviderAdmissionEnvelopeV1 {
        &self.envelope
    }
    /// Execute a terminal signed revocation and persist its actual durable finality.
    pub fn revoke(&mut self) {
        let revocation = self.revocation();
        self.commit(
            |tx| {
                apply(Action::Revoke(native::encode(&revocation).unwrap()), tx).unwrap();
            },
            true,
        );
    }
    pub(crate) fn revocation(&self) -> ProviderAdmissionRevocationV1 {
        let digest = compute_envelope_digest(&self.envelope).unwrap();
        let mut result = ProviderAdmissionRevocationV1 {
            version: sorafs_manifest::provider_admission::PROVIDER_ADMISSION_REVOCATION_VERSION_V1,
            network_id: self.policy.network_id,
            policy_id: self.policy.policy_id,
            policy_revision: self.policy.revision,
            policy_digest: self.policy.canonical_digest().unwrap(),
            transition_revision: self.envelope.admission_revision + 1,
            expected_current_event_digest: digest,
            provider_id: self.envelope.proposal.provider_id,
            envelope_digest: digest,
            revoked_at: self.now + 1,
            reason: "compromised endpoint".into(),
            council_signatures: vec![],
            notes: None,
        };
        result.council_signatures.push(CouncilSignature {
            signer: raw(&self.signer),
            signature: Signature::new(self.signer.private_key(), &result.digest().unwrap())
                .payload()
                .to_vec(),
        });
        result
    }
}
pub(crate) fn sign_envelope(
    envelope: &mut ProviderAdmissionEnvelopeV1,
    policy: &ProviderAdmissionCouncilPolicyV1,
    signer: &KeyPair,
) {
    envelope.network_id = policy.network_id;
    envelope.policy_id = policy.policy_id;
    envelope.policy_revision = policy.revision;
    envelope.policy_digest = policy.canonical_digest().unwrap();
    envelope.council_signatures.clear();
    envelope.council_signatures.push(CouncilSignature {
        signer: raw(signer),
        signature: Signature::new(
            signer.private_key(),
            &compute_envelope_authorization_digest(envelope).unwrap(),
        )
        .payload()
        .to_vec(),
    });
}

/// Apply an admission effect inside a test-only State overlay; Parliament authority is outside this fixture.
#[cfg(test)]
pub(crate) fn enact_for_test(action: Action, tx: &mut StateTransaction<'_, '_>) {
    assert!(apply(action, tx).unwrap());
}
