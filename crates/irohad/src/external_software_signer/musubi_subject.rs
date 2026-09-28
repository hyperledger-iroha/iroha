//! Canonical Musubi software-custody subject checks, independent of Unix transport.
use super::protocol::{SignerPurposeBindingV1, SignerRoleV1, SoftwareSignerPublicBindingV1};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    account::{AccountController, AccountId},
    musubi::MusubiProviderBundleVerificationPayloadV1,
    sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1,
};

pub(super) fn subject(
    binding: &SignerPurposeBindingV1,
) -> Result<(AccountId, ProviderIngestCompletionSignerPolicyV1), ()> {
    let SignerPurposeBindingV1::MusubiProviderAttestation {
        owner_account_id,
        policy_id,
        policy_revision,
        predecessor_digest,
        policy_digest,
        ..
    } = binding
    else {
        return Err(());
    };
    if !binding.validates_role(SignerRoleV1::MusubiProviderAttestation) {
        return Err(());
    }
    let owner: AccountId = norito::decode_canonical(owner_account_id).map_err(|_| ())?;
    iroha_data_model::musubi::validate_musubi_account_id_v1(&owner).map_err(|_| ())?;
    if norito::encode_canonical(&owner).map_err(|_| ())? != *owner_account_id {
        return Err(());
    }
    Ok((
        owner,
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: *policy_id,
            revision: *policy_revision,
            predecessor_digest: *predecessor_digest,
            policy_digest: *policy_digest,
        },
    ))
}
pub(super) fn member_weight(owner: &AccountId, key: &PublicKey) -> Option<u32> {
    match owner.controller() {
        AccountController::Single(expected) => (expected == key).then_some(1),
        AccountController::Multisig(policy) => policy
            .members()
            .iter()
            .find(|member| member.public_key() == key)
            .map(|member| u32::from(member.weight())),
    }
}
/// Validate the exact role and controller membership before provisioning or reopening custody.
pub(super) fn validate_key_subject(
    purpose: &SignerPurposeBindingV1,
    key: &PublicKey,
) -> Result<(), ()> {
    let (owner, _) = subject(purpose)?;
    member_weight(&owner, key).ok_or(())?;
    Ok(())
}
/// Derive signing bytes only from a complete validated payload, never an untyped caller digest.
pub(super) fn validated_signing_message(
    binding: &SoftwareSignerPublicBindingV1,
    encoded: &[u8],
) -> Result<Vec<u8>, ()> {
    if binding.role != SignerRoleV1::MusubiProviderAttestation {
        return Err(());
    }
    let payload: MusubiProviderBundleVerificationPayloadV1 =
        norito::decode_canonical(encoded).map_err(|_| ())?;
    payload.validate().map_err(|_| ())?;
    if norito::encode_canonical(&payload).map_err(|_| ())? != encoded {
        return Err(());
    }
    let (owner, policy) = subject(&binding.purpose_binding)?;
    validate_key_subject(&binding.purpose_binding, &binding.public_key)?;
    let SignerPurposeBindingV1::MusubiProviderAttestation {
        network_id,
        provider_id,
        ..
    } = &binding.purpose_binding
    else {
        return Err(());
    };
    if payload.binding.network_id.as_bytes() != network_id
        || payload.binding.provider_id.as_bytes() != provider_id
        || payload.binding.completed_by != owner
        || payload.binding.completion_authority.provider_owner != owner
        || payload.binding.completion_authority.signer_policy != policy
    {
        return Err(());
    }
    Ok(payload.signing_hash().as_ref().to_vec())
}
