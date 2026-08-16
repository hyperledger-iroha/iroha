//! Shared ownership for the deterministic collective-public-key polynomial.
use super::{
    ZkAmsMkheErrorV1, ZkAmsMkheGovernedActiveRosterV1, ZkAmsMkheRnsPolynomialWireV1,
    release_profile_v1, zk_ams_mkhe_active_collective_public_a_v1,
};
/// Prepared common public `a` for one governed roster and transcript.
///
/// Preparing once and sharing this immutable context across all eight party
/// generators keeps the release-sized common polynomial in one allocation.
/// Cloning the context or moving its polynomial into a share clones only the
/// wire's `Arc<Vec<u64>>`, never the `38 MiB` release residue vector.
#[derive(Clone)]
pub struct ZkAmsMkhePreparedCollectivePublicAV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    public_a: ZkAmsMkheRnsPolynomialWireV1,
}
impl core::fmt::Debug for ZkAmsMkhePreparedCollectivePublicAV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkhePreparedCollectivePublicAV1")
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("epoch", &self.epoch)
            .field("transcript_digest", &hex::encode(self.transcript_digest))
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkhePreparedCollectivePublicAV1 {
    /// Transcript for which the common polynomial was prepared.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }
    /// Borrow the single prepared common polynomial allocation.
    #[must_use]
    pub const fn public_a(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        &self.public_a
    }
    pub(super) fn validate_for(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        roster.validate()?;
        if self.profile_digest != roster.profile_digest()
            || self.roster_digest != roster.roster_digest()
            || self.key_material_digest != roster.key_material_digest()
            || self.epoch != roster.epoch()
            || self.transcript_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.public_a.encoded_len()?;
        Ok(())
    }
    pub(super) fn shared_public_a(&self) -> ZkAmsMkheRnsPolynomialWireV1 {
        self.public_a.clone()
    }
}
/// Derive and validate the release common `a` exactly once for an eight-party
/// collective-public-key generation batch.
pub fn prepare_zk_ams_mkhe_collective_public_a_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
) -> Result<ZkAmsMkhePreparedCollectivePublicAV1, ZkAmsMkheErrorV1> {
    roster.validate()?;
    if transcript_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let profile = release_profile_v1();
    profile.validate()?;
    let prepared = ZkAmsMkhePreparedCollectivePublicAV1 {
        profile_digest: roster.profile_digest(),
        roster_digest: roster.roster_digest(),
        key_material_digest: roster.key_material_digest(),
        epoch: roster.epoch(),
        transcript_digest,
        public_a: zk_ams_mkhe_active_collective_public_a_v1(roster, transcript_digest)?,
    };
    prepared.validate_for(roster)?;
    Ok(prepared)
}
#[cfg(test)]
mod tests {
    #[test]
    fn production_share_owns_only_an_arc_to_common_a() {
        let source = include_str!("../collective.rs");
        let share = source
            .split("pub struct ZkAmsMkheCollectivePublicKeyShareV1")
            .nth(1)
            .expect("collective share")
            .split("impl ZkAmsMkheCollectivePublicKeyShareV1")
            .next()
            .expect("collective share fields");
        assert!(share.contains("public_a: ZkAmsMkheRnsPolynomialWireV1"));
        assert!(!share.contains("public_a: Vec<u64>"));
        let accessor = source
            .split("impl ZkAmsMkheCollectivePublicKeyShareV1")
            .nth(1)
            .expect("collective share implementation")
            .split("pub const fn party_public_b")
            .next()
            .expect("common-a accessor boundary");
        assert!(accessor.contains("pub const fn public_a"));
        let wire = include_str!("../wire.rs");
        let polynomial = wire
            .split("pub struct ZkAmsMkheRnsPolynomialWireV1")
            .nth(1)
            .expect("wire polynomial")
            .split('}')
            .next()
            .expect("wire polynomial fields");
        assert!(polynomial.contains("residues: Arc<Vec<u64>>"));
        let generator = source
            .split("pub fn generate_zk_ams_mkhe_collective_party_state_with_prepared_public_a_v1")
            .nth(1)
            .expect("prepared generator")
            .split("fn aggregate_zk_ams_mkhe_collective_public_key_v1")
            .next()
            .expect("prepared generator body");
        assert!(generator.contains("let public_a = prepared.shared_public_a()"));
        assert!(!generator.contains("zk_ams_mkhe_active_collective_public_a_v1("));
    }

    #[test]
    fn retained_common_a_and_party_b_require_exact_vec_capacity() {
        let wire = include_str!("../wire.rs");
        let exact = wire
            .split("pub(super) fn new_exact_capacity_v1")
            .nth(1)
            .expect("exact wire constructor")
            .split("fn new_with_dimensions")
            .next()
            .expect("exact constructor boundary");
        for needle in [
            "residues.capacity() != expected",
            "polynomial.residues.capacity() != expected",
            "polynomial.residues.as_slice().as_ptr() != allocation",
        ] {
            assert!(exact.contains(needle));
        }
        let active = include_str!("../active.rs")
            .split("pub fn zk_ams_mkhe_active_collective_public_a_v1")
            .nth(1)
            .expect("common-a creator")
            .split("/// Prove and authenticate")
            .next()
            .expect("common-a boundary");
        assert!(active.contains("new_exact_capacity_v1(polynomial.coefficients)"));
        let collective = include_str!("../collective.rs")
            .split("pub fn generate_zk_ams_mkhe_collective_party_state_with_prepared_public_a_v1")
            .nth(1)
            .expect("party generator")
            .split("fn aggregate_zk_ams_mkhe_collective_public_key_v1")
            .next()
            .expect("party generator boundary");
        assert!(collective.contains(
            "ZkAmsMkheRnsPolynomialWireV1::new_exact_capacity_v1(party_public_b_native.coefficients)"
        ));
    }
}
