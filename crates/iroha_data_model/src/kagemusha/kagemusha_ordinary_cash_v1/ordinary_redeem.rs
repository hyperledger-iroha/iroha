//! Acyclic ordinary redemption data. Native custody and financial proofs remain separate.
use super::*;

/// Exact pre-candidate ordinary redemption semantic domain, including NUL.
pub const KAGEMUSHA_ORDINARY_REDEEM_OUTPUT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-redeem-output\0";

/// Pre-W2 redemption selection. This record contains no terminal approval, certificate or proof.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryRedemptionOutputV1")]
pub struct KagemushaOrdinaryRedemptionOutputV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Actual threshold-admitted release.
    pub release_id: [u8; 32],
    /// Actual genesis network.
    pub network_id: [u8; 32],
    /// Model-normalized authoritative asset.
    pub normalized_asset_id: [u8; 32],
    /// Actual asset incarnation.
    pub asset_incarnation: [u8; 32],
    /// Actual governed asset scale.
    pub scale: u32,
    /// Actual reserve liability pool.
    pub reserve_pool_id: [u8; 32],
    /// Positive amount in the governed scale.
    pub amount: u128,
    /// Sole ordinary account-domain binding of the exact retained beneficiary original.
    pub beneficiary_account_binding: [u8; 32],
    /// Actual consumed financial State commitment.
    pub sender_before_commitment: [u8; 32],
    /// Actual selected financial successor commitment.
    pub sender_after_commitment: [u8; 32],
    /// Maintained ordinary predecessor conflict/nullifier.
    pub transition_nullifier: [u8; 32],
    /// Actual released lifecycle context, preceding candidate/W1.
    pub lifecycle_digest: [u8; 32],
    /// Complete canonical manifest digest from the same admitted release.
    pub artifact_manifest_digest: [u8; 32],
    /// Exact original Native preparation clock binding.
    pub clock_context_digest: [u8; 32],
    /// Same preparation interval's conservative upper bound.
    pub prepared_at_ms: u64,
}
impl KagemushaOrdinaryRedemptionOutputV1 {
    /// Sole fixed mathematical payload width, excluding the domain.
    pub const PAYLOAD_BYTES: usize = 414;

    /// Sole fixed transcript and semantic ranges; this projection creates no authority.
    #[must_use]
    pub fn binding_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let mut t = Transcript::new(KAGEMUSHA_ORDINARY_REDEEM_OUTPUT_DOMAIN_V1);
        t.raw("version", &self.version.to_le_bytes());
        t.raw("release_id", &self.release_id);
        t.raw("network_id", &self.network_id);
        t.raw("normalized_asset_id", &self.normalized_asset_id);
        t.raw("asset_incarnation", &self.asset_incarnation);
        t.raw("scale", &self.scale.to_le_bytes());
        t.raw("reserve_pool_id", &self.reserve_pool_id);
        t.raw("amount", &self.amount.to_le_bytes());
        t.raw(
            "beneficiary_account_binding",
            &self.beneficiary_account_binding,
        );
        t.raw("sender_before_commitment", &self.sender_before_commitment);
        t.raw("sender_after_commitment", &self.sender_after_commitment);
        t.raw("transition_nullifier", &self.transition_nullifier);
        t.raw("lifecycle_digest", &self.lifecycle_digest);
        t.raw("artifact_manifest_digest", &self.artifact_manifest_digest);
        t.raw("clock_context_digest", &self.clock_context_digest);
        t.raw("prepared_at_ms", &self.prepared_at_ms.to_le_bytes());
        t.finish()
    }

    /// Validate the bounded acyclic data shape, without creating a redemption grant.
    /// # Errors
    /// Refuses missing scope, zero amount/time, invalid scale or an unchanged State.
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.version != 1
            || self.amount == 0
            || self.prepared_at_ms == 0
            || self.scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
            || self.sender_before_commitment == self.sender_after_commitment
            || [
                self.release_id,
                self.network_id,
                self.normalized_asset_id,
                self.asset_incarnation,
                self.reserve_pool_id,
                self.beneficiary_account_binding,
                self.sender_before_commitment,
                self.sender_after_commitment,
                self.transition_nullifier,
                self.lifecycle_digest,
                self.artifact_manifest_digest,
                self.clock_context_digest,
            ]
            .contains(&[0; 32])
        {
            return Err("ordinary redemption output shape rejected".into());
        }
        Ok(())
    }

    /// Join the exact original preparation interval, without treating the projection as a clock.
    /// # Errors
    /// Refuses changed original bounds, clock identity or preparation upper bound.
    pub fn validate_against_clock(
        &self,
        clock: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        clock.validate_shape()?;
        if self.clock_context_digest != clock.binding_digest()?
            || self.prepared_at_ms != clock.upper_at_ms
        {
            return Err("ordinary redemption preparation clock differs".into());
        }
        Ok(())
    }

    /// Join actual retained beneficiary and the same authenticated release's complete manifest.
    /// The caller must independently select the beneficiary from its held Native financial owner.
    /// # Errors
    /// Refuses changed account, release/network or full manifest original.
    pub fn validate_against_originals(
        &self,
        beneficiary: &crate::account::AccountId,
        clock: &KagemushaOrdinaryCashClockContextV1,
        release: &super::super::KagemushaAuthenticatedReleaseV1,
    ) -> Result<(), String> {
        self.validate_against_clock(clock)?;
        if self.beneficiary_account_binding
            != super::super::kagemusha_ordinary_app_account_binding_v1(beneficiary)
            || self.release_id != release.release_id()
            || self.network_id != *release.network_id().as_bytes()
            || self.artifact_manifest_digest != release.manifest_digest()
        {
            return Err("ordinary redemption retained originals differ".into());
        }
        release
            .canonical_manifest_original()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Sole pre-candidate semantic commitment; candidate, W1 and final proofs are excluded.
    /// # Errors
    /// Refuses invalid shape or a changed fixed transcript width.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        let t = self.binding_transcript();
        if t.bytes.len() != KAGEMUSHA_ORDINARY_REDEEM_OUTPUT_DOMAIN_V1.len() + Self::PAYLOAD_BYTES {
            return Err("ordinary redemption transcript width differs".into());
        }
        Ok(Sha256::digest(t.bytes).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn specimen() -> (
        KagemushaOrdinaryRedemptionOutputV1,
        KagemushaOrdinaryCashClockContextV1,
    ) {
        let clock = KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [1; 32],
            signed_observations_original_digest: [2; 32],
            lower_at_ms: 1000,
            upper_at_ms: 1002,
        };
        let output = KagemushaOrdinaryRedemptionOutputV1 {
            version: 1,
            release_id: [3; 32],
            network_id: [4; 32],
            normalized_asset_id: [5; 32],
            asset_incarnation: [6; 32],
            scale: 2,
            reserve_pool_id: [7; 32],
            amount: 123,
            beneficiary_account_binding: [8; 32],
            sender_before_commitment: [9; 32],
            sender_after_commitment: [10; 32],
            transition_nullifier: [11; 32],
            lifecycle_digest: [12; 32],
            artifact_manifest_digest: [13; 32],
            clock_context_digest: clock.binding_digest().unwrap(),
            prepared_at_ms: clock.upper_at_ms,
        };
        (output, clock)
    }
    #[test]
    fn ordinary_redeem_transcript_commits_every_semantic_byte_before_any_candidate() {
        let (output, _) = specimen();
        let t = output.binding_transcript();
        assert_eq!(t.fields.len(), 16);
        assert_eq!(
            t.bytes.len(),
            KAGEMUSHA_ORDINARY_REDEEM_OUTPUT_DOMAIN_V1.len() + 414
        );
        assert_eq!(
            output.binding_digest().unwrap(),
            <[u8; 32]>::from(Sha256::digest(&t.bytes))
        );
        for field in t.fields {
            for offset in field.range {
                let mut bytes = t.bytes.clone();
                bytes[offset] ^= 1;
                assert_ne!(
                    <[u8; 32]>::from(Sha256::digest(bytes)),
                    output.binding_digest().unwrap()
                );
            }
        }
    }
    #[test]
    fn ordinary_redeem_refuses_changed_clock_and_inert_financial_selection() {
        let (output, clock) = specimen();
        output.validate_against_clock(&clock).unwrap();
        for mutation in 0..4 {
            let mut changed = clock;
            match mutation {
                0 => changed.request_nonce[0] ^= 1,
                1 => changed.lower_at_ms += 1,
                2 => changed.upper_at_ms += 1,
                _ => changed.signed_observations_original_digest[0] ^= 1,
            }
            assert!(output.validate_against_clock(&changed).is_err());
        }
        for mutation in 0..5 {
            let mut changed = output;
            match mutation {
                0 => changed.amount = 0,
                1 => changed.beneficiary_account_binding = [0; 32],
                2 => changed.artifact_manifest_digest = [0; 32],
                3 => changed.sender_after_commitment = changed.sender_before_commitment,
                _ => changed.prepared_at_ms += 1,
            }
            assert!(changed.validate_against_clock(&clock).is_err());
        }
    }
}
