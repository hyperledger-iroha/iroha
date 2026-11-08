//! Resource provenance and actual provider retry tests; proof fixtures are explicit DATA.
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    test_support::{
        BOOT_A, TestOwnerV1, advance_request, bootstrap_capsule, bootstrapped_device, next_capsule,
        prepared_slot, released, wallet_fixture,
    },
    *,
};
use iroha_data_model::kagemusha::{
    KagemushaDeviceSignatureV1, KagemushaWalletCompletionRecordV1, KagemushaWalletRecoveryCapsuleV1,
};

fn tiny() -> norito::DecodeLimits {
    norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 64)
}

#[test]
fn actual_enclosing_refusal_retries_the_same_original_and_completed_invalid_is_distinct() {
    let bytes = norito::encode_canonical(&vec![7_u8; 32]).unwrap();
    let refused =
        norito::with_decode_limits_scope(tiny(), || read_canonical::<Vec<u8>>(&bytes, 1024));
    assert_eq!(
        refused,
        Err(Error::DecodeResource(
            KagemushaWalletDecodeResourceV1::EnclosingLimit
        ))
    );
    assert_eq!(
        read_canonical::<Vec<u8>>(&bytes, 1024).unwrap(),
        vec![7_u8; 32]
    );
    let mut corrupt = bytes.clone();
    corrupt.push(0);
    assert_eq!(
        completed(read_canonical::<Vec<u8>>(&corrupt, 1024)).unwrap(),
        None
    );
    assert!(matches!(completed(refused), Err(Error::DecodeResource(_))));
}

#[test]
fn replayed_retired_codec_error_cannot_claim_fresh_local_resource_provenance() {
    let bytes = norito::encode_canonical(&vec![7_u8; 32]).unwrap();
    let retired = norito::with_decode_limits_scope(tiny(), || {
        norito::core::classify_decode_attempt(|| {
            norito::decode_canonical_with_limits::<Vec<u8>>(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )
        })
    })
    .unwrap_err();
    assert_eq!(
        retired.kind(),
        norito::core::DecodeAttemptErrorKind::EnclosingLimit
    );
    let retired = retired.into_error();
    assert_eq!(
        read_model::<Vec<u8>>(
            || Err(KagemushaWalletValidationErrorV1::Codec(retired)),
            "retired"
        ),
        Err(Error::Invalid { field: "retired" })
    );
    assert_eq!(
        read_canonical::<Vec<u8>>(&bytes, 1024).unwrap(),
        vec![7_u8; 32]
    );
}

struct RefusingOwner;
impl
    KagemushaWalletTransitionOwnerV1<
        KagemushaWalletRecoveryCapsuleV1,
        KagemushaWalletCompletionRecordV1,
    > for RefusingOwner
{
    fn receipt_body(
        &self,
        capsule: &KagemushaWalletRecoveryCapsuleV1,
        digest: &[u8; 32],
    ) -> Result<Vec<u8>, Error> {
        let bytes = norito::encode_canonical(&vec![7_u8; 32]).unwrap();
        norito::with_decode_limits_scope(tiny(), || read_canonical::<Vec<u8>>(&bytes, 1024))?;
        TestOwnerV1.receipt_body(capsule, digest)
    }
    fn require_completion(
        &self,
        capsule: &KagemushaWalletRecoveryCapsuleV1,
        record: &KagemushaWalletCompletionRecordV1,
    ) -> Result<(), Error> {
        TestOwnerV1.require_completion(capsule, record)
    }
    fn assemble(
        &self,
        capsule: &KagemushaWalletRecoveryCapsuleV1,
        digest: &[u8; 32],
        signature: &KagemushaDeviceSignatureV1,
    ) -> Result<KagemushaWalletCompletionRecordV1, Error> {
        TestOwnerV1.assemble(capsule, digest, signature)
    }
}

#[test]
fn pre_admission_resource_refusal_does_not_sign_or_select_and_exact_request_retries() {
    let (device, fixture, slot, bootstrap) =
        bootstrapped_device(KagemushaWalletAnchorPolicyV1::NotRequired, 0x75);
    let mut provider = device.open();
    let before = provider.status(&slot).unwrap().marker().unwrap().clone();
    let next = next_capsule(&fixture, &bootstrap);
    let request = advance_request(&before, &next);
    let signs = device.platform.with(|state| state.sign_calls);
    let originals = device
        .fs
        .visible_names(&kagemusha_wallet_capsules_dir_v1(&slot));
    assert!(matches!(
        provider.advance(&slot, &RefusingOwner, &request),
        Err(Error::DecodeResource(_))
    ));
    assert_eq!(provider.status(&slot).unwrap().marker(), Some(&before));
    assert_eq!(device.platform.with(|state| state.sign_calls), signs);
    assert_eq!(
        device
            .fs
            .visible_names(&kagemusha_wallet_capsules_dir_v1(&slot)),
        originals
    );
    let first = released(provider.advance(&slot, &TestOwnerV1, &request).unwrap());
    let retry = released(provider.advance(&slot, &TestOwnerV1, &request).unwrap());
    assert_eq!(first.frame, retry.frame);
    assert_eq!(first.completion_digest, retry.completion_digest);
    assert_eq!(device.platform.with(|state| state.sign_calls), signs + 1);
}

#[test]
fn unfinished_redundant_pair_read_never_marks_or_repairs_originals_as_corrupt() {
    let fixture = wallet_fixture(0x76);
    let (fs, store) = prepared_slot(&fixture);
    let KagemushaWalletMarkerPublicationV1::Durable(enrollment) =
        kagemusha_wallet_publish_marker_v1(&store, fixture.enrollment_record(BOOT_A)).unwrap()
    else {
        panic!("durable enrollment");
    };
    let capsule = bootstrap_capsule(&fixture);
    let digest = capsule.custody_digest();
    kagemusha_wallet_stage_capsule_v1(
        &store,
        &enrollment,
        &Ok(BOOT_A),
        &capsule,
        &fixture.scheme_id(),
    )
    .unwrap();
    let dir = kagemusha_wallet_capsules_dir_v1(&fixture.slot);
    let originals: Vec<_> = fs
        .visible_names(&dir)
        .into_iter()
        .map(|name| {
            let bytes = fs.visible_file(&dir, &name).unwrap();
            (name, bytes)
        })
        .collect();
    let refused = norito::with_decode_limits_scope(tiny(), || {
        kagemusha_wallet_load_capsule_v1::<_, KagemushaWalletRecoveryCapsuleV1>(
            &store,
            &fixture.slot,
            1,
            &digest,
            &fixture.scheme_id(),
        )
    });
    assert!(matches!(refused, Err(Error::DecodeResource(_))));
    for (name, bytes) in originals {
        assert_eq!(fs.visible_file(&dir, &name).unwrap(), bytes);
    }
    let loaded = kagemusha_wallet_load_capsule_v1::<_, KagemushaWalletRecoveryCapsuleV1>(
        &store,
        &fixture.slot,
        1,
        &digest,
        &fixture.scheme_id(),
    )
    .unwrap();
    assert_eq!(loaded.frame(), capsule.to_canonical_bytes());
}
