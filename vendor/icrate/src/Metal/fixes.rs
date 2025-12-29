use core::fmt;

use crate::common::*;
use objc2::encode::Encoding;

pub type CFTimeInterval = c_double;

pub const MTLResourceCPUCacheModeShift: NSUInteger = 0;
pub const MTLResourceCPUCacheModeMask: NSUInteger = 0xf << MTLResourceCPUCacheModeShift;

pub const MTLResourceStorageModeShift: NSUInteger = 4;
pub const MTLResourceStorageModeMask: NSUInteger = 0xf << MTLResourceStorageModeShift;

pub const MTLResourceHazardTrackingModeShift: NSUInteger = 8;
pub const MTLResourceHazardTrackingModeMask: NSUInteger = 0x3 << MTLResourceHazardTrackingModeShift;

/// Packed three-component float used by Metal convenience types.
#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct MTLPackedFloat3 {
    pub x: c_float,
    pub y: c_float,
    pub z: c_float,
}

impl MTLPackedFloat3 {
    /// Construct a packed vector from its components.
    #[inline]
    pub const fn new(x: c_float, y: c_float, z: c_float) -> Self {
        Self { x, y, z }
    }
}

impl From<[c_float; 3]> for MTLPackedFloat3 {
    #[inline]
    fn from(value: [c_float; 3]) -> Self {
        Self {
            x: value[0],
            y: value[1],
            z: value[2],
        }
    }
}

impl From<MTLPackedFloat3> for [c_float; 3] {
    #[inline]
    fn from(value: MTLPackedFloat3) -> Self {
        [value.x, value.y, value.z]
    }
}

impl fmt::Debug for MTLPackedFloat3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MTLPackedFloat3")
            .field(&self.x)
            .field(&self.y)
            .field(&self.z)
            .finish()
    }
}

impl_encode! {
    MTLPackedFloat3 = Encoding::Struct(
        "_MTLPackedFloat3",
        &[
            c_float::ENCODING,
            c_float::ENCODING,
            c_float::ENCODING
        ],
    );
}
