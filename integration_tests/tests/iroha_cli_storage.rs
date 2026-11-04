//! Governed provider prerequisites for CLI control-plane publication tests.
use eyre::Result;
use iroha_crypto::Hash;
use iroha_data_model::{
    isi::sorafs::{
        DecideSorafsReserveMovement, RegisterCapacityDeclaration, RegisterSorafsReserveAccount,
        RequestSorafsReserveMovement, SetProviderIngestCompletionAuthority, SetSorafsReservePolicy,
        UpsertProviderCredit,
    },
    prelude::*,
    sorafs::{
        capacity::ProviderId,
        pin_registry::StorageClass,
        pin_registry::{
            ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
            SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1,
        },
        pricing::ProviderCreditRecord,
        reserve::{
            RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReserveDuration,
            ReserveMovementKindV1, ReservePolicyV1, ReserveProviderTermsV1, ReserveTier,
        },
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCompleteSorafsReplicationOrder, CanDeclareSorafsCapacity, CanSetSorafsReservePolicy,
    CanUpsertSorafsProviderCredit,
};
use iroha_model_base::{domain::DomainId, metadata::Metadata};
use iroha_test_network::{Network, NetworkBuilder};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use sorafs_manifest::{
    capacity::{
        CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, CapacityMetadataEntry,
        ChunkerCommitmentV1,
    },
    provider_advert::StakePointer,
};
use std::time::{SystemTime, UNIX_EPOCH};

const PROVIDERS: [[u8; 32]; 6] = [
    [0xa0; 32], [0xa1; 32], [0xa2; 32], [0xa3; 32], [0xa4; 32], [0xa5; 32],
];
fn reserve_asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("clistorage", "universal").expect("fixture domain"),
        "soracloud_provider_reserve"
            .parse()
            .expect("fixture asset name"),
    )
}
fn provider_storage_class(index: usize) -> (StorageClass, &'static str) {
    if index < 3 {
        (StorageClass::Hot, "hot")
    } else {
        (StorageClass::Warm, "warm")
    }
}

/// Admit three Hot and three Warm provider identities through the signed genesis initializer.
pub(super) fn builder() -> Result<NetworkBuilder> {
    let authority = super::sorafs_publication_authority::PublicationAuthorityFixture::new(
        &PROVIDERS,
        &vec![ALICE_ID.clone(); PROVIDERS.len()],
    )?;
    let mut builder = NetworkBuilder::new().with_genesis_instruction(Grant::account_permission(
        Permission::from(CanDeclareSorafsCapacity),
        ALICE_ID.clone(),
    ));
    builder = builder
        .with_genesis_instruction(Register::domain(Domain::new(DomainId::try_new(
            "clistorage",
            "universal",
        )?)))
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
            reserve_asset(),
            "CLI provider reserve",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("clistorage", "universal")?),
        )))
        .with_genesis_instruction(Mint::asset_quantity(
            100_000_u32,
            AssetId::of(reserve_asset(), ALICE_ID.clone()),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanSetSorafsReservePolicy),
            ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanUpsertSorafsProviderCredit),
            ALICE_ID.clone(),
        ));
    for provider in PROVIDERS {
        builder = builder.with_genesis_instruction(Grant::account_permission(
            Permission::from(CanCompleteSorafsReplicationOrder {
                provider_id: ProviderId::new(provider),
            }),
            BOB_ID.clone(),
        ));
    }
    for instruction in authority.genesis_instructions()? {
        builder = builder.with_genesis_instruction(instruction);
    }
    Ok(builder)
}

fn declarations(now: u64) -> Result<Vec<InstructionBox>> {
    let policy = ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: reserve_asset(),
        custody_account: BOB_ID.clone(),
        treasury_account: ALICE_ID.clone(),
        operations_authority: ALICE_ID.clone(),
        decision_authority: ALICE_ID.clone(),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: "1000".parse()?,
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    };
    let policy_digest = policy.digest()?;
    let mut instructions = vec![SetSorafsReservePolicy::new(policy).into()];
    for (index, provider) in PROVIDERS.into_iter().enumerate() {
        let id = ProviderId::new(provider);
        instructions.push(
            SetProviderIngestCompletionAuthority::new(
                id,
                None,
                ProviderIngestCompletionAuthorityV1::new(
                    ALICE_ID.clone(),
                    BOB_ID.clone(),
                    ProviderIngestCompletionSignerPolicyV1 {
                        policy_id: provider,
                        revision: 1,
                        predecessor_digest: None,
                        policy_digest: *Hash::new(provider).as_ref(),
                    },
                ),
            )
            .into(),
        );
        let (storage_class, storage_label) = provider_storage_class(index);
        instructions.push(
            RegisterSorafsReserveAccount::new(
                ReserveProviderTermsV1 {
                    provider_id: id,
                    provider_account: ALICE_ID.clone(),
                    tier: ReserveTier::TierA,
                    storage_class,
                    duration: ReserveDuration::Monthly,
                    capacity_gib: 128,
                },
                policy_digest,
            )
            .into(),
        );
        let movement =
            *Hash::new([b"cli-reserve-".as_slice(), provider.as_slice()].concat()).as_ref();
        instructions.push(
            RequestSorafsReserveMovement::new(
                movement,
                id,
                ReserveMovementKindV1::TopUp,
                "1000".parse()?,
                1,
                policy_digest,
            )
            .into(),
        );
        instructions.push(
            DecideSorafsReserveMovement::new(
                movement,
                2,
                policy_digest,
                true,
                "owner-funded CLI publication reserve".into(),
            )
            .into(),
        );
        instructions.push(
            UpsertProviderCredit {
                expected_current: None,
                record: ProviderCreditRecord::new(
                    id,
                    Quantity::from(1000_u32),
                    Quantity::from(1000_u32),
                    Quantity::from(1_u32),
                    Quantity::zero(),
                    now,
                    now,
                    Metadata::default(),
                ),
            }
            .into(),
        );
        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: provider,
            stake: StakePointer {
                pool_id: provider,
                stake_amount: "1".parse()?,
            },
            committed_capacity_gib: 128,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 128,
                capability_refs: Vec::new(),
            }],
            lane_commitments: Vec::new(),
            pricing: None,
            valid_from: now.saturating_sub(120),
            // Admission must cover the entire order window measured from the
            // later pin transaction, not merely 24 hours from fixture setup.
            valid_until: now
                .checked_add(2 * u64::from(SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1))
                .ok_or_else(|| eyre::eyre!("fixture epoch overflow"))?,
            metadata: vec![
                CapacityMetadataEntry {
                    key: "sorafs.owner_account_id".into(),
                    value: ALICE_ID.to_string(),
                },
                CapacityMetadataEntry {
                    key: "sorafs.storage_class".into(),
                    value: storage_label.into(),
                },
            ],
        };
        declaration.validate()?;
        instructions
            .push(RegisterCapacityDeclaration::new(norito::encode_canonical(&declaration)?).into());
    }
    Ok(instructions)
}

/// Finalize owner-signed completion authorities and matching capacity before publication.
pub(super) async fn prepare(network: &Network) -> Result<()> {
    let client = network.client();
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let transaction = super::sorafs_network::prepare_transaction(
        &client,
        declarations(now)?,
        Metadata::default(),
    )
    .await?;
    super::sorafs_network::submit_transaction(&client, &transaction).await?;
    Ok(())
}

#[test]
fn provider_declarations_preserve_replication_prerequisites() -> Result<()> {
    let instructions = declarations(1_800_000_000)?;
    assert_eq!(instructions.len(), 37);
    for (index, (pair, provider)) in instructions[1..].chunks_exact(6).zip(PROVIDERS).enumerate() {
        let authority = pair[0]
            .as_any()
            .downcast_ref::<SetProviderIngestCompletionAuthority>()
            .expect("completion authority");
        assert_eq!(authority.provider_id, ProviderId::new(provider));
        assert!(authority.next.is_valid());
        assert_eq!(authority.next.provider_owner, ALICE_ID.clone());
        let declaration = pair[5]
            .as_any()
            .downcast_ref::<RegisterCapacityDeclaration>()
            .expect("capacity declaration");
        let decoded: CapacityDeclarationV1 = norito::decode_canonical(&declaration.declaration)?;
        decoded.validate()?;
        assert_eq!(decoded.provider_id, provider);
        assert_eq!(decoded.committed_capacity_gib, 128);
        assert_eq!(decoded.valid_until, 1_800_172_800);
        assert_eq!(decoded.metadata[1].value, provider_storage_class(index).1);
    }
    assert!(declarations(u64::MAX).is_err());
    Ok(())
}
