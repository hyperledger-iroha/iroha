//! Selected carrier fee exposure and original-source refusal regressions.

use super::*;
use crate::state::World;
use iroha_data_model::{
    isi::{Log, OpaqueInstructionV1},
    level::Level,
    transaction::TransactionBuilder,
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

fn envelope() -> InstructionBox {
    OpaqueInstructionV1::new("test.carrier.fee.v1", vec![1, 2, 3])
        .unwrap()
        .into()
}

fn signed(executable: Executable, gas: Option<u64>) -> SignedTransaction {
    TransactionBuilder::new(
        iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"carrier-fee-test")),
        ),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), gas.and_then(core::num::NonZeroU64::new)),
    )
    .with_executable(executable)
    .sign(ALICE_KEYPAIR.private_key())
}

fn carrier_world() -> World {
    let world = World::new();
    let catalog = crate::runtime::release_catalog().unwrap();
    let mut selection = world.runtime_selection.block();
    selection.get_mut().kernel = catalog.generic_carriers.unwrap().kernel;
    selection.commit();
    world
}

#[test]
fn source_signature_borrows_the_original_intent_and_refuses_substitution() {
    let original = signed(Executable::from(vec![envelope()]), Some(8_000_000));
    verify_source_signature(&original).unwrap();
    let mut forged = original.clone();
    forged.set_signature(iroha_data_model::transaction::TransactionSignature(
        iroha_crypto::SignatureOf::try_new(
            iroha_test_samples::BOB_KEYPAIR.private_key(),
            original.payload(),
        )
        .unwrap(),
    ));
    assert!(matches!(
        verify_source_signature(&forged),
        Err(ExecutionAttemptError::Rejected(_))
    ));
}

#[test]
fn valid_signature_cannot_authorize_retired_fee_metadata() {
    let mut metadata = iroha_model_base::metadata::Metadata::default();
    metadata.insert(
        "gas_limit".parse().unwrap(),
        iroha_primitives::json::Json::new(8_000_000_u64),
    );
    let valid = signed(Executable::from(vec![envelope()]), Some(8_000_000));
    let builder = TransactionBuilder::new(
        valid.network_id().copied().unwrap(),
        ALICE_ID.clone(),
        valid.fee_payment_intent().clone(),
    )
    .with_instructions([envelope()])
    .with_metadata(metadata);
    // The external-signature constructor deliberately leaves admission to its caller.
    let signature =
        iroha_crypto::Signature::new(ALICE_KEYPAIR.private_key(), builder.payload_hash().as_ref());
    let original = builder.build_with_signature(signature);
    let hash = iroha_crypto::HashOf::try_new(original.payload()).unwrap();
    iroha_crypto::verify_signature_borrowed(
        &original.signature().0,
        ALICE_ID.try_signatory().unwrap(),
        hash.as_ref(),
    )
    .unwrap();
    assert!(matches!(
        verify_source_signature(&original),
        Err(ExecutionAttemptError::Rejected(_))
    ));
}

#[test]
fn selected_carrier_quotes_signed_vm_exposure_without_changing_native_metering() {
    let world = carrier_world();
    let gas = crate::gas::DEFAULT_ZK_GAS;
    let carrier = signed(Executable::from(vec![envelope()]), Some(8_000_000));
    let quote = fee_bound(&world.view(), carrier.payload(), &gas).unwrap();
    assert_eq!((quote.1, quote.2), (1, 8_000_000));
    assert_eq!(
        quote.0,
        norito::canonical_frame_len(carrier.payload()).unwrap()
    );

    let native = signed(
        Executable::from(vec![InstructionBox::from(Log::new(
            Level::INFO,
            "native".into(),
        ))]),
        Some(8_000_000),
    );
    assert_eq!(
        fee_bound(&world.view(), native.payload(), &gas).unwrap(),
        fee_bound_for_admission_payload(native.payload(), &gas).unwrap()
    );
    let baseline = World::new();
    assert_eq!(
        fee_bound(&baseline.view(), carrier.payload(), &gas).unwrap(),
        fee_bound_for_admission_payload(carrier.payload(), &gas).unwrap()
    );
}

#[test]
fn selected_carrier_requires_a_signed_limit_while_native_no_limit_is_unchanged() {
    let world = carrier_world();
    let gas = crate::gas::DEFAULT_ZK_GAS;
    let carrier = signed(Executable::from(vec![envelope()]), None);
    assert!(matches!(
        fee_bound(&world.view(), carrier.payload(), &gas),
        Err(ExecutionAttemptError::Rejected(_))
    ));
    let baseline = World::new();
    assert_eq!(
        fee_bound(&baseline.view(), carrier.payload(), &gas).unwrap(),
        fee_bound_for_admission_payload(carrier.payload(), &gas).unwrap()
    );
}

#[test]
fn original_signature_and_carrier_fee_quote_stream_without_payload_allocation() {
    let world = carrier_world();
    let carrier = signed(Executable::from(vec![envelope()]), Some(8_000_000));
    let expected = fee_bound(
        &world.view(),
        carrier.payload(),
        &crate::gas::DEFAULT_ZK_GAS,
    )
    .unwrap();
    // Both paths stream the exact original scalar/envelope payload. A zero
    // allocation allowance must not manufacture a resource refusal. Typed
    // original-refusal conversion is exercised by execution_attempt's adapters.
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    norito::with_decode_limits_scope(limits, || verify_source_signature(&carrier)).unwrap();
    let quote = norito::with_decode_limits_scope(limits, || {
        fee_bound(
            &world.view(),
            carrier.payload(),
            &crate::gas::DEFAULT_ZK_GAS,
        )
    });
    assert_eq!(quote.unwrap(), expected);
    // The nested scope does not change a later original attempt's result.
    verify_source_signature(&carrier).unwrap();
    assert_eq!(
        fee_bound(
            &world.view(),
            carrier.payload(),
            &crate::gas::DEFAULT_ZK_GAS
        )
        .unwrap()
        .2,
        8_000_000
    );
}

#[test]
fn callback_transaction_cannot_derive_original_carrier_execution() {
    crate::validation_fee::tests::with_validation_fee_payout_block_at_time(
        2,
        2_000,
        |block, authority, _, _| {
            let mut state = block.transaction_for_callback_testing();
            let catalog = crate::runtime::release_catalog().unwrap();
            state.world.runtime_selection.get_mut().kernel =
                catalog.generic_carriers.unwrap().kernel;
            let transaction = signed(Executable::from(vec![envelope()]), Some(8_000_000));
            let result = Executor::execute_generic_carrier_transaction(
                &mut state,
                authority,
                &transaction,
                InstructionFeeContext {
                    tx_bytes_len: 0,
                    gas_limit_md: Some(8_000_000),
                    require_gas_limit: true,
                    gas_asset_opt: None,
                    fee_sponsor: None,
                    skip_nexus_fee: true,
                },
            );
            assert!(matches!(result, Err(ValidationFail::NotPermitted(_))));
            assert_eq!(state.last_tx_gas_used, 0);
            assert!(state.world.runtime_generic_values.iter().next().is_none());
        },
    );
}
