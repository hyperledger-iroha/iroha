//! Selected repair-handler effects and rollback on a certified four-validator fixture.
//!
//! The shared boundary validator authenticates all four signatures, the actual fixture parent,
//! roster and exact catalog transition. Only the hard-pinned production network scope is supplied
//! by the explicit test seam. Successful handler effects are fixture World setup before an empty
//! certified block; these tests do not claim a production repair transaction or replay qualification.

use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    IntoKeyValue, NetworkId, Registrable,
    account::Account,
    block::BlockHeader,
    isi::sorafs_repair::ApplySorafsRepairV1,
    runtime::sorafs_repair::{
        SORAFS_REPAIR_SIGNATURE_DOMAIN_V1, SORAFS_REPAIR_VERSION_V1, SorafsRepairCertificateV1,
        SorafsRepairPlanV1,
    },
    sorafs::{
        bootstrap::SorafsBootstrapGrantV1,
        provider_admission::{
            PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1, ProviderAdmissionCouncilPolicyV1,
        },
    },
};
use iroha_executor_data_model::permission::{
    governance::CanManageParliament,
    sorafs::{
        CanBindSorafsAlias, CanCompleteSorafsReplicationOrder, CanSetSorafsReservePolicy,
        CanUpsertSorafsProviderCredit,
    },
};
use sorafs_manifest::{
    ProviderAdmissionEnvelopeV1, provider_admission::ProviderAdmissionGenesisMaterialV1,
};

use super::*;
use crate::{
    query::provider_admission::read_finalized_provider_admission_v1,
    smartcontracts::isi::sorafs_provider_admission::test_fixture::{
        NOW, enact_for_test, key, raw, sign_envelope,
    },
    state::{StateReadOnly, World, threshold_key_lifecycle_roster_v1},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig, fixture_keys},
};

const ENVELOPE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
));

pub(crate) fn account(seed: u8) -> AccountId {
    AccountId::new(key(seed).public_key().clone())
}

/// Accounts: 1 owner, 2 completion signer, 3 reserve manager, 4 alias operator,
/// 5 compliance operator, 6 submitter.
pub(crate) struct Fixture {
    pub(crate) chain: CertifiedTestChain,
    council: KeyPair,
    time: u64,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        Self::with_world(World::new())
    }

    pub(crate) fn with_world(mut world: World) -> Self {
        for seed in 1..=6 {
            let id = account(seed);
            let (id, account) = Account::new(id.clone()).build(&id).into_key_value();
            world.accounts.insert(id, account);
        }
        let chain = CertifiedTestChain::start(TestChainConfig::new(world, (NOW - 1) * 1000))
            .map_err(|failure| failure.error)
            .expect("the certified chain starts without SoraFS initialization");
        Self {
            chain,
            council: key(0x45),
            time: NOW,
        }
    }

    pub(crate) fn next_time_ms(&mut self) -> u64 {
        self.time += 1;
        self.time * 1000
    }

    fn policy(&self) -> ProviderAdmissionCouncilPolicyV1 {
        ProviderAdmissionCouncilPolicyV1 {
            network_id: *self.chain.network_id().as_bytes(),
            policy_id: [0xC1; 32],
            version: PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            trusted_signers: vec![raw(&self.council)],
            signature_threshold: 1,
            paused: false,
        }
    }

    fn envelope_at(&self, index: u8) -> ProviderAdmissionEnvelopeV1 {
        let mut envelope: ProviderAdmissionEnvelopeV1 =
            norito::decode_from_bytes(ENVELOPE).expect("canonical envelope fixture");
        envelope.admission_revision = 1;
        envelope.expected_current_event_digest = None;
        envelope.issued_at = NOW;
        envelope.retention_epoch = NOW + 3600;
        envelope.notes = None;
        envelope.proposal.provider_id = [0x11 + index; 32];
        envelope.advert_body.provider_id = envelope.proposal.provider_id;
        sign_envelope(&mut envelope, &self.policy(), &self.council);
        envelope
    }

    fn envelope(&self) -> ProviderAdmissionEnvelopeV1 {
        self.envelope_at(0)
    }

    fn provider(&self) -> ProviderId {
        ProviderId::new(self.envelope().proposal.provider_id)
    }

    pub(crate) fn payload(&self) -> SorafsBootstrapPayloadV1 {
        let mut grants = vec![
            SorafsBootstrapGrantV1 {
                account: account(3),
                permission: CanSetSorafsReservePolicy.into(),
            },
            SorafsBootstrapGrantV1 {
                account: account(3),
                permission: CanUpsertSorafsProviderCredit.into(),
            },
            SorafsBootstrapGrantV1 {
                account: account(4),
                permission: CanBindSorafsAlias.into(),
            },
            SorafsBootstrapGrantV1 {
                account: account(2),
                permission: CanCompleteSorafsReplicationOrder {
                    provider_id: self.provider(),
                }
                .into(),
            },
        ];
        grants.sort();
        SorafsBootstrapPayloadV1 {
            council: norito::encode_canonical(&self.policy()).expect("policy"),
            providers: (0..3)
                .map(
                    |index| iroha_data_model::sorafs::bootstrap::SorafsBootstrapProviderV1 {
                        owner: account(1),
                        envelope: norito::encode_canonical(&self.envelope_at(index))
                            .expect("envelope"),
                    },
                )
                .collect(),
            grants,
            compliance_operators: vec![account(5)],
        }
    }

    /// The exact next-boundary plan signed by all four ordered native fixture validators.
    pub(crate) fn certificate(
        &self,
        payload: &SorafsBootstrapPayloadV1,
    ) -> SorafsRepairCertificateV1 {
        let height = self.chain.height() + 1;
        let view = self.chain.state().view();
        let roster = threshold_key_lifecycle_roster_v1(view.world(), height)
            .expect("scheduled fixture committee");
        let network = self.chain.network_id();
        let tip = view
            .native_execution_tip()
            .expect("certified fixture parent");
        let catalog = crate::runtime::release_catalog().expect("release catalog");
        let change_set = catalog
            .sorafs_repair_activation(
                view.world().runtime_selection(),
                &crate::runtime::change_set::PublishedDefinitions(
                    view.world().runtime_definitions(),
                ),
                network,
                height,
                [0x31; 32],
            )
            .expect("dedicated repair successor");
        let plan = SorafsRepairPlanV1 {
            version: SORAFS_REPAIR_VERSION_V1,
            chain_id: view.chain_id.clone(),
            network_id: network,
            operation_id: [0x32; 32],
            parent_height: tip.height(),
            parent_block_hash: tip.core_hash().0,
            parent_result_hash: tip.result().0,
            change_set,
            committee: roster
                .iter()
                .map(|peer| peer.public_key().clone())
                .collect::<Vec<_>>()
                .try_into()
                .expect("four validators"),
            expires_at_unix_ms: (NOW + 3600) * 1000,
            authorization_digest: crate::runtime::sorafs_repair::AUTHORIZATION_DIGEST,
            release_digest: [0x33; 32],
            source_closure_digest: [0x34; 32],
            payload: payload.clone(),
        };
        sign_plan(plan)
    }

    /// Apply the authenticated selected handler in a rollback-owned fixture transaction.
    /// Rejected transactions and their entire block overlay are discarded. This is not full
    /// signed transaction admission: the test seam changes only the expected deployment scope.
    pub(crate) fn submit(&mut self, certificate: SorafsRepairCertificateV1) -> Option<String> {
        let time = self.next_time_ms();
        let state = std::sync::Arc::clone(self.chain.state());
        let view = state.view();
        let header = BlockHeader::new(
            std::num::NonZeroU64::new(self.chain.height() + 1).unwrap(),
            view.latest_block_hash(),
            None,
            time,
            0,
        );
        drop(view);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let roster = tx
            .threshold_key_lifecycle_frozen_roster_v1()
            .expect("fixture roster");
        let committee = roster
            .iter()
            .map(|peer| peer.public_key().clone())
            .collect::<Vec<_>>();
        let successor = match crate::runtime::sorafs_repair::validate_fixture_boundary(
            &certificate,
            &*tx.world,
            tx.native_execution_tip(),
            &tx.chain_id,
            tx.network_id,
            tx.block_height(),
            tx.block_unix_timestamp_ms(),
            &committee,
        ) {
            Ok(value) => value,
            Err(error) => return Some(format!("{error:?}")),
        };
        for descriptor in &certificate.plan.change_set.publish {
            tx.world
                .runtime_definitions
                .insert(descriptor.id().unwrap(), descriptor.clone());
        }
        *tx.world.runtime_selection.get_mut() = successor;
        tx.sorafs_repair_entrypoint = Some(certificate.plan.id().expect("signed plan"));
        if let Err(error) = (ApplySorafsRepairV1 { certificate }).execute(&account(6), &mut tx) {
            return Some(format!("{error:?}"));
        }
        tx.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("fixture World effects publish");
        self.chain.commit_at(time, Vec::new());
        None
    }

    fn world_root(&self) -> iroha_crypto::Hash {
        self.chain
            .state()
            .verify_world_state_accumulator()
            .expect("cold capture authenticates the complete committed World")
    }

    fn assert_uninitialized(&self) {
        let view = self.chain.state().view();
        assert!(
            read_sorafs_bootstrap_record_v1(view.world())
                .unwrap()
                .is_none()
        );
        assert!(
            crate::query::provider_admission::read_policy(view.world())
                .unwrap()
                .is_none()
        );
        assert!(
            !view.world().account_contains_inherent_permission(
                &account(3),
                &CanSetSorafsReservePolicy.into()
            )
        );
        assert!(view.world().roles().get(&compliance_role()).is_none());
    }
}

fn sign_plan(plan: SorafsRepairPlanV1) -> SorafsRepairCertificateV1 {
    // Sign canonical bytes directly so negative tests can offer genuinely signed malformed
    // plans. Production encoding refuses those plans before signing; verification must too.
    let frame = norito::encode_canonical(&plan).expect("fixture plan frame");
    let bytes = [SORAFS_REPAIR_SIGNATURE_DOMAIN_V1, frame.as_slice()].concat();
    let signatures = std::array::from_fn(|index| {
        let signer = fixture_keys()
            .into_iter()
            .find(|key| key.public_key() == &plan.committee[index])
            .expect("fixture validator");
        Signature::new(signer.private_key(), &bytes)
    });
    SorafsRepairCertificateV1 { plan, signatures }
}

#[test]
fn bootstrap_preserves_existing_bpng_accounts_balances_and_application_state() {
    use iroha_data_model::{
        asset::{Asset, AssetBalancePolicy, AssetDefinition, AssetDefinitionId, AssetId},
        domain::Domain,
    };
    use iroha_model_base::{domain::DomainId, metadata::Metadata};
    use iroha_primitives::{json::Json, numeric::Quantity};

    let owner = account(7);
    let customer = account(8);
    let domain = DomainId::try_new("bpng", "universal").expect("BPNG fixture domain");
    let definition = AssetDefinitionId::derive_from_components(
        domain.clone(),
        "preserved_balance".parse().expect("fixture asset name"),
    );
    let owner_asset = AssetId::new(definition.clone(), owner.clone());
    let customer_asset = AssetId::new(definition.clone(), customer.clone());
    let mut metadata = Metadata::default();
    metadata.insert(
        "institution".parse().expect("metadata key"),
        Json::new("BPNG preserved account"),
    );
    let mut world = World::with_assets(
        [Domain::new(domain.clone()).build(&owner)],
        [
            Account::new(owner.clone())
                .with_metadata(metadata.clone())
                .build(&owner),
            Account::new(customer.clone())
                .with_metadata(metadata)
                .build(&customer),
        ],
        [AssetDefinition::numeric(
            definition.clone(),
            "BPNG preservation fixture".to_owned(),
            AssetBalancePolicy::Global,
            Some(domain.clone()),
        )
        .build(&owner)],
        [
            Asset::new(owner_asset.clone(), Quantity::from(75_000_u32)),
            Asset::new(customer_asset.clone(), Quantity::from(25_000_u32)),
        ],
        [],
    );
    let application_path: StatePath = "bpng/preservation/committed_data"
        .parse()
        .expect("unrelated application path");
    world
        .smart_contract_state
        .insert(application_path.clone(), b"retained BPNG data".to_vec());
    let mut fixture = Fixture::with_world(world);
    let projection = |fixture: &Fixture| {
        let view = fixture.chain.state().view();
        let world = view.world();
        // Compare complete canonical account values: Account equality itself is ID-only.
        let accounts = world
            .accounts()
            .iter()
            .map(|(id, value)| {
                (
                    id.clone(),
                    norito::encode_canonical(value).expect("canonical account value"),
                )
            })
            .collect::<Vec<_>>();
        (
            accounts,
            norito::encode_canonical(world.domains().get(&domain).expect("retained domain"))
                .expect("canonical domain"),
            norito::encode_canonical(
                world
                    .asset_definitions()
                    .get(&definition)
                    .expect("retained asset definition"),
            )
            .expect("canonical asset definition"),
            world
                .assets()
                .get(&owner_asset)
                .expect("owner balance")
                .clone(),
            world
                .assets()
                .get(&customer_asset)
                .expect("customer balance")
                .clone(),
            world
                .smart_contract_state()
                .get(&application_path)
                .expect("retained BPNG application data")
                .clone(),
        )
    };
    let before = projection(&fixture);
    let original_network = fixture.chain.network_id();
    let original_height = fixture.chain.height();
    // The third envelope fails only after the council, first two admissions and owner
    // bindings have been staged. The full cold World root covers runtime selection,
    // published definitions, BPNG records and every other committed storage table.
    let before_root = fixture.world_root();
    let failed = fixture.certificate(&unsigned_third_provider(&fixture));
    assert!(
        failed.verify().is_ok(),
        "all four validators authenticated the plan"
    );
    assert!(fixture.submit(failed).is_some());
    assert_eq!(fixture.world_root(), before_root);
    assert_eq!(projection(&fixture), before);
    fixture.assert_uninitialized();

    let certificate = fixture.certificate(&fixture.payload());
    assert_eq!(fixture.submit(certificate.clone()), None);
    assert_eq!(fixture.chain.network_id(), original_network);
    assert_eq!(fixture.chain.height(), original_height + 1);
    assert_eq!(projection(&fixture), before);
    let committed_root = fixture.world_root();
    assert!(fixture.submit(certificate).is_some());
    assert_eq!(fixture.world_root(), committed_root);
    assert_eq!(projection(&fixture), before);
}

#[test]
fn all_four_certificate_initializes_exactly_three_providers_once() {
    let mut fixture = Fixture::new();
    fixture.assert_uninitialized();
    let payload = fixture.payload();
    let certificate = fixture.certificate(&payload);
    certificate.verify().expect("all four ordered signatures");
    let session = certificate.plan.id().expect("plan identity");
    assert_eq!(fixture.submit(certificate.clone()), None);
    let height = fixture.chain.height();
    let view = fixture.chain.state().view();
    // The bootstrap admission is an ordinary council-verified record that the finalized
    // reader authenticates against the certified fixture chain, like a Parliament admission.
    for index in 0..3 {
        let provider = ProviderId::new(fixture.envelope_at(index).proposal.provider_id);
        let admitted = read_finalized_provider_admission_v1(&view, provider, fixture.time)
            .expect("authenticated admission")
            .expect("the provider is admitted");
        assert_eq!(
            admitted.envelope().proposal.provider_id,
            *provider.as_bytes()
        );
        assert_eq!(
            view.world().provider_owners().get(&provider),
            Some(&account(1))
        );
    }
    for grant in &payload.grants {
        assert!(
            view.world()
                .account_contains_inherent_permission(&grant.account, &grant.permission),
            "{grant:?}"
        );
    }
    assert!(
        view.world()
            .account_roles_iter(&account(5))
            .any(|role| role == &compliance_role())
    );
    for seed in 1..=6 {
        assert!(
            !view
                .world()
                .account_contains_inherent_permission(&account(seed), &CanManageParliament.into())
        );
    }
    let record = read_sorafs_bootstrap_record_v1(view.world())
        .expect("canonical record")
        .expect("retained record");
    assert_eq!(record.height, height);
    assert_eq!(record.network_id, *fixture.chain.network_id().as_bytes());
    assert_eq!(record.session_id, session);
    let receipt_path: StatePath = "sorafs/repair/certificate".parse().unwrap();
    let receipt = view
        .world()
        .smart_contract_state()
        .get(&receipt_path)
        .expect("complete all-four authority receipt");
    assert_eq!(
        SorafsRepairCertificateV1::from_bytes(receipt).unwrap(),
        certificate
    );
    drop(view);

    // Replaying the same authenticated plan cannot change the selected root or receipt.
    let committed_root = fixture.world_root();
    assert!(fixture.submit(certificate).is_some());
    assert_eq!(fixture.world_root(), committed_root);
}

#[test]
fn certificate_requires_all_four_exact_validators_and_bound_context() {
    let mut fixture = Fixture::new();
    let before = fixture.world_root();
    let valid = fixture.certificate(&fixture.payload());
    let mut rejected = Vec::new();

    // Repeating one of three signatures cannot substitute for the fourth signer.
    let mut three = valid.clone();
    three.signatures[3] = three.signatures[2].clone();
    rejected.push(("only three validators", three));
    let mut forged = valid.clone();
    let foreign = KeyPair::from_seed(vec![0x99; 32], Algorithm::BlsNormal);
    forged.signatures[3] = Signature::new(
        foreign.private_key(),
        &forged.plan.signing_bytes().expect("valid plan"),
    );
    rejected.push(("foreign fourth signature", forged));

    let mut reordered = valid.plan.clone();
    reordered.committee.swap(0, 1);
    rejected.push(("signed but reordered committee", sign_plan(reordered)));
    let mut foreign_network = valid.plan.clone();
    foreign_network.network_id = NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([7; 32])),
    );
    foreign_network.change_set.network = foreign_network.network_id;
    rejected.push(("signed foreign network", sign_plan(foreign_network)));
    let mut foreign_chain = valid.plan.clone();
    foreign_chain.chain_id = "other-chain".parse().expect("chain id");
    rejected.push(("signed foreign chain", sign_plan(foreign_chain)));
    let mut late = valid.plan.clone();
    late.parent_height += 1;
    late.change_set.effective_height += 1;
    rejected.push(("signed different height", sign_plan(late)));
    let mut different_parent = valid.plan.clone();
    different_parent.parent_block_hash = [0x91; 32];
    rejected.push((
        "signed different native parent",
        sign_plan(different_parent),
    ));
    let mut different_result = valid.plan.clone();
    different_result.parent_result_hash = [0x92; 32];
    rejected.push((
        "signed different native result",
        sign_plan(different_result),
    ));
    let mut expired = valid.plan.clone();
    expired.expires_at_unix_ms = NOW * 1000;
    rejected.push(("signed expired plan", sign_plan(expired)));
    let mut foreign_authorization = valid.plan.clone();
    foreign_authorization.authorization_digest = [0x93; 32];
    rejected.push((
        "signed different owner authorization",
        sign_plan(foreign_authorization),
    ));
    let mut missing_provider = valid.plan.clone();
    missing_provider.payload.providers.pop();
    rejected.push(("signed only two providers", sign_plan(missing_provider)));
    let mut broadened = valid.plan.clone();
    broadened.change_set.publish.pop();
    rejected.push(("signed different catalog transition", sign_plan(broadened)));

    for (reason, certificate) in rejected {
        let error = fixture.submit(certificate).expect(reason);
        assert!(error.contains("repair rejected"), "{reason}: {error}");
        assert_eq!(fixture.world_root(), before, "{reason}");
        fixture.assert_uninitialized();
    }
}

#[test]
fn certificate_cannot_exceed_or_override_existing_sorafs_roots() {
    let mut fixture = Fixture::new();
    let before = fixture.world_root();
    // Non-SoraFS authority is outside the closed allowlist.
    let mut parliament = fixture.payload();
    parliament.grants.push(SorafsBootstrapGrantV1 {
        account: account(3),
        permission: CanManageParliament.into(),
    });
    parliament.grants.sort();
    let error = fixture
        .submit(fixture.certificate(&parliament))
        .expect("parliament manager");
    assert!(
        error.contains("invalid SoraFS bootstrap payload"),
        "{error}"
    );
    assert_eq!(fixture.world_root(), before);
    // A worker permission for a provider the bootstrap does not admit.
    let mut foreign_provider = fixture.payload();
    foreign_provider.grants.push(SorafsBootstrapGrantV1 {
        account: account(2),
        permission: CanCompleteSorafsReplicationOrder {
            provider_id: ProviderId::new([0xEE; 32]),
        }
        .into(),
    });
    foreign_provider.grants.sort();
    let error = fixture
        .submit(fixture.certificate(&foreign_provider))
        .expect("foreign provider");
    assert!(error.contains("outside this bootstrap"), "{error}");
    assert_eq!(fixture.world_root(), before);
    fixture.assert_uninitialized();

    // An existing holder already roots the reserve policy permission.
    let time = fixture.next_time_ms();
    fixture.chain.setup_world_at(time, |tx| {
        tx.world
            .add_account_permission(&account(6), CanSetSorafsReservePolicy.into());
    });
    fixture.chain.commit_at(time, Vec::new());
    let before = fixture.world_root();
    let error = fixture
        .submit(fixture.certificate(&fixture.payload()))
        .expect("existing holder");
    assert!(error.contains("already has a holder"), "{error}");
    assert_eq!(fixture.world_root(), before);

    // Admission already initialized (here by a Parliament-equivalent council effect).
    let mut initialized = Fixture::new();
    let policy = norito::encode_canonical(&initialized.policy()).expect("policy");
    let time = initialized.next_time_ms();
    initialized.chain.setup_world_at(time, |tx| {
        enact_for_test(&Action::ConfigureCouncil(policy), tx)
    });
    initialized.chain.commit_at(time, Vec::new());
    let before = initialized.world_root();
    let error = initialized
        .submit(initialized.certificate(&initialized.payload()))
        .expect("initialized admission");
    assert!(error.contains("already initialized"), "{error}");
    assert_eq!(initialized.world_root(), before);
}

/// A structurally valid third envelope without the required admission council signature.
/// The first two providers execute before this one fails inside the same transaction.
fn unsigned_third_provider(fixture: &Fixture) -> SorafsBootstrapPayloadV1 {
    let envelope = fixture.envelope_at(2);
    let material = ProviderAdmissionGenesisMaterialV1 {
        proposal: envelope.proposal.clone(),
        advert_body: envelope.advert_body.clone(),
        issued_at: envelope.issued_at,
        retention_epoch: envelope.retention_epoch,
    };
    let policy = fixture.policy();
    let unsigned = material
        .project(
            policy.network_id,
            policy.policy_id,
            policy.canonical_digest().expect("policy digest"),
        )
        .expect("projection");
    let mut payload = fixture.payload();
    payload.providers[2].envelope = norito::encode_canonical(&unsigned).expect("unsigned");
    payload
}

#[test]
fn late_unsigned_genesis_envelope_rolls_back_selection_and_all_staged_effects() {
    let mut fixture = Fixture::new();
    let before = fixture.world_root();
    let certificate = fixture.certificate(&unsigned_third_provider(&fixture));
    certificate
        .verify()
        .expect("all four signatures authenticate this payload");
    let error = fixture.submit(certificate).expect("unsigned material");
    assert!(
        error.contains("admitted") || error.contains("admission"),
        "{error}"
    );
    assert_eq!(fixture.world_root(), before);
    fixture.assert_uninitialized();
    // A clean all-four plan is still admissible after the discarded partial attempt.
    assert_eq!(
        fixture.submit(fixture.certificate(&fixture.payload())),
        None
    );
}

#[test]
fn handler_requires_the_exact_selected_boundary() {
    let mut fixture = Fixture::new();
    let before = fixture.world_root();
    let certificate = fixture.certificate(&fixture.payload());
    let state = std::sync::Arc::clone(fixture.chain.state());
    let view = state.view();
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(fixture.chain.height() + 1).unwrap(),
        view.latest_block_hash(),
        None,
        fixture.next_time_ms(),
        0,
    );
    drop(view);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    for entrypoint in [None, Some(certificate.plan.id().unwrap())] {
        tx.sorafs_repair_entrypoint = entrypoint;
        let error = (ApplySorafsRepairV1 {
            certificate: certificate.clone(),
        })
        .execute(&account(6), &mut tx)
        .expect_err("unselected predecessor cannot execute repair");
        assert!(format!("{error:?}").contains("exact unconsumed signed repair boundary"));
    }
    drop(tx);
    drop(block);
    assert_eq!(fixture.world_root(), before);
    fixture.assert_uninitialized();
}

#[test]
fn an_orphan_retained_certificate_is_never_overwritten() {
    let mut fixture = Fixture::new();
    let retained = fixture.certificate(&fixture.payload()).to_bytes().unwrap();
    let time = fixture.next_time_ms();
    fixture.chain.setup_world_at(time, |tx| {
        tx.world
            .smart_contract_state
            .insert(certificate_path(), retained.clone());
    });
    fixture.chain.commit_at(time, Vec::new());
    fixture.assert_uninitialized();
    let before = fixture.world_root();
    let error = fixture
        .submit(fixture.certificate(&fixture.payload()))
        .expect("orphan authority receipt must not be replaced");
    assert!(
        error.contains("retained repair certificate already exists"),
        "{error}"
    );
    assert_eq!(fixture.world_root(), before);
    assert_eq!(
        fixture
            .chain
            .state()
            .view()
            .world()
            .smart_contract_state()
            .get(&certificate_path()),
        Some(&retained)
    );
    fixture.assert_uninitialized();
}

#[test]
fn signed_executor_enforces_the_production_network_pin_before_any_effect() {
    use crate::{executor::Executor, smartcontracts::ivm::cache::IvmCache};
    use iroha_model_base::topology::DataSpaceId;

    let mut fixture = Fixture::new();
    let before = fixture.world_root();
    let certificate = fixture.certificate(&fixture.payload());
    certificate
        .verify()
        .expect("the complete four-validator certificate is valid");
    let time = fixture.next_time_ms();
    let source = fixture
        .chain
        .sign(&key(6), [ApplySorafsRepairV1 { certificate }.into()], time);
    // Exercise the actual newly typed instruction and complete certificate on the wire.
    let frame = norito::encode_canonical(&source).unwrap();
    let source: iroha_data_model::transaction::SignedTransaction =
        norito::decode_from_bytes(&frame).expect("canonical signed repair transaction");
    let state = std::sync::Arc::clone(fixture.chain.state());
    let view = state.view();
    assert!(!crate::runtime::sorafs_repair::queue_admits(&view, &source));
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(fixture.chain.height() + 1).unwrap(),
        view.latest_block_hash(),
        None,
        time,
        0,
    );
    drop(view);
    let mut block = state.block(header);
    let mut tx =
        block.transaction_for_fastpq_testing(iroha_crypto::Hash::from(source.hash_as_entrypoint()));
    tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let expected = crate::runtime::envelope::admit_envelopes(
        tx.world.runtime_selection(),
        tx.block_height(),
        source.instructions(),
    )
    .expect_err("the predecessor has not selected the repair wire");
    let error = Executor::Initial
        .execute_transaction(&mut tx, &account(6), &source, &mut IvmCache::new())
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect_err("a signed synthetic-network repair has no production Taira authorization");
    assert_eq!(
        error, expected,
        "failed exceptional qualification keeps the predecessor rejection"
    );
    assert_eq!(tx.sorafs_repair_entrypoint, None);
    drop(tx);
    drop(block);
    assert_eq!(fixture.world_root(), before);
    fixture.assert_uninitialized();
    // This negative integration check does not qualify a successful Taira repair block;
    // that requires the actual signed transaction over a preserved copy of its live state.
}

#[test]
fn bootstrap_grants_exactly_the_signed_permissions_without_implicit_owner_authority() {
    use iroha_executor_data_model::permission::sorafs::CanOperateSorafsRepair;
    use std::collections::BTreeMap;

    let mut fixture = Fixture::new();
    let payload = fixture.payload();
    let mut expected = {
        let view = fixture.chain.state().view();
        view.world()
            .account_permissions()
            .iter()
            .map(|(account, permissions)| (account.clone(), permissions.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    for grant in &payload.grants {
        expected
            .entry(grant.account.clone())
            .or_default()
            .insert(grant.permission.clone());
    }
    let certificate = fixture.certificate(&payload);
    assert_eq!(fixture.submit(certificate), None);
    let view = fixture.chain.state().view();
    let actual = view
        .world()
        .account_permissions()
        .iter()
        .map(|(account, permissions)| (account.clone(), permissions.clone()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual, expected,
        "no account receives an unsigned permission"
    );
    for index in 0..3 {
        let provider_id = ProviderId::new(fixture.envelope_at(index).proposal.provider_id);
        assert_eq!(
            view.world().provider_owners().get(&provider_id),
            Some(&account(1))
        );
        assert!(
            !view.world().account_contains_inherent_permission(
                &account(1),
                &CanOperateSorafsRepair { provider_id }.into()
            ),
            "establishing the owner must not implicitly authorize its repair worker"
        );
    }
    assert_eq!(
        view.world()
            .role(&compliance_role())
            .expect("explicit compliance role")
            .permissions()
            .len(),
        0
    );
}

#[test]
fn bootstrap_rejects_existing_musubi_reverse_reference_without_changing_world() {
    use iroha_data_model::musubi::{
        ArchiveId, MusubiArchiveLocationIdV1, MusubiArchiveLocationKeyV1,
    };

    let mut fixture = Fixture::new();
    let provider = fixture.provider();
    let location = MusubiArchiveLocationKeyV1::new(
        ArchiveId::new([0xA1; 32]),
        MusubiArchiveLocationIdV1::new([0xA2; 32]),
    );
    let reverse = MusubiProviderLocationKeyV1::new(provider, location);
    let time = fixture.next_time_ms();
    // Retain an existing reverse reference as an adversarial initial-state fixture. The repair
    // must fail closed before interpreting or refreshing any foreign Musubi location state.
    fixture.chain.setup_world_at(time, |tx| {
        tx.world.musubi_locations_by_provider.insert(reverse, ());
    });
    fixture.chain.commit_at(time, Vec::new());
    let before = fixture.world_root();
    let certificate = fixture.certificate(&fixture.payload());
    let error = fixture
        .submit(certificate)
        .expect("existing reverse reference blocks a first owner");
    assert!(error.contains("existing Musubi locations"), "{error}");
    assert_eq!(
        fixture.world_root(),
        before,
        "the complete World, selection and permissions roll back"
    );
    fixture.assert_uninitialized();
    let view = fixture.chain.state().view();
    assert_eq!(
        view.world().musubi_locations_by_provider().get(&reverse),
        Some(&())
    );
    assert!(view.world().provider_owners().get(&provider).is_none());
}
