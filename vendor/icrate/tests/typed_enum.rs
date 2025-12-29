#![cfg(feature = "Foundation_NSString")]

#[cfg(any(
    all(feature = "Foundation_NSStream", feature = "apple"),
    all(feature = "Foundation_NSNotification", feature = "apple")
))]
use objc2::rc::autoreleasepool;
#[cfg(all(feature = "Foundation_NSNotification", feature = "apple"))]
use objc2::rc::Id;

#[cfg(any(
    all(feature = "Foundation_NSStream", feature = "apple"),
    all(feature = "Foundation_NSNotification", feature = "apple")
))]
use icrate::Foundation::NSString;

#[cfg(feature = "Foundation_NSStream")]
use icrate::Foundation::{
    NSStreamSocketSecurityLevel, NSStreamSocketSecurityLevelTLSv1,
};

#[cfg(feature = "Foundation_NSNotification")]
use icrate::Foundation::NSNotificationName;

#[cfg(all(feature = "Foundation_NSStream", feature = "apple"))]
#[test]
fn typed_enum_supports_deref_and_comparisons() {
    let level: &NSStreamSocketSecurityLevel = unsafe { NSStreamSocketSecurityLevelTLSv1 };
    autoreleasepool(|pool| {
        let base: &NSString = level.as_raw();
        assert_eq!(level, base);
        assert_eq!(base, level);
        assert!(!base.as_str(pool).is_empty());
    });
}

#[cfg(all(feature = "Foundation_NSNotification", feature = "apple"))]
#[test]
fn typed_extensible_enum_roundtrip() {
    autoreleasepool(|pool| {
        let owned = NSString::from_str("MyNotification");
        let typed_owned = NSNotificationName::from_id(owned);
        assert_eq!(typed_owned.as_ref().as_str(pool), "MyNotification");

        let raw_back: Id<NSString> = typed_owned.clone().into();
        assert_eq!(raw_back.as_str(pool), "MyNotification");

        let borrowed_source = NSString::from_str("BorrowedNotification");
        let borrowed_ref: &NSString = &borrowed_source;
        let typed_borrowed = NSNotificationName::from_raw(borrowed_ref);
        assert_eq!(typed_borrowed.as_str(pool), "BorrowedNotification");
    });
}
