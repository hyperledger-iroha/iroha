//! Complete blacklist originals in the coordinator's actual immutable object archive.
//!
//! Only the fixed Blacklist Refresh class retains this local reference in PolicyUpdate.
//! It is never a signed header, alternate list layout, issuer verdict or fold authority.
//! The actual Native owner publishes and reads back the complete original before proving,
//! and resolves it again before the existing issuer-role and full-list admission checks.

use super::{Error, ObjectStore, archive};
use crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1 as object_digest;
use iroha_data_model::kagemusha::*;

const REFERENCE_MAX_BYTES: usize = 512;

/// Opaque exact complete-list source; no caller can replace its hash or allocation bound.
#[derive(Debug, Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::BlacklistOriginalReferenceV1")]
pub(crate) struct BlacklistOriginalReferenceV1 {
    version: u16,
    scheme_id: [u8; 32],
    original_digest: [u8; 32],
    original_bytes: u32,
}

impl BlacklistOriginalReferenceV1 {
    /// Derive a local reference from a complete canonical list, never from its signed header.
    /// This pure helper conveys no publication or issuer-authentication authority.
    pub(crate) fn for_original(scheme_id: &[u8; 32], original: &[u8]) -> Result<Self, Error> {
        KagemushaWalletBlacklistV1::decode_canonical(original, scheme_id)
            .map_err(|_| Error::WitnessLost("blacklist source canonical original"))?;
        let reference = Self {
            version: 1,
            scheme_id: *scheme_id,
            original_digest: object_digest(original),
            original_bytes: u32::try_from(original.len())
                .map_err(|_| Error::WitnessLost("blacklist source length"))?,
        };
        reference.require(scheme_id)?;
        Ok(reference)
    }

    fn require(&self, scheme_id: &[u8; 32]) -> Result<usize, Error> {
        let maximum = usize::try_from(self.original_bytes)
            .map_err(|_| Error::WitnessLost("blacklist source bound"))?;
        if self.version != 1
            || self.scheme_id == [0; 32]
            || self.scheme_id != *scheme_id
            || self.original_digest == [0; 32]
            || maximum == 0
            || maximum > KAGEMUSHA_WALLET_BLACKLIST_MAX_BYTES_V1
        {
            return Err(Error::WitnessLost("blacklist source binding"));
        }
        Ok(maximum)
    }

    /// Decode only the one current local reference codec under its fixed allocation bound.
    pub(crate) fn decode_canonical(original: &[u8], scheme_id: &[u8; 32]) -> Result<Self, Error> {
        if original.is_empty() || original.len() > REFERENCE_MAX_BYTES {
            return Err(Error::WitnessLost("blacklist reference frame bound"));
        }
        let reference: Self = archive::decode(original)?;
        reference.require(scheme_id)?;
        Ok(reference)
    }

    /// Exact capsule-local bytes; the complete standalone list cap remains unchanged.
    pub(crate) fn to_canonical_bytes(&self) -> Result<Vec<u8>, Error> {
        self.require(&self.scheme_id)?;
        let original = archive::encode(self)?;
        if original.len() > REFERENCE_MAX_BYTES {
            return Err(Error::WitnessLost("blacklist reference frame bound"));
        }
        Ok(original)
    }

    /// Exact content object identity for an actual source-selected custody index.
    pub(crate) const fn object_digest(&self) -> [u8; 32] {
        self.original_digest
    }

    /// Independently bind every full-original byte, length, scheme, entry order and root.
    /// The existing PreparationV1 still verifies the actual issuer and signature afterward.
    pub(crate) fn verify_original(
        &self,
        scheme_id: &[u8; 32],
        original: &[u8],
    ) -> Result<(), Error> {
        let maximum = self.require(scheme_id)?;
        if original.len() != maximum || object_digest(original) != self.original_digest {
            return Err(Error::WitnessLost("blacklist source exact original"));
        }
        KagemushaWalletBlacklistV1::decode_canonical(original, scheme_id)
            .map_err(|_| Error::WitnessLost("blacklist source canonical original"))?;
        Ok(())
    }
}

/// Publish through the actual coordinator/archive capability and authenticate exact readback.
/// Successful publication changes no manifest, selected monetary head, receipt or proof.
pub(crate) fn publish_blacklist_original(
    objects: &mut dyn ObjectStore,
    scheme_id: &[u8; 32],
    original: &[u8],
) -> Result<BlacklistOriginalReferenceV1, Error> {
    let reference = BlacklistOriginalReferenceV1::for_original(scheme_id, original)?;
    let key = objects.write_object(original, KAGEMUSHA_WALLET_BLACKLIST_MAX_BYTES_V1)?;
    if key != reference.original_digest {
        return Err(Error::WitnessLost("blacklist source publication key"));
    }
    let retained = read_blacklist_original(objects, &reference, scheme_id)?;
    if retained != original {
        return Err(Error::WitnessLost("blacklist source publication original"));
    }
    Ok(reference)
}

/// Read the complete original named by an actual source-selected opaque reference.
/// Native still joins its signed object digest to the current state before using it.
pub(crate) fn read_blacklist_original(
    objects: &mut dyn ObjectStore,
    reference: &BlacklistOriginalReferenceV1,
    scheme_id: &[u8; 32],
) -> Result<Vec<u8>, Error> {
    let maximum = reference.require(scheme_id)?;
    let original = objects.read_object(&reference.original_digest, maximum)?;
    reference.verify_original(scheme_id, &original)?;
    Ok(original)
}

/// Bind the complete original consumed by preparation to the capsule's fixed update kind.
/// Blacklist requires the reference codec; all other kinds require exact inline equality.
pub(crate) fn verify_policy_update_original(
    kind: KagemushaWalletPolicyUpdateKindV1,
    scheme_id: &[u8; 32],
    retained: &[u8],
    complete: &[u8],
) -> Result<(), Error> {
    if kind == KagemushaWalletPolicyUpdateKindV1::Blacklist {
        BlacklistOriginalReferenceV1::decode_canonical(retained, scheme_id)?
            .verify_original(scheme_id, complete)
    } else if retained.is_empty() || retained != complete {
        Err(Error::WitnessLost("policy source exact inline original"))
    } else {
        Ok(())
    }
}

/// Resolve exact PolicyUpdate custody from a source already authenticated by the coordinator.
/// Fixed Blacklist dispatch accepts the reference codec only; the other four classes retain
/// their exact inline originals. No shape/size heuristic, old-layout fallback or issuer verdict.
pub(crate) fn restore_policy_update(
    objects: &mut dyn ObjectStore,
    capsule: &KagemushaWalletRecoveryCapsuleV1,
) -> Result<Vec<u8>, Error> {
    let KagemushaWalletEffectV1::RefreshPolicy { update_kind, .. } = capsule.statement.effect
    else {
        return Err(Error::WitnessLost("policy original operation"));
    };
    if capsule.kind != KagemushaWalletOperationKindV1::RefreshPolicy {
        return Err(Error::WitnessLost("policy original operation"));
    }
    let mut selected = capsule
        .retained_inputs
        .iter()
        .filter(|input| input.role == KagemushaWalletRetainedInputRoleV1::PolicyUpdate);
    let original = &selected
        .next()
        .ok_or(Error::WitnessLost("policy original absent"))?
        .bytes;
    if original.is_empty() || selected.next().is_some() {
        return Err(Error::WitnessLost("policy original inventory"));
    }
    if update_kind == KagemushaWalletPolicyUpdateKindV1::Blacklist {
        let reference =
            BlacklistOriginalReferenceV1::decode_canonical(original, &capsule.scheme_id)?;
        read_blacklist_original(objects, &reference, &capsule.scheme_id)
    } else {
        Ok(original.clone())
    }
}

#[cfg(test)]
#[path = "policy_custody/tests.rs"]
mod tests;
