//! Rejection-tail sizing uses the frozen sponsor and canonical committed receiver.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::{
    Registrable,
    account::{
        AccountAddress,
        rekey::{AccountAlias, AccountAliasDomain, AccountRekeyRecord},
    },
    isi::Log,
    nexus::DataSpaceCatalog,
    sns::{NameControllerV1, NameRecordV1},
    transaction::TransactionBuilder,
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
use nonzero_ext::nonzero;

fn fixture() -> (State, FeeSponsorProgramId, AssetDefinitionId) {
    let domain = DomainId::try_new("fees", "universal").unwrap();
    let mut world = World::with(
        [Domain::new(domain.clone()).build(&ALICE_ID)],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&BOB_ID),
        ],
        [],
    );
    let alias = AccountAlias::new(
        "collector".parse().unwrap(),
        Some(AccountAliasDomain::new(domain.name().clone())),
        DataSpaceId::UNIVERSAL,
    );
    let selector =
        crate::sns::selector_for_account_alias(&alias, &DataSpaceCatalog::default()).unwrap();
    let address = AccountAddress::from_account_id(&ALICE_ID).unwrap();
    let lease = NameRecordV1::new(
        selector.clone(),
        ALICE_ID.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        100,
        200,
        300,
        iroha_model_base::metadata::Metadata::default(),
    );
    world.smart_contract_state_mut_for_testing().insert(
        crate::sns::record_storage_key(&selector),
        norito::codec::Encode::encode(&lease),
    );
    world
        .account_aliases
        .insert(alias.clone(), ALICE_ID.clone());
    world
        .replace_account_rekey_record_for_testing(AccountRekeyRecord::new(alias, ALICE_ID.clone()));
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let program = FeeSponsorProgramId::new(ALICE_ID.clone(), "tail-gas".parse().unwrap());
    let asset = AssetDefinitionId::derive_from_components(domain, "gas".parse().unwrap());
    (state, program, asset)
}

fn source(state: &State, program: FeeSponsorProgramId) -> SignedTransaction {
    TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::sponsor(program, 1, Vec::new(), None),
    )
    .with_instructions([Log::new(
        iroha_logger::Level::INFO,
        "rejection tail".to_owned(),
    )])
    .sign(ALICE_KEYPAIR.private_key())
}

fn quote(program: FeeSponsorProgramId, asset: AssetDefinitionId) -> FeeAdmissionQuote {
    FeeAdmissionQuote {
        charges: vec![FeeChargeBound {
            kind: FeeChargeKind::PipelineGas,
            asset_definition_id: asset,
            max_bound: Quantity::one(),
        }],
        debit_source: FeeDebitSource::SponsorProgram(program),
        program_revision: Some(1),
        relay_leases: BTreeMap::new(),
        capacities: BTreeMap::new(),
        authority_balances: BTreeMap::new(),
        authority_charge_assets: BTreeMap::new(),
    }
}

#[test]
fn sponsored_tail_uses_canonical_receiver_without_creating_transfer_work() {
    let (state, program, asset) = fixture();
    let signed = source(&state, program.clone());
    let quote = quote(program, asset);
    let hash = iroha_crypto::Hash::from(signed.hash_as_entrypoint());
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 50, 0));
    let mut tx = block.transaction_for_fastpq_testing(hash);
    tx.pipeline.gas.tech_account_id = AccountAddress::from_account_id(&ALICE_ID)
        .unwrap()
        .to_i105_for_discriminant(427)
        .unwrap();
    tx.nexus.fees.sponsor_vault_custody_account_id = BOB_ID.clone();
    assert_eq!(
        crate::pipeline_gas_policy::receiver_id(&tx.pipeline.gas.tech_account_id).unwrap(),
        *ALICE_ID
    );
    let before = tx.fastpq_rejection_tail_context(hash).unwrap();
    preflight(&mut tx, &ALICE_ID, &signed, Some(&quote), None).unwrap();
    assert_eq!(tx.fastpq_rejection_tail_context(hash).unwrap(), before);
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
    assert_eq!(tx.last_tx_gas_used, 0);
}

#[test]
fn an_alias_cannot_replace_the_committed_fee_receiver_before_work() {
    let (state, program, asset) = fixture();
    let signed = source(&state, program.clone());
    let quote = quote(program, asset);
    let hash = iroha_crypto::Hash::from(signed.hash_as_entrypoint());
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 50, 0));
    let mut tx = block.transaction_for_fastpq_testing(hash);
    tx.pipeline.gas.tech_account_id = "collector@fees.universal".to_owned();
    tx.nexus.fees.sponsor_vault_custody_account_id = BOB_ID.clone();
    assert_eq!(
        parse_account_id_literal(
            &tx.world,
            &tx.nexus.dataspace_catalog,
            &tx.pipeline.gas.tech_account_id,
            50
        )
        .unwrap(),
        Some(ALICE_ID.clone())
    );
    assert!(matches!(
        preflight(&mut tx, &ALICE_ID, &signed, Some(&quote), None),
        Err(ValidationFail::InternalError(_))
    ));
    assert!(tx.fastpq_rejection_tail_context(hash).is_err());
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
    assert_eq!(tx.last_tx_gas_used, 0);
}

#[test]
fn foreign_sponsor_quote_cannot_supply_a_tail_owner() {
    let (state, program, asset) = fixture();
    let signed = source(&state, program);
    let foreign = FeeSponsorProgramId::new(BOB_ID.clone(), "foreign-tail".parse().unwrap());
    let quote = quote(foreign, asset);
    let hash = iroha_crypto::Hash::from(signed.hash_as_entrypoint());
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 50, 0));
    let mut tx = block.transaction_for_fastpq_testing(hash);
    tx.pipeline.gas.tech_account_id = ALICE_ID.to_string();
    assert!(matches!(
        preflight(&mut tx, &ALICE_ID, &signed, Some(&quote), None),
        Err(ValidationFail::InternalError(_))
    ));
    assert!(tx.fastpq_rejection_tail_context(hash).is_err());
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
    assert_eq!(tx.last_tx_gas_used, 0);
}

/// The exact same signed sponsor source must size the predecessor account, not a later recipe.
#[test]
fn sponsored_tail_sizes_original_custody_across_local_and_pending_substitution() {
    use iroha_data_model::parameter::{
        BlockParameter, FastpqMandatorySourcePolicyV1, FastpqSourcePolicyV1, Parameter,
    };

    let original = super::tests::mldsa_tail_account(1, 17);
    let pending = super::tests::mldsa_tail_account(2, 23);
    let local = BOB_ID.clone();
    assert_ne!(original, pending);
    assert_ne!(original, local);
    assert_ne!(pending, local);
    for short in [false, true] {
        let (state, program, asset) = fixture();
        let signed = source(&state, program.clone());
        let quote = quote(program, asset.clone());
        let hash = iroha_crypto::Hash::from(signed.hash_as_entrypoint());
        let sizes = [&local, &original, &pending].map(|payer| {
            measure_tail(
                hash,
                &[TransferIdentities {
                    from: payer,
                    to: &ALICE_ID,
                    asset: &asset,
                }],
                FastpqSourcePolicyV1::bootstrap().intrinsic,
            )
            .expect("public identity shape fits the measured corpus")
        });
        assert!(sizes[0].input_transcript_bytes < sizes[1].input_transcript_bytes);
        assert!(sizes[1].input_transcript_bytes < sizes[2].input_transcript_bytes);
        let mut intrinsic = FastpqSourcePolicyV1::bootstrap().intrinsic;
        intrinsic.max_transcripts = 1;
        intrinsic.max_deltas = 1;
        intrinsic.max_input_transcript_bytes =
            u64::try_from(sizes[1].input_transcript_bytes).unwrap() - u64::from(short);
        intrinsic.max_statement_bytes = u64::try_from(sizes[1].max_statement_bytes).unwrap();
        intrinsic.max_total_statement_bytes = intrinsic.max_statement_bytes;
        {
            let mut parameters = crate::state::parameter_cell_block_for_testing(&state.world)
                .expect("funded original policy fixture");
            let mut fees = parameters.get().root_fees().unwrap().clone();
            fees.sponsor_vault_custody_account_id = original.clone();
            crate::state::set_parameter_cell_for_testing(
                &mut parameters,
                Parameter::Custom(fees.into_custom_parameter().unwrap()),
                state.world.sccp_committee_generations.allocation_budget(),
            )
            .unwrap();
            let source_policy = FastpqSourcePolicyV1::from_sizing(
                parameters.get().block().execution_output(),
                intrinsic,
                FastpqMandatorySourcePolicyV1 {
                    max_retained_obligations: 1,
                    per_obligation: intrinsic,
                },
                1,
            )
            .expect("valid exact component source envelope");
            crate::state::set_parameter_cell_for_testing(
                &mut parameters,
                Parameter::Block(BlockParameter::FastpqSource(source_policy)),
                state.world.sccp_committee_generations.allocation_budget(),
            )
            .unwrap();
            parameters.commit();
        }
        // Height two deliberately selects ordinary execution custody, even in this
        // component-only source fixture. No authenticated Network publication is claimed.
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 50, 0));
        let mut tx = block.transaction_for_fastpq_testing(hash);
        tx.pipeline.gas.tech_account_id = AccountAddress::from_account_id(&ALICE_ID)
            .unwrap()
            .to_i105_for_discriminant(427)
            .unwrap();
        assert_eq!(
            crate::pipeline_gas_policy::receiver_id(&tx.pipeline.gas.tech_account_id).unwrap(),
            *ALICE_ID
        );
        tx.nexus.fees.sponsor_vault_custody_account_id = local.clone();
        let mut current = tx.root_fee_policy().unwrap().clone();
        current.sponsor_vault_custody_account_id = pending.clone();
        // Direct test injection proves a pending projection cannot replace original custody;
        // ordinary policy admission still rejects changing sponsor custody after genesis.
        tx.world
            .set_parameter_for_testing(Parameter::Custom(current.into_custom_parameter().unwrap()))
            .unwrap();
        assert_eq!(
            tx.world
                .parameters()
                .root_fees()
                .unwrap()
                .sponsor_vault_custody_account_id,
            pending
        );
        assert_eq!(
            crate::executor::root_fees::sponsor_vault_custody(&tx).unwrap(),
            &original
        );
        let before = tx.fastpq_rejection_tail_context(hash).unwrap();
        assert_eq!(before, intrinsic);
        let result = preflight(&mut tx, &ALICE_ID, &signed, Some(&quote), None);
        if short {
            assert!(
                matches!(result, Err(ValidationFail::NotPermitted(ref reason))
                    if reason == SOURCE_INTRINSIC_REJECTION),
                "one byte below the original account must refuse before work: {result:?}"
            );
        } else {
            result.expect("the exact original bound fits despite a larger pending account");
        }
        assert_eq!(tx.fastpq_rejection_tail_context(hash).unwrap(), before);
        assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
        assert_eq!(tx.last_tx_gas_used, 0);
    }
}
