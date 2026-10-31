//! Genuine finality QCs over explicit synthetic outcomes; no World execution claim.

use super::*;

#[test]
fn verified_abort_requires_successful_original_execution_at_the_final_tip() {
    for rejected in [false, true] {
        let mut native = NativeFinalityFixture::start("private-settlement-sdk-abort");
        let mut client = client_with_base_url(base_url());
        client.network_id = native.network_id();
        let (request, certificates) = prepared(&client);
        let manifest = request.manifest_with(&certificates).unwrap();
        let abort = AbortAtomicPrivateSettlementV1::new(
            manifest.clone(),
            PrivateSettlementAbortReasonV1::ParticipantRejected,
        );
        let (block, evidence) = certified_instructions(
            &mut native,
            &client,
            manifest.public_fee_intent.clone(),
            vec![abort.into()],
            rejected,
        );
        let result =
            VerifiedPrivateSettlementAbort::verify_recovered(&block, &manifest, evidence.clone());
        if rejected {
            assert!(matches!(
                result,
                Err(PrivateSettlementReceiptError::Rejected)
            ));
            continue;
        }
        let verified = result.unwrap();
        assert_eq!(verified.manifest(), &manifest);
        assert_eq!(
            verified.receipt().manifest_digest,
            manifest.manifest_digest().unwrap()
        );
        assert_eq!(verified.receipt().bundle_id, manifest.bundle_id);
        assert_eq!(verified.receipt().network_id, manifest.network_id);
        assert_eq!(verified.receipt().finalized_height, block.height());
        assert_eq!(verified.context_id(), block.context_id());
        assert_eq!(verified.evidence(), &evidence);
        assert!(verified.nexus_fee_receipt().is_none());
        VerifiedPrivateSettlementAbort::verify(
            &block,
            &manifest,
            verified.transaction_hash(),
            evidence.clone(),
        )
        .unwrap();
        let wrong = HashOf::from_untyped_unchecked(Hash::new(b"different Abort carrier"));
        assert!(matches!(
            VerifiedPrivateSettlementAbort::verify(&block, &manifest, wrong, evidence.clone()),
            Err(PrivateSettlementReceiptError::BindingMismatch)
        ));
        let mut substituted = manifest.clone();
        substituted.legs[0].availability_certificate_digest =
            Hash::new(b"substituted availability");
        assert!(
            VerifiedPrivateSettlementAbort::verify_recovered(
                &block,
                &substituted,
                evidence.clone(),
            )
            .is_err()
        );
        let mut substituted = evidence;
        substituted.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"foreign Abort block"));
        assert!(
            VerifiedPrivateSettlementAbort::verify_recovered(&block, &manifest, substituted,)
                .is_err()
        );
    }
}

#[test]
fn verified_abort_rejects_composite_success_and_a_finalize_receipt() {
    for composite in [false, true] {
        let mut native = NativeFinalityFixture::start("private-settlement-sdk-abort-shape");
        let mut client = client_with_base_url(base_url());
        client.network_id = native.network_id();
        let (request, certificates) = prepared(&client);
        let (barrier, bundle) = bundle(&request, &checkpoint(&request, &certificates));
        let instructions = if composite {
            let abort = AbortAtomicPrivateSettlementV1::new(
                barrier.manifest.clone(),
                PrivateSettlementAbortReasonV1::ParticipantRejected,
            );
            vec![abort.clone().into(), abort.into()]
        } else {
            vec![FinalizeAtomicPrivateSettlementV1::new(bundle).into()]
        };
        let (block, evidence) = certified_instructions(
            &mut native,
            &client,
            barrier.manifest.public_fee_intent.clone(),
            instructions,
            false,
        );
        assert!(matches!(
            VerifiedPrivateSettlementAbort::verify_recovered(&block, &barrier.manifest, evidence),
            Err(PrivateSettlementReceiptError::BindingMismatch)
        ));
    }
}
