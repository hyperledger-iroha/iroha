//! Borrowed Musubi text validation, exact errors, codec stability and heap traffic.
// This isolated test binary observes production validators through GlobalAlloc.
#![allow(unsafe_code)]

use iroha_data_model::musubi::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
};

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static REQUESTS: Cell<usize> = const { Cell::new(0) };
    static REQUESTED_BYTES: Cell<usize> = const { Cell::new(0) };
}

struct TrackingAllocator;
#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn record_request(bytes: usize) {
    let _ = TRACKING.try_with(|tracking| {
        if tracking.get() {
            let _ = REQUESTS.try_with(|requests| requests.set(requests.get() + 1));
            let _ = REQUESTED_BYTES.try_with(|requests| requests.set(requests.get() + bytes));
        }
    });
}

// SAFETY: every operation forwards the original pointer and layout to System.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_request(layout.size());
        // SAFETY: the caller's allocation contract is passed through unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_request(layout.size());
        // SAFETY: the caller's allocation contract is passed through unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_request(size);
        // SAFETY: the live System allocation and requested size are forwarded.
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout are from the matching System allocation.
        unsafe { System.dealloc(pointer, layout) }
    }
}

fn measured<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    struct StopTracking;
    impl Drop for StopTracking {
        fn drop(&mut self) {
            TRACKING.with(|tracking| tracking.set(false));
        }
    }
    REQUESTS.with(|requests| requests.set(0));
    REQUESTED_BYTES.with(|requests| requests.set(0));
    TRACKING.with(|tracking| assert!(!tracking.replace(true), "nested measurement"));
    let stop = StopTracking;
    let result = operation();
    drop(stop);
    (result, REQUESTS.with(Cell::get))
}

fn cases(maximum: usize, kebab: bool) -> Vec<(String, bool)> {
    let mut values = vec![
        ("a".into(), true),
        ("a0-b9".into(), true),
        ("a".repeat(maximum), true),
        ("a".repeat(maximum + 1), false),
        (String::new(), false),
        (" a".into(), false),
        ("a ".into(), false),
        ("\u{2003}a".into(), false),
        ("a\u{2003}".into(), false),
        ("a\0b".into(), false),
        ("a\nb".into(), false),
        ("a\tb".into(), false),
        ("a\u{7f}b".into(), false),
        ("a\u{85}b".into(), false),
    ];
    for raw in [
        "A",
        "-a",
        "a-",
        "a--b",
        "a_b",
        "a.b",
        "a/b",
        "a@b",
        "a:b",
        "a b",
        "é",
        "e\u{301}",
        "a\u{202e}b",
        "a\u{200d}b",
    ] {
        values.push((raw.into(), !kebab));
    }
    values.push(("é".repeat(maximum / 2), !kebab));
    values.push((format!("{}a", "é".repeat(maximum / 2)), false));
    values
}

macro_rules! text_control {
    ($test:ident, $type:ty, $maximum:expr, $kebab:expr, $error:literal) => {
        #[test]
        fn $test() {
            for (raw, valid) in cases($maximum, $kebab) {
                // Decode is deliberately outside the measurement: the retained value already
                // owns its text. Invalid decoded strings must be rejected by validate().
                let decoded: $type = norito::json::from_value(norito::json::Value::Array(vec![
                    norito::json::Value::String(raw.clone()),
                ]))
                .expect("decode structural text fixture");
                let before = norito::encode_canonical(&decoded).expect("encode fixture");
                let binary: $type = norito::decode_canonical(&before).expect("decode fixture");
                let expected = if valid { Ok(()) } else { Err($error) };
                let (actual, requests) = measured(|| {
                    black_box(&binary)
                        .validate()
                        .map_err(|error| error.reason())
                });
                assert_eq!(actual, expected, "raw {raw:?}");
                assert_eq!(requests, 0, "validation allocated for {raw:?}");
                assert_eq!(
                    raw.parse::<$type>()
                        .map(|_| ())
                        .map_err(|error| error.reason()),
                    expected,
                    "constructor disagreement for {raw:?}",
                );
                assert_eq!(norito::encode_canonical(&binary).unwrap(), before);
                assert_eq!(
                    norito::json::to_value(&binary).unwrap(),
                    norito::json::Value::Array(vec![norito::json::Value::String(raw)])
                );
            }
        }
    };
}

text_control!(
    package_name_borrows_valid_and_invalid_decoded_text,
    MusubiPackageNameV1,
    MUSUBI_MAX_PACKAGE_NAME_BYTES_V1,
    true,
    "Musubi package name must be lowercase ASCII kebab text"
);
text_control!(
    keyword_borrows_valid_and_invalid_decoded_text,
    MusubiKeywordV1,
    64,
    true,
    "Musubi keyword must be lowercase ASCII kebab text"
);
text_control!(
    alias_borrows_valid_and_invalid_decoded_text,
    MusubiAliasNameV1,
    MUSUBI_MAX_ALIAS_BYTES_V1,
    true,
    "Musubi alias must be 1-32 lowercase ASCII kebab characters"
);
text_control!(
    description_borrows_valid_and_invalid_decoded_text,
    MusubiDescriptionV1,
    4_096,
    false,
    "Musubi description is empty, noncanonical, or exceeds 4096 bytes"
);
text_control!(
    document_ref_borrows_valid_and_invalid_decoded_text,
    MusubiDocumentRefV1,
    2_048,
    false,
    "Musubi document reference is empty, noncanonical, or exceeds 2048 bytes"
);
text_control!(
    reason_borrows_valid_and_invalid_decoded_text,
    MusubiReasonV1,
    1_024,
    false,
    "Musubi reason is empty, noncanonical, or exceeds 1024 bytes"
);

#[test]
fn release_metadata_revalidation_borrows_all_nested_text() {
    let value = MusubiReleaseMetadataV1 {
        description: Some(MusubiDescriptionV1::new(&"é".repeat(2_048)).unwrap()),
        readme: Some(MusubiDocumentRefV1::new(&"r".repeat(2_048)).unwrap()),
        license: Some(MusubiDocumentRefV1::new("MIT OR Apache-2.0").unwrap()),
        repository: Some(MusubiDocumentRefV1::new("https://example.invalid/source").unwrap()),
        keywords: vec!["alpha".parse().unwrap(), "beta-2".parse().unwrap()],
    };
    let before = norito::encode_canonical(&value).unwrap();
    let (actual, requests) = measured(|| black_box(&value).validate());
    actual.unwrap();
    assert_eq!(requests, 0);
    assert_eq!(norito::encode_canonical(&value).unwrap(), before);
}

#[test]
fn release_metadata_preserves_first_text_error_without_allocating() {
    let invalid =
        |raw: &str| norito::json::Value::Array(vec![norito::json::Value::String(raw.into())]);
    let mut value = MusubiReleaseMetadataV1 {
        description: Some(norito::json::from_value(invalid(" bad")).unwrap()),
        readme: Some(norito::json::from_value(invalid(" bad")).unwrap()),
        license: None,
        repository: None,
        keywords: vec![norito::json::from_value(invalid("UPPER")).unwrap()],
    };
    for expected in [
        "Musubi description is empty, noncanonical, or exceeds 4096 bytes",
        "Musubi document reference is empty, noncanonical, or exceeds 2048 bytes",
        "Musubi keyword must be lowercase ASCII kebab text",
    ] {
        let (result, requests) =
            measured(|| black_box(&value).validate().map_err(|error| error.reason()));
        assert_eq!(result, Err(expected));
        assert_eq!(requests, 0);
        if value.description.take().is_none() {
            value.readme = None;
        }
    }
}

#[test]
fn observer_counts_real_allocation_and_growth_requests() {
    let (value, requests) = measured(|| {
        let mut value = String::with_capacity(black_box(1));
        value.push('a');
        value.reserve_exact(black_box(1_024));
        black_box(value)
    });
    assert!(requests >= 2, "allocation and growth must both be observed");
    assert_eq!(value, "a");
}

fn decoded_namespace(raw: &str) -> MusubiNamespaceV1 {
    norito::json::from_value(norito::json::Value::Array(vec![
        norito::json::Value::String(raw.into()),
    ]))
    .unwrap()
}

#[test]
fn namespace_borrowed_ascii_preserves_exact_errors_and_codec_bytes() {
    let mut cases = vec![
        (String::new(), Err("Musubi namespace must not be empty")),
        ("a".into(), Ok(())),
        ("domain.dataspace".into(), Ok(())),
        (" a".into(), Err("Musubi namespace is not canonical")),
        ("a ".into(), Err("Musubi namespace is not canonical")),
        ("a\0b".into(), Err("Musubi namespace is not canonical")),
        ("a/b.c.d".into(), Err("Musubi namespace is not canonical")),
        ("a@b.c.d".into(), Err("Musubi namespace is not canonical")),
        ("a:b.c.d".into(), Err("Musubi namespace is not canonical")),
        (
            "a.b.c".into(),
            Err("Musubi namespace must be `<dataspace>` or `<domain>.<dataspace>`"),
        ),
        (
            "a..b".into(),
            Err("Musubi namespace must be `<dataspace>` or `<domain>.<dataspace>`"),
        ),
        (
            "a..".into(),
            Err("Musubi namespace must be `<dataspace>` or `<domain>.<dataspace>`"),
        ),
        (".".into(), Err("Musubi namespace segment is invalid")),
        (".a".into(), Err("Musubi namespace segment is invalid")),
        ("a.".into(), Err("Musubi namespace segment is invalid")),
        ("a b".into(), Err("Musubi namespace segment is invalid")),
        ("a#b".into(), Err("Musubi namespace segment is invalid")),
        ("a$b".into(), Err("Musubi namespace segment is invalid")),
    ];
    cases.extend([
        ("a".repeat(255), Ok(())),
        ("a".repeat(256), Err("Musubi namespace is not canonical")),
        (format!("{}.{}", "a".repeat(127), "b".repeat(127)), Ok(())),
    ]);
    iroha_model_base::name::Name::validate_canonical("warm").unwrap();
    for (raw, expected) in cases {
        let decoded = decoded_namespace(&raw);
        let before = norito::encode_canonical(&decoded).unwrap();
        let binary: MusubiNamespaceV1 = norito::decode_canonical(&before).unwrap();
        let (actual, requests) = measured(|| binary.validate().map_err(|error| error.reason()));
        assert_eq!(actual, expected, "{raw:?}");
        assert_eq!(requests, 0, "borrowed ASCII {raw:?}");
        let (parsed, requests) = measured(|| raw.parse::<MusubiNamespaceV1>());
        assert_eq!(
            parsed
                .as_ref()
                .map(|_| ())
                .map_err(iroha_model_base::error::ParseError::reason),
            expected
        );
        assert_eq!(
            requests,
            usize::from(expected.is_ok()),
            "constructor retains only final String for {raw:?}"
        );
        if let Ok(parsed) = parsed {
            assert_eq!(parsed.as_str(), raw);
        }
        assert_eq!(norito::encode_canonical(&binary).unwrap(), before);
        assert_eq!(
            norito::json::to_value(&binary).unwrap(),
            norito::json::Value::Array(vec![norito::json::Value::String(raw)])
        );
    }
}

#[test]
fn namespace_unicode_uses_canonical_name_semantics_and_only_bounded_icu_scratch() {
    use iroha_model_base::name::Name;
    Name::validate_canonical("warm").unwrap();
    let corpus = [
        "é".to_owned(),
        "e\u{301}".into(),
        "Å".into(),
        "A\u{30a}".into(),
        "가".into(),
        "\u{1100}\u{1161}".into(),
        "\u{212b}".into(),
        "Ａ".into(),
        "a\u{200d}b".into(),
        "a\u{202e}b".into(),
        "a\u{2066}b".into(),
        "a\u{2003}b".into(),
        "é".repeat(127),
        format!("{}a", "é".repeat(127)),
        format!("q{}", "\u{301}".repeat(100)),
        format!("q{}\u{300}", "\u{315}".repeat(100)),
    ];
    for raw in corpus {
        let decoded = decoded_namespace(&raw);
        let (name, name_requests) = measured(|| Name::validate_canonical(&raw));
        let name_bytes = REQUESTED_BYTES.with(Cell::get);
        let expected = name.map_err(|_| "Musubi namespace segment is invalid");
        let (namespace, namespace_requests) =
            measured(|| decoded.validate().map_err(|error| error.reason()));
        let namespace_bytes = REQUESTED_BYTES.with(Cell::get);
        assert_eq!(namespace, expected, "{raw:?}");
        assert_eq!(
            (namespace_requests, namespace_bytes),
            (name_requests, name_bytes),
            "namespace adds no owned copies over Name for {raw:?}"
        );
        // Existing Name/ICU profile audit: <=255 input scalars, <=4 decomposed
        // scalars each, one SmallVec growing 32..1024 u32 slots. Charge policy
        // sums every replacement request: (2*1024 - 32)*4 = 8064 bytes.
        assert!(
            namespace_bytes <= 8064,
            "unaccounted request beyond audited scratch for {raw:?}"
        );
        let before = norito::encode_canonical(&decoded).unwrap();
        let binary: MusubiNamespaceV1 = norito::decode_canonical(&before).unwrap();
        assert_eq!(binary.validate().map_err(|error| error.reason()), expected);
        assert_eq!(norito::encode_canonical(&binary).unwrap(), before);
        assert_eq!(
            raw.parse::<MusubiNamespaceV1>()
                .map(|value| value.as_str() == raw)
                .map_err(|error| error.reason()),
            expected.map(|()| true)
        );
    }
    let overlong = "é".repeat(128);
    assert_eq!(
        decoded_namespace(&overlong)
            .validate()
            .map_err(|error| error.reason()),
        Err("Musubi namespace is not canonical")
    );
    assert_eq!(
        overlong
            .parse::<MusubiNamespaceV1>()
            .map(|_| ())
            .map_err(|error| error.reason()),
        Err("Musubi namespace is not canonical")
    );
    for (raw, expected) in [
        ("\u{2003}a", Err("Musubi namespace is not canonical")),
        ("a\u{85}b", Err("Musubi namespace is not canonical")),
        ("é.東京", Ok(())),
        ("e\u{301}.東京", Err("Musubi namespace segment is invalid")),
        (
            "e\u{301}.東京.a",
            Err("Musubi namespace must be `<dataspace>` or `<domain>.<dataspace>`"),
        ),
        (
            "e\u{301}@東京.a.b",
            Err("Musubi namespace is not canonical"),
        ),
        ("é.\u{202e}a", Err("Musubi namespace segment is invalid")),
    ] {
        assert_eq!(
            decoded_namespace(raw)
                .validate()
                .map_err(|error| error.reason()),
            expected
        );
        assert_eq!(
            raw.parse::<MusubiNamespaceV1>()
                .map(|_| ())
                .map_err(|error| error.reason()),
            expected
        );
    }
}

#[test]
fn namespace_scratch_plan_is_borrowed_and_covers_all_sequential_segment_requests() {
    use iroha_model_base::name::Name;
    let mark = format!("q{}", "\u{301}".repeat(40));
    let large = format!("q{}", "\u{301}".repeat(70));
    let two = format!("{mark}.{large}");
    let invalid_nfc = format!("q{}\u{300}", "\u{315}".repeat(40));
    for raw in [
        "ascii".to_owned(),
        two.clone(),
        format!("{large}.{mark}"),
        format!("a.{invalid_nfc}"),
        format!(".{mark}"),
        format!("{mark}."),
    ] {
        let namespace = decoded_namespace(&raw);
        let expected = raw
            .split('.')
            .map(Name::canonical_validation_scratch_bytes)
            .max()
            .unwrap_or(0);
        let (demand, requests) = measured(|| namespace.validation_scratch_bytes());
        assert_eq!(
            requests, 0,
            "planning must not enter ICU or construct a replacement"
        );
        assert_eq!(demand, expected);
        let (_, _) = measured(|| namespace.validate());
        let bytes = REQUESTED_BYTES.with(Cell::get);
        // Both segments are sequential, so cumulative requests may use the same
        // reservation twice. Each Name check is independently observed above.
        let segments = raw.split('.').count();
        assert!(bytes <= demand * segments);
    }
    let namespace = decoded_namespace(&two);
    assert_eq!(
        namespace.validation_scratch_bytes(),
        Name::canonical_validation_scratch_bytes(&large)
    );
    assert!(namespace.validation_scratch_bytes() > 0);
    for raw in [
        String::new(),
        format!("{mark}/x"),
        format!("{mark}.x.y"),
        "a".repeat(256),
        format!(" {mark}"),
    ] {
        let namespace = decoded_namespace(&raw);
        let (demand, requests) = measured(|| namespace.validation_scratch_bytes());
        assert_eq!((demand, requests), (0, 0));
        let (outcome, requests) = measured(|| namespace.validate());
        assert!(outcome.is_err());
        assert_eq!(
            requests, 0,
            "the same preliminary rejection does not reach normalization"
        );
    }
}

mod signature_custody {
    use super::{measured, *};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        NetworkId,
        account::{AccountId, MultisigMember, MultisigPolicy},
        block::BlockHeader,
        sorafs::{
            capacity::ProviderId,
            pin_registry::{
                ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
                ProviderIngestFinalizedAnchorV1, ReplicationOrderId,
            },
        },
    };
    use iroha_model_base::topology::DataSpaceId;

    fn algorithms() -> Vec<Algorithm> {
        let mut values = vec![Algorithm::Ed25519, Algorithm::Secp256k1];
        #[cfg(feature = "pqc")]
        values.push(Algorithm::MlDsa);
        #[cfg(feature = "bls")]
        values.extend([Algorithm::BlsNormal, Algorithm::BlsSmall]);
        #[cfg(feature = "gost")]
        values.extend([
            Algorithm::Gost3410_2012_256ParamSetA,
            Algorithm::Gost3410_2012_256ParamSetB,
            Algorithm::Gost3410_2012_256ParamSetC,
            Algorithm::Gost3410_2012_512ParamSetA,
            Algorithm::Gost3410_2012_512ParamSetB,
        ]);
        #[cfg(feature = "sm")]
        values.push(Algorithm::Sm2);
        #[cfg(all(feature = "pqc", feature = "bls", feature = "gost", feature = "sm"))]
        assert_eq!(values.len(), 11, "all shipping algorithms must execute");
        values
    }

    struct Fixtures {
        provider: MusubiProviderBundleVerificationAttestationV1,
        ingress: MusubiSeedIngressReceiptV1,
        delegation: MusubiNamespaceDelegationV1,
        namespace: MusubiNamespaceBindingV1,
    }

    fn fixtures(owner: AccountId, keys: &[KeyPair]) -> Fixtures {
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new([0x23; 32])),
        );
        let provider_binding = MusubiProviderBundleVerificationBindingV1 {
            network_id,
            provider_id: ProviderId::new([0x24; 32]),
            completed_by: owner.clone(),
            completion_authority: ProviderIngestCompletionAuthorityV1::new(
                owner.clone(),
                ProviderIngestCompletionSignerPolicyV1 {
                    policy_id: [0x21; 32],
                    revision: 1,
                    predecessor_digest: None,
                    policy_digest: [0x22; 32],
                },
            ),
            replication_order: ReplicationOrderId::new([0x25; 32]),
            assignment_revision: 3,
            completion_epoch: 9,
            finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                height: 77,
                block_hash: [0x26; 32],
            },
            archive_id: ArchiveId::new([0x27; 32]),
            bundle_digest: MusubiContentDigestV1::new([0x28; 32]),
            descriptor_digest: MusubiContentDigestV1::new([0x29; 32]),
            semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x2a; 32]),
            verification_lock_digest: MusubiVerificationLockDigestV1::new([0x2b; 32]),
            source_tree_digest: MusubiContentDigestV1::new([0x2c; 32]),
        };
        let provider_payload = MusubiProviderBundleVerificationPayloadV1 {
            version: 1,
            binding: provider_binding,
        };
        let mut provider = MusubiProviderBundleVerificationAttestationV1 {
            approvals: keys
                .iter()
                .map(|key| MusubiProviderBundleVerificationApprovalV1 {
                    public_key: key.public_key().clone(),
                    signature: SignatureOf::try_from_hash(
                        key.private_key(),
                        provider_payload.signing_hash(),
                    )
                    .unwrap(),
                })
                .collect(),
            payload: provider_payload,
        };
        provider
            .approvals
            .sort_by(|a, b| a.public_key.cmp(&b.public_key));
        let ingress_payload = MusubiSeedIngressReceiptPayloadV1 {
            version: 1,
            issued_at_ms: 10,
            expires_at_ms: 20,
            binding: MusubiSeedIngressReceiptBindingV1 {
                network_id,
                publisher: owner.clone(),
                ingress_broker: owner.clone(),
                seed_provider: ProviderId::new([0x31; 32]),
                semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x32; 32]),
                archive_id: ArchiveId::new([0x33; 32]),
                car_body_digest: MusubiContentDigestV1::new([0x34; 32]),
                car_body_length: 1_024,
                nonce: [0x35; 32],
            },
        };
        let mut ingress = MusubiSeedIngressReceiptV1 {
            approvals: keys
                .iter()
                .map(|key| MusubiSeedIngressReceiptApprovalV1 {
                    public_key: key.public_key().clone(),
                    signature: SignatureOf::try_from_hash(
                        key.private_key(),
                        ingress_payload.signing_hash(),
                    )
                    .unwrap(),
                })
                .collect(),
            payload: ingress_payload,
        };
        ingress
            .approvals
            .sort_by(|a, b| a.public_key.cmp(&b.public_key));
        let namespace = MusubiNamespaceBindingV1 {
            namespace: "dex.universal".parse().unwrap(),
            home_dataspace: DataSpaceId::new(7),
            scope: MusubiPackageScopeV1::Domain("dex".parse().unwrap()),
            generation: 4,
        };
        let delegation_payload = MusubiNamespaceDelegationPayloadV1 {
            version: 1,
            namespace_binding: namespace.digest(),
            owner_generation: 4,
            owner: owner.clone(),
            delegate: owner,
            expires_at_height: 100,
        };
        let mut delegation = MusubiNamespaceDelegationV1 {
            approvals: keys
                .iter()
                .map(|key| MusubiNamespaceDelegationApprovalV1 {
                    public_key: key.public_key().clone(),
                    signature: SignatureOf::try_from_hash(
                        key.private_key(),
                        delegation_payload.signing_hash(),
                    )
                    .unwrap(),
                })
                .collect(),
            payload: delegation_payload,
        };
        delegation
            .approvals
            .sort_by(|a, b| a.public_key.cmp(&b.public_key));
        Fixtures {
            provider,
            ingress,
            delegation,
            namespace,
        }
    }

    fn verify(f: &Fixtures) -> [Result<(), &'static str>; 3] {
        [
            f.provider
                .verify(&f.provider.payload.binding)
                .map_err(|e| e.reason()),
            f.ingress
                .verify(&f.ingress.payload.binding, 15)
                .map_err(|e| e.reason()),
            f.delegation
                .verify(
                    &f.namespace,
                    &f.delegation.payload.owner,
                    4,
                    &f.delegation.payload.delegate,
                    100,
                )
                .map_err(|e| e.reason()),
        ]
    }

    fn assert_measured(f: &Fixtures, expected: [Result<(), &'static str>; 3]) {
        let (result, allocations) = measured(|| verify(f));
        assert_eq!(result, expected);
        assert_eq!(
            allocations, 0,
            "borrowed production signature validators allocated"
        );
    }

    const BAD_SIGNATURE: [Result<(), &str>; 3] = [
        Err("Musubi provider bundle signature failed"),
        Err("Musubi seed-ingress receipt signature failed"),
        Err("Musubi namespace delegation signature failed"),
    ];

    #[test]
    fn every_shipping_algorithm_uses_uncached_borrowed_musubi_signature_validation() {
        for algorithm in algorithms() {
            let pair = KeyPair::try_from_seed(vec![algorithm as u8 + 0x31; 32], algorithm).unwrap();
            let fixture = fixtures(AccountId::new(pair.public_key().clone()), &[pair]);
            // Construct and sign off-thread. The measured thread has never
            // parsed a key or populated an ordinary positive-verdict cache.
            std::thread::spawn(move || {
                let mut f = fixture;
                assert_measured(&f, [Ok(()); 3]);
                // Wrong authentic statement, same retained canonical keys/signatures.
                f.provider.payload.binding.completion_epoch += 1;
                f.ingress.payload.expires_at_ms += 1;
                f.delegation.payload.expires_at_height += 1;
                assert_measured(&f, BAD_SIGNATURE);
                f.provider.approvals[0].signature =
                    SignatureOf::from_signature(Signature::from_bytes(&vec![
                        0;
                        f.provider.approvals
                            [0]
                        .signature
                        .payload()
                        .len()
                    ]));
                f.ingress.approvals[0].signature =
                    SignatureOf::from_signature(Signature::from_bytes(&vec![
                        0;
                        f.ingress.approvals[0]
                            .signature
                            .payload()
                            .len()
                    ]));
                f.delegation.approvals[0].signature =
                    SignatureOf::from_signature(Signature::from_bytes(&vec![
                        0;
                        f.delegation.approvals
                            [0]
                        .signature
                        .payload()
                        .len()
                    ]));
                assert_measured(&f, BAD_SIGNATURE);
                f.provider.approvals[0].signature =
                    SignatureOf::from_signature(Signature::from_bytes(&[1]));
                f.ingress.approvals[0].signature =
                    SignatureOf::from_signature(Signature::from_bytes(&[1]));
                f.delegation.approvals[0].signature =
                    SignatureOf::from_signature(Signature::from_bytes(&[1]));
                assert_measured(
                    &f,
                    [Err("Musubi approval signature payload length is invalid"); 3],
                );
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn borrowed_multisig_custody_preserves_threshold_and_signature_rejection_order() {
        let keys: Vec<_> = [0x41, 0x42]
            .into_iter()
            .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap())
            .collect();
        let owner = AccountId::new_multisig(
            MultisigPolicy::new(
                2,
                keys.iter()
                    .map(|k| MultisigMember::new(k.public_key().clone(), 1).unwrap())
                    .collect(),
            )
            .unwrap(),
        );
        let mut f = fixtures(owner, &keys);
        assert_measured(&f, [Ok(()); 3]);
        f.provider.approvals.pop();
        f.ingress.approvals.pop();
        f.delegation.approvals.pop();
        assert_measured(
            &f,
            [
                Err("Musubi provider bundle approvals do not meet provider-owner threshold"),
                Err("Musubi seed-ingress receipt does not meet broker threshold"),
                Err("Musubi namespace delegation does not meet owner threshold"),
            ],
        );
        f.provider.payload.binding.completion_epoch += 1;
        f.ingress.payload.expires_at_ms += 1;
        f.delegation.payload.expires_at_height += 1;
        assert_measured(&f, BAD_SIGNATURE);
    }
}
