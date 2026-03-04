#![cfg(feature = "Foundation_NSAttributedString")]
#![cfg(feature = "Foundation_NSString")]
use objc2::rc::{autoreleasepool, Id};
use objc2::runtime::AnyObject;

use icrate::Foundation::{self, NSAttributedString, NSObject, NSString};

#[test]
fn test_new() {
    let s = NSAttributedString::new();
    assert_eq!(&s.string().to_string(), "");
}

#[test]
fn test_string_bound_to_attributed() {
    let attr_s = {
        let source = NSString::from_str("Hello world!");
        NSAttributedString::from_nsstring(&source)
    };
    let s = autoreleasepool(|_| attr_s.string());
    assert_eq!(s.len(), 12);
}

#[test]
fn test_from_nsstring() {
    let s = NSAttributedString::from_nsstring(&NSString::from_str("abc"));
    assert_eq!(&s.string().to_string(), "abc");
}

#[test]
#[cfg(feature = "Foundation_NSMutableAttributedString")]
fn test_copy() {
    use Foundation::{NSCopying, NSMutableCopying, NSObjectProtocol};

    let s1 = NSAttributedString::from_nsstring(&NSString::from_str("abc"));
    let s2 = s1.copy();
    // NSAttributedString performs this optimization in GNUStep's runtime,
    // but not in Apple's; so we don't test for it!
    // assert_eq!(Id::as_ptr(&s1), Id::as_ptr(&s2));
    assert!(s2.is_kind_of::<NSAttributedString>());

    let s3 = s1.mutableCopy();
    assert_ne!(Id::as_ptr(&s1), Id::as_ptr(&s3).cast());
    assert!(s3.is_kind_of::<Foundation::NSMutableAttributedString>());
}

#[test]
#[cfg(feature = "Foundation_NSDictionary")]
fn test_debug() {
    let s = NSAttributedString::from_nsstring(&NSString::from_str("abc"));
    let expected = if cfg!(feature = "gnustep-1-7") {
        "abc{}"
    } else {
        "abc{\n}"
    };
    assert_eq!(format!("{s:?}"), expected);

    let obj = Id::into_super(NSObject::new());
    let ptr: *const AnyObject = &*obj;
    let s = unsafe {
        NSAttributedString::new_with_attributes(
            &NSString::from_str("abc"),
            &Foundation::NSDictionary::from_keys_and_objects(
                &[&*NSString::from_str("test")],
                vec![obj],
            ),
        )
    };
    let expected = if cfg!(feature = "gnustep-1-7") {
        format!("abc{{test = \"<NSObject: {ptr:?}>\"; }}")
    } else {
        format!("abc{{\n    test = \"<NSObject: {ptr:?}>\";\n}}")
    };
    assert_eq!(format!("{s:?}"), expected);
}

#[test]
#[cfg(feature = "Foundation_NSMutableAttributedString")]
fn test_new_mutable() {
    let s = Foundation::NSMutableAttributedString::new();
    assert_eq!(&s.string().to_string(), "");
}

#[test]
#[cfg(feature = "Foundation_NSMutableAttributedString")]
fn test_copy_mutable() {
    use Foundation::{NSCopying, NSMutableCopying, NSObjectProtocol};

    let s1 = Foundation::NSMutableAttributedString::from_nsstring(&NSString::from_str("abc"));
    let s2 = s1.copy();
    assert_ne!(Id::as_ptr(&s1).cast(), Id::as_ptr(&s2));
    assert!(s2.is_kind_of::<NSAttributedString>());

    let s3 = s1.mutableCopy();
    assert_ne!(Id::as_ptr(&s1), Id::as_ptr(&s3));
    assert!(s3.is_kind_of::<Foundation::NSMutableAttributedString>());
}

#[test]
#[cfg(all(
    feature = "Foundation_NSMutableAttributedString",
    feature = "Foundation_NSDictionary"
))]
fn test_new_mutable_with_attributes() {
    use objc2::ClassType;

    let source = NSString::from_str("abc");
    let key = NSString::from_str("test");
    let value = Id::into_super(NSObject::new());
    let value_ptr: *const AnyObject = &*value;

    let attrs =
        Foundation::NSDictionary::from_keys_and_objects(&[&*key], vec![value]);

    let attributed = unsafe {
        Foundation::NSMutableAttributedString::new_with_attributes(&source, &attrs)
    };
    let base = attributed.as_ref().as_super();
    assert_eq!(base.length(), source.len());

    let mut effective_range = Foundation::NSRange::default();
    let attrs_roundtrip =
        unsafe { base.attributesAtIndex_effectiveRange(0, &mut effective_range) };
    assert_eq!(effective_range, Foundation::NSRange::new(0, source.len()));

    let lookup_key = NSString::from_str("test");
    let retrieved = attrs_roundtrip
        .get(&*lookup_key)
        .expect("expected attribute value");
    let retrieved_ptr = retrieved as *const AnyObject;
    assert_eq!(retrieved_ptr, value_ptr);
}
