//! Transient FI HTTP proof resource held under the installed ordinary Native backend.
//!
//! This Rust-only family borrows the genuine completed app identity and the unretired
//! Native account selection. It authenticates no FI token and grants no FI session,
//! current S/W renewal, offline capability, ledger signature or financial operation.
//! The signed Native inventory supplies the exact FI origin and external prefix.
//!
//! This dedicated original invocation/result holder is process-local, with no
//! restart-safe journal claim. Ordinary HTTP originals may be held privately in
//! memory; no separate per-proof WAL or monetary journal is required by this API.
//! No C/JNI opcode or managed constructor exposes the resource. Its managed transfer
//! must use the fixed original Android key consumer and retain the exact holder,
//! original DER/result and final immutable request/auth-operation/token snapshot.
//! Unknown outcomes cannot reset this holder or lend its input again. A new proof
//! is not automatic transport retry. Existing enrollment and financial W/E journal
//! requirements stay separate and unchanged.
//! TODO: join that genuine held-original transfer and fixed managed transport.

use super::*;
use iroha_core_zk::kagemusha_v1_state::{
    KagemushaOrdinaryFiHttpRequestDataV1 as RequestData,
    KagemushaPreparedOrdinaryFiHttpProofV1 as Prepared,
};
use iroha_data_model::kagemusha::KagemushaAppKeySecurityLevelV1;
use zeroize::Zeroizing;

enum Stage {
    Prepared,
    PlatformStarted,
    Completed,
    Failed,
}

/// Opaque genuine Native HTTP preparation, with no decoder, clone, key/JKT setter
/// or public constructor. Request/token fields are DATA; FI admits the session.
/// This holder deliberately has no `Debug` or serialization implementation.
pub struct KagemushaNativePreparedOrdinaryFiHttpProofV1 {
    backend: Arc<OrdinaryBackend>,
    core_handle: u64,
    prepared: Prepared,
    stage: Stage,
}

fn require_backend_originals(backend: &Arc<OrdinaryBackend>) -> Result<(), Error> {
    let installed = INSTALL.lock().map_err(|_| Error::Rejected)?;
    if !installed.succeeded {
        return Err(Error::Unavailable);
    }
    let active = ACTIVE.get().ok_or(Error::Unavailable)?;
    if !Arc::ptr_eq(active, backend)
        || installed.attempted_path.as_deref() != backend.path.to_str()
    {
        return Err(Error::Rejected);
    }
    // Unlike fresh ledger/financial signing, HTTP app-key possession does not
    // renew or consume a finite S/W observation. Actual retirement remains strict.
    backend.source.recheck_retained_owner_originals(&backend.path)
}

fn require_endpoint_originals(
    backend: &OrdinaryBackend,
    request: &RequestData,
) -> Result<(), Error> {
    let session = backend
        .source
        .native_account_session
        .as_ref()
        .ok_or(Error::Unavailable)?;
    let (origin, prefix) = session.fi_http_endpoint_originals()?;
    if request.fi_https_origin != origin || request.fi_external_path_prefix != prefix {
        return Err(Error::Rejected);
    }
    Ok(())
}

/// Prepare an actual held-key HTTP proof from one immutable final-request/token
/// snapshot. Only the installed backend supplies pending/final identity, selected
/// clock/account/runtime and signed endpoint originals. No mobile authority tuple
/// or decoded FI response can create those resources.
///
/// This constructor is Rust-only. New registration must first acquire an authentic
/// Native generated-ledger-key/account-control resource independently of FI WhoAmI;
/// the existing selected-account resource here does not implement that transition.
/// # Errors
/// Refuses absent/foreign/closed backend or account, retired key, changed endpoint,
/// unavailable final app credential, stale clock, malformed request or OS randomness.
pub fn prepare_kagemusha_native_ordinary_fi_http_proof_v1(
    core_handle: u64,
    request: RequestData,
) -> Result<KagemushaNativePreparedOrdinaryFiHttpProofV1, Error> {
    if core_handle == 0 {
        return Err(Error::Rejected);
    }
    let backend = Arc::clone(ACTIVE.get().ok_or(Error::Unavailable)?);
    require_backend_originals(&backend)?;
    require_endpoint_originals(&backend, &request)?;
    let owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
    if owner.handle != Some(core_handle) {
        return Err(Error::Rejected);
    }
    let pending = owner
        .attempt
        .as_ref()
        .ok_or(Error::Unavailable)?
        .retained_pending_identity()
        .map_err(|_| Error::Rejected)?;
    let possession = owner.possession.as_ref().ok_or(Error::Unavailable)?;
    let prepared = Prepared::prepare(
        pending,
        possession,
        Arc::clone(&backend.source.selected),
        request,
    )
    .map_err(|_| Error::Rejected)?;
    require_endpoint_originals(
        &backend,
        prepared.request_data(pending, possession).map_err(|_| Error::Rejected)?,
    )?;
    require_backend_originals(&backend)?;
    drop(owner);
    Ok(KagemushaNativePreparedOrdinaryFiHttpProofV1 {
        backend,
        core_handle,
        prepared,
        stage: Stage::Prepared,
    })
}

impl KagemushaNativePreparedOrdinaryFiHttpProofV1 {
    fn require_current_with_owner(&self, owner: &Owner) -> Result<(), Error> {
        if matches!(self.stage, Stage::Failed) || owner.handle != Some(self.core_handle) {
            return Err(Error::Rejected);
        }
        require_backend_originals(&self.backend)?;
        let pending = owner
            .attempt
            .as_ref()
            .ok_or(Error::Unavailable)?
            .retained_pending_identity()
            .map_err(|_| Error::Rejected)?;
        let possession = owner.possession.as_ref().ok_or(Error::Unavailable)?;
        let request = self.prepared.request_data(pending, possession).map_err(|_| Error::Rejected)?;
        require_endpoint_originals(&self.backend, request)?;
        require_backend_originals(&self.backend)
    }

    /// Recheck genuine current Native retirement, descriptor, final identity and
    /// proof interval. This supplies no transport dispatch or session admission.
    /// # Errors
    /// Refuses closed/replaced/retired originals, failed holder or expired proof.
    pub fn require_current(&self) -> Result<(), Error> {
        let owner = self.backend.owner.lock().map_err(|_| Error::Rejected)?;
        self.require_current_with_owner(&owner)
    }

    /// Compare the caller's privately held final transport snapshot with the exact
    /// token/request DATA retained by Core. No trimming, token reread, URL rebuild,
    /// normalization or token parsing occurs. The hash is correlation DATA; DPoP
    /// does not claim to sign the query, body, request ID or idempotency originals.
    /// # Errors
    /// Refuses any mutation, foreign operation correlation or stale actual owner.
    pub fn require_final_request_originals(
        &self,
        method: &str,
        htu: &str,
        access_token: &[u8],
        original_request_id: &str,
        operation_reference: &[u8],
        request_snapshot_sha256: &[u8; 32],
    ) -> Result<(), Error> {
        let owner = self.backend.owner.lock().map_err(|_| Error::Rejected)?;
        self.require_current_with_owner(&owner)?;
        let pending = owner.attempt.as_ref().ok_or(Error::Unavailable)?
            .retained_pending_identity().map_err(|_| Error::Rejected)?;
        let possession = owner.possession.as_ref().ok_or(Error::Unavailable)?;
        let request = self.prepared.request_data(pending, possession).map_err(|_| Error::Rejected)?;
        if request.method != method || request.htu != htu
            || request.access_token.as_slice() != access_token
            || request.original_request_id != original_request_id
            || request.operation_reference.as_slice() != operation_reference
            || &request.request_snapshot_sha256 != request_snapshot_sha256
        {
            return Err(Error::Rejected);
        }
        self.require_current_with_owner(&owner)
    }

    /// Closed purpose header DATA from the genuinely retained Core request.
    /// # Errors
    /// Refuses changed/retired Native originals or proof interval.
    pub fn purpose_headers(&self) -> Result<(&'static str, &'static str), Error> {
        let owner = self.backend.owner.lock().map_err(|_| Error::Rejected)?;
        self.require_current_with_owner(&owner)?;
        let pending = owner.attempt.as_ref().ok_or(Error::Unavailable)?
            .retained_pending_identity().map_err(|_| Error::Rejected)?;
        let possession = owner.possession.as_ref().ok_or(Error::Unavailable)?;
        let purpose = self.prepared.request_data(pending, possession)
            .map_err(|_| Error::Rejected)?.purpose;
        self.require_current_with_owner(&owner)?;
        Ok((purpose.authorization_scheme(), purpose.proof_header_name()))
    }

    /// Core-derived held-key JKT DATA. It neither accepts an offered JKT nor admits FI.
    /// # Errors
    /// Refuses stale Native originals or proof interval.
    pub fn jkt(&self) -> Result<String, Error> {
        let owner = self.backend.owner.lock().map_err(|_| Error::Rejected)?;
        self.require_current_with_owner(&owner)?;
        let pending = owner.attempt.as_ref().ok_or(Error::Unavailable)?
            .retained_pending_identity().map_err(|_| Error::Rejected)?;
        let possession = owner.possession.as_ref().ok_or(Error::Unavailable)?;
        let original = self.prepared.jkt(pending, possession).map_err(|_| Error::Rejected)?.to_owned();
        self.require_current_with_owner(&owner)?;
        Ok(original)
    }

    /// Native-random original proof UUID DATA, with no caller nonce or retry grant.
    /// # Errors
    /// Refuses stale Native originals or proof interval.
    pub fn jti(&self) -> Result<String, Error> {
        let owner = self.backend.owner.lock().map_err(|_| Error::Rejected)?;
        self.require_current_with_owner(&owner)?;
        let pending = owner.attempt.as_ref().ok_or(Error::Unavailable)?
            .retained_pending_identity().map_err(|_| Error::Rejected)?;
        let possession = owner.possession.as_ref().ok_or(Error::Unavailable)?;
        let original = self.prepared.jti(pending, possession).map_err(|_| Error::Rejected)?.to_owned();
        self.require_current_with_owner(&owner)?;
        Ok(original)
    }

    /// Lend the original Core-prepared Android key/input once in this held process.
    /// The actual fixed hardware consumer is still required; no signer callback or
    /// caller alias enters this method. This start has no restart-safe fsync claim.
    /// # Errors
    /// Refuses a previous start/completion/failure, retirement or changed originals.
    pub fn begin_original_key_loan(&mut self) -> Result<KagemushaNativeOrdinaryFiHttpKeyLoanV1<'_>, Error> {
        let owner = self.backend.owner.lock().map_err(|_| Error::Rejected)?;
        self.require_current_with_owner(&owner)?;
        if !matches!(self.stage, Stage::Prepared) {
            return Err(Error::Rejected);
        }
        // A projection or postcheck failure after this mark permanently freezes
        // this Native holder. It cannot expose another possible hardware call.
        self.stage = Stage::Failed;
        let pending = owner.attempt.as_ref().ok_or(Error::Unavailable)?
            .retained_pending_identity().map_err(|_| Error::Rejected)?;
        let possession = owner.possession.as_ref().ok_or(Error::Unavailable)?;
        let core = self.prepared.begin_platform(pending, possession).map_err(|_| Error::Rejected)?;
        let signing_input = Zeroizing::new(core.signing_input().map_err(|_| Error::Rejected)?.to_vec());
        let alias = core.original_alias().map_err(|_| Error::Rejected)?.to_owned();
        let point = *core.public_key().map_err(|_| Error::Rejected)?;
        let key_id = core.attested_key_id().map_err(|_| Error::Rejected)?;
        let challenge = core.attestation_challenge().map_err(|_| Error::Rejected)?;
        let security_level = core.security_level().map_err(|_| Error::Rejected)?;
        drop(core);
        require_backend_originals(&self.backend)?;
        require_endpoint_originals(&self.backend,
            self.prepared.request_data(pending, possession).map_err(|_| Error::Rejected)?)?;
        self.stage = Stage::PlatformStarted;
        drop(owner);
        Ok(KagemushaNativeOrdinaryFiHttpKeyLoanV1 {
            prepared: self,
            signing_input,
            alias,
            point,
            key_id,
            challenge,
            security_level,
        })
    }

    /// Verify complete original canonical platform DER against the actual held
    /// P256 point/input through Core, preserving original r/s in ES256. This returns
    /// confidential proof DATA only. The product must recheck its privately retained
    /// final request/token and actual Native retirement immediately before dispatch.
    /// Exact repeated completion reads the same result and authorizes no second call.
    /// # Errors
    /// Refuses another DER/key/input, unstarted operation, retirement or stale proof.
    /// An uncertain failure permanently freezes this process-held resource.
    pub fn complete_original(&mut self, original_der: &[u8]) -> Result<Zeroizing<String>, Error> {
        let owner = self.backend.owner.lock().map_err(|_| Error::Rejected)?;
        self.require_current_with_owner(&owner)?;
        if !matches!(self.stage, Stage::PlatformStarted | Stage::Completed) {
            return Err(Error::Rejected);
        }
        self.stage = Stage::Failed;
        let pending = owner.attempt.as_ref().ok_or(Error::Unavailable)?
            .retained_pending_identity().map_err(|_| Error::Rejected)?;
        let possession = owner.possession.as_ref().ok_or(Error::Unavailable)?;
        let compact = Zeroizing::new(self.prepared.complete_original(pending, possession, original_der)
            .map_err(|_| Error::Rejected)?.to_owned());
        require_endpoint_originals(&self.backend,
            self.prepared.request_data(pending, possession).map_err(|_| Error::Rejected)?)?;
        require_backend_originals(&self.backend)?;
        self.stage = Stage::Completed;
        Ok(compact)
    }
}

/// Borrowed original-key loan, constructed only after Native and Core mark start.
/// No clone, decoder, public constructor or Debug exposes a factory/secret dump.
/// Its lifetime prevents completing or moving its held Native preparation while
/// borrowed; every projection repeats actual current/retirement/interval checks.
pub struct KagemushaNativeOrdinaryFiHttpKeyLoanV1<'a> {
    prepared: &'a KagemushaNativePreparedOrdinaryFiHttpProofV1,
    signing_input: Zeroizing<Vec<u8>>,
    alias: String,
    point: [u8; 65],
    key_id: [u8; 32],
    challenge: [u8; 32],
    security_level: KagemushaAppKeySecurityLevelV1,
}
impl KagemushaNativeOrdinaryFiHttpKeyLoanV1<'_> {
    /// Recheck the exact unretired source, held account/key and started operation.
    /// # Errors
    /// Refuses a completed, changed, retired or expired original holder.
    pub fn require_current(&self) -> Result<(), Error> {
        if !matches!(self.prepared.stage, Stage::PlatformStarted) {
            return Err(Error::Rejected);
        }
        self.prepared.require_current()
    }
    /// Exact Core JOSE ASCII input for the sole original SHA256withECDSA call.
    /// # Errors
    /// Refuses stale Native custody.
    pub fn signing_input(&self) -> Result<&[u8], Error> { self.require_current()?; Ok(self.signing_input.as_slice()) }
    /// Actual original generation alias; callers cannot substitute another alias.
    /// # Errors
    /// Refuses stale Native custody.
    pub fn original_alias(&self) -> Result<&str, Error> { self.require_current()?; Ok(&self.alias) }
    /// Authentic original uncompressed P256 point.
    /// # Errors
    /// Refuses stale Native custody.
    pub fn public_key(&self) -> Result<&[u8; 65], Error> { self.require_current()?; Ok(&self.point) }
    /// SHA256 of that genuine original point.
    /// # Errors
    /// Refuses stale Native custody.
    pub fn attested_key_id(&self) -> Result<[u8; 32], Error> { self.require_current()?; Ok(self.key_id) }
    /// Original generation-time certificate challenge.
    /// # Errors
    /// Refuses stale Native custody.
    pub fn attestation_challenge(&self) -> Result<[u8; 32], Error> { self.require_current()?; Ok(self.challenge) }
    /// Genuine TEE/StrongBox metadata, without a single-use requirement.
    /// # Errors
    /// Refuses stale Native custody.
    pub fn security_level(&self) -> Result<KagemushaAppKeySecurityLevelV1, Error> { self.require_current()?; Ok(self.security_level) }
}
