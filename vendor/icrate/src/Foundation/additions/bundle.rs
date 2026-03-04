#![cfg(feature = "Foundation_NSBundle")]
use core::panic::{RefUnwindSafe, UnwindSafe};

use crate::common::*;
use crate::Foundation::{self, NSBundle};

// SAFETY: Bundles are documented as thread-safe.
unsafe impl Sync for NSBundle {}
unsafe impl Send for NSBundle {}

impl UnwindSafe for NSBundle {}
impl RefUnwindSafe for NSBundle {}

impl NSBundle {
    #[cfg(feature = "Foundation_NSString")]
    #[cfg(feature = "Foundation_NSDictionary")]
    pub fn name(&self) -> Option<Id<Foundation::NSString>> {
        use Foundation::{NSCopying, NSString};

        let info = self.infoDictionary()?;
        let key = crate::ns_string!("CFBundleName");
        let name = info.get(key)?;
        if !name.is_kind_of::<NSString>() {
            return None;
        }
        let ptr: *const AnyObject = name;
        let ptr: *const NSString = ptr.cast();
        // SAFETY: `name` is confirmed to be an `NSString` instance above, so
        // casting the object pointer to `&NSString` is sound.
        let name = unsafe { ptr.as_ref().unwrap_unchecked() };
        Some(name.copy())
    }
}
