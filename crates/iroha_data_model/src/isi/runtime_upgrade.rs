#[cfg(feature = "json")]
use norito::json::{FastJsonWrite, JsonSerialize};

use super::*;
use crate::runtime::RuntimeUpgradeId;

isi! {
    /// Propose a runtime upgrade by submitting a manifest.
    pub struct ProposeRuntimeUpgrade {
        /// Canonical V1 manifest bytes encoded as Norito JSON/binary at admission time.
        pub manifest_bytes: Vec<u8>,
    }
}

isi! {
    /// Activate a previously proposed runtime upgrade at the window start height.
    pub struct ActivateRuntimeUpgrade {
        /// Content-address (blake2b32) of the canonical manifest bytes.
        pub id: RuntimeUpgradeId,
    }
}

isi! {
    /// Cancel a proposed runtime upgrade prior to its start height.
    pub struct CancelRuntimeUpgrade {
        /// Content-address (blake2b32) of the canonical manifest bytes.
        pub id: RuntimeUpgradeId,
    }
}

// Seal implementations so these types participate as instructions
impl crate::seal::Instruction for ProposeRuntimeUpgrade {}
impl crate::seal::Instruction for ActivateRuntimeUpgrade {}
impl crate::seal::Instruction for CancelRuntimeUpgrade {}

// Stable wire IDs for runtime-upgrade instructions
impl ProposeRuntimeUpgrade {
    /// Norito wire identifier for proposing a runtime upgrade.
    pub const WIRE_ID: &'static str = "iroha.runtime_upgrade.propose";
}
impl ActivateRuntimeUpgrade {
    /// Norito wire identifier for activating a runtime upgrade.
    pub const WIRE_ID: &'static str = "iroha.runtime_upgrade.activate";
}
impl CancelRuntimeUpgrade {
    /// Norito wire identifier for canceling a runtime upgrade.
    pub const WIRE_ID: &'static str = "iroha.runtime_upgrade.cancel";
}

#[cfg(feature = "json")]
impl FastJsonWrite for ProposeRuntimeUpgrade {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"manifest_bytes\":");
        JsonSerialize::json_serialize(&self.manifest_bytes, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for ActivateRuntimeUpgrade {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"id\":");
        JsonSerialize::json_serialize(&self.id, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for CancelRuntimeUpgrade {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"id\":");
        JsonSerialize::json_serialize(&self.id, out);
        out.push('}');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_runtime_upgrade_isis() {
        let pid = RuntimeUpgradeId([0xAA; 32]);
        let propose = ProposeRuntimeUpgrade {
            manifest_bytes: vec![1, 2, 3],
        };
        let act = ActivateRuntimeUpgrade { id: pid };
        let can = CancelRuntimeUpgrade { id: pid };
        let b1 = norito::to_bytes(&propose).unwrap();
        let b2 = norito::to_bytes(&act).unwrap();
        let b3 = norito::to_bytes(&can).unwrap();
        let r1: ProposeRuntimeUpgrade = norito::decode_from_bytes(&b1).unwrap();
        let r2: ActivateRuntimeUpgrade = norito::decode_from_bytes(&b2).unwrap();
        let r3: CancelRuntimeUpgrade = norito::decode_from_bytes(&b3).unwrap();
        assert_eq!(propose, r1);
        assert_eq!(act, r2);
        assert_eq!(can, r3);
    }
}
