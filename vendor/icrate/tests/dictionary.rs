#![cfg(feature = "Foundation_NSDictionary")]
#![cfg(feature = "Foundation_NSString")]
#[cfg(feature = "Foundation_NSMutableDictionary")]
use core::ptr::NonNull;
use objc2::rc::{Id, autoreleasepool};

#[cfg(feature = "Foundation_NSMutableDictionary")]
use icrate::Foundation::NSMutableDictionary;
#[cfg(feature = "Foundation_NSMutableString")]
use icrate::Foundation::NSMutableString;
use icrate::Foundation::{NSDictionary, NSObject, NSString};

fn sample_dict(key: &str) -> Id<NSDictionary<NSString, NSObject>> {
    let string = NSString::from_str(key);
    let obj = NSObject::new();
    NSDictionary::from_keys_and_objects(&[&*string], vec![obj])
}

#[test]
fn test_len() {
    let dict = sample_dict("abcd");
    assert_eq!(dict.len(), 1);
}

#[test]
fn test_get() {
    let dict = sample_dict("abcd");

    let string = NSString::from_str("abcd");
    assert!(dict.get(&string).is_some());

    let string = NSString::from_str("abcde");
    assert!(dict.get(&string).is_none());
}

#[test]
fn test_keys() {
    let dict = sample_dict("abcd");
    let keys = dict.keys_vec();

    assert_eq!(keys.len(), 1);
    autoreleasepool(|pool| {
        assert_eq!(keys[0].as_str(pool), "abcd");
    });
}

#[test]
fn test_values() {
    let dict = sample_dict("abcd");
    let vals = dict.values_vec();

    assert_eq!(vals.len(), 1);
}

#[test]
fn test_keys_and_objects() {
    let dict = sample_dict("abcd");
    let (keys, objs) = dict.to_vecs();

    assert_eq!(keys.len(), 1);
    assert_eq!(objs.len(), 1);
    autoreleasepool(|pool| {
        assert_eq!(keys[0].as_str(pool), "abcd");
    });
    assert_eq!(objs[0], dict.get(keys[0]).unwrap());
}

#[test]
#[cfg(feature = "Foundation_NSEnumerator")]
fn test_iter_keys() {
    let dict = sample_dict("abcd");
    assert_eq!(dict.keys().count(), 1);
    autoreleasepool(|pool| {
        assert_eq!(dict.keys().next().unwrap().as_str(pool), "abcd");
    });
}

#[test]
#[cfg(feature = "Foundation_NSEnumerator")]
fn test_iter_values() {
    let dict = sample_dict("abcd");
    assert_eq!(dict.values().count(), 1);
}

#[test]
#[cfg(feature = "Foundation_NSEnumerator")]
fn test_into_keys() {
    let dict = sample_dict("abcd");
    let mut iter = NSDictionary::into_keys(dict);
    autoreleasepool(|pool| {
        let key = iter.next().expect("expected key");
        assert_eq!(key.as_str(pool), "abcd");
    });
    assert!(iter.next().is_none());
}

#[test]
#[cfg(feature = "Foundation_NSArray")]
fn test_arrays() {
    let dict = sample_dict("abcd");

    let keys = unsafe { dict.allKeys() };
    assert_eq!(keys.len(), 1);
    autoreleasepool(|pool| {
        assert_eq!(keys[0].as_str(pool), "abcd");
    });

    // let objs = NSDictionary::into_values_array(dict);
    // assert_eq!(objs.len(), 1);
}

#[test]
fn test_debug() {
    let key = NSString::from_str("a");
    let val = NSString::from_str("b");
    let dict = NSDictionary::from_keys_and_objects(&[&*key], vec![val]);
    assert_eq!(format!("{dict:?}"), r#"{"a": "b"}"#);
}

#[test]
#[cfg(feature = "Foundation_NSMutableDictionary")]
fn test_mutable_dictionary_insert_and_remove_return_ids() {
    let mut dict = NSMutableDictionary::new();
    let key = NSString::from_str("key");

    let first = NSObject::new();
    let first_ptr = Id::as_ptr(&first);
    assert!(dict.insert(key.clone(), first).is_none());

    let second = NSObject::new();
    let second_ptr = Id::as_ptr(&second);
    let replaced = dict.insert(key.clone(), second).expect("existing value");
    assert_eq!(Id::as_ptr(&replaced), first_ptr);

    let current = dict.get(&key).expect("value present after second insert");
    assert_eq!(NonNull::from(current).as_ptr(), second_ptr);

    let removed = dict.remove(&key).expect("value removed");
    assert_eq!(Id::as_ptr(&removed), second_ptr);
    assert!(dict.get(&key).is_none());
}

#[test]
#[cfg(all(
    feature = "Foundation_NSMutableDictionary",
    feature = "Foundation_NSMutableString",
    feature = "Foundation_NSEnumerator"
))]
fn test_mutable_dictionary_values_mut_updates_in_place() {
    let mut dict = NSMutableDictionary::new();
    let key1 = NSString::from_str("first");
    let key2 = NSString::from_str("second");
    dict.insert(key1.clone(), NSMutableString::from_str("alpha"));
    dict.insert(key2.clone(), NSMutableString::from_str("beta"));

    for value in dict.values_mut() {
        value.appendString(&NSString::from_str("!"));
    }

    autoreleasepool(|pool| {
        assert_eq!(dict.get(&key1).expect("first value").as_str(pool), "alpha!");
        assert_eq!(dict.get(&key2).expect("second value").as_str(pool), "beta!");
    });
}
