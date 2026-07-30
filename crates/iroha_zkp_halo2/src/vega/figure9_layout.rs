//! Exact deterministic-CBOR byte layout for the closed Figure 9 release profile.

use core::{cmp::Ordering, ops::Range};

use once_cell::sync::Lazy;

use super::{
    VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1,
    VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1, VEGA_MDL_MSO_PAYLOAD_BYTES_V1,
};
#[cfg(test)]
use super::{
    VEGA_MDL_BIRTH_RANDOM_BYTES_V1, VEGA_MDL_FULL_DATE_TEXT_BYTES_V1,
    VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1,
};

pub(super) static FIGURE9_LAYOUT: Lazy<Figure9Layout> = Lazy::new(Figure9Layout::build);

/// Canonical byte range of the per-element random salt in the birth record.
pub(super) const FIGURE9_BIRTH_RANDOM_RANGE: Range<usize> = 13..29;

#[derive(Clone, Debug)]
pub(super) struct Figure9Layout {
    pub(super) issuer_template: Vec<u8>,
    pub(super) issuer_fixed: Vec<bool>,
    pub(super) birth_template: Vec<u8>,
    pub(super) birth_fixed: Vec<bool>,
    pub(super) issuer_birth_digest: Range<usize>,
    pub(super) issuer_device_x: Range<usize>,
    pub(super) issuer_device_y: Range<usize>,
    pub(super) issuer_signed_datetime: Range<usize>,
    pub(super) issuer_valid_from_datetime: Range<usize>,
    pub(super) issuer_valid_until_datetime: Range<usize>,
    pub(super) birth_random: Range<usize>,
    pub(super) birth_date: Range<usize>,
}

impl Figure9Layout {
    fn build() -> Self {
        const BIRTH_DIGEST: [u8; 32] = [
            0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad,
            0xae, 0xaf, 0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xbb,
            0xbc, 0xbd, 0xbe, 0xbf,
        ];
        const DEVICE_X: [u8; 32] = [
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d,
            0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b,
            0x2c, 0x2d, 0x2e, 0x2f,
        ];
        const DEVICE_Y: [u8; 32] = [
            0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d,
            0x4e, 0x4f, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x5b,
            0x5c, 0x5d, 0x5e, 0x5f,
        ];
        const RANDOM: [u8; 16] = [
            0xd0, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xdb, 0xdc, 0xdd,
            0xde, 0xdf,
        ];
        const BIRTH_DATE: &[u8] = b"1987-06-05";
        const SIGNED: &[u8] = b"2025-01-02T03:04:05Z";
        const VALID_FROM: &[u8] = b"2025-02-03T04:05:06Z";
        const VALID_UNTIL: &[u8] = b"2035-08-17T12:34:56Z";

        let birth_inner = cbor_map(vec![
            (cbor_text(b"digestID"), cbor_unsigned(1)),
            (cbor_text(b"random"), cbor_bytes(&RANDOM)),
            (cbor_text(b"elementIdentifier"), cbor_text(b"birth_date")),
            (cbor_text(b"elementValue"), cbor_text(BIRTH_DATE)),
        ]);
        let birth_template = cbor_tag(24, cbor_bytes(&birth_inner));

        let device_key = cbor_map(vec![
            (cbor_unsigned(1), cbor_unsigned(2)),
            (cbor_negative(-1), cbor_unsigned(1)),
            (cbor_negative(-2), cbor_bytes(&DEVICE_X)),
            (cbor_negative(-3), cbor_bytes(&DEVICE_Y)),
        ]);
        let validity_info = cbor_map(vec![
            (cbor_text(b"signed"), cbor_tag(0, cbor_text(SIGNED))),
            (cbor_text(b"validFrom"), cbor_tag(0, cbor_text(VALID_FROM))),
            (
                cbor_text(b"validUntil"),
                cbor_tag(0, cbor_text(VALID_UNTIL)),
            ),
        ]);
        let value_digests = cbor_map(vec![(
            cbor_text(b"org.iso.18013.5.1"),
            cbor_map(vec![(cbor_unsigned(1), cbor_bytes(&BIRTH_DIGEST))]),
        )]);
        let mso_inner = cbor_map(vec![
            (cbor_text(b"version"), cbor_text(b"1.0")),
            (cbor_text(b"digestAlgorithm"), cbor_text(b"SHA-256")),
            (cbor_text(b"valueDigests"), value_digests),
            (
                cbor_text(b"deviceKeyInfo"),
                cbor_map(vec![(cbor_text(b"deviceKey"), device_key)]),
            ),
            (cbor_text(b"docType"), cbor_text(b"org.iso.18013.5.1.mDL")),
            (cbor_text(b"validityInfo"), validity_info),
        ]);
        let mso_payload = cbor_tag(24, cbor_bytes(&mso_inner));
        let issuer_template = cbor_array(vec![
            cbor_text(b"Signature1"),
            cbor_bytes(&[0xa1, 0x01, 0x26]),
            cbor_bytes(&[]),
            cbor_bytes(&mso_payload),
        ]);
        assert_eq!(
            issuer_template.len(),
            VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1
        );
        assert_eq!(mso_payload.len(), VEGA_MDL_MSO_PAYLOAD_BYTES_V1);
        assert_eq!(
            birth_template.len(),
            VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1
        );

        let issuer_birth_digest = find_exact(&issuer_template, &BIRTH_DIGEST);
        let issuer_device_x = find_exact(&issuer_template, &DEVICE_X);
        let issuer_device_y = find_exact(&issuer_template, &DEVICE_Y);
        let issuer_signed_datetime = find_exact(&issuer_template, SIGNED);
        let issuer_valid_from_datetime = find_exact(&issuer_template, VALID_FROM);
        let issuer_valid_until_datetime = find_exact(&issuer_template, VALID_UNTIL);
        let birth_random = find_exact(&birth_template, &RANDOM);
        assert_eq!(
            birth_random, FIGURE9_BIRTH_RANDOM_RANGE,
            "Figure 9 birth random offset drifted"
        );
        let birth_date = find_exact(&birth_template, BIRTH_DATE);

        let mut issuer_fixed = vec![true; issuer_template.len()];
        for range in [
            issuer_birth_digest.clone(),
            issuer_device_x.clone(),
            issuer_device_y.clone(),
            issuer_signed_datetime.clone(),
            issuer_valid_from_datetime.clone(),
            issuer_valid_until_datetime.clone(),
        ] {
            issuer_fixed[range].fill(false);
        }
        let mut birth_fixed = vec![true; birth_template.len()];
        for range in [birth_random.clone(), birth_date.clone()] {
            birth_fixed[range].fill(false);
        }
        Self {
            issuer_template,
            issuer_fixed,
            birth_template,
            birth_fixed,
            issuer_birth_digest,
            issuer_device_x,
            issuer_device_y,
            issuer_signed_datetime,
            issuer_valid_from_datetime,
            issuer_valid_until_datetime,
            birth_random,
            birth_date,
        }
    }
}

fn find_exact(haystack: &[u8], needle: &[u8]) -> Range<usize> {
    let matches = haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == needle).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "Figure 9 placeholder must be unique");
    matches[0]..matches[0] + needle.len()
}

fn cbor_head(major: u8, value: u64) -> Vec<u8> {
    let mut encoded = Vec::new();
    match value {
        0..=23 => encoded.push((major << 5) | value as u8),
        24..=0xff => {
            encoded.push((major << 5) | 24);
            encoded.push(value as u8);
        }
        0x100..=0xffff => {
            encoded.push((major << 5) | 25);
            encoded.extend_from_slice(&(value as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            encoded.push((major << 5) | 26);
            encoded.extend_from_slice(&(value as u32).to_be_bytes());
        }
        _ => {
            encoded.push((major << 5) | 27);
            encoded.extend_from_slice(&value.to_be_bytes());
        }
    }
    encoded
}

fn cbor_unsigned(value: u64) -> Vec<u8> {
    cbor_head(0, value)
}

fn cbor_negative(value: i64) -> Vec<u8> {
    let argument = u64::try_from(-(i128::from(value)) - 1).expect("negative CBOR value");
    cbor_head(1, argument)
}

fn cbor_bytes(value: &[u8]) -> Vec<u8> {
    let mut encoded = cbor_head(2, value.len() as u64);
    encoded.extend_from_slice(value);
    encoded
}

fn cbor_text(value: &[u8]) -> Vec<u8> {
    let mut encoded = cbor_head(3, value.len() as u64);
    encoded.extend_from_slice(value);
    encoded
}

fn cbor_array(values: Vec<Vec<u8>>) -> Vec<u8> {
    let mut encoded = cbor_head(4, values.len() as u64);
    for value in values {
        encoded.extend_from_slice(&value);
    }
    encoded
}

fn cbor_map(mut entries: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
    entries.sort_by(|left, right| deterministic_key_cmp(&left.0, &right.0));
    let mut encoded = cbor_head(5, entries.len() as u64);
    for (key, value) in entries {
        encoded.extend_from_slice(&key);
        encoded.extend_from_slice(&value);
    }
    encoded
}

fn cbor_tag(tag: u64, value: Vec<u8>) -> Vec<u8> {
    let mut encoded = cbor_head(6, tag);
    encoded.extend_from_slice(&value);
    encoded
}

fn deterministic_key_cmp(left: &[u8], right: &[u8]) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_layout_has_stable_lengths_offsets_and_no_unclassified_bytes() {
        let layout = &*FIGURE9_LAYOUT;
        assert_eq!(
            layout.issuer_template.len(),
            VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1
        );
        assert_eq!(
            layout.birth_template.len(),
            VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1
        );
        assert_eq!(layout.issuer_birth_digest, 212..244);
        assert_eq!(layout.issuer_device_x, 277..309);
        assert_eq!(layout.issuer_device_y, 312..344);
        assert_eq!(layout.issuer_signed_datetime, 91..111);
        assert_eq!(layout.issuer_valid_from_datetime, 123..143);
        assert_eq!(layout.issuer_valid_until_datetime, 156..176);
        assert_eq!(layout.birth_date, 53..63);
        assert_eq!(
            layout.issuer_signed_datetime.len(),
            VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1
        );
        assert_eq!(
            layout.issuer_valid_from_datetime.len(),
            VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1
        );
        assert_eq!(
            layout.issuer_valid_until_datetime.len(),
            VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1
        );
        assert_eq!(layout.birth_random.len(), VEGA_MDL_BIRTH_RANDOM_BYTES_V1);
        assert_eq!(layout.birth_date.len(), VEGA_MDL_FULL_DATE_TEXT_BYTES_V1);
        assert_eq!(layout.issuer_fixed.len(), layout.issuer_template.len());
        assert_eq!(layout.birth_fixed.len(), layout.birth_template.len());
        assert_eq!(
            layout
                .birth_fixed
                .iter()
                .enumerate()
                .filter_map(|(offset, fixed)| (!fixed).then_some(offset))
                .collect::<Vec<_>>(),
            (13..29).chain(53..63).collect::<Vec<_>>()
        );
        assert_eq!(
            (layout.issuer_template.len() + 9).div_ceil(64),
            6,
            "368-byte issuer message requires six SHA-256 blocks"
        );
        assert_eq!(
            (layout.birth_template.len() + 9).div_ceil(64),
            2,
            "92-byte birth item requires two SHA-256 blocks"
        );
    }
}
