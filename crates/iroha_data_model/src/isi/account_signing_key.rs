//! Install, rotate or revoke an account's off-chain signing key
//! (`crate::account::signing_key`, `specs/account_signing_keys.md`).
use super::*;
isi! {
 /// Install, rotate or revoke the off-chain signing key of `account`.
 ///
 /// `Some(key)` installs `key` or replaces the active key with it; `None` revokes the active
 /// key. Every change raises the key generation by one. Only the account itself, in a
 /// transaction its controller authorised, can change its signing key. A key that is a member
 /// of the account's controller, controls an account, or ever was a signing key is refused.
 #[derive(crate::DeriveJsonSerialize,crate::DeriveJsonDeserialize)]
 #[norito(deny_unknown_fields)]
 #[norito_schema(name="iroha_data_model::isi::account_signing_key::SetAccountSigningKeyV1")]
 pub struct SetAccountSigningKeyV1 {
  /// The account whose signing key changes; it must be the transaction authority.
  pub account: crate::account::AccountId,
  /// The new signing key, or `None` to revoke the active one.
  #[norito(required)]
  pub signing_key: Option<iroha_crypto::PublicKey>,
 }
}
impl crate::seal::Instruction for SetAccountSigningKeyV1 {}
impl SetAccountSigningKeyV1 {
    /// Stable wire identity.
    pub const WIRE_ID: &'static str = "iroha.account.signing_key.set.v1";

    /// Install `signing_key` for `account`, or rotate to it.
    #[must_use]
    pub fn install(
        account: crate::account::AccountId,
        signing_key: iroha_crypto::PublicKey,
    ) -> Self {
        Self {
            account,
            signing_key: Some(signing_key),
        }
    }

    /// Revoke the active signing key of `account`.
    #[must_use]
    pub fn revoke(account: crate::account::AccountId) -> Self {
        Self {
            account,
            signing_key: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    #[test]
    fn instruction_roundtrips_through_its_wire_id() {
        let account = crate::account::AccountId::new(
            KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let signing_key = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519)
            .public_key()
            .clone();
        for instruction in [
            SetAccountSigningKeyV1::install(account.clone(), signing_key),
            SetAccountSigningKeyV1::revoke(account),
        ] {
            let boxed = InstructionBox::from(instruction.clone());
            assert_eq!(
                crate::isi::instruction_wire_id(&boxed),
                Some(SetAccountSigningKeyV1::WIRE_ID)
            );
            let bytes = norito::encode_canonical(&instruction).unwrap();
            assert_eq!(
                norito::decode_canonical::<SetAccountSigningKeyV1>(&bytes).unwrap(),
                instruction
            );
        }
    }
}
