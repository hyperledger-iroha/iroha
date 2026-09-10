// One explicit owner for a returned grant until its authoritative publication is settled.
// Included at module scope; no provider operation is performed by Drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GrantPublicationDispositionV1 {
    Unpublished,
    Committed,
    Uncertain,
}
#[derive(Debug)]
struct EvidenceViewerPersistenceFailureV1 {
    error: EvidenceViewerErrorV1,
    publication: GrantPublicationDispositionV1,
}
impl From<EvidenceViewerErrorV1> for EvidenceViewerPersistenceFailureV1 {
    fn from(error: EvidenceViewerErrorV1) -> Self {
        Self {
            error,
            publication: GrantPublicationDispositionV1::Unpublished,
        }
    }
}
impl From<EvidenceViewerPersistenceFailureV1> for EvidenceViewerErrorV1 {
    fn from(failure: EvidenceViewerPersistenceFailureV1) -> Self {
        failure.error
    }
}
struct PendingEvidenceViewerGrantV1 {
    token: OpaqueEvidenceViewerSecretV1,
    disposition: GrantPublicationDispositionV1,
}
impl PendingEvidenceViewerGrantV1 {
    fn persist(
        &mut self,
        service: &EvidenceViewerServiceV1,
        state: &mut EvidenceViewerStateV1,
    ) -> Result<(), EvidenceViewerErrorV1> {
        match service.persist_locked(state) {
            Ok(()) => {
                self.disposition = GrantPublicationDispositionV1::Committed;
                Ok(())
            }
            Err(failure) => {
                self.disposition = failure.publication;
                Err(failure.error)
            }
        }
    }
}
impl QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerGrantBoundaryV1> {
    // The same captured provider issued this exact credential. Retirement only may still run
    // after qualification drift; it cannot authorize issuance, verification or any other target.
    // The raw provider may reject/fail (including its own remote transport fence), which remains
    // unresolved until expiry. Returning an error never means that the credential was reclaimed.
    fn retire_owned_unpublished_grant(
        &self,
        token: &OpaqueEvidenceViewerSecretV1,
    ) -> Result<(), EvidenceViewerExternalErrorV1> {
        self.provider.revoke(token.digest())
    }
    fn issue_for_publication<T>(
        &self,
        claims: &EvidenceViewerGrantClaimsV1,
        publish: impl FnOnce(&mut PendingEvidenceViewerGrantV1) -> Result<T, EvidenceViewerErrorV1>,
    ) -> Result<(OpaqueEvidenceViewerSecretV1, T), EvidenceViewerErrorV1> {
        self.revalidate()
            .map_err(|_| EvidenceViewerErrorV1::RuntimeUnavailable)?;
        let token = self.provider.issue(claims).map_err(map_external_error)?;
        // Establish ownership before post-call qualification can reject the returned credential.
        let mut pending = PendingEvidenceViewerGrantV1 {
            token,
            disposition: GrantPublicationDispositionV1::Unpublished,
        };
        let result = self
            .revalidate()
            .map_err(|_| EvidenceViewerErrorV1::RuntimeUnavailable)
            .and_then(|()| publish(&mut pending));
        match result {
            Ok(value) if pending.disposition == GrantPublicationDispositionV1::Committed => {
                Ok((pending.token, value))
            }
            result => {
                // The callback has returned: all service state guards have been released. This
                // is one bounded-count, exact-issued retirement attempt, not I/O hidden in Drop.
                if pending.disposition == GrantPublicationDispositionV1::Unpublished
                    && self.retire_owned_unpublished_grant(&pending.token).is_err()
                {
                    iroha_logger::warn!(
                        "unpublished evidence-viewer grant revocation unavailable; provider expiry remains required"
                    );
                }
                // Keep committed/maybe-committed grants intact while suppressing caller output.
                // No secret is persisted for reconciliation; exact authority identity and expiry
                // remain the existing recovery boundary. A panic/crash is not a settled return.
                Err(result
                    .err()
                    .unwrap_or(EvidenceViewerErrorV1::CheckpointUnavailable))
            }
        }
    }
}
fn fresh_grant_issuance_nonce() -> Result<[u8; 32], EvidenceViewerErrorV1> {
    use rand::TryRngCore as _;
    let mut nonce = [0; 32];
    for _ in 0..4 {
        rand::rngs::OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| EvidenceViewerErrorV1::RuntimeUnavailable)?;
        if !is_zero_digest(nonce) {
            return Ok(nonce);
        }
    }
    Err(EvidenceViewerErrorV1::RuntimeUnavailable)
}
