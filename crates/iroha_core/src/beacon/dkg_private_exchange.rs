//! Authenticated per-seat private exchange for the all-edge beacon DKG.

use iroha_crypto::{
    Algorithm, KeyPair, Signature,
    hybrid::{HybridKemCiphertext, HybridPublicKey, HybridSecretKey},
    threshold_bls::{
        BeaconPurpose, DasRenPrivateShare, open_das_ren_private_share, seal_das_ren_private_share,
    },
};
use iroha_data_model::consensus::{
    GlobalThresholdBeaconDkgDealerCommitmentV1, GlobalThresholdBeaconDkgEncryptedShareV1,
    GlobalThresholdBeaconDkgRecipientKeyV1, GlobalThresholdBeaconDkgSessionV1,
    GlobalThresholdBeaconDkgShareAcceptanceV1,
};
use iroha_model_base::peer::PeerId;

use super::{
    GlobalThresholdBeaconError, adaptive_beacon_parameters,
    global_threshold_beacon_dkg_dealer_commitment_hash_v1,
    global_threshold_beacon_dkg_dealer_commitment_preimage_v1,
    global_threshold_beacon_dkg_encrypted_share_hash_v1,
    global_threshold_beacon_dkg_encrypted_share_preimage_v1,
    global_threshold_beacon_dkg_private_edge_aad_v1,
    global_threshold_beacon_dkg_recipient_key_hash_v1,
    global_threshold_beacon_dkg_recipient_key_preimage_v1,
    global_threshold_beacon_dkg_share_acceptance_preimage_v1, verify_adaptive_dealer,
    verify_global_threshold_beacon_dkg_dealer_commitment_signature_v1,
    verify_global_threshold_beacon_dkg_encrypted_share_v1,
    verify_global_threshold_beacon_dkg_recipient_key_v1,
};

/// Authenticate one dealer's public coefficient and knowledge-proof broadcast.
///
/// # Errors
/// Rejects a mismatched seat or BLS signing failure.
pub fn sign_global_threshold_beacon_dkg_dealer_commitment_v1(
    session: &GlobalThresholdBeaconDkgSessionV1,
    dealer_key: &GlobalThresholdBeaconDkgRecipientKeyV1,
    signer: &KeyPair,
    mut commitment: GlobalThresholdBeaconDkgDealerCommitmentV1,
) -> Result<GlobalThresholdBeaconDkgDealerCommitmentV1, GlobalThresholdBeaconError> {
    verify_global_threshold_beacon_dkg_recipient_key_v1(session, dealer_key)?;
    if dealer_key.recipient_index != commitment.dealer_index
        || dealer_key.validator.public_key() != signer.public_key()
    {
        return Err(GlobalThresholdBeaconError::DealerCommitmentEquivocation);
    }
    commitment.signature = Signature::try_new(
        signer.private_key(),
        &global_threshold_beacon_dkg_dealer_commitment_preimage_v1(session, &commitment),
    )
    .map_err(|_| GlobalThresholdBeaconError::DealerCommitmentEquivocation)?;
    verify_global_threshold_beacon_dkg_dealer_commitment_signature_v1(
        session,
        dealer_key,
        &commitment,
    )?;
    Ok(commitment)
}

/// Sign one fresh encryption key for an exact target seat and attempt.
///
/// # Errors
/// Rejects a non-BLS validator, invalid seat or signing failure.
pub fn sign_global_threshold_beacon_dkg_recipient_key_v1(
    session: &GlobalThresholdBeaconDkgSessionV1,
    recipient_index: u16,
    validator: &KeyPair,
    encryption_key: &HybridPublicKey,
) -> Result<GlobalThresholdBeaconDkgRecipientKeyV1, GlobalThresholdBeaconError> {
    if recipient_index == 0
        || recipient_index > session.committee_size
        || validator.public_key().algorithm() != Algorithm::BlsNormal
    {
        return Err(GlobalThresholdBeaconError::InvalidDkgRecipientKey);
    }
    let mut key = GlobalThresholdBeaconDkgRecipientKeyV1 {
        recipient_index,
        validator: PeerId::new(validator.public_key().clone()),
        x25519_public_key: encryption_key.x25519_bytes(),
        mlkem768_public_key: encryption_key.kyber_bytes().to_vec(),
        signature: Signature::from_bytes(&[]),
    };
    key.signature = Signature::try_new(
        validator.private_key(),
        &global_threshold_beacon_dkg_recipient_key_preimage_v1(session, &key),
    )
    .map_err(|_| GlobalThresholdBeaconError::InvalidDkgRecipientKey)?;
    verify_global_threshold_beacon_dkg_recipient_key_v1(session, &key)?;
    Ok(key)
}

/// Encrypt and sign one dealer's validated private contribution for one seat.
///
/// # Errors
/// Rejects a mismatched dealer, recipient, commitment, phase or crypto failure.
pub fn seal_global_threshold_beacon_dkg_private_edge_v1(
    session: &GlobalThresholdBeaconDkgSessionV1,
    dealer_key: &GlobalThresholdBeaconDkgRecipientKeyV1,
    dealer_signer: &KeyPair,
    dealer_commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
    recipient_key: &GlobalThresholdBeaconDkgRecipientKeyV1,
    share: &DasRenPrivateShare<BeaconPurpose>,
    delivery_height: u64,
) -> Result<GlobalThresholdBeaconDkgEncryptedShareV1, GlobalThresholdBeaconError> {
    verify_global_threshold_beacon_dkg_recipient_key_v1(session, dealer_key)?;
    verify_global_threshold_beacon_dkg_recipient_key_v1(session, recipient_key)?;
    verify_global_threshold_beacon_dkg_dealer_commitment_signature_v1(
        session,
        dealer_key,
        dealer_commitment,
    )?;
    if dealer_key.validator.public_key() != dealer_signer.public_key()
        || dealer_key.recipient_index != dealer_commitment.dealer_index
        || share.dealer_index() != dealer_commitment.dealer_index
        || share.recipient_index() != recipient_key.recipient_index
        || delivery_height < session.commitments_end_height
        || delivery_height >= session.deliveries_end_height
    {
        return Err(GlobalThresholdBeaconError::InvalidDkgEncryptedShare);
    }
    let parameters = adaptive_beacon_parameters(session)?;
    let validated_dealer = verify_adaptive_dealer(&parameters, dealer_commitment)?;
    let components = share.components_for_authenticated_encryption();
    let _ = DasRenPrivateShare::from_components(
        &parameters,
        &validated_dealer,
        share.recipient_index(),
        components[0],
        components[1],
        components[2],
    )?;
    let recipient = HybridPublicKey::from_bytes(
        recipient_key.x25519_public_key,
        &recipient_key.mlkem768_public_key,
    )
    .map_err(|_| GlobalThresholdBeaconError::InvalidDkgRecipientKey)?;
    let mut edge = GlobalThresholdBeaconDkgEncryptedShareV1 {
        dealer_index: dealer_commitment.dealer_index,
        recipient_index: recipient_key.recipient_index,
        dealer_commitment_hash: global_threshold_beacon_dkg_dealer_commitment_hash_v1(
            session,
            dealer_commitment,
        ),
        recipient_key_hash: global_threshold_beacon_dkg_recipient_key_hash_v1(
            session,
            recipient_key,
        ),
        delivery_height,
        ephemeral_x25519_public_key: [0; 32],
        mlkem768_ciphertext: Vec::new(),
        encrypted_share: Vec::new(),
        signature: Signature::from_bytes(&[]),
    };
    let aad = global_threshold_beacon_dkg_private_edge_aad_v1(session, &edge);
    let (kem, encrypted_share) = seal_das_ren_private_share(share, &recipient, &aad)?;
    edge.ephemeral_x25519_public_key = *kem.ephemeral_public();
    edge.mlkem768_ciphertext = kem.kyber_ciphertext().to_vec();
    edge.encrypted_share = encrypted_share;
    edge.signature = Signature::try_new(
        dealer_signer.private_key(),
        &global_threshold_beacon_dkg_encrypted_share_preimage_v1(session, &edge),
    )
    .map_err(|_| GlobalThresholdBeaconError::InvalidDkgEncryptedShare)?;
    verify_global_threshold_beacon_dkg_encrypted_share_v1(
        session,
        dealer_commitment,
        dealer_key,
        recipient_key,
        &edge,
    )?;
    Ok(edge)
}

/// Decrypt, verify and acknowledge one exact private dealer edge.
///
/// The recipient signature is made only after the committed share equation has
/// been checked. The returned share remains non-serializable and zeroizing.
///
/// # Errors
/// Rejects a foreign key, edge, commitment, invalid share or signing failure.
pub fn accept_global_threshold_beacon_dkg_private_edge_v1(
    session: &GlobalThresholdBeaconDkgSessionV1,
    dealer_key: &GlobalThresholdBeaconDkgRecipientKeyV1,
    recipient_key: &GlobalThresholdBeaconDkgRecipientKeyV1,
    recipient_signer: &KeyPair,
    recipient_secret: &HybridSecretKey,
    dealer_commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
    edge: &GlobalThresholdBeaconDkgEncryptedShareV1,
    accepted_height: u64,
) -> Result<
    (
        DasRenPrivateShare<BeaconPurpose>,
        GlobalThresholdBeaconDkgShareAcceptanceV1,
    ),
    GlobalThresholdBeaconError,
> {
    verify_global_threshold_beacon_dkg_dealer_commitment_signature_v1(
        session,
        dealer_key,
        dealer_commitment,
    )?;
    verify_global_threshold_beacon_dkg_encrypted_share_v1(
        session,
        dealer_commitment,
        dealer_key,
        recipient_key,
        edge,
    )?;
    if recipient_key.validator.public_key() != recipient_signer.public_key()
        || recipient_secret.public().x25519_bytes() != recipient_key.x25519_public_key
        || recipient_secret.public().kyber_bytes() != recipient_key.mlkem768_public_key
        || accepted_height < session.deliveries_end_height
        || accepted_height >= session.acceptances_end_height
    {
        return Err(GlobalThresholdBeaconError::InvalidDkgShareAcceptance);
    }
    let kem = HybridKemCiphertext::from_parts(
        edge.ephemeral_x25519_public_key,
        &edge.mlkem768_ciphertext,
    )
    .map_err(|_| GlobalThresholdBeaconError::InvalidDkgEncryptedShare)?;
    let parameters = adaptive_beacon_parameters(session)?;
    let validated_dealer = verify_adaptive_dealer(&parameters, dealer_commitment)?;
    let share = open_das_ren_private_share(
        &parameters,
        &validated_dealer,
        edge.recipient_index,
        recipient_secret,
        &kem,
        &edge.encrypted_share,
        &global_threshold_beacon_dkg_private_edge_aad_v1(session, edge),
    )?;
    let mut acceptance = GlobalThresholdBeaconDkgShareAcceptanceV1 {
        dealer_index: edge.dealer_index,
        recipient_index: edge.recipient_index,
        dealer_commitment_hash: edge.dealer_commitment_hash,
        encrypted_share_hash: global_threshold_beacon_dkg_encrypted_share_hash_v1(session, edge),
        accepted_height,
        signature: Signature::from_bytes(&[]),
    };
    acceptance.signature = Signature::try_new(
        recipient_signer.private_key(),
        &global_threshold_beacon_dkg_share_acceptance_preimage_v1(session, &acceptance),
    )
    .map_err(|_| GlobalThresholdBeaconError::InvalidDkgShareAcceptance)?;
    Ok((share, acceptance))
}
