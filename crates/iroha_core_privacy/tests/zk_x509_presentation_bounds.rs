//! Ordinary-build holder preflight over the same genuinely signed DER fixture encoder as reference tests.
//!
//! This target imports no reference relation, witness codec, release fixture,
//! prover preparation or proof construction API. Run without
//! `privacy-release-evidence` to verify the ordinary library dependency.

#[path = "common/zk_x509_der.rs"]
mod fixture_der;

use fixture_der::*;
use iroha_core_privacy::privacy_engines::zk_x509::{
    errors::{ZkX509DerErrorV1, ZkX509RelationErrorV1},
    presentation::derive_zk_x509_presentation_bounds_v1,
};
use iroha_data_model::privacy::{
    PrivacyZkX509CertificateValidityV1, PrivacyZkX509CrlUpdateIntervalV1,
    PrivacyZkX509PresentationBoundsV1, PrivacyZkX509PresentationWindowV1,
};

const T: u64 = 1_672_531_200;

fn validity(start: u64, end: u64) -> PrivacyZkX509CertificateValidityV1 {
    PrivacyZkX509CertificateValidityV1::new(start, end)
}

fn signed_der(
    validities: &[PrivacyZkX509CertificateValidityV1],
    this_update: u64,
    next_update: u64,
) -> (Vec<Vec<u8>>, Vec<u8>) {
    assert!((2..=3).contains(&validities.len()));
    let keys = [
        p256_key(2).unwrap(),
        p256_key(3).unwrap(),
        p256_key(1).unwrap(),
    ];
    let names = [
        name(&[
            (OID_COUNTRY_NAME, 0x13, b"IL"),
            (OID_ORGANIZATION_NAME, 0x0c, b"Iroha"),
            (OID_ORGANIZATIONAL_UNIT_NAME, 0x0c, b"Holder Tests"),
            (OID_COMMON_NAME, 0x0c, b"Holder"),
        ]),
        name(&[(OID_COMMON_NAME, 0x0c, b"Intermediate")]),
        name(&[(OID_COMMON_NAME, 0x0c, b"Root")]),
    ];
    let ids = [[0x22_u8; 20], [0x33_u8; 20], [0x11_u8; 20]];
    let indices: &[usize] = if validities.len() == 2 {
        &[0, 2]
    } else {
        &[0, 1, 2]
    };
    let mut chain = Vec::new();
    for (position, (&index, validity)) in indices.iter().zip(validities).enumerate() {
        let parent = indices[(position + 1).min(indices.len() - 1)];
        chain.push(certificate(
            &[u8::try_from(position + 1).unwrap()],
            &names[parent],
            &names[index],
            &keys[index],
            &keys[parent],
            &ids[index],
            &ids[parent],
            position
                .checked_sub(1)
                .map(|depth| u32::try_from(depth).unwrap()),
            &validity_der_v1(*validity).unwrap(),
        ));
    }
    let issuer = indices[1];
    let crl = crl(
        &names[issuer],
        &keys[issuer],
        &ids[issuer],
        7,
        &[],
        this_update,
        next_update,
        this_update,
    )
    .unwrap();
    (chain, crl)
}

fn assert_bounds_match_signed_intervals(
    validities: &[PrivacyZkX509CertificateValidityV1],
    this_update: u64,
    next_update: u64,
) -> PrivacyZkX509PresentationBoundsV1 {
    let (chain, crl) = signed_der(validities, this_update, next_update);
    let actual = derive_zk_x509_presentation_bounds_v1(&chain, &crl).unwrap();
    assert_eq!(
        actual,
        PrivacyZkX509PresentationBoundsV1::from_signed_intervals(
            validities,
            PrivacyZkX509CrlUpdateIntervalV1::new(this_update, next_update),
        )
        .unwrap()
    );
    actual
}

#[test]
fn ordinary_two_and_three_certificate_paths_match_signed_interval_definition() {
    for depth in [2, 3] {
        let intervals = vec![validity(T - 60, T + 900); depth];
        let bounds = assert_bounds_match_signed_intervals(&intervals, T, T + 301);
        assert_eq!(bounds.earliest_start_unix_seconds(), T);
        assert_eq!(bounds.latest_end_unix_seconds(), T + 300);
        assert!(
            bounds
                .admit(PrivacyZkX509PresentationWindowV1::new(T, T + 300))
                .is_ok()
        );
    }
}

#[test]
fn ordinary_every_certificate_binds_inclusive_signed_endpoints() {
    for depth in [2, 3] {
        for position in 0..depth {
            let mut intervals = vec![validity(T - 60, T + 900); depth];
            intervals[position] = validity(T + 20, T + 80);
            let bounds = assert_bounds_match_signed_intervals(&intervals, T, T + 301);
            assert_eq!(bounds.earliest_start_unix_seconds(), T + 20);
            assert_eq!(bounds.latest_end_unix_seconds(), T + 80);
            assert!(
                bounds
                    .admit(PrivacyZkX509PresentationWindowV1::new(T + 20, T + 80))
                    .is_ok()
            );
            assert!(
                bounds
                    .admit(PrivacyZkX509PresentationWindowV1::new(T + 19, T + 80))
                    .is_err()
            );
            assert!(
                bounds
                    .admit(PrivacyZkX509PresentationWindowV1::new(T + 20, T + 81))
                    .is_err()
            );
        }
    }
}

#[test]
fn ordinary_crl_next_update_is_exclusive_and_freshness_is_exactly_300_seconds() {
    let intervals = [validity(T - 60, T + 900); 2];
    let exclusive = assert_bounds_match_signed_intervals(&intervals, T, T + 80);
    assert_eq!(exclusive.latest_end_unix_seconds(), T + 79);
    assert!(exclusive.contains_unix_seconds(T + 79));
    assert!(!exclusive.contains_unix_seconds(T + 80));
    let fresh = assert_bounds_match_signed_intervals(&intervals, T, T + 900);
    assert_eq!(fresh.latest_end_unix_seconds(), T + 300);
    assert!(fresh.contains_unix_seconds(T + 300));
    assert!(!fresh.contains_unix_seconds(T + 301));
    assert!(!fresh.contains_unix_seconds(T - 1));
}

#[test]
fn ordinary_disjoint_and_reversed_intervals_preserve_typed_rejections() {
    for intervals in [
        [validity(T - 60, T - 10), validity(T, T + 900)],
        [validity(T + 400, T + 900); 2],
    ] {
        let (chain, crl) = signed_der(&intervals, T, T + 301);
        assert_eq!(
            derive_zk_x509_presentation_bounds_v1(&chain, &crl),
            Err(ZkX509RelationErrorV1::CertificateNotValid)
        );
    }
    let (chain, crl) = signed_der(&[validity(T + 1, T); 2], T, T + 301);
    assert_eq!(
        derive_zk_x509_presentation_bounds_v1(&chain, &crl),
        Err(ZkX509RelationErrorV1::InvalidTime)
    );
    for next_update in [T - 1, T] {
        let (chain, crl) = signed_der(&[validity(T - 60, T + 900); 2], T, next_update);
        assert_eq!(
            derive_zk_x509_presentation_bounds_v1(&chain, &crl),
            Err(ZkX509RelationErrorV1::InvalidCrl)
        );
    }
}

#[test]
fn ordinary_malformed_der_path_depth_and_retry_keep_exact_error_families() {
    let (chain, crl) = signed_der(&[validity(T - 60, T + 900); 2], T, T + 301);
    let good = derive_zk_x509_presentation_bounds_v1(&chain, &crl).unwrap();
    for depth in [0, 1, 4] {
        let malformed = vec![chain[0].clone(); depth];
        assert_eq!(
            derive_zk_x509_presentation_bounds_v1(&malformed, &crl),
            Err(ZkX509RelationErrorV1::WitnessMismatch)
        );
    }
    let mut truncated = chain.clone();
    truncated[0].pop();
    assert!(matches!(
        derive_zk_x509_presentation_bounds_v1(&truncated, &crl),
        Err(ZkX509RelationErrorV1::Der(_))
    ));
    assert!(matches!(
        derive_zk_x509_presentation_bounds_v1(&chain, &crl[..crl.len() - 1]),
        Err(ZkX509RelationErrorV1::Der(_))
    ));
    let mut empty = chain.clone();
    empty[0].clear();
    assert_eq!(
        derive_zk_x509_presentation_bounds_v1(&empty, &crl),
        Err(ZkX509RelationErrorV1::Der(ZkX509DerErrorV1::EmptyInput))
    );
    assert_eq!(
        derive_zk_x509_presentation_bounds_v1(&chain, &crl),
        Ok(good)
    );
}

#[test]
fn ordinary_strict_der_rejects_nonminimal_length_excess_nesting_and_document_size() {
    let (mut chain, crl) = signed_der(&[validity(T - 60, T + 900); 2], T, T + 301);
    chain[0] = vec![0x30, 0x81, 0x00];
    assert_eq!(
        derive_zk_x509_presentation_bounds_v1(&chain, &crl),
        Err(ZkX509RelationErrorV1::Der(
            ZkX509DerErrorV1::NonMinimalLength
        ))
    );
    let mut nested = tlv(0x05, &[]);
    for _ in 0..17 {
        nested = tlv(0x30, &nested);
    }
    chain[0] = nested;
    assert!(matches!(
        derive_zk_x509_presentation_bounds_v1(&chain, &crl),
        Err(ZkX509RelationErrorV1::Der(
            ZkX509DerErrorV1::NestingTooDeep { .. }
        ))
    ));
    chain[0] = vec![0; 16 * 1024 + 1];
    assert_eq!(
        derive_zk_x509_presentation_bounds_v1(&chain, &crl),
        Err(ZkX509RelationErrorV1::Der(
            ZkX509DerErrorV1::InputTooLarge {
                actual: 16 * 1024 + 1,
                max: 16 * 1024
            }
        ))
    );
}

#[test]
fn ordinary_signed_utc_and_generalized_time_boundary_matches_existing_calendar() {
    for second in [0, 2_524_607_999, 2_524_608_000, 253_402_300_498] {
        let intervals = [validity(second, second + 300); 2];
        let bounds = assert_bounds_match_signed_intervals(&intervals, second, second + 301);
        assert_eq!(bounds.earliest_start_unix_seconds(), second);
        assert_eq!(bounds.latest_end_unix_seconds(), second + 300);
    }
}
