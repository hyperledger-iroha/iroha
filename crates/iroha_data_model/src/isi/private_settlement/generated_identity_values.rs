//! Typed private-settlement instruction values for generated-record identity capture.

use norito::json::Value;

use super::super::generated_record_identity_tests::capture;
use super::{
    AbortAtomicPrivateSettlementV1, ActivatePrivateSettlementPoolV1,
    FinalizeAtomicPrivateSettlementV1, RegisterAtomicPrivateSettlementPrepareV1,
    RotatePrivateSettlementPoolPolicyV1,
};
use crate::nexus::{
    PrivateSettlementAbortReasonV1, PrivateSettlementCommitBundleV1,
    PrivateSettlementPrepareBarrierV1, measured_private_settlement_receipt,
};

/// Build the missing private-settlement generated-record rows from validated fixtures.
pub(crate) fn values() -> Vec<Value> {
    let activation = super::tests::pool_activation();
    activation
        .validate()
        .expect("canonical private-settlement activation fixture");
    let rotation = super::tests::pool_rotation();
    rotation
        .validate()
        .expect("canonical private-settlement rotation fixture");

    let receipt = measured_private_settlement_receipt(2);
    receipt
        .validate_shape()
        .expect("canonical private-settlement receipt fixture");
    let mut barrier = PrivateSettlementPrepareBarrierV1 {
        version: receipt.version,
        manifest: receipt.manifest.clone(),
        authority_catalog: receipt.authority_catalog.clone(),
        deltas: receipt.legs.iter().map(|leg| leg.delta.clone()).collect(),
        prepare_certificates: receipt.legs.iter().map(|leg| leg.prepare.clone()).collect(),
        prepared_bundle_digest: iroha_crypto::Hash::new(b"private-settlement-capture-pending"),
    };
    barrier.prepared_bundle_digest = barrier
        .computed_prepared_bundle_digest()
        .expect("private-settlement Prepare barrier digest");
    barrier
        .validate_shape()
        .expect("canonical private-settlement Prepare barrier fixture");
    let commit_bundle = PrivateSettlementCommitBundleV1 {
        version: receipt.version,
        manifest: receipt.manifest.clone(),
        authority_catalog: receipt.authority_catalog.clone(),
        legs: receipt.legs.clone(),
    };
    commit_bundle
        .clone()
        .into_receipt(receipt.finalized_height)
        .validate_shape()
        .expect("canonical private-settlement commit-bundle fixture");
    let abort = AbortAtomicPrivateSettlementV1::new(
        receipt.manifest,
        PrivateSettlementAbortReasonV1::Expired,
    );

    vec![
        capture::<ActivatePrivateSettlementPoolV1>(activation),
        capture::<RotatePrivateSettlementPoolPolicyV1>(rotation),
        capture::<RegisterAtomicPrivateSettlementPrepareV1>(
            RegisterAtomicPrivateSettlementPrepareV1::new(barrier),
        ),
        capture::<AbortAtomicPrivateSettlementV1>(abort),
        capture::<FinalizeAtomicPrivateSettlementV1>(FinalizeAtomicPrivateSettlementV1::new(
            commit_bundle,
        )),
    ]
}
