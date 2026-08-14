//! Platform-gated JNI exports and their Java conversion helpers.
#![cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#![allow(clippy::missing_safety_doc)]
use super::*;
include!("platform_jni/part_1.rs");
include!("platform_jni/part_2.rs");
include!("platform_jni/part_3.rs");
