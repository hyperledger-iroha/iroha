use super::*;

pub(super) fn localnet_client_identity(
    base_seed: Option<&[u8]>,
    fresh_random_keys: bool,
) -> Result<LocalnetClientIdentity> {
    if fresh_random_keys {
        let seed = base_seed.ok_or_else(|| {
            eyre!("fresh localnet client generation requires process-local OS entropy")
        })?;
        let (public_key, private_key) = generate_account_key_pair(seed.into(), b"client-root")?;
        return Ok(LocalnetClientIdentity {
            account_id: AccountId::new(public_key.clone()),
            public_key,
            private_key: Zeroizing::new(private_key.to_string()),
        });
    }
    let public_key = CLIENT_ACCOUNT_PUBLIC
        .parse::<iroha_crypto::PublicKey>()
        .expect("localnet client public key must parse");
    Ok(LocalnetClientIdentity {
        account_id: AccountId::new(public_key.clone()),
        public_key,
        private_key: Zeroizing::new(CLIENT_ACCOUNT_PRIVATE.to_owned()),
    })
}
