//! Canonical controls and exact domain-label rejection for game wire fixtures.

use std::fmt::Debug;

use norito::{
    codec::{Decode, Encode},
    core as ncore,
};

/// Keep framing valid while replacing one canonical label with an ambiguous one.
pub(crate) fn assert_ambiguous_domain_label_rejected<T>(value: &T, label: &str)
where
    T: Decode
        + for<'__frame> norito::NoritoDeserialize<'__frame>
        + norito::NoritoSerialize
        + Debug
        + PartialEq,
{
    let canonical = value.encode();
    assert_eq!(&T::decode(&mut canonical.as_slice()).unwrap(), value);
    let flags = ncore::default_encode_flags();
    let frame = ncore::frame_bare_with_header_flags::<T>(&canonical, flags).unwrap();
    assert_eq!(&norito::decode_from_bytes::<T>(&frame).unwrap(), value);

    let positions = canonical
        .windows(label.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == label.as_bytes()).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(
        positions.len(),
        1,
        "the fixture must contain exactly one target label"
    );
    let separator = label
        .find('-')
        .expect("a canonical label with one replaceable separator");
    let mut malformed = canonical.clone();
    malformed[positions[0] + separator] = b'.';
    assert_ne!(malformed, canonical);
    assert!(matches!(
        T::decode(&mut malformed.as_slice()),
        Err(ncore::Error::NonCanonicalEncoding)
    ));
    // Recompute the frame checksum so rejection proves the domain invariant,
    // rather than merely detecting an invalid envelope around its payload.
    let frame = ncore::frame_bare_with_header_flags::<T>(&malformed, flags).unwrap();
    assert!(matches!(
        norito::decode_from_bytes::<T>(&frame),
        Err(ncore::Error::NonCanonicalEncoding)
    ));
}
