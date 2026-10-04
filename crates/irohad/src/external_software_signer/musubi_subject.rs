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
) -> Result<(AccountId, AccountId, ProviderIngestCompletionSignerPolicyV1), ()> {
    let SignerPurposeBindingV1::MusubiProviderAttestation {
        provider_owner_account_id,
        completion_signer_account_id,
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
    let decode_account = |encoded: &[u8]| -> Result<AccountId, ()> {
        let account: AccountId = norito::decode_canonical(encoded).map_err(|_| ())?;
        iroha_data_model::musubi::validate_musubi_account_id_v1(&account).map_err(|_| ())?;
        if norito::encode_canonical(&account).map_err(|_| ())? != encoded {
            return Err(());
        }
        Ok(account)
    };
    Ok((
        decode_account(provider_owner_account_id)?,
        decode_account(completion_signer_account_id)?,
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
    let (_, signer, _) = subject(purpose)?;
    member_weight(&signer, key).ok_or(())?;
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
    let (owner, signer, policy) = subject(&binding.purpose_binding)?;
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
        || payload.binding.completed_by != signer
        || payload.binding.completion_authority.completion_signer != signer
        || payload.binding.completion_authority.provider_owner != owner
        || payload.binding.completion_authority.signer_policy != policy
    {
        return Err(());
    }
    Ok(payload.signing_hash().as_ref().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    #[test]
    fn purpose_keeps_both_canonical_accounts_and_only_completion_controller_signs() {
        let owner_key = KeyPair::from_seed(vec![71; 32], Algorithm::Ed25519);
        let signer_key = KeyPair::from_seed(vec![72; 32], Algorithm::Ed25519);
        let owner = AccountId::new(owner_key.public_key().clone());
        let signer = AccountId::new(signer_key.public_key().clone());
        let binding = SignerPurposeBindingV1::MusubiProviderAttestation {
            network_id: [1; 32],
            provider_id: [2; 32],
            provider_owner_account_id: norito::encode_canonical(&owner).unwrap(),
            completion_signer_account_id: norito::encode_canonical(&signer).unwrap(),
            policy_id: [3; 32],
            policy_revision: 1,
            predecessor_digest: None,
            policy_digest: [4; 32],
        };
        let (actual_owner, actual_signer, policy) = subject(&binding).unwrap();
        assert_eq!(actual_owner, owner);
        assert_eq!(actual_signer, signer);
        assert_ne!(actual_owner, actual_signer);
        assert_eq!(policy.revision, 1);
        assert!(validate_key_subject(&binding, signer_key.public_key()).is_ok());
        assert!(validate_key_subject(&binding, owner_key.public_key()).is_err());
        assert!(subject(&SignerPurposeBindingV1::NativeOrPromotion).is_err());
        for which in 0..2 {
            let mut changed = binding.clone();
            let SignerPurposeBindingV1::MusubiProviderAttestation {
                provider_owner_account_id,
                completion_signer_account_id,
                ..
            } = &mut changed
            else {
                unreachable!()
            };
            if which == 0 {
                provider_owner_account_id.push(0);
            } else {
                completion_signer_account_id.push(0);
            }
            assert!(subject(&changed).is_err());
            assert!(validate_key_subject(&changed, signer_key.public_key()).is_err());
        }
    }
}
