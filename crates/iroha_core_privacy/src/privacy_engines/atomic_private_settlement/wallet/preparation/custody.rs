//! Bounded owner-only custody of original authorized inputs before proof preparation.

use super::*;
use iroha_data_model::privacy::PrivacyCommitmentV1;
use norito::{
    Archived, Decode, DeserializePayload, Encode, SerializePayload,
    core::{DecodeFromSlice, Encoder},
};

// Each copied primitive owns its wipe immediately. In particular an error decoding a
// later authentication-path element cannot bypass cleanup of already decoded secret bytes.
struct SecretByte(u8);
#[cfg(test)]
std::thread_local! { static DESTROYED_SECRET_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
impl Drop for SecretByte {
    fn drop(&mut self) {
        self.0.zeroize();
        #[cfg(test)]
        {
            assert_eq!(self.0, 0);
            DESTROYED_SECRET_BYTES.with(|count| count.set(count.get() + 1));
        }
    }
}
impl SerializePayload for SecretByte {
    fn serialize(&self, encoder: &mut Encoder<'_>) -> Result<(), norito::Error> {
        self.0.serialize(encoder)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
impl<'a> DeserializePayload<'a> for SecretByte {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("checked secret byte")
    }
    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, norito::Error> {
        <u8 as DeserializePayload>::try_deserialize(archived.cast()).map(Self)
    }
}
impl<'a> DecodeFromSlice<'a> for SecretByte {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        <u8 as DecodeFromSlice>::decode_from_slice(bytes).map(|(value, used)| (Self(value), used))
    }
}
struct SecretPosition(u32);
impl Drop for SecretPosition {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
impl SerializePayload for SecretPosition {
    fn serialize(&self, encoder: &mut Encoder<'_>) -> Result<(), norito::Error> {
        self.0.serialize(encoder)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
impl<'a> DeserializePayload<'a> for SecretPosition {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("checked secret position")
    }
    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, norito::Error> {
        <u32 as DeserializePayload>::try_deserialize(archived.cast()).map(Self)
    }
}
impl<'a> DecodeFromSlice<'a> for SecretPosition {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        <u32 as DecodeFromSlice>::decode_from_slice(bytes).map(|(value, used)| (Self(value), used))
    }
}

/// Maximum canonical plaintext bytes in one opaque native reservation.
/// The software wallet encrypts these bytes before durable publication.
pub const ATOMIC_PRIVATE_SETTLEMENT_RESERVATION_MAX_BYTES_V1: usize = 512 * 1024;

/// Validated, non-witness bindings for local input exclusion and successful cleanup.
///
/// These values do not prove membership, a ledger outcome or a prepared statement. Output
/// commitments are derived by the existing native preparation owner and are deliberately absent.
#[derive(Clone, PartialEq, Eq)]
pub struct AtomicPrivateSettlementReservationInspectionV1 {
    /// Stable active-note exclusions, bound to network, pool, program and input commitment.
    /// Round, bundle, routing and committee incarnation do not change these identities.
    pub input_ids: Vec<Hash>,
    /// Exact nullifiers rederived from the original two secret inputs.
    pub nullifiers: [PrivacyNullifierV1; PRIVATE_SETTLEMENT_INPUT_SLOTS_V1],
    /// Canonical original restricted terms and authorized output openings commitment.
    pub audit_plaintext_commitment: Hash,
    /// Existing relation's exact ordered input-openings commitment.
    pub audit_input_commitment: [u8; 32],
}
impl std::fmt::Debug for AtomicPrivateSettlementReservationInspectionV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AtomicPrivateSettlementReservationInspectionV1(<restricted bindings>)")
    }
}

// Only this native owner serializes input secrets; the public input/wallet types remain
// non-serializable and expose no spending secret or membership-path getters.
#[derive(Encode, Decode)]
struct StoredInput {
    spending_secret: [SecretByte; 32],
    leaf_position: SecretPosition,
    authentication_path: [[SecretByte; 32]; PRIVATE_NOTE_TREE_DEPTH_V1],
}
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::atomic_private_settlement::wallet::StoredReservationV1")]
struct StoredReservation {
    intent_binding: Hash,
    plaintext: PrivateSettlementAuditPlaintextV1,
    inputs: [StoredInput; PRIVATE_SETTLEMENT_INPUT_SLOTS_V1],
}

fn binding(
    intent: &PrivateSettlementProofIntentV1,
    ordinal: u8,
) -> Result<Hash, AtomicPrivateSettlementPreparationErrorV1> {
    Ok(Hash::new(
        norito::encode_canonical(&(
            "iroha:private-settlement:wallet-reservation:v1".to_owned(),
            intent.clone(),
            ordinal,
        ))
        .map_err(|_| AtomicPrivateSettlementPreparationErrorV1::Encoding)?,
    ))
}

fn input_id(
    intent: &PrivateSettlementProofIntentV1,
    ordinal: u8,
    commitment: PrivacyCommitmentV1,
) -> Result<Hash, AtomicPrivateSettlementPreparationErrorV1> {
    let leg = intent
        .legs
        .get(usize::from(ordinal))
        .ok_or(AtomicPrivateSettlementPreparationErrorV1::Binding)?;
    Ok(Hash::new(
        norito::encode_canonical(&(
            "iroha:private-settlement:wallet-input:v1".to_owned(),
            intent.network_id,
            leg.pool_id,
            atomic_private_settlement_program_id_v1()?,
            commitment,
        ))
        .map_err(|_| AtomicPrivateSettlementPreparationErrorV1::Encoding)?,
    ))
}

fn inspect(
    intent: &PrivateSettlementProofIntentV1,
    ordinal: u8,
    authorized: &AtomicPrivateSettlementAuthorizedWalletLegV1,
) -> Result<AtomicPrivateSettlementReservationInspectionV1, AtomicPrivateSettlementPreparationErrorV1>
{
    use AtomicPrivateSettlementPreparationErrorV1 as E;
    let plaintext = &authorized.plaintext;
    if plaintext.leg_ordinal != ordinal {
        return Err(E::Binding);
    }
    plaintext
        .validate_preparation_against_proof_intent(intent)
        .map_err(|_| E::Binding)?;
    let secrets = Zeroizing::new(core::array::from_fn(|index| {
        authorized.inputs[index].spending_secret
    }));
    let mut normalized = plaintext.inputs.clone();
    let nullifiers = prepare_atomic_private_settlement_authorization_inputs_v1(
        intent,
        ordinal,
        &mut normalized,
        &secrets,
    )?;
    if normalized != plaintext.inputs {
        return Err(E::Binding);
    }
    super::super::super::relation::validate_wallet_authorizations_v1(plaintext, &nullifiers)?;
    let input_ids = normalized
        .iter()
        .filter(|opening| opening.active)
        .map(|opening| input_id(intent, ordinal, opening.commitment))
        .collect::<Result<Vec<_>, _>>()?;
    if input_ids.is_empty() {
        return Err(E::Binding);
    }
    Ok(AtomicPrivateSettlementReservationInspectionV1 {
        input_ids,
        nullifiers,
        audit_plaintext_commitment: plaintext.commitment().map_err(|_| E::Encoding)?,
        audit_input_commitment: atomic_private_settlement_audit_input_commitment_v1(
            &plaintext.inputs,
        )?,
    })
}

/// Validate and encode the original authorized native wallet inputs before proving.
///
/// The returned bytes contain restricted openings and spending secrets. They must remain in
/// the software wallet's authenticated encryption owner, never a public journal, log or network.
/// Existing payer/view signatures and exact input derivation are checked before any bytes escape.
/// This operation does not prove membership or reserve notes by itself.
/// # Errors
/// Malformed/foreign intent, substituted inputs, invalid authorization, or bounded codec refusal.
pub fn encode_atomic_private_settlement_reservation_v1(
    intent: &PrivateSettlementProofIntentV1,
    ordinal: u8,
    authorized: &AtomicPrivateSettlementAuthorizedWalletLegV1,
) -> Result<
    (
        Zeroizing<Vec<u8>>,
        AtomicPrivateSettlementReservationInspectionV1,
    ),
    AtomicPrivateSettlementPreparationErrorV1,
> {
    use AtomicPrivateSettlementPreparationErrorV1 as E;
    let inspection = inspect(intent, ordinal, authorized)?;
    let stored = StoredReservation {
        intent_binding: binding(intent, ordinal)?,
        plaintext: authorized.plaintext.clone(),
        inputs: core::array::from_fn(|index| StoredInput {
            spending_secret: authorized.inputs[index].spending_secret.map(SecretByte),
            leaf_position: SecretPosition(authorized.inputs[index].leaf_position),
            authentication_path: authorized.inputs[index]
                .authentication_path
                .map(|row| row.map(SecretByte)),
        }),
    };
    if norito::canonical_frame_len(&stored).map_err(|_| E::Encoding)?
        > ATOMIC_PRIVATE_SETTLEMENT_RESERVATION_MAX_BYTES_V1
    {
        return Err(E::Wallet(
            AtomicPrivateSettlementWalletErrorV1::InvalidBundle,
        ));
    }
    let material = Zeroizing::new(norito::encode_canonical(&stored).map_err(|_| E::Encoding)?);
    Ok((material, inspection))
}

/// Reopen the exact original native reservation under the independently retained intent.
///
/// The owned plaintext byte buffer is wiped on success and every failure. Native secret fields
/// retain their existing zeroizing destruction. No alternate layout or decoder is accepted.
/// # Errors
/// Excessive/malformed encoding, wrong intent/ordinal, changed secrets or invalid authorization.
pub fn decode_atomic_private_settlement_reservation_v1(
    material: Zeroizing<Vec<u8>>,
    intent: &PrivateSettlementProofIntentV1,
    ordinal: u8,
) -> Result<
    (
        AtomicPrivateSettlementAuthorizedWalletLegV1,
        AtomicPrivateSettlementReservationInspectionV1,
    ),
    AtomicPrivateSettlementPreparationErrorV1,
> {
    use AtomicPrivateSettlementPreparationErrorV1 as E;
    if material.is_empty() || material.len() > ATOMIC_PRIVATE_SETTLEMENT_RESERVATION_MAX_BYTES_V1 {
        return Err(E::Wallet(
            AtomicPrivateSettlementWalletErrorV1::InvalidBundle,
        ));
    }
    let stored: StoredReservation = norito::decode_canonical_with_limits(
        &material,
        norito::canonical_decode_limits(material.len()),
    )
    .map_err(|_| E::Encoding)?;
    if stored.intent_binding != binding(intent, ordinal)? {
        return Err(E::Binding);
    }
    let mut inputs = Vec::with_capacity(PRIVATE_SETTLEMENT_INPUT_SLOTS_V1);
    for input in &stored.inputs {
        let spending_secret =
            Zeroizing::new(core::array::from_fn(|index| input.spending_secret[index].0));
        let authentication_path = Zeroizing::new(core::array::from_fn(|level| {
            core::array::from_fn(|index| input.authentication_path[level][index].0)
        }));
        inputs.push(AtomicPrivateSettlementInputSecretV1::new(
            *spending_secret,
            input.leaf_position.0,
            &authentication_path,
        )?);
    }
    let inputs = inputs.try_into().map_err(|_| E::Binding)?;
    let authorized = AtomicPrivateSettlementAuthorizedWalletLegV1::new(stored.plaintext, inputs);
    let inspection = inspect(intent, ordinal, &authorized)?;
    Ok((authorized, inspection))
}

#[cfg(test)]
#[path = "custody/tests.rs"]
mod tests;

/// Build original signature-valid reservation inputs for custody tests, without proving.
/// Output commitments and memos remain genuinely unprepared; no public proof is claimed.
#[cfg(any(test, feature = "test-utils"))]
#[doc(hidden)]
pub fn atomic_private_settlement_reservation_fixture_v1(
    network: Option<iroha_data_model::NetworkId>,
) -> (
    PrivateSettlementProofIntentV1,
    iroha_data_model::nexus::PrivateSettlementPoolGovernanceV1,
    PrivateSettlementAuditPolicyV1,
    AtomicPrivateSettlementAuthorizedWalletLegV1,
) {
    use crate::privacy_engines::atomic_private_settlement::test_fixtures::{
        aps_sidecar_material_fixture_v1, authorize_output_view_keys, authorize_payer_inputs,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    let fixture = aps_sidecar_material_fixture_v1();
    let mut manifest = fixture.sidecar.manifest;
    let mut plaintext = fixture.plaintext;
    if let Some(network) = network {
        manifest.network_id = network;
        plaintext.network_id = network;
    }
    manifest.fee_intent_digest = manifest.computed_fee_intent_digest().unwrap();
    plaintext.fee_intent_digest = manifest.fee_intent_digest;
    manifest.reimbursement_terms_commitment = plaintext.reimbursement_terms_commitment().unwrap();
    manifest.bundle_id = manifest.computed_bundle_id().unwrap();
    plaintext.bundle_id = manifest.bundle_id;
    let intent = manifest.proof_intent();
    let spending_secrets = Zeroizing::new([[0x81; 32], [0x82; 32]]);
    let nullifiers = prepare_atomic_private_settlement_authorization_inputs_v1(
        &intent,
        0,
        &mut plaintext.inputs,
        &spending_secrets,
    )
    .unwrap();
    let payer = KeyPair::from_seed(vec![0x38; 32], Algorithm::Ed25519);
    let recipient = KeyPair::from_seed(vec![0x39; 32], Algorithm::Ed25519);
    let sponsor = KeyPair::from_seed(vec![0x23; 32], Algorithm::Ed25519);
    authorize_payer_inputs(&mut plaintext, &nullifiers, &payer);
    authorize_output_view_keys(&mut plaintext, [&recipient, &payer, &sponsor]);
    for output in &mut plaintext.outputs {
        output.note.commitment = PrivacyCommitmentV1::new([0; 32]);
        output.note.memo_digest = [0; 32];
    }
    let inputs = core::array::from_fn(|index| {
        AtomicPrivateSettlementInputSecretV1::new(
            spending_secrets[index],
            index as u32,
            &core::array::from_fn(|level| [0x40 + level as u8; 32]),
        )
        .unwrap()
    });
    (
        intent,
        fixture.pool_governance,
        fixture.sidecar.policy,
        AtomicPrivateSettlementAuthorizedWalletLegV1::new(plaintext, inputs),
    )
}
