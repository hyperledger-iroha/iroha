//! Private temporary artifacts and canonical test client identities.

use super::*;

/// Temporary root whose generated-artifact child has native owner-only custody.
pub(super) struct PrivateTempDir {
    _temporary: tempfile::TempDir,
    path: PathBuf,
}

impl PrivateTempDir {
    /// Return the canonical private child used by generator fixtures.
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

/// Create the same private artifact boundary used by a real managed generation.
pub(super) fn private_tempdir() -> std::io::Result<PrivateTempDir> {
    let temporary = tempfile::tempdir()?;
    let private = iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("private"))?;
    Ok(PrivateTempDir {
        path: private.path().to_path_buf(),
        _temporary: temporary,
    })
}

pub(super) fn localnet_client_identity(
    base_seed: Option<&[u8]>,
    derive_from_seed: bool,
) -> Result<LocalnetClientIdentity> {
    if derive_from_seed {
        let seed = base_seed
            .ok_or_else(|| eyre!("derived localnet client fixture requires seed material"))?;
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
