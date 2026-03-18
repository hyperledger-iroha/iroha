#![cfg(feature = "Foundation_NSValue")]
use icrate::Foundation::NSRange;

#[test]
fn make_and_max_range() {
    let range = NSRange::new(3, 2);
    assert_eq!(range, NSRange::from(3..5));
    assert_eq!(range.end(), 5);
}

#[test]
fn contains_location() {
    let range = NSRange::from(10..15);
    assert!(range.contains(10));
    assert!(range.contains(14));
    assert!(!range.contains(15));
    assert!(!range.contains(9));
}

#[test]
fn equality() {
    let a = NSRange::from(1..4);
    let b = NSRange::from(1..4);
    let c = NSRange::from(2..5);
    assert_eq!(a, b);
    assert_ne!(a, c);
}
