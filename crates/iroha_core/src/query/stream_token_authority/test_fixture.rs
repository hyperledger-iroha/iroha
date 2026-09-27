//! Reusable test-only actual stream custody execution and fixed-roster finality.
use crate::{
    kura::Kura,
    query::{
        signer_check::fixture, store::LiveQueryStore,
        stream_token_custody::read_stream_token_custody_control_at_v1,
    },
    state::{State, World},
};
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::{Account, AccountId},
    isi::{InstructionBox, sorafs::MutateSorafsStreamTokenCustody},
    permission::{Permission, Permissions},
    sorafs::{capacity::ProviderId, stream_token_custody::SorafsStreamTokenCustodyActionV1},
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamToken, CanManageSorafsStreamTokenCustody, CanOperateSorafsStreamToken,
};
use iroha_sccp::{
    SCCP_TAIRA_CHAIN_ID_V1, SccpFinalizedBlockTestFixtureV1, sccp_taira_finality_network_id_v1,
};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAuthorityV1,
        SignerCustodyBindingV1, SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    custody_control::SignerCustodyPolicyV1,
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
};
use std::{sync::Arc, time::Duration};
fn account(seed: u8) -> AccountId {
    AccountId::new(fixture::key(seed).public_key().clone())
}
fn sign(state: &Arc<State>, instruction: InstructionBox, seed: u8, now: u64) -> SignedTransaction {
    let mut builder = TransactionBuilder::new(
        *state.network_id_ref(),
        account(seed),
        FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(now));
    builder
        .with_instructions([instruction])
        .try_sign(fixture::key(seed).private_key())
        .unwrap()
}
/// Test-only exact native custody fixture using actual typed executor results and revision-4 QCs.
/// It does not qualify ordinary fee/queue admission or a live four-process deployment.
pub struct StreamTokenRuntimeTestFixtureV1 {
    /// Actual applied State shared with production consumers under test.
    pub state: Arc<State>,
    /// Exact governed provider identity.
    pub provider: ProviderId,
    /// Governed public binding and attester policy.
    pub policy: SignerCustodyPolicyV1,
    /// Canonical signed active custody record material.
    pub record: Vec<u8>,
    finalized: Vec<SccpFinalizedBlockTestFixtureV1>,
}
impl StreamTokenRuntimeTestFixtureV1 {
    /// Remove one durable QC to test production refusal without exposing the fixture's Kura.
    ///
    /// # Errors
    /// Returns the filesystem error when the selected artifact cannot be removed.
    pub fn remove_finality_for_test(&self, height: u64) -> std::io::Result<()> {
        std::fs::remove_file(
            self.state
                .kura()
                .v2_finality_artifact_path_for_testing(height),
        )
    }

    /// Establish software custody around an independently supplied current Unix millisecond time.
    #[must_use]
    pub fn new_at(now: u64) -> Self {
        let provider = ProviderId::new([19; 32]);
        let mut world = World::new();
        for seed in 1..=3 {
            let (id, value) = Account::new(account(seed))
                .build(&account(1))
                .into_key_value();
            world.accounts.insert(id, value);
        }
        world.provider_owners.insert(provider, account(2));
        for (seed, permission) in [
            (
                1,
                Permission::from(CanManageSorafsStreamTokenCustody {
                    provider_id: provider,
                }),
            ),
            (
                2,
                Permission::from(CanOperateSorafsStreamToken {
                    provider_id: provider,
                }),
            ),
            (
                3,
                Permission::from(CanCheckSorafsStreamToken {
                    provider_id: provider,
                }),
            ),
        ] {
            let mut permissions = Permissions::new();
            permissions.insert(permission);
            world.account_permissions.insert(account(seed), permissions);
        }
        // Queue authority uses exactly the same four BLS validators as the durable test QCs.
        let validators = (1..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal).unwrap()
            })
            .collect::<Vec<_>>();
        let peers = validators
            .iter()
            .map(|key| iroha_model_base::peer::PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        world.peers = mv::cell::Cell::new(peers.iter().cloned().collect());
        for key in &validators {
            let owner = AccountId::new(key.public_key().clone());
            let (id, value) = Account::new(owner.clone()).build(&owner).into_key_value();
            world.accounts.insert(id, value);
            let pop = iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap();
            world.register_validator_pop_for_testing(key.public_key().clone(), pop.clone());
            let id = crate::state::derive_committee_key_id(key.public_key());
            world.consensus_keys.insert(
                id.clone(),
                iroha_data_model::consensus::ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: key.public_key().clone(),
                    pop: Some(pop),
                    activation_height: 0,
                    expiry_height: None,
                    replaces: None,
                    status: iroha_data_model::consensus::ConsensusKeyStatus::Active,
                },
            );
            let public = key.public_key().to_string();
            let mut by_key = world
                .consensus_keys_by_pk
                .view()
                .get(&public)
                .cloned()
                .unwrap_or_default();
            by_key.push(id);
            world.consensus_keys_by_pk.insert(public, by_key);
        }
        let mut state = State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            SCCP_TAIRA_CHAIN_ID_V1.parse().unwrap(),
            sccp_taira_finality_network_id_v1(),
        );
        state.commit_topology = mv::cell::Cell::new(peers);
        let mut nexus = state.nexus_snapshot();
        nexus.fees.base_fee = iroha_primitives::numeric::Quantity::zero();
        nexus.fees.per_byte_fee = iroha_primitives::numeric::Quantity::zero();
        nexus.fees.per_instruction_fee = iroha_primitives::numeric::Quantity::zero();
        nexus.fees.per_gas_unit_fee = iroha_primitives::numeric::Quantity::zero();
        state.set_nexus_from_config(nexus).unwrap();
        let statuses = state
            .nexus_snapshot()
            .lane_catalog
            .lanes()
            .iter()
            .map(|lane| {
                (
                    lane.id,
                    crate::governance::manifest::LaneManifestStatus {
                        lane: lane.id,
                        alias: lane.alias.clone(),
                        dataspace: lane.dataspace_id,
                        visibility: lane.visibility.clone(),
                        storage: lane.storage.clone(),
                        governance: None,
                        manifest_path: Some(std::path::PathBuf::from(format!(
                            "/test/stream-token-lane-{}.json",
                            lane.id.as_u32()
                        ))),
                        governance_rules: Some(crate::governance::manifest::GovernanceRules {
                            validators: validators
                                .iter()
                                .map(|key| AccountId::new(key.public_key().clone()))
                                .collect(),
                            ..Default::default()
                        }),
                        privacy_commitments: Vec::new(),
                    },
                )
            })
            .collect();
        state.install_lane_manifests(&Arc::new(
            crate::governance::manifest::LaneManifestRegistry::from_statuses(statuses),
        ));
        let state = Arc::new(state);
        let policy = SignerCustodyPolicyV1 {
            binding: SignerCustodyBindingV1 {
                chain_id: SCCP_TAIRA_CHAIN_ID_V1.into(),
                network_id: *state.network_id_ref().as_bytes(),
                runtime_handle: "software://sorafs/stream-token/primary".into(),
                key_handle: "software://sorafs/stream-token/key-1".into(),
                service_id: "stream-token-service".into(),
                administrator_id: "stream-token-admin".into(),
                role: SignerRoleV1::StreamToken,
                purpose: SignerPurposeBindingV1::StreamToken {
                    provider_id: *provider.as_bytes(),
                },
                algorithm: SignerKeyAlgorithmV1::Ed25519,
                public_key: fixture::key(4).public_key().clone(),
                key_revision: 1,
                policy_revision: 1,
                policy_digest: [5; 32],
            },
            attester_authority: SignerCustodyAuthorityV1 {
                service_id: "custody-service".into(),
                administrator_id: "custody-admin".into(),
                key_revision: 1,
                policy_revision: 1,
                policy_digest: [6; 32],
            },
            attester_public_key: fixture::key(7).public_key().clone(),
            active_from_unix_ms: now - 2_000,
            active_until_unix_ms: now + 3_600_000,
            max_validity_ms: 3_600_000,
            max_anchor_age_ms: 3_600_000,
        };
        let mut fixture = Self {
            state,
            provider,
            policy,
            finalized: Vec::new(),
            record: Vec::new(),
        };
        let configure = MutateSorafsStreamTokenCustody {
            provider_id: provider,
            expected_revision: 0,
            expected_digest: [0; 32],
            action: SorafsStreamTokenCustodyActionV1::Configure(
                norito::encode_canonical(&fixture.policy).unwrap(),
            ),
        };
        fixture.execute(configure.into(), 1, now - 1_000);
        let current = read_stream_token_custody_control_at_v1(
            &fixture.state.view(),
            &fixture.policy.binding,
            1,
        )
        .unwrap()
        .unwrap();
        let statement = SignerCustodyStatementV1 {
            magic: SIGNER_CUSTODY_MAGIC_V1,
            version: SIGNER_CUSTODY_VERSION_V1,
            binding: fixture.policy.binding.clone(),
            authority: fixture.policy.attester_authority.clone(),
            anchor: current.anchor,
            sequence: current.state.next_sequence,
            predecessor_digest: current.state.predecessor_digest,
            issued_at_unix_ms: now - 500,
            expires_at_unix_ms: now + 3_000_000,
            evidence_digest: [8; 32],
            revoked: false,
        };
        let signature = Signature::try_new(
            fixture::key(7).private_key(),
            &statement.signing_payload().unwrap(),
        )
        .unwrap();
        let enrollment = SignerCustodyRecordV1 {
            statement,
            attestation: signature.payload().try_into().unwrap(),
        };
        let enroll = MutateSorafsStreamTokenCustody {
            provider_id: provider,
            expected_revision: 1,
            expected_digest: current.anchor.state_digest,
            action: SorafsStreamTokenCustodyActionV1::Enroll(
                norito::encode_canonical(&enrollment).unwrap(),
            ),
        };
        fixture.record = norito::encode_canonical(&enrollment).unwrap();
        fixture.execute(enroll.into(), 1, now - 500);
        fixture
    }
    fn execute(&mut self, instruction: InstructionBox, seed: u8, now: u64) {
        let signed = sign(&self.state, instruction, seed, now);
        assert_eq!(
            fixture::commit_native_operation(
                &self.state,
                &mut self.finalized,
                now,
                vec![signed],
                true,
                true
            ),
            [true]
        );
    }
    /// Deterministic fixture signing key: manager 1, operator 2, observer 3, role 4, attester 7.
    #[must_use]
    pub fn key(seed: u8) -> KeyPair {
        fixture::key(seed)
    }
    /// Execute an exact supplied signed transaction and retain its actual success/failure and QC.
    pub fn commit_signed(&mut self, transaction: SignedTransaction, now: u64) -> bool {
        fixture::commit_native_operation(
            &self.state,
            &mut self.finalized,
            now,
            vec![transaction],
            true,
            true,
        )[0]
    }
    /// Sign and execute an allowed native test instruction with its actual success result and QC.
    pub fn commit_instruction(&mut self, instruction: InstructionBox, seed: u8, now: u64) -> bool {
        self.commit_signed(sign(&self.state, instruction, seed, now), now)
    }
    /// Finalize removal of the configured Check or Operate permission using the native executor.
    pub fn revoke_runtime_permission(&mut self, observer: bool, now: u64) -> bool {
        let (permission, seed) = if observer {
            (
                Permission::from(CanCheckSorafsStreamToken {
                    provider_id: self.provider,
                }),
                3,
            )
        } else {
            (
                Permission::from(CanOperateSorafsStreamToken {
                    provider_id: self.provider,
                }),
                2,
            )
        };
        self.commit_instruction(
            iroha_data_model::isi::Revoke::account_permission(permission, account(seed)).into(),
            1,
            now,
        )
    }
}
