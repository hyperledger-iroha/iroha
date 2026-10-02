//! Canonical issuer-side C reserve/publication custody, separate from signing authority.
//!
//! The genuine Native parent must independently admit the current FI customer, exact request
//! and installed selected owner, and retain the dedicated Ed C signing descriptor. This component
//! cannot construct that parent, accept a public owner DTO, sign arbitrary bytes, issue a device
//! credential or grant money. It returns a model-owned purpose-specific request for the protected
//! C encoder and accepts only its exact verified original. No HTTP route installs this object.
//! TODO: Join the actual current FI admission and installed Native parent, and authenticate its
//! Python `core_preparation_public_key` projection against this same selected issuer public key.
//! The FD12 app-attestation key is a different purpose and cannot fill that projection.

use super::super::private_journal::{PrivateJournal, PrivateJournalFormat};
use super::{
    Custody, KagemushaOrdinaryIdentityErrorV1, KagemushaOrdinaryPreparationCarrierV1,
    KagemushaOrdinaryPreparationSelectedOriginalsV1, Rejected, Result,
};
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryAppEnrollmentChallengeV1, KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
};
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};
use std::{path::Path, sync::Arc};

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-issuer-c.norito.wal",
    magic: b"KGMCISS1",
    hash_domain: b"iroha:kagemusha:v1:ordinary-issuer-c-journal\0",
    maximum_payload_bytes: 2048,
};

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::IssuerPreparationRecordV1")]
enum Record {
    Reserved {
        original_body_sha256: [u8; 32],
        challenge: KagemushaOrdinaryAppEnrollmentChallengeV1,
        signing_request: Vec<u8>,
    },
    Published {
        signed_original: Vec<u8>,
        authenticated_at_ms: u64,
    },
}
fn encode(record: &Record) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(record).map_err(|_| Rejected)?;
    if bytes.is_empty() || bytes.len() > FORMAT.maximum_payload_bytes as usize {
        return Err(Rejected);
    }
    Ok(bytes)
}
fn decode(raw: &[u8]) -> Result<Record> {
    if raw.is_empty() || raw.len() > FORMAT.maximum_payload_bytes as usize {
        return Err(Rejected);
    }
    let value: Record =
        norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
            .map_err(|_| Rejected)?;
    if encode(&value)? != raw {
        return Err(Rejected);
    }
    Ok(value)
}

/// Held original issuer C reservation. It grants byte custody, not a FI or signing capability.
///
/// There is no decoder or generic signing callback. The installing Native caller must retain
/// actual FI admission and dedicated protected Ed C signer custody through every operation.
pub struct KagemushaOrdinaryIssuerPreparationAttemptV1 {
    selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
    carrier: KagemushaOrdinaryPreparationCarrierV1,
    original_body_sha256: [u8; 32],
    challenge: KagemushaOrdinaryAppEnrollmentChallengeV1,
    signing_request: Vec<u8>,
    reserved_record: Vec<u8>,
    published: Option<(Vec<u8>, u64, Vec<u8>)>,
    journal: PrivateJournal,
}
impl KagemushaOrdinaryIssuerPreparationAttemptV1 {
    fn path(
        root: &Path,
        selected: &KagemushaOrdinaryPreparationSelectedOriginalsV1,
        carrier: &KagemushaOrdinaryPreparationCarrierV1,
    ) -> Result<std::path::PathBuf> {
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-issuer-c-slot\0");
        hash.update(selected.enrollment_id()?);
        hash.update(carrier.client_nonce);
        Ok(root.join(hex::encode(hash.finalize())))
    }

    /// Durably fix the sole C body before a dedicated protected signer may be invoked.
    /// The current time and all authority fields derive from the existing admitted Native owner;
    /// only the original client correlation fields arrive in the shared six-field carrier.
    /// The caller supplies the exact body digest from its retained FI request admission.
    /// Fresh authenticated HTTP nonce retries can recover that unchanged attempt/body.
    /// # Errors
    /// Rejects substituted originals, an existing slot, invalid current policy or uncertain WAL.
    pub fn reserve(
        root: &Path,
        selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
        carrier: KagemushaOrdinaryPreparationCarrierV1,
        original_body_sha256: [u8; 32],
    ) -> Result<Self> {
        if original_body_sha256 == [0; 32] {
            return Err(Rejected);
        }
        let now = selected.trusted_time_ms()?;
        let mut server_nonce = [0; 32];
        OsRng
            .try_fill_bytes(&mut server_nonce)
            .map_err(|_| Custody)?;
        let challenge = selected.issuer_challenge_for_carrier(
            &carrier,
            server_nonce,
            now,
            selected.preparation_expiry_at(now)?,
        )?;
        let signing_request = challenge
            .to_signing_request(selected.preparation_issuer_key()?)
            .map_err(|_| Rejected)?;
        let reserved_record = encode(&Record::Reserved {
            original_body_sha256,
            challenge: challenge.clone(),
            signing_request: signing_request.clone(),
        })?;
        let mut journal =
            PrivateJournal::create_new(&Self::path(root, &selected, &carrier)?, FORMAT)
                .map_err(|_| Custody)?;
        journal
            .append(&reserved_record)
            .map_err(|_| KagemushaOrdinaryIdentityErrorV1::UnknownOutcome)?;
        let owner = Self {
            selected,
            carrier,
            original_body_sha256,
            challenge,
            signing_request,
            reserved_record,
            published: None,
            journal,
        };
        owner.recheck()?;
        Ok(owner)
    }

    /// Reopen exactly the same slot without new randomness, C interval or replacement signer.
    /// Recovery after C expiry can return only a previously published original; it cannot sign.
    /// # Errors
    /// Rejects missing/truncated/extra records, another request or changed native selected policy.
    pub fn open_existing(
        root: &Path,
        selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
        carrier: KagemushaOrdinaryPreparationCarrierV1,
        original_body_sha256: [u8; 32],
    ) -> Result<Self> {
        if original_body_sha256 == [0; 32] {
            return Err(Rejected);
        }
        let mut journal =
            PrivateJournal::open_existing(&Self::path(root, &selected, &carrier)?, FORMAT)
                .map_err(|_| Custody)?;
        let (sequence, raw) = journal.replay_next().map_err(|_| Custody)?.ok_or(Custody)?;
        let Record::Reserved {
            original_body_sha256: saved,
            challenge,
            signing_request,
        } = decode(&raw)?
        else {
            return Err(Rejected);
        };
        if sequence != 0 || saved != original_body_sha256 {
            return Err(Rejected);
        }
        let mut published = None;
        if let Some((sequence, raw)) = journal.replay_next().map_err(|_| Custody)? {
            let Record::Published {
                signed_original,
                authenticated_at_ms,
            } = decode(&raw)?
            else {
                return Err(Rejected);
            };
            if sequence != 1 {
                return Err(Rejected);
            }
            published = Some((signed_original, authenticated_at_ms, raw));
        }
        // One extra bounded sentinel observation refuses every suffix; it never skips frames.
        if journal.replay_next().map_err(|_| Custody)?.is_some() {
            return Err(Rejected);
        }
        let reserved_record = encode(&Record::Reserved {
            original_body_sha256: saved,
            challenge: challenge.clone(),
            signing_request: signing_request.clone(),
        })?;
        let owner = Self {
            selected,
            carrier,
            original_body_sha256,
            challenge,
            signing_request,
            reserved_record,
            published,
            journal,
        };
        owner.recheck()?;
        Ok(owner)
    }

    fn recheck(&self) -> Result<()> {
        let now = self.selected.trusted_time_ms()?;
        let expected = self.selected.issuer_challenge_for_carrier(
            &self.carrier,
            self.challenge.server_nonce,
            self.challenge.issued_at_ms,
            self.challenge.expires_at_ms,
        )?;
        if now < self.challenge.issued_at_ms
            || expected != self.challenge
            || expected
                .to_signing_request(self.selected.preparation_issuer_key()?)
                .map_err(|_| Rejected)?
                != self.signing_request
        {
            return Err(Custody);
        }
        let expected_records = 1 + u64::from(self.published.is_some());
        if self
            .journal
            .recovery_prefix()
            .map_err(|_| Custody)?
            .sequence
            != expected_records
        {
            return Err(Custody);
        }
        self.journal
            .scan_complete(|sequence, raw| {
                let expected = if sequence == 0 {
                    self.reserved_record.as_slice()
                } else {
                    self.published
                        .as_ref()
                        .ok_or(super::super::private_journal::PrivateJournalError::Corrupt)?
                        .2
                        .as_slice()
                };
                if raw != expected {
                    return Err(super::super::private_journal::PrivateJournalError::Corrupt);
                }
                Ok(())
            })
            .map_err(|_| Custody)?;
        if let Some((original, admitted_at, _)) = &self.published {
            if now < *admitted_at {
                return Err(Custody);
            }
            KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(original)
                .map_err(|_| Custody)?
                .authenticate(
                    self.selected.preparation_issuer_key()?,
                    &self.challenge,
                    *admitted_at,
                )
                .map_err(|_| Custody)?;
        }
        Ok(())
    }

    /// Project the original exact purpose-specific C encoder request after current recheck.
    /// This is data only; the real parent separately owns private signer/caller admission.
    /// # Errors
    /// Rejects published, expired, changed or uncertain original custody.
    pub fn signing_request(&self) -> Result<&[u8]> {
        self.recheck()?;
        if self.published.is_some()
            || self.selected.trusted_time_interval()?.upper_ms() >= self.challenge.expires_at_ms
        {
            return Err(Custody);
        }
        Ok(&self.signing_request)
    }

    /// Publish only the exact model original signed by the independently selected Ed C issuer.
    /// Another scope/key/signature cannot enter the durable slot. Identical lost-result retry
    /// returns the retained original; no new signer effect or challenge is requested here.
    /// # Errors
    /// Rejects changed body/current policy, signature, expired fresh C or uncertain append.
    pub fn publish_original(
        &mut self,
        original: &[u8],
        original_body_sha256: [u8; 32],
    ) -> Result<Vec<u8>> {
        self.recheck()?;
        if original_body_sha256 != self.original_body_sha256 {
            return Err(Rejected);
        }
        if let Some((saved, _, _)) = &self.published {
            if saved.as_slice() != original {
                return Err(Rejected);
            }
            return Ok(saved.clone());
        }
        let now = self.selected.trusted_time_ms()?;
        let signed =
            KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(original)
                .map_err(|_| Rejected)?;
        self.selected.trusted_time_interval()?.check_both(|point| {
            signed
                .authenticate(
                    self.selected.preparation_issuer_key()?,
                    &self.challenge,
                    point,
                )
                .map(|_| ())
                .map_err(|_| Rejected)
        })?;
        self.recheck()?;
        let raw = encode(&Record::Published {
            signed_original: original.to_vec(),
            authenticated_at_ms: now,
        })?;
        self.journal
            .append(&raw)
            .map_err(|_| KagemushaOrdinaryIdentityErrorV1::UnknownOutcome)?;
        self.published = Some((original.to_vec(), now, raw));
        self.recheck()?;
        Ok(original.to_vec())
    }

    /// Recover a previously durably authenticated original under the current selected policy.
    /// This does not renew an expired C or authorize a new platform/key/signing invocation.
    /// # Errors
    /// Rejects missing publication, changed native scope or lost original journal custody.
    pub fn published_original(&self, original_body_sha256: [u8; 32]) -> Result<&[u8]> {
        self.recheck()?;
        if original_body_sha256 != self.original_body_sha256 {
            return Err(Rejected);
        }
        self.published
            .as_ref()
            .map(|row| row.0.as_slice())
            .ok_or(Custody)
    }
}

#[cfg(test)]
#[path = "issuer_preparation_tests.rs"]
mod tests;
