//! Exact geometry identity and bounded, value-independent lookup.

use super::*;

fn items<'message>(messages: &[&'message [u8]]) -> Vec<Ed25519BatchItem<'message>> {
    messages
        .iter()
        .map(|message| Ed25519BatchItem {
            message,
            signature: [0; 64],
            public_key: [0; 32],
        })
        .collect()
}

#[test]
fn exact_order_and_padding_boundaries_are_distinct_even_at_equal_totals() {
    let bytes = [7; 1024];
    let mut messages = [&bytes[..32]; MIN_ITEMS];
    messages[0] = &bytes[..47];
    messages[1] = &bytes[..49];
    let first = items(&messages);
    let key = GeometryKey::capture(MessageGeometry::new(&first).unwrap());
    messages[0] = &bytes[..48];
    messages[1] = &bytes[..48];
    let second = items(&messages);
    assert_eq!(
        MessageGeometry::new(&first).unwrap().total_bytes(),
        MessageGeometry::new(&second).unwrap().total_bytes()
    );
    assert!(!key.matches(MessageGeometry::new(&second).unwrap()));
    let mut reordered = first.clone();
    reordered.swap(0, 1);
    assert!(!key.matches(MessageGeometry::new(&reordered).unwrap()));
    assert!(MessageGeometry::new(&first[..MIN_ITEMS - 1]).is_none());
    let mut longer = first.clone();
    longer.push(first[0]);
    assert!(!key.matches(MessageGeometry::new(&longer).unwrap()));
}

#[test]
fn identity_retains_lengths_only_and_empty_messages_are_supported() {
    let first = items(&[&[1; 32][..]; MIN_ITEMS]);
    let mut second = items(&[&[2; 32][..]; MIN_ITEMS]);
    for item in &mut second {
        item.signature = [3; 64];
        item.public_key = [4; 32];
    }
    let key = GeometryKey::capture(MessageGeometry::new(&first).unwrap());
    assert!(key.matches(MessageGeometry::new(&second).unwrap()));
    let empty = items(&[&[] as &[u8]; MIN_ITEMS]);
    let geometry = MessageGeometry::new(&empty).unwrap();
    assert_eq!(geometry.total_bytes(), 0);
    assert!(GeometryKey::capture(geometry).matches(geometry));
}

#[test]
fn bounded_lookup_declines_larger_geometry_without_changing_input() {
    let message = vec![0xa5; MAX_MESSAGE_BYTES + 1];
    let short = items(&[&message[..0]; MIN_ITEMS - 1]);
    assert!(MessageGeometry::new(&short).is_none());
    let many = items(&[&[] as &[u8]; MAX_ITEMS + 1]);
    assert!(MessageGeometry::new(&many).is_none());
    let mut inputs = items(&[&message[..0]; MAX_ITEMS]);
    inputs[0].message = &message;
    assert!(MessageGeometry::new(&inputs).is_none());
    for item in &mut inputs[..9] {
        item.message = &message[..MAX_MESSAGE_BYTES];
    }
    assert!(MessageGeometry::new(&inputs).is_none());
    inputs[8].message = &[];
    let geometry = MessageGeometry::new(&inputs).unwrap();
    assert_eq!(geometry.total_bytes(), MAX_TOTAL_BYTES);
    assert_eq!(geometry.len(), MAX_ITEMS);
    assert_eq!(message, vec![0xa5; MAX_MESSAGE_BYTES + 1]);
}
