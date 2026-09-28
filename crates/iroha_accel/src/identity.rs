//! Persistent physical identity independent of enumeration order and reloads.

/// Driver-observed device identity, not a consumer or configuration generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceIdentity {
    /// Driver-provided device UUID (including a MIG instance when exposed).
    pub uuid: [u8; 16],
    /// CUDA Driver API version observed by this process.
    pub driver_version: i32,
}

#[cfg(any(feature = "cuda", test))]
impl DeviceIdentity {
    pub(crate) fn same_physical_device(self, other: Self) -> bool {
        self.uuid == other.uuid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_version_change_does_not_create_fresh_physical_health() {
        let previous = DeviceIdentity {
            uuid: [7; 16],
            driver_version: 12000,
        };
        let replacement = DeviceIdentity {
            driver_version: 13000,
            ..previous
        };
        assert_ne!(previous, replacement);
        assert!(previous.same_physical_device(replacement));
        assert!(!previous.same_physical_device(DeviceIdentity {
            uuid: [8; 16],
            ..previous
        }));
    }
}
