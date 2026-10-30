//! Complete original-authority cleanup envelope measurements, not proof qualification.

use super::*;
use crate::{
    isi::private_settlement::{
        AbortAtomicPrivateSettlementV1, RegisterAtomicPrivateSettlementPrepareV1,
    },
    privacy::{
        PRIVATE_SETTLEMENT_MAX_CARRIER_SIGNERS_V1,
        PRIVATE_SETTLEMENT_MAX_CLEANUP_TRANSACTION_BYTES_V1,
        validate_private_settlement_sponsor_v1,
    },
    transaction::{FeeChargeKind, FeeChargeLimit, SignedTransaction, TransactionBuilder},
};
use std::{
    num::{NonZeroU32, NonZeroU64},
    time::Duration,
};

fn original_barrier(sponsor: AccountId, legs: usize) -> PrivateSettlementPrepareBarrierV1 {
    // Reuse the existing maximum-shape fixture. Its proof/certificate bytes are synthetic;
    // this test measures transport and genuinely signs the complete sponsor transactions.
    let mut receipt = measured_receipt(legs);
    receipt.manifest.sponsor = sponsor;
    let asset = crate::asset::AssetDefinitionId::from_uuid_bytes([
        0x51, 0x51, 0x51, 0x51, 0x51, 0x51, 0x41, 0x51, 0x81, 0x51, 0x51, 0x51, 0x51, 0x51, 0x51,
        0x51,
    ])
    .unwrap();
    let largest_amount: iroha_primitives::numeric::Quantity = concat!(
        "670390396497129854978701249910292306373968291029619668886178072186088201503677",
        "3488400937149083451713845015929093243025426876941405973284973216824503042047",
    )
    .parse()
    .unwrap();
    receipt.manifest.public_fee_intent = FeePaymentIntent::authority(
        vec![
            FeeChargeLimit::new(FeeChargeKind::Nexus, asset.clone(), largest_amount.clone()),
            FeeChargeLimit::new(FeeChargeKind::PipelineGas, asset, largest_amount),
        ],
        Some(NonZeroU64::MAX),
    );
    receipt.manifest.fee_intent_digest = receipt.manifest.computed_fee_intent_digest().unwrap();
    receipt.manifest.bundle_id = receipt.manifest.computed_bundle_id().unwrap();
    for (commitment, leg) in receipt.manifest.legs.iter_mut().zip(&mut receipt.legs) {
        leg.delta.bundle_id = receipt.manifest.bundle_id;
        commitment.delta_digest = leg.delta.digest().unwrap();
    }
    let digest = receipt.manifest.manifest_digest().unwrap();
    for leg in &mut receipt.legs {
        leg.prepare.body.bundle_id = receipt.manifest.bundle_id;
        leg.prepare.body.manifest_digest = digest;
        leg.prepare.body.delta_digest = leg.delta.digest().unwrap();
    }
    let mut barrier = PrivateSettlementPrepareBarrierV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        manifest: receipt.manifest,
        authority_catalog: receipt.authority_catalog,
        deltas: receipt.legs.iter().map(|leg| leg.delta.clone()).collect(),
        prepare_certificates: receipt.legs.iter().map(|leg| leg.prepare.clone()).collect(),
        prepared_bundle_digest: hash(1),
    };
    barrier.prepared_bundle_digest = barrier.computed_prepared_bundle_digest().unwrap();
    barrier.validate_shape().unwrap();
    barrier
}

fn builder(barrier: &PrivateSettlementPrepareBarrierV1, abort: bool) -> TransactionBuilder {
    let instruction: crate::isi::InstructionBox = if abort {
        AbortAtomicPrivateSettlementV1::new(
            barrier.manifest.clone(),
            PrivateSettlementAbortReasonV1::Expired,
        )
        .into()
    } else {
        RegisterAtomicPrivateSettlementPrepareV1::new(barrier.clone()).into()
    };
    let mut metadata = iroha_model_base::metadata::Metadata::default();
    for name in ["tx_sequence", "expires_at_height"] {
        metadata.insert(
            name.parse().unwrap(),
            iroha_primitives::json::Json::new(u64::MAX),
        );
    }
    let mut builder = TransactionBuilder::new(
        barrier.manifest.network_id,
        barrier.manifest.sponsor.clone(),
        barrier.manifest.public_fee_intent.clone(),
    )
    .with_metadata(metadata)
    .with_instructions([instruction]);
    builder.set_nonce(NonZeroU32::MAX);
    builder.set_creation_time(Duration::from_millis(u64::MAX - 1_000));
    builder.set_ttl(Duration::from_millis(1_000));
    builder
}

fn bytes(transaction: &SignedTransaction) -> usize {
    SignedTransaction::canonical_frame_len_from_wire_len_v1(transaction.encode_wire_v1().unwrap().len()).unwrap()
}

#[test]
fn original_large_roster_cleanup_fits_the_complete_admitted_carrier_envelope() {
    let count = PRIVATE_SETTLEMENT_MAX_CARRIER_SIGNERS_V1;
    let signers: Vec<_> = (0..count)
        .map(|index| {
            let mut seed = vec![0x4c; 32];
            seed[..8].copy_from_slice(&(index as u64).to_le_bytes());
            KeyPair::from_seed(seed, Algorithm::MlDsa)
        })
        .collect();
    let signing_members: Vec<_> = signers
        .iter()
        .map(|key| MultisigMember::new(key.public_key().clone(), 1).unwrap())
        .collect();
    // Non-signing public members remain part of the exact original controller. The account
    // codec accepts these canonical ML-DSA public encodings; no private keys are invented.
    let (_, template) = signers[0].public_key().to_bytes();
    let extra: Vec<_> = (0_u64..2_048)
        .map(|index| {
            let mut public = template.to_vec();
            public[..32].copy_from_slice(Hash::new(index.to_le_bytes()).as_ref());
            MultisigMember::new(
                iroha_crypto::PublicKey::from_bytes(Algorithm::MlDsa, &public).unwrap(),
                1,
            )
            .unwrap()
        })
        .collect();
    let sponsor = |additional: usize| {
        AccountId::new_multisig(
            MultisigPolicy::new(
                count as u16,
                signing_members
                    .iter()
                    .cloned()
                    .chain(extra[..additional].iter().cloned())
                    .collect(),
            )
            .unwrap(),
        )
    };
    let initial = original_barrier(sponsor(0), ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1);
    let signed = builder(&initial, false)
        .try_sign_multisig(signers.iter().map(KeyPair::private_key))
        .unwrap();
    signed.verify_signature().unwrap();
    let limit = PRIVATE_SETTLEMENT_MAX_CLEANUP_TRANSACTION_BYTES_V1 as usize;
    assert!(bytes(&signed) < limit);

    // Counting reuses fixed-width signature encodings only as a transport probe. It never
    // verifies or submits these substituted messages. The selected originals are signed below.
    let measured = |additional| {
        let barrier = original_barrier(sponsor(additional), ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1);
        let probe = builder(&barrier, false)
            .with_multisig_signatures(signed.multisig_signatures().unwrap().clone())
            .build_with_signature(signed.signature().payload().clone());
        bytes(&probe)
    };
    assert!(measured(extra.len()) > limit);
    let (mut fitting, mut refused) = (0, extra.len());
    while fitting + 1 < refused {
        let middle = (fitting + refused) / 2;
        if measured(middle) <= limit {
            fitting = middle;
        } else {
            refused = middle;
        }
    }
    let barrier = original_barrier(sponsor(fitting), ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1);
    validate_private_settlement_sponsor_v1(&barrier.manifest.sponsor).unwrap();
    assert!(
        fitting > count,
        "the account retains many original non-signing members"
    );
    let register = builder(&barrier, false)
        .try_sign_multisig(signers.iter().map(KeyPair::private_key))
        .unwrap();
    let abort = builder(&barrier, true)
        .try_sign_multisig(signers.iter().map(KeyPair::private_key))
        .unwrap();
    register.verify_signature().unwrap();
    abort.verify_signature().unwrap();
    assert_eq!(register.signature_count(), count);
    assert_eq!(abort.signature_count(), count);
    assert_eq!(register.authority(), abort.authority());
    assert_eq!(register.fee_payment_intent(), abort.fee_payment_intent());
    assert!(bytes(&register) <= limit);
    assert!(measured(fitting + 1) > limit);
    assert!(bytes(&abort) < bytes(&register));
    eprintln!(
        "PRIVATE_CLEANUP_ENVELOPE signers={count} original_members={} legs={} register={} abort={} next_member_register={} ceiling={limit}",
        count + fitting,
        ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1,
        bytes(&register),
        bytes(&abort),
        measured(fitting + 1),
    );
}
