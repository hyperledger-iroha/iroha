//! Synthetic native State rows under genuine certified fixture blocks.
use super::*;
use iroha_data_model::{
    runtime::definition::{
        DefinitionId, ProjectionV1, SchemaV1, StorageDescriptorV1, StorageShapeV1,
    },
    sumeragi_finality::{
        KeyedStateRowV1, keyed_state::KeyedStateTreeV1, test_fixtures::NativeFinalityFixture,
    },
};

pub(crate) fn selection() -> (RootSelectionV1, Vec<DefinitionDescriptorV1>) {
    let mut definitions: Vec<_> = [
        (
            LEDGER_TABLE,
            StorageShapeV1::Table {
                key: SchemaV1::Codec(codec::<KagemushaWalletLedgerKeyV1>()),
            },
            codec::<Vec<u8>>(),
        ),
        (
            SELECTION_CELL,
            StorageShapeV1::Cell,
            codec::<RootSelectionV1>(),
        ),
    ]
    .into_iter()
    .map(|(namespace, shape, value)| {
        DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
            namespace: namespace.into(),
            shape,
            value: SchemaV1::Codec(value),
            initial: None,
            invariants: [1; 32],
            projection: ProjectionV1::BareNorito,
        })
    })
    .collect();
    definitions.sort_by_key(|definition| definition.id().unwrap());
    (
        RootSelectionV1 {
            kernel: DefinitionId([1; 32]),
            commitment_profile: iroha_data_model::runtime::keyed_state::profile_id(),
            services_profile: DefinitionId([2; 32]),
            installed_storage: definitions.iter().map(|d| d.id().unwrap()).collect(),
            active: vec![],
            bindings: vec![],
            restrictions: vec![],
            completion_closures: vec![],
        },
        definitions,
    )
}

pub(crate) fn historical_selection() -> (RootSelectionV1, Vec<DefinitionDescriptorV1>) {
    let (mut selection, definitions) = selection();
    selection.commitment_profile = iroha_data_model::sumeragi_finality::lthash16_world_profile_v1();
    (selection, definitions)
}

pub(crate) fn payout() -> KagemushaWalletPayoutRecordV1 {
    KagemushaWalletPayoutRecordV1 {
        key: KagemushaWalletPayoutKeyV1::Fee([2; 32]),
        source: [3; 32],
        amount: 17,
        transaction: [5; 32],
    }
}

pub(crate) fn keyed_cut(
    native: &mut NativeFinalityFixture,
    scheme: [u8; 32],
    present: bool,
) -> (
    iroha_data_model::sumeragi_finality::SumeragiFinalityProof,
    KeyedStateRowsV1,
    Vec<u8>,
) {
    let bundle = archive::encode(&selection()).unwrap();
    let (selection, definitions) = selection_original(&bundle).unwrap();
    let schema = SelectedStateSchemaV1::from_selection(&selection, &definitions).unwrap();
    let keys = fee_payout_state_keys_v1(scheme, [2; 32]);
    let budget = iroha_allocation::AllocationBudget::new(1024 * 1024);
    let mut tree = KeyedStateTreeV1::new(schema.digest(), &budget);
    if present {
        tree.replace(
            &keys[0],
            Some(&norito::codec::encode_adaptive(
                &norito::to_bytes(&payout()).unwrap(),
            )),
        )
        .unwrap();
    }
    tree.replace(&keys[1], Some(&norito::codec::encode_adaptive(&selection)))
        .unwrap();
    let block = native.block_with_submitted_work(native.next_header());
    let proof = native.certify_with_world_root(block, tree.root());
    let block = native.verifier().verify_retained_decision(&proof).unwrap();
    let rows = KeyedStateRowsV1 {
        height: block.height(),
        block_hash: block.header().hash(),
        schema_hash: schema.digest(),
        rows: keys
            .iter()
            .map(|key| KeyedStateRowV1 {
                position: key.clone(),
                value: tree.get(key).map(<[u8]>::to_vec),
                witness: tree.prove(key).unwrap(),
            })
            .collect(),
    };
    (proof, rows, bundle)
}

#[test]
fn keyed_payout_accepts_exact_rows_and_rejects_wrong_block_keys_absence_and_selection() {
    let mut native = NativeFinalityFixture::start("wallet-keyed-payout-rows");
    let (proof, rows, bundle) = keyed_cut(&mut native, [9; 32], true);
    let block = native.verifier().verify_retained_decision(&proof).unwrap();
    let (selection, definitions) = selection_original(&bundle).unwrap();
    let schema = SelectedStateSchemaV1::from_selection(&selection, &definitions).unwrap();
    let verify =
        |rows: &KeyedStateRowsV1, block: &VerifiedSumeragiBlock, selection: &RootSelectionV1| {
            FinalizedPayoutStateV1::Keyed {
                rows,
                schema: &schema,
                selection,
            }
            .verify(block, [9; 32], [2; 32], &payout())
        };
    verify(&rows, &block, &selection).unwrap();
    let mut wrong_hash = rows.clone();
    wrong_hash.block_hash = iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
        b"wrong payout block",
    ));
    assert!(verify(&wrong_hash, &block, &selection).is_err());
    let (other, absent, _) = keyed_cut(&mut native, [9; 32], false);
    let other = native.verifier().verify_retained_decision(&other).unwrap();
    assert!(verify(&rows, &other, &selection).is_err());
    // Proven absence is valid lookup evidence, but cannot acknowledge a payout.
    absent
        .verify(&other, &schema, &fee_payout_state_keys_v1([9; 32], [2; 32]))
        .unwrap();
    assert!(matches!(
        verify(&absent, &other, &selection),
        Err(Error::Invalid("finalized fee payout row"))
    ));
    for mode in 0..4 {
        let mut changed = rows.clone();
        match mode {
            0 => {
                changed.rows.pop();
            }
            1 => changed.rows.swap(0, 1),
            2 => changed.rows[0].position.key.push(0),
            _ => changed.rows[1].value = None,
        }
        assert!(verify(&changed, &block, &selection).is_err());
    }
    let mut changed = selection.clone();
    changed.services_profile = DefinitionId([8; 32]);
    assert!(matches!(
        verify(&rows, &block, &changed),
        Err(Error::Invalid("payout certified selection"))
    ));
    changed.commitment_profile = iroha_data_model::sumeragi_finality::lthash16_world_profile_v1();
    assert!(verify(&rows, &block, &changed).is_err());
}

#[test]
fn selection_bundle_is_bounded_canonical_and_requires_each_exact_definition_once() {
    let (selection, definitions) = selection();
    selection_original(&archive::encode(&(selection.clone(), definitions.clone())).unwrap())
        .unwrap();
    assert!(selection_original(&[]).is_err());
    assert!(selection_original(&vec![0; PAYOUT_SELECTION_MAX_BYTES_V1 + 1]).is_err());
    let mut bytes = archive::encode(&(selection.clone(), definitions.clone())).unwrap();
    bytes.push(0);
    assert!(selection_original(&bytes).is_err());
    for definitions in [vec![], vec![definitions[0].clone(), definitions[0].clone()]] {
        assert!(
            selection_original(&archive::encode(&(selection.clone(), definitions)).unwrap())
                .is_err()
        );
    }
}
