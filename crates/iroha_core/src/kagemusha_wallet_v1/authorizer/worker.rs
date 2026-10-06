//! Bounded publication preparation from the canonical pending index. Scheduling cursors are
//! disposable; committed issuance and the first published voucher are the recovery journal.
use super::*;
use crate::state::StateReadOnly as _;
use zeroize::Zeroize as _;

/// Maximum encoded private keyring admitted by the online service.
pub const LOAD_AUTHORIZER_KEYRING_MAX_BYTES: usize = 65_536;
/// Maximum historical signer bindings retained in one configured online worker.
pub const LOAD_AUTHORIZER_MAX_KEYS: usize = 32;

/// One role-separated online software key in an owner-held Norito custody file.
/// This type deliberately has no `Debug` implementation. Dropping it wipes its scalar.
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::LoadAuthorizerKeyV1")]
pub struct LoadAuthorizerKeyV1 {
    /// Exact scheme root and relation identity.
    pub scheme: KagemushaWalletSchemeV1,
    /// Root-certified historical LoadAuthorization role.
    pub certificate: KagemushaWalletSignerCertificateV1,
    /// Canonical nonzero P-256 scalar, big endian. Never use a handset payment key here.
    pub secret: [u8; 32],
}
impl Drop for LoadAuthorizerKeyV1 {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

/// Canonical private file format for the optional online publisher. Old certificate keys
/// remain in this bounded ring until their already-issued loads have been published.
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::LoadAuthorizerKeyringV1")]
pub struct LoadAuthorizerKeyringV1 {
    /// Exactly one. Other layouts are rejected without fallback.
    pub version: u16,
    /// One to 32 distinct `(scheme, historical certificate)` bindings.
    pub keys: Vec<LoadAuthorizerKeyV1>,
}

struct Slot {
    signer: LoadAuthorizer,
    cursor: Option<Digest>,
}
/// Restartable online preparation owner. It cannot sign caller-supplied bodies and contains
/// no completion boolean: every poll requires a newly source-verified finalized World cut.
pub struct PublicationWorker {
    slots: Vec<Slot>,
    next: usize,
}
impl PublicationWorker {
    /// Require every configured historical signer to belong to this node's genesis identity.
    ///
    /// # Errors
    /// Rejects any key bound to another network, before the worker can start submitting.
    pub fn require_network(&self, network: Digest) -> Result<()> {
        if self
            .slots
            .iter()
            .any(|slot| slot.signer.scheme.network_id != network)
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
    /// Admit a bounded canonical private keyring and verify every scalar, root and role.
    /// No secret is included in an error or a diagnostic representation.
    ///
    /// # Errors
    /// Rejects oversized/noncanonical files, another version, duplicate bindings, invalid
    /// role certificates, or a scalar that does not match its certified public key.
    pub fn from_canonical_keyring(bytes: &[u8]) -> Result<Self> {
        let ring: LoadAuthorizerKeyringV1 =
            storage::decode(bytes, LOAD_AUTHORIZER_KEYRING_MAX_BYTES)?;
        if ring.version != 1 || !(1..=LOAD_AUTHORIZER_MAX_KEYS).contains(&ring.keys.len()) {
            return Err(Error::Binding);
        }
        let mut identities = std::collections::BTreeSet::new();
        let mut slots = Vec::with_capacity(ring.keys.len());
        for record in ring.keys {
            let identity = (
                record.scheme.scheme_id(),
                record.certificate.certificate_digest(),
            );
            if !identities.insert(identity) {
                return Err(Error::Conflict);
            }
            let key = SigningKey::from_slice(&record.secret).map_err(|_| Error::Binding)?;
            slots.push(Slot {
                signer: LoadAuthorizer::new(record.scheme, record.certificate, key)?,
                cursor: None,
            });
        }
        Ok(Self { slots, next: 0 })
    }

    /// Prepare one bounded page, rotating fairly between configured historical signers.
    /// An empty page wraps that signer's cursor on its next turn. Dropping or restarting this
    /// worker loses only scheduling hints; deterministic preparation recovers the same voucher
    /// bytes, and a finalized first publication disappears from the pending index.
    ///
    /// # Errors
    /// Refuses invalid capacity or any unavailable, unfinalized or changed original source.
    /// The caller must submit through ordinary transaction admission and permissions. A queue
    /// error or unknown outcome is not completion and never removes pending ledger work.
    pub fn prepare_page(
        &mut self,
        source: &FinalizedLedger<'_, '_>,
        maximum: usize,
    ) -> Result<Vec<PreparedVoucher>> {
        if !(1..=MAX_PENDING_PAGE).contains(&maximum) {
            return Err(Error::Binding);
        }
        self.require_network(*source.view.network_id().as_bytes())?;
        let index = self.next;
        self.next = (self.next + 1) % self.slots.len();
        let slot = &mut self.slots[index];
        let scheme = slot.signer.scheme.scheme_id();
        let certificate = slot.signer.certificate.certificate_digest();
        let pending = source.pending_publications(scheme, certificate, slot.cursor, maximum)?;
        let mut result = Vec::with_capacity(pending.len());
        for identity in &pending {
            result.push(
                slot.signer
                    .prepare(source, identity.wallet, identity.request)?,
            );
        }
        slot.cursor = pending.last().map(|identity| identity.cursor());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_wallet_v1::tests::{Memory, certificate};

    fn record() -> LoadAuthorizerKeyV1 {
        let registration = Memory::new().registration;
        LoadAuthorizerKeyV1 {
            scheme: registration.scheme,
            certificate: registration.load_authorizer,
            secret: [0x34; 32],
        }
    }
    #[test]
    fn keyring_is_canonical_bounded_role_separated_and_duplicate_free() {
        let good = LoadAuthorizerKeyringV1 {
            version: 1,
            keys: vec![record()],
        };
        let bytes = storage::encode(&good).unwrap();
        let worker = PublicationWorker::from_canonical_keyring(&bytes).unwrap();
        worker
            .require_network(good.keys[0].scheme.network_id)
            .unwrap();
        assert!(worker.require_network([0xff; 32]).is_err());
        for ring in [
            LoadAuthorizerKeyringV1 {
                version: 2,
                keys: vec![record()],
            },
            LoadAuthorizerKeyringV1 {
                version: 1,
                keys: Vec::new(),
            },
            LoadAuthorizerKeyringV1 {
                version: 1,
                keys: vec![record(), record()],
            },
        ] {
            assert!(
                PublicationWorker::from_canonical_keyring(&storage::encode(&ring).unwrap())
                    .is_err()
            );
        }
        for invalid in [[0; 32], [0x35; 32]] {
            let mut wrong = record();
            wrong.secret = invalid;
            let ring = LoadAuthorizerKeyringV1 {
                version: 1,
                keys: vec![wrong],
            };
            assert!(
                PublicationWorker::from_canonical_keyring(&storage::encode(&ring).unwrap())
                    .is_err()
            );
        }
        let mut wrong = record();
        wrong.certificate =
            certificate(&wrong.scheme, KagemushaWalletSignerRoleV1::Enrollment, 0x34);
        let ring = LoadAuthorizerKeyringV1 {
            version: 1,
            keys: vec![wrong],
        };
        assert!(
            PublicationWorker::from_canonical_keyring(&storage::encode(&ring).unwrap()).is_err()
        );
        assert!(
            PublicationWorker::from_canonical_keyring(&vec![
                0;
                LOAD_AUTHORIZER_KEYRING_MAX_BYTES + 1
            ])
            .is_err()
        );
        let mut trailing = bytes;
        trailing.push(0);
        assert!(PublicationWorker::from_canonical_keyring(&trailing).is_err());
    }
}
