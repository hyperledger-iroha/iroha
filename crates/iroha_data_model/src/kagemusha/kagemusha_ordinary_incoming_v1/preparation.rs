//! Fixed acyclic incoming preparation transcript. No decoder creates a Native approval or clock.
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Fixed complete incoming preparation payload, before any W2 or W1/proof original exists.
pub const KAGEMUSHA_ORDINARY_INCOMING_PREPARATION_BYTES_V1: usize = 306;

/// Data-only exact incoming financial preparation selected by the actual Main owner.
/// Original Mint funding FI/clock remains in its separate incoming reservation. This record
/// names the independently fresh incoming FI and clock used by W2 and the same private State
/// edge, without importing a sender elapsed clock or claiming hardware monotonic storage.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryIncomingPreparationV1")]
pub struct KagemushaOrdinaryIncomingPreparationV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Sole full IncomingReservation digest; excludes this later preparation and all proofs.
    pub reservation_digest: [u8; 32],
    /// Native source/incoming operation identity, fixed before invocation and shared with W2.
    pub operation_id: [u8; 32],
    /// Fresh Native W2 nonce, independent of dedicated Mint and original receiver signatures.
    pub nonce: [u8; 32],
    /// Full sole TransitionProofStatement digest, opening the actual amount and both State edges.
    pub transition_statement_digest: [u8; 32],
    /// Actual hiding predecessor State commitment.
    pub predecessor_state_commitment: [u8; 32],
    /// Actual hiding successor State commitment, before its proof original exists.
    pub successor_state_commitment: [u8; 32],
    /// SHA256 of the exact separately acknowledged incoming FI control original.
    pub financial_control_original_sha256: [u8; 32],
    /// Sole digest of the fresh actual incoming clock context; full signed observations are separate.
    pub clock_context_digest: [u8; 32],
    /// Actual financial approval index, separate from local Apple counter and software journal.
    pub financial_index_before: u128,
    /// Exact next financial approval index; an app key does not enforce its monotonicity.
    pub financial_index_after: u128,
    /// Actual Main financial journal revision before this edge, without a cast from State sequence.
    pub logical_journal_sequence_before: u64,
    /// Exact next Main financial journal revision.
    pub logical_journal_sequence_after: u64,
}
impl KagemushaOrdinaryIncomingPreparationV1 {
    /// Check pure data shape, never FI/clock/source/State authority.
    /// # Errors
    /// Refuses missing original selectors, reused State head or any overflowing exact-next index.
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.version != 1
            || [
                self.reservation_digest,
                self.operation_id,
                self.nonce,
                self.transition_statement_digest,
                self.predecessor_state_commitment,
                self.successor_state_commitment,
                self.financial_control_original_sha256,
                self.clock_context_digest,
            ]
            .contains(&[0; 32])
            || self.predecessor_state_commitment == self.successor_state_commitment
            || self.financial_index_before.checked_add(1) != Some(self.financial_index_after)
            || self.logical_journal_sequence_before.checked_add(1)
                != Some(self.logical_journal_sequence_after)
        {
            return Err(
                "ordinary incoming preparation originals or exact financial edge differ".into(),
            );
        }
        Ok(())
    }
    /// Sole fixed transcript. All integers are little endian; no canonical proof bytes or
    /// unsigned receiver clock projections are accepted in place of their original selectors.
    /// # Errors
    /// Refuses invalid data shape.
    pub fn binding_transcript(
        &self,
    ) -> Result<[u8; KAGEMUSHA_ORDINARY_INCOMING_PREPARATION_BYTES_V1], String> {
        self.validate_shape()?;
        let mut body = [0; KAGEMUSHA_ORDINARY_INCOMING_PREPARATION_BYTES_V1];
        let mut at = 0;
        let mut add = |bytes: &[u8]| {
            body[at..at + bytes.len()].copy_from_slice(bytes);
            at += bytes.len();
        };
        add(&self.version.to_le_bytes());
        for digest in [
            self.reservation_digest,
            self.operation_id,
            self.nonce,
            self.transition_statement_digest,
            self.predecessor_state_commitment,
            self.successor_state_commitment,
            self.financial_control_original_sha256,
            self.clock_context_digest,
        ] {
            add(&digest);
        }
        add(&self.financial_index_before.to_le_bytes());
        add(&self.financial_index_after.to_le_bytes());
        add(&self.logical_journal_sequence_before.to_le_bytes());
        add(&self.logical_journal_sequence_after.to_le_bytes());
        if at != body.len() {
            return Err("ordinary incoming preparation transcript width differs".into());
        }
        Ok(body)
    }
    /// Purpose-bound complete fixed preparation identity used by normalized Guard intent.
    /// # Errors
    /// Refuses invalid data shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        hash(
            b"iroha:kagemusha:v1:ordinary-incoming-preparation\0",
            &self.binding_transcript()?,
        )
    }
    /// Purpose-bound immutable recovery identity of the same full preparation. This includes
    /// no future W2/W1/proof or global receipt, so its original can be persisted before invocation.
    /// # Errors
    /// Refuses invalid data shape.
    pub fn recovery_binding_digest(&self) -> Result<[u8; 32], String> {
        let mut full = self.binding_transcript()?.to_vec();
        full.extend_from_slice(&self.binding_digest()?);
        hash(b"iroha:kagemusha:v1:ordinary-incoming-recovery\0", &full)
    }
}
fn hash(domain: &[u8], body: &[u8]) -> Result<[u8; 32], String> {
    let length = u64::try_from(body.len()).map_err(|_| "ordinary incoming transcript length")?;
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(length.to_le_bytes());
    hash.update(body);
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn specimen() -> KagemushaOrdinaryIncomingPreparationV1 {
        KagemushaOrdinaryIncomingPreparationV1 {
            version: 1,
            reservation_digest: [1; 32],
            operation_id: [2; 32],
            nonce: [3; 32],
            transition_statement_digest: [4; 32],
            predecessor_state_commitment: [5; 32],
            successor_state_commitment: [6; 32],
            financial_control_original_sha256: [7; 32],
            clock_context_digest: [8; 32],
            financial_index_before: (1_u128 << 100) + 17,
            financial_index_after: (1_u128 << 100) + 18,
            logical_journal_sequence_before: 21,
            logical_journal_sequence_after: 22,
        }
    }
    #[test]
    fn ordinary_incoming_preparation_opens_all_fresh_originals_and_full_u128_index() {
        let original = specimen();
        let transcript = original.binding_transcript().unwrap();
        assert_eq!(transcript.len(), 306);
        assert_eq!(
            &transcript[258..274],
            &original.financial_index_before.to_le_bytes()
        );
        assert_eq!(
            &transcript[274..290],
            &original.financial_index_after.to_le_bytes()
        );
        let canonical = norito::encode_canonical(&original).unwrap();
        let decoded: KagemushaOrdinaryIncomingPreparationV1 = norito::decode_canonical_with_limits(
            &canonical,
            norito::canonical_decode_limits(canonical.len()),
        )
        .unwrap();
        assert_eq!(decoded, original);
        let digest = original.binding_digest().unwrap();
        let recovery = original.recovery_binding_digest().unwrap();
        assert_ne!(digest, recovery);
        for index in 0..8 {
            let mut changed = original;
            let field = match index {
                0 => &mut changed.reservation_digest,
                1 => &mut changed.operation_id,
                2 => &mut changed.nonce,
                3 => &mut changed.transition_statement_digest,
                4 => &mut changed.predecessor_state_commitment,
                5 => &mut changed.successor_state_commitment,
                6 => &mut changed.financial_control_original_sha256,
                _ => &mut changed.clock_context_digest,
            };
            field[0] ^= 1;
            assert_ne!(changed.binding_digest().unwrap(), digest);
            assert_ne!(changed.recovery_binding_digest().unwrap(), recovery);
        }
    }
    #[test]
    fn ordinary_incoming_preparation_rejects_index_and_journal_aliasing() {
        let original = specimen();
        for changed in [
            KagemushaOrdinaryIncomingPreparationV1 {
                financial_index_after: original.financial_index_before,
                ..original
            },
            KagemushaOrdinaryIncomingPreparationV1 {
                financial_index_before: u128::MAX,
                financial_index_after: 0,
                ..original
            },
            KagemushaOrdinaryIncomingPreparationV1 {
                logical_journal_sequence_before: u64::MAX,
                logical_journal_sequence_after: 0,
                ..original
            },
            KagemushaOrdinaryIncomingPreparationV1 {
                nonce: [0; 32],
                ..original
            },
            KagemushaOrdinaryIncomingPreparationV1 {
                successor_state_commitment: original.predecessor_state_commitment,
                ..original
            },
        ] {
            assert!(changed.validate_shape().is_err());
            assert!(changed.binding_digest().is_err());
        }
    }
}
