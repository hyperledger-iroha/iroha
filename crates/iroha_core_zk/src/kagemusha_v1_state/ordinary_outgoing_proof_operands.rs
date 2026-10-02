//! Native-owned outgoing proof operands, sealed and retained before State proving.
//! These data grant no proof, Reserve, terminal approval, StateAdvance or outbox authority.
//! Sealing protects private Native operands; it makes no OEM hardware-sealing claim.

use super::*;
use crate::kagemusha_v1_recursion::KagemushaAuthenticatedOrdinaryPreparationGuardV1;
use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1, KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1,
    KagemushaOrdinaryPreparedOutgoingV1, kagemusha_ordinary_payment_body_digest_v1,
    kagemusha_ordinary_sealed_recovery_seeds_digest_v1,
    kagemusha_ordinary_sealed_transition_inputs_digest_v1,
};
use zeroize::Zeroizing;

/// Decoded durable originals are data only. Every proof use independently re-admits
/// them against the same captured W2, actual states, Guard and separate financial-secret loan.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryOutgoingProofOperandsV1")]
pub(super) struct OutgoingProofOperandOriginals {
    operation: DigestV1,
    nonce: DigestV1,
    approval_original_sha256: DigestV1,
    guard_original_sha256: DigestV1,
    guard_original: Vec<u8>,
    prepared: KagemushaOrdinaryPreparedOutgoingV1,
    transition_stream: Vec<u8>,
    recovery_stream: Vec<u8>,
}

impl OutgoingProofOperandOriginals {
    pub(super) fn create(
        selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        selection.recheck_selected_originals_and_current_custody()?;
        guard.recheck_preparation_selection(selection)?;
        let challenge = selection.challenge();
        let mut streams = None;
        selection.with_borrowed_financial_secret(&mut |secret| {
            if streams.is_some() {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            let transition = transition_plaintext(selection)?;
            let recovery = Zeroizing::new(selected_recovery_seed(selection, secret)?);
            streams = Some((
                seal_stream(selection, secret, 1, &transition)?,
                seal_stream(selection, secret, 2, recovery.as_ref())?,
            ));
            Ok(())
        })?;
        let (transition_stream, recovery_stream) =
            streams.ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        require_stream_lengths(&transition_stream, &recovery_stream)?;
        let prepared = prepared_original(selection, guard, &transition_stream, &recovery_stream)?;
        let originals = Self {
            operation: challenge.operation_id,
            nonce: challenge.nonce,
            approval_original_sha256: Sha256::digest(selection.original()).into(),
            guard_original_sha256: Sha256::digest(guard.original()).into(),
            guard_original: guard.original().to_vec(),
            prepared,
            transition_stream,
            recovery_stream,
        };
        originals.recheck(selection, guard)?;
        Ok(originals)
    }

    pub(super) fn recheck(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        selection.recheck_selected_originals_and_current_custody()?;
        guard.recheck_preparation_selection(selection)?;
        let challenge = selection.challenge();
        require_stream_lengths(&self.transition_stream, &self.recovery_stream)?;
        if challenge.operation_id != self.operation
            || challenge.nonce != self.nonce
            || self.approval_original_sha256
                != <DigestV1>::from(Sha256::digest(selection.original()))
            || self.guard_original_sha256 != <DigestV1>::from(Sha256::digest(guard.original()))
            || self.guard_original != guard.original()
            || self.prepared
                != prepared_original(
                    selection,
                    guard,
                    &self.transition_stream,
                    &self.recovery_stream,
                )?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let mut entered = false;
        selection.with_borrowed_financial_secret(&mut |secret| {
            if entered {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            entered = true;
            let transition = transition_plaintext(selection)?;
            let recovery = Zeroizing::new(selected_recovery_seed(selection, secret)?);
            if self.transition_stream != seal_stream(selection, secret, 1, &transition)?
                || self.recovery_stream != seal_stream(selection, secret, 2, recovery.as_ref())?
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            Ok(())
        })?;
        if !entered {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        selection.recheck_selected_originals_and_current_custody()
    }

    pub(super) fn guard_original(&self) -> &[u8] {
        &self.guard_original
    }

    pub(super) fn prepared(&self) -> &KagemushaOrdinaryPreparedOutgoingV1 {
        &self.prepared
    }
    pub(super) fn transition_stream(&self) -> &[u8] {
        &self.transition_stream
    }
    pub(super) fn recovery_stream(&self) -> &[u8] {
        &self.recovery_stream
    }
}

fn prepared_original(
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    transition: &[u8],
    recovery: &[u8],
) -> Result<KagemushaOrdinaryPreparedOutgoingV1, KagemushaStateErrorV1> {
    require_stream_lengths(transition, recovery)?;
    let statement = selection.transition_statement();
    let outgoing = selection.outgoing_transport_originals()?;
    let (operation, request_digest, manifest, semantic) = match outgoing {
        crate::kagemusha_v1_recursion::KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
            request,
            output,
            ..
        } => (
            2,
            request.canonical_original_digest().map_err(material)?,
            [0; 32],
            kagemusha_ordinary_payment_body_digest_v1(
                output.binding_digest().map_err(material)?,
                output.encrypted_credit_digest,
            )
            .map_err(material)?,
        ),
        crate::kagemusha_v1_recursion::KagemushaOrdinaryLineageOutgoingOriginalsV1::Redeem {
            output,
            ..
        } => (
            4,
            [0; 32],
            output.artifact_manifest_digest,
            output.binding_digest().map_err(material)?,
        ),
    };
    let prepared = KagemushaOrdinaryPreparedOutgoingV1 {
        version: 1,
        operation,
        predecessor_state: selection.selected_predecessor_state().state_commitment,
        successor_state: selection.selected_successor_state().state_commitment,
        transition_digest: statement.digest()?,
        prepared_transition_binding_digest: statement.prepared_transition_binding_digest,
        projection_semantic_digest: semantic,
        lifecycle_binding_digest: statement.lifecycle_binding_digest,
        request_digest,
        artifact_manifest_digest: manifest,
        preparation_guard_digest: guard.original_digests()[0],
        reservation_digest: selection
            .outbox_reservation_original()?
            .canonical_commitment()
            .map_err(material)?,
        preparation_authorization_digest: guard.original_digests()[2],
        stream_lengths: [
            u64::try_from(transition.len()).map_err(material)?,
            u64::try_from(recovery.len()).map_err(material)?,
        ],
        stream_digests: [
            kagemusha_ordinary_sealed_transition_inputs_digest_v1(transition).map_err(material)?,
            kagemusha_ordinary_sealed_recovery_seeds_digest_v1(recovery).map_err(material)?,
        ],
    };
    prepared.validate_shape().map_err(material)?;
    Ok(prepared)
}

fn transition_plaintext(
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
) -> Result<Zeroizing<Vec<u8>>, KagemushaStateErrorV1> {
    selection.recheck_selected_originals_and_current_custody()?;
    // Full actual states and statement, never a caller-projected balance/nonce/candidate.
    let before = Zeroizing::new(
        norito::encode_canonical(selection.selected_predecessor_state()).map_err(material)?,
    );
    let after = Zeroizing::new(
        norito::encode_canonical(selection.selected_successor_state()).map_err(material)?,
    );
    let statement = Zeroizing::new(
        norito::encode_canonical(selection.transition_statement()).map_err(material)?,
    );
    let mut original = Zeroizing::new(Vec::new());
    for part in [before.as_slice(), after.as_slice(), statement.as_slice()] {
        original.extend_from_slice(&u64::try_from(part.len()).map_err(material)?.to_le_bytes());
        original.extend_from_slice(part);
    }
    Ok(original)
}

pub(super) fn selected_recovery_seed(
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
    secret: &[u8; 32],
) -> Result<[u8; 32], KagemushaStateErrorV1> {
    selection.recheck_selected_originals_and_current_custody()?;
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-outgoing-recovery-seed\0");
    hash.update(secret);
    hash.update(selection.challenge().operation_id);
    hash.update(selection.challenge().nonce);
    hash.update(Sha256::digest(selection.original()));
    hash.update(selection.transition_statement().digest()?);
    let seed: DigestV1 = hash.finalize().into();
    if seed == [0; 32] {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(seed)
}

fn seal_stream(
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
    secret: &[u8; 32],
    role: u8,
    plaintext: &[u8],
) -> Result<Vec<u8>, KagemushaStateErrorV1> {
    let challenge = selection.challenge();
    let binding: DigestV1 = Sha256::digest(selection.original()).into();
    seal_exact_stream(
        secret,
        challenge.operation_id,
        challenge.nonce,
        binding,
        role,
        plaintext,
    )
}

fn seal_exact_stream(
    secret: &[u8; 32],
    operation: DigestV1,
    nonce: DigestV1,
    approval: DigestV1,
    role: u8,
    plaintext: &[u8],
) -> Result<Vec<u8>, KagemushaStateErrorV1> {
    if secret == &[0; 32]
        || operation == [0; 32]
        || nonce == [0; 32]
        || approval == [0; 32]
        || !matches!(role, 1 | 2)
        || plaintext.is_empty()
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    // Domain-separated per-original key and nonce bind the complete plaintext. Any changed
    // operand obtains a different key/nonce pair and is separately rejected by the actual owner.
    let mut key_hash = Sha256::new();
    key_hash.update(b"iroha:kagemusha:v1:ordinary-native-stream-key\0");
    key_hash.update(secret);
    key_hash.update(operation);
    key_hash.update(nonce);
    key_hash.update(approval);
    key_hash.update([role]);
    key_hash.update(Sha256::digest(plaintext));
    let key = Zeroizing::new(<DigestV1>::from(key_hash.finalize()));
    let mut nonce_hash = Sha256::new();
    nonce_hash.update(b"iroha:kagemusha:v1:ordinary-native-stream-nonce\0");
    nonce_hash.update(operation);
    nonce_hash.update(nonce);
    nonce_hash.update(approval);
    nonce_hash.update([role]);
    let physical_nonce = nonce_hash.finalize();
    let mut aad = Vec::from(b"iroha:kagemusha:v1:ordinary-native-stream\0".as_slice());
    aad.extend_from_slice(&operation);
    aad.extend_from_slice(&nonce);
    aad.extend_from_slice(&approval);
    aad.push(role);
    let cipher =
        SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(key.as_ref()).map_err(material)?;
    let ciphertext = cipher
        .encrypt(&physical_nonce[..12], aad.as_slice(), plaintext)
        .map_err(material)?;
    let mut original = vec![1, role];
    original.extend_from_slice(&physical_nonce[..12]);
    original.extend_from_slice(&ciphertext);
    let maximum = if role == 1 {
        KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1
    } else {
        KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1
    };
    if original.len() > maximum as usize {
        return Err(KagemushaStateErrorV1::InvalidRecoveryMaterial);
    }
    Ok(original)
}

fn require_stream_lengths(transition: &[u8], recovery: &[u8]) -> Result<(), KagemushaStateErrorV1> {
    if !(31..=KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize).contains(&transition.len())
        || !(31..=KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize).contains(&recovery.len())
        || transition.get(..2) != Some(&[1, 1][..])
        || recovery.get(..2) != Some(&[1, 2][..])
    {
        return Err(KagemushaStateErrorV1::InvalidRecoveryMaterial);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_stream_retains_exact_original_and_separates_every_selected_role() {
        let original = seal_exact_stream(
            &[1; 32],
            [2; 32],
            [3; 32],
            [4; 32],
            1,
            b"complete private TESTDATA",
        )
        .unwrap();
        assert_eq!(
            original,
            seal_exact_stream(
                &[1; 32],
                [2; 32],
                [3; 32],
                [4; 32],
                1,
                b"complete private TESTDATA"
            )
            .unwrap()
        );
        for changed in [
            seal_exact_stream(
                &[5; 32],
                [2; 32],
                [3; 32],
                [4; 32],
                1,
                b"complete private TESTDATA",
            )
            .unwrap(),
            seal_exact_stream(
                &[1; 32],
                [5; 32],
                [3; 32],
                [4; 32],
                1,
                b"complete private TESTDATA",
            )
            .unwrap(),
            seal_exact_stream(
                &[1; 32],
                [2; 32],
                [5; 32],
                [4; 32],
                1,
                b"complete private TESTDATA",
            )
            .unwrap(),
            seal_exact_stream(
                &[1; 32],
                [2; 32],
                [3; 32],
                [5; 32],
                1,
                b"complete private TESTDATA",
            )
            .unwrap(),
            seal_exact_stream(
                &[1; 32],
                [2; 32],
                [3; 32],
                [4; 32],
                2,
                b"complete private TESTDATA",
            )
            .unwrap(),
            seal_exact_stream(
                &[1; 32],
                [2; 32],
                [3; 32],
                [4; 32],
                1,
                b"changed private TESTDATA",
            )
            .unwrap(),
        ] {
            assert_ne!(original, changed);
        }
    }
    #[test]
    fn native_stream_rejects_absent_owner_role_and_over_bound_private_originals() {
        assert!(seal_exact_stream(&[0; 32], [2; 32], [3; 32], [4; 32], 1, b"x").is_err());
        assert!(seal_exact_stream(&[1; 32], [0; 32], [3; 32], [4; 32], 1, b"x").is_err());
        assert!(seal_exact_stream(&[1; 32], [2; 32], [3; 32], [4; 32], 0, b"x").is_err());
        assert!(seal_exact_stream(&[1; 32], [2; 32], [3; 32], [4; 32], 1, &vec![7; 2048]).is_err());
        assert!(seal_exact_stream(&[1; 32], [2; 32], [3; 32], [4; 32], 2, &vec![7; 512]).is_err());
    }
}
