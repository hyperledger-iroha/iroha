//! Canonical complete service transport data; no decoder can grant a receipt or Native owner.
use super::*;
type Result<T> = core::result::Result<T, String>;
use crate::kagemusha_v1_state::{
    KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
    KagemushaOrdinaryNativeSignedClockOriginalV1,
};

/// Raw canonical service cap; HTTP Base64/JSON has its own independently larger ceiling.
pub const KAGEMUSHA_ORDINARY_LINEAGE_SERVICE_ORIGINAL_MAX_BYTES_V1: usize = 64 * 1024 * 1024;
#[derive(Clone, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_lineage::ReserveServiceDataV1")]
struct Reserve {
    state_bundle_original: Vec<u8>,
    predecessor_state_original: Vec<u8>,
    neutral_reservation_original: Vec<u8>,
    preparation_signed_clock_original: Vec<u8>,
}
#[derive(Clone, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_lineage::ServiceDataOperationV1")]
enum Operation {
    Anchor(Box<Vec<u8>>),
    Reserve(Box<Reserve>),
    Commit(Box<Vec<u8>>),
}
/// Sole canonical outer transport original. Anchor/Reserve Model proof SHA remains the exact
/// inner State bundle SHA; it never silently changes to SHA of this wrapping transport frame.
#[derive(Clone, norito::Decode, norito::Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_lineage::ServiceOriginalV1")]
pub struct KagemushaOrdinaryLineageServiceOriginalV1 {
    version: u16,
    operation: Operation,
}
impl KagemushaOrdinaryLineageServiceOriginalV1 {
    /// Assemble bounded public data from the actual already-retained zero proof originals.
    /// # Errors
    /// Refuses malformed/oversized sole State bundle data.
    pub fn anchor(bundle_original: Vec<u8>) -> Result<Self> {
        let this = Self {
            version: 1,
            operation: Operation::Anchor(Box::new(bundle_original)),
        };
        this.validate_data()?;
        Ok(this)
    }
    /// Assemble complete public data required for an independently verified reservation.
    /// # Errors
    /// Refuses missing/substituted framing or any component outside its finite bound.
    pub fn reservation(
        bundle_original: Vec<u8>,
        predecessor_state_original: Vec<u8>,
        neutral_reservation_original: Vec<u8>,
        preparation_signed_clock_original: Vec<u8>,
    ) -> Result<Self> {
        let this = Self {
            version: 1,
            operation: Operation::Reserve(Box::new(Reserve {
                state_bundle_original: bundle_original,
                predecessor_state_original,
                neutral_reservation_original,
                preparation_signed_clock_original,
            })),
        };
        this.validate_data()?;
        Ok(this)
    }
    /// Wrap the sole whole Commit bundle, including all three complete signed-clock originals.
    /// # Errors
    /// Refuses malformed/oversized whole Commit data.
    pub fn commit(bundle_original: Vec<u8>) -> Result<Self> {
        let this = Self {
            version: 1,
            operation: Operation::Commit(Box::new(bundle_original)),
        };
        this.validate_data()?;
        Ok(this)
    }
    fn validate_data(&self) -> Result<()> {
        if self.version != 1 {
            return Err("ordinary lineage service data rejected".into());
        }
        match &self.operation {
            Operation::Anchor(raw) => {
                KagemushaOrdinaryLineageStateProofBundleV1::decode_original(raw)?;
            }
            Operation::Reserve(r) => {
                KagemushaOrdinaryLineageStateProofBundleV1::decode_original(
                    &r.state_bundle_original,
                )?;
                KagemushaOrdinaryLineageStateOriginalV1::decode_original(
                    &r.predecessor_state_original,
                )?;
                if r.neutral_reservation_original.is_empty()
                    || r.neutral_reservation_original.len() > 4096
                    || r.preparation_signed_clock_original.len()
                        > KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1
                {
                    return Err("ordinary lineage service data rejected".into());
                }
                KagemushaOrdinaryNativeSignedClockOriginalV1::decode_original(
                    &r.preparation_signed_clock_original,
                )
                .map_err(|e| e.to_string())?;
            }
            Operation::Commit(raw) => {
                KagemushaOrdinaryLineageCommitProofBundleV1::decode_original(raw)?;
            }
        }
        Ok(())
    }
    /// Sole canonical complete outer transport bytes, granting no proof or DATA authority.
    /// # Errors
    /// Refuses malformed data, canonical failure or full raw transport over the bound.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate_data()?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > KAGEMUSHA_ORDINARY_LINEAGE_SERVICE_ORIGINAL_MAX_BYTES_V1 {
            return Err("ordinary lineage service data rejected".into());
        }
        Ok(raw)
    }
    /// Strict bounded complete data decoder; proof verification and current authority are separate.
    /// # Errors
    /// Refuses trailing, substituted, malformed or excessive transport bytes.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_LINEAGE_SERVICE_ORIGINAL_MAX_BYTES_V1 {
            return Err("ordinary lineage service data rejected".into());
        }
        let this: Self =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|e| e.to_string())?;
        if this.canonical_bytes()? != raw {
            return Err("ordinary lineage service data rejected".into());
        }
        Ok(this)
    }
    /// Exact sole inner zero State bundle, without proof admission.
    pub fn anchor_bundle_original(&self) -> Option<&[u8]> {
        match &self.operation {
            Operation::Anchor(raw) => Some(raw),
            _ => None,
        }
    }
    /// Exact inner outgoing State bundle, full prior State, neutral reservation and signed clock.
    pub fn reservation_originals(&self) -> Option<[&[u8]; 4]> {
        match &self.operation {
            Operation::Reserve(r) => Some([
                &r.state_bundle_original,
                &r.predecessor_state_original,
                &r.neutral_reservation_original,
                &r.preparation_signed_clock_original,
            ]),
            _ => None,
        }
    }
    /// Exact sole whole Commit bundle, without current FI, receipt or DATA authority.
    pub fn commit_bundle_original(&self) -> Option<&[u8]> {
        match &self.operation {
            Operation::Commit(raw) => Some(raw),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_service_carrier_refuses_missing_originals_for_each_distinct_operation() {
        assert!(KagemushaOrdinaryLineageServiceOriginalV1::anchor(Vec::new()).is_err());
        assert!(
            KagemushaOrdinaryLineageServiceOriginalV1::reservation(
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .is_err()
        );
        assert!(KagemushaOrdinaryLineageServiceOriginalV1::commit(Vec::new()).is_err());
        assert!(KagemushaOrdinaryLineageServiceOriginalV1::decode_original(&[]).is_err());
        let future = KagemushaOrdinaryLineageServiceOriginalV1 {
            version: 2,
            operation: Operation::Anchor(Box::new(vec![1])),
        };
        let encoded = norito::encode_canonical(&future).unwrap();
        assert!(KagemushaOrdinaryLineageServiceOriginalV1::decode_original(&encoded).is_err());
    }
}
