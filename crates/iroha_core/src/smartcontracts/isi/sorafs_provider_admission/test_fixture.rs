//! Native provider admission over a certified test chain (test support).
//!
//! The chain is a [`CertifiedTestChain`]: a signed genesis with a fixed four-validator committee
//! and blocks certified by real BLS `CommitQC`s, so the finalized admission readers accept it as
//! they accept a running node's. Council configuration and admission transitions are Parliament
//! effects in production; this fixture enacts them directly (setup only, [`enact_for_test`]) in
//! the World of the next block, which it then commits. Each commit is one second after the
//! previous block: genesis at `now - 1`, the first commit at `now + 1`.

use std::sync::Arc;

use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::{Account, AccountId},
    sorafs::{
        capacity::ProviderId,
        provider_admission::{
            PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1, ProviderAdmissionCouncilPolicyV1,
            governance::ProviderAdmissionGovernanceActionV1 as Action,
        },
    },
};
use sorafs_manifest::{
    CouncilSignature, ProviderAdmissionEnvelopeV1, ProviderAdmissionRevocationV1,
    provider_admission::{
        PROVIDER_ADMISSION_REVOCATION_VERSION_V1, compute_advert_body_digest,
        compute_envelope_authorization_digest, compute_envelope_digest, compute_proposal_digest,
    },
};

use crate::{
    state::{State, StateTransaction, World},
    sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig},
};

/// The fixture's reference time in Unix seconds.
pub const NOW: u64 = 1_700_000_000;

/// The canonical first-release envelope fixture the admission is built from.
const ENVELOPE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
));

/// A deterministic Ed25519 fixture key.
///
/// # Panics
/// Never for a fixed seed.
#[must_use]
pub fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

/// The raw 32-byte Ed25519 public key of `key`.
///
/// # Panics
/// `key` is not Ed25519.
#[must_use]
pub fn raw(key: &KeyPair) -> [u8; 32] {
    key.public_key()
        .to_bytes()
        .1
        .try_into()
        .expect("Ed25519 public key")
}

/// Bind `envelope` to `policy` and re-sign it with `signer` alone (digests recomputed).
///
/// # Panics
/// The envelope or policy does not encode.
pub fn sign_envelope(
    envelope: &mut ProviderAdmissionEnvelopeV1,
    policy: &ProviderAdmissionCouncilPolicyV1,
    signer: &KeyPair,
) {
    envelope.network_id = policy.network_id;
    envelope.policy_id = policy.policy_id;
    envelope.policy_revision = policy.revision;
    envelope.policy_digest = policy.canonical_digest().expect("valid fixture policy");
    envelope.proposal_digest = compute_proposal_digest(&envelope.proposal).expect("proposal");
    envelope.advert_body_digest =
        compute_advert_body_digest(&envelope.advert_body).expect("advert body");
    envelope.council_signatures.clear();
    let digest = compute_envelope_authorization_digest(envelope).expect("authorization digest");
    envelope.council_signatures = vec![CouncilSignature {
        signer: raw(signer),
        signature: Signature::new(signer.private_key(), &digest)
            .payload()
            .to_vec(),
    }];
}

/// Enact one admission action as Parliament would after certifying it (setup only).
///
/// # Panics
/// The action does not apply.
pub fn enact_for_test(action: Action, tx: &mut StateTransaction<'_, '_>) {
    assert!(
        super::apply(action, tx).expect("the fixture admission action applies"),
        "the fixture admission action changed nothing"
    );
}

/// A certified chain with one council policy and one provider envelope ready to admit.
pub struct ProviderAdmissionTestFixtureV1 {
    chain: CertifiedTestChain,
    /// The chain's State.
    pub state: Arc<State>,
    /// The council's single signer.
    pub signer: KeyPair,
    /// The council policy [`Self::admit`] configures.
    pub policy: ProviderAdmissionCouncilPolicyV1,
    /// The provider envelope [`Self::admit`] admits.
    pub envelope: ProviderAdmissionEnvelopeV1,
    /// Unix seconds of the last committed block.
    time: u64,
}

impl core::fmt::Debug for ProviderAdmissionTestFixtureV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProviderAdmissionTestFixtureV1")
            .field("chain", &self.chain)
            .field("time", &self.time)
            .finish_non_exhaustive()
    }
}

impl Default for ProviderAdmissionTestFixtureV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAdmissionTestFixtureV1 {
    /// A fixture at [`NOW`].
    #[must_use]
    pub fn new() -> Self {
        Self::new_at(NOW)
    }

    /// A fixture whose envelope is issued at `now` (Unix seconds) and retained for an hour; the
    /// provider owner is `key(1)`'s account.
    ///
    /// # Panics
    /// The certified chain does not start.
    #[must_use]
    pub fn new_at(now: u64) -> Self {
        let owner = AccountId::new(key(1).public_key().clone());
        let mut world = World::new();
        let (id, account) = Account::new(owner.clone()).build(&owner).into_key_value();
        world.accounts.insert(id, account);
        let chain = CertifiedTestChain::start(TestChainConfig::new(
            world,
            now.saturating_sub(1).saturating_mul(1000),
        ))
        .map_err(|failure| failure.error)
        .expect("the certified fixture chain starts");
        let signer = key(0x45);
        let policy = ProviderAdmissionCouncilPolicyV1 {
            network_id: *chain.network_id().as_bytes(),
            policy_id: [0xC1; 32],
            version: PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            trusted_signers: vec![raw(&signer)],
            signature_threshold: 1,
            paused: false,
        };
        let mut envelope: ProviderAdmissionEnvelopeV1 =
            norito::decode_from_bytes(ENVELOPE).expect("canonical envelope fixture");
        envelope.admission_revision = 1;
        envelope.expected_current_event_digest = None;
        envelope.issued_at = now;
        envelope.retention_epoch = now + 3600;
        envelope.notes = None;
        sign_envelope(&mut envelope, &policy, &signer);
        Self {
            state: Arc::clone(chain.state()),
            chain,
            signer,
            policy,
            envelope,
            time: now,
        }
    }

    /// The chain's State.
    #[must_use]
    pub fn state(&self) -> &Arc<State> {
        &self.state
    }

    /// The provider envelope.
    #[must_use]
    pub fn envelope(&self) -> &ProviderAdmissionEnvelopeV1 {
        &self.envelope
    }

    /// The provider the envelope admits.
    #[must_use]
    pub fn provider(&self) -> ProviderId {
        ProviderId::new(self.envelope.proposal.provider_id)
    }

    /// The certified chain.
    #[must_use]
    pub fn chain(&self) -> &CertifiedTestChain {
        &self.chain
    }

    /// Commit the next block, one second after the previous one, after `setup` edited the World
    /// it is read at (setup outside consensus, see [`CertifiedTestChain::setup_world_at`]).
    /// Without `certified`, the block's local `CommitQC` does not verify (two of four signers).
    pub fn commit(&mut self, setup: impl FnOnce(&mut StateTransaction<'_, '_>), certified: bool) {
        self.time += 1;
        let time_ms = self.time * 1000;
        self.chain.setup_world_at(time_ms, setup);
        let signers = if certified {
            Signers::Quorum
        } else {
            Signers::BelowQuorum
        };
        self.chain.commit_with(Some(time_ms), Vec::new(), signers);
    }

    /// Configure the council and admit the provider (owned by `key(1)`) in one certified block.
    pub fn admit(&mut self) {
        let owner = AccountId::new(key(1).public_key().clone());
        let provider = self.provider();
        let policy = norito::encode_canonical(&self.policy).expect("policy");
        let envelope = norito::encode_canonical(&self.envelope).expect("envelope");
        self.commit(
            |tx| {
                tx.world.provider_owners.insert(provider, owner);
                enact_for_test(Action::ConfigureCouncil(policy), tx);
                enact_for_test(Action::Admit(envelope), tx);
            },
            true,
        );
    }

    /// The council-signed revocation of the admitted envelope.
    ///
    /// # Panics
    /// The envelope or policy does not encode.
    #[must_use]
    pub fn revocation(&self) -> ProviderAdmissionRevocationV1 {
        let digest = compute_envelope_digest(&self.envelope).expect("envelope digest");
        let mut revocation = ProviderAdmissionRevocationV1 {
            version: PROVIDER_ADMISSION_REVOCATION_VERSION_V1,
            network_id: self.policy.network_id,
            policy_id: self.policy.policy_id,
            policy_revision: self.policy.revision,
            policy_digest: self.policy.canonical_digest().expect("policy digest"),
            transition_revision: 2,
            expected_current_event_digest: digest,
            provider_id: self.envelope.proposal.provider_id,
            envelope_digest: digest,
            revoked_at: self.envelope.issued_at,
            reason: "fixture revocation".into(),
            council_signatures: Vec::new(),
            notes: None,
        };
        let signed = revocation.digest().expect("revocation digest");
        revocation.council_signatures = vec![CouncilSignature {
            signer: raw(&self.signer),
            signature: Signature::new(self.signer.private_key(), &signed)
                .payload()
                .to_vec(),
        }];
        revocation
    }

    /// Revoke the admitted provider in the next certified block.
    pub fn revoke(&mut self) {
        let revocation = norito::encode_canonical(&self.revocation()).expect("revocation");
        self.commit(|tx| enact_for_test(Action::Revoke(revocation), tx), true);
    }
}
