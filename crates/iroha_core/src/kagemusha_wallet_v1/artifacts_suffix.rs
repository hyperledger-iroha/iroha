//! Shared native verifier adapter for consensus-owned suffix stages.
use super::*;
use iroha_core_zk::{
    kagemusha_wallet_preparation_v1::UnloadChargeOriginalsV1,
    kagemusha_wallet_suffix_unload_v1::{
        SuffixAnchorInputV1, SuffixCreditWitnessV1, SuffixErrorV1, SuffixPriorUnloadInputV1,
        SuffixReceiveInputV1, SuffixSegmentV1, SuffixStepInputV1, SuffixUnloadClaimsV1,
        SuffixVerifierV1,
    },
};
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::{
    KagemushaWalletSuffixAnchorV1 as SuffixAnchor, KagemushaWalletSuffixCreditWitnessV1 as Credit,
    KagemushaWalletSuffixHeaderV1 as SuffixHeader, KagemushaWalletSuffixOriginalRoleV1 as Role,
    KagemushaWalletSuffixStepKindV1 as StepKind, KagemushaWalletSuffixStepV1 as SuffixStep,
    KagemushaWalletSuffixUnloadV1 as SuffixUnload,
};

/// A refusal of the exact bytes is a deterministic rejection; a local interruption stays a
/// deferral and never becomes a verdict.
fn suffix_error(error: SuffixErrorV1) -> Error {
    if error.is_rejection() {
        Error::Proof
    } else {
        Error::ArtifactsUnavailable
    }
}

fn anchor_input(anchor: &SuffixAnchor) -> SuffixAnchorInputV1<'_> {
    SuffixAnchorInputV1 {
        credential: &anchor.credential,
        certificates: &anchor.certificates,
        lineage: &anchor.lineage,
        state: &anchor.state,
    }
}

fn pair(openings: &[Vec<u8>]) -> Result<[&[u8]; 2]> {
    match openings {
        [low, empty] => Ok([low.as_slice(), empty.as_slice()]),
        _ => Err(Error::Binding),
    }
}

fn step_input(step: &SuffixStep) -> Result<SuffixStepInputV1<'_>> {
    Ok(match &step.kind {
        StepKind::Receive { credit } => {
            let original = |role| step.original(role).ok_or(Error::Binding);
            SuffixStepInputV1::Receive(SuffixReceiveInputV1 {
                before: &step.before,
                after: &step.after,
                statement: &step.statement,
                sigma: &step.sigma,
                receipt: &step.receipt,
                consumed: pair(&step.map_openings)?,
                credit: match credit {
                    Credit::Present { membership } => SuffixCreditWitnessV1::Present { membership },
                    Credit::Inserted { low, empty } => {
                        SuffixCreditWitnessV1::Inserted { low, empty }
                    }
                },
                request: original(Role::Request)?,
                payment: original(Role::Payment)?,
                payer_credential: original(Role::PayerCredential)?,
                payer_certificates: original(Role::PayerCertificates)?,
            })
        }
        StepKind::PriorUnload {
            proof_digest,
            charge,
        } => SuffixStepInputV1::PriorUnload(SuffixPriorUnloadInputV1 {
            before: &step.before,
            after: &step.after,
            statement: &step.statement,
            sigma: &step.sigma,
            receipt: &step.receipt,
            proof_digest: *proof_digest,
            recovery: pair(&step.map_openings)?,
            charge: charge.as_ref().map(|charge| UnloadChargeOriginalsV1 {
                quote: &charge.quote,
                certificate_set: &charge.certificate_set,
            }),
        }),
    })
}

impl crate::kagemusha_wallet_v1::SuffixUnloadVerifier for LedgerVerifier {
    fn verify_begin(
        &self,
        scheme: &KagemushaWalletSchemeV1,
        header: &SuffixHeader,
        anchor: &SuffixAnchor,
    ) -> Result<()> {
        let artifacts = self.mounted(scheme)?;
        #[expect(
            clippy::default_trait_access,
            reason = "the verifier's budget type is not re-exported by iroha_core_zk"
        )]
        let verifier = SuffixVerifierV1::ledger(&artifacts, Default::default());
        let point = verifier
            .verify_anchor(anchor_input(anchor), None)
            .map_err(suffix_error)?;
        if point.identity().0 != header.anchor_sequence {
            return Err(Error::Binding);
        }
        verifier
            .verify_unload_sigma(&header.statement, &header.sigma, None)
            .map_err(suffix_error)?;
        Ok(())
    }
    fn verify_chunk(
        &self,
        scheme: &KagemushaWalletSchemeV1,
        _: &SuffixHeader,
        anchor: &SuffixAnchor,
        start: &KagemushaWalletLineagePublicV1,
        steps: &[SuffixStep],
    ) -> Result<crate::kagemusha_wallet_v1::SuffixChunkVerdict> {
        let artifacts = self.mounted(scheme)?;
        #[expect(
            clippy::default_trait_access,
            reason = "the verifier's budget type is not re-exported by iroha_core_zk"
        )]
        let verifier = SuffixVerifierV1::ledger(&artifacts, Default::default()).batched();
        let credential = verifier
            .authenticate_credential(&anchor.credential, &anchor.certificates)
            .map_err(suffix_error)?;
        let first = steps.first().ok_or(Error::Binding)?;
        let mut cursor = verifier
            .cursor_at_claim(&credential, &first.before, start)
            .map_err(suffix_error)?;
        for step in steps {
            cursor
                .verify_step(&verifier, step_input(step)?, None)
                .map_err(suffix_error)?;
        }
        cursor.settle(&verifier, None).map_err(suffix_error)?;
        let segment = cursor.segment().map_err(suffix_error)?;
        Ok(crate::kagemusha_wallet_v1::SuffixChunkVerdict {
            end: segment.end_adjusted,
            record: segment.to_canonical_bytes().map_err(suffix_error)?,
        })
    }
    fn verify_finalize(
        &self,
        scheme: &KagemushaWalletSchemeV1,
        header: &SuffixHeader,
        authority: &Digest,
        anchor: &SuffixAnchor,
        records: &[Vec<u8>],
        tip: &KagemushaWalletStateV1,
        unload: &SuffixUnload,
    ) -> Result<()> {
        let artifacts = self.mounted(scheme)?;
        #[expect(
            clippy::default_trait_access,
            reason = "the verifier's budget type is not re-exported by iroha_core_zk"
        )]
        let verifier = SuffixVerifierV1::ledger(&artifacts, Default::default());
        // Signatures of the Unload terms, including τ over the authority digest.
        unload.verify_terms(scheme, header, authority)?;
        // The committed Begin row verified this exact anchor; only its row is restored here.
        let point = verifier
            .committed_anchor(anchor_input(anchor))
            .map_err(suffix_error)?;
        let segments = records
            .iter()
            .map(|record| SuffixSegmentV1::decode_canonical(record))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(suffix_error)?;
        let linked = verifier.link(&point, &segments).map_err(suffix_error)?;
        let quote = header
            .charge
            .quote()
            .map(|quote| quote.to_canonical_bytes())
            .transpose()?;
        let certificate_set =
            norito::encode_canonical(&unload.certificates).map_err(|_| Error::Binding)?;
        let claims = SuffixUnloadClaimsV1 {
            authority_digest: *authority,
            anchor_sequence: header.anchor_sequence,
            step_count: header.step_count,
            tip: header.tip,
            tip_sequence: header.tip_sequence,
            tip_adjusted: &header.tip_adjusted,
            statement: &header.statement,
            sigma: &header.sigma,
            after: &unload.after,
            recovery: pair(&unload.map_openings)?,
            charge: quote.as_ref().map(|quote| UnloadChargeOriginalsV1 {
                quote,
                certificate_set: &certificate_set,
            }),
            receipt: &unload.receipt,
        };
        let verified = verifier
            .finish(linked, claims, None)
            .map_err(suffix_error)?;
        let terms = header.payout_terms()?;
        if verified.tip().0 != tip
            || verified.authority_digest() != *authority
            || verified.credential() != &unload.credential
            || verified.installation() != (header.scheme_id, header.artifact_manifest)
            || verified.nullifier() != (terms.nullifier, terms.redeem_ordinal)
            || verified.terms() != (terms.amount, terms.online_charge, terms.charge_quote)
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_verifier_failures_are_deferred_but_invalid_originals_are_rejected() {
        for error in [
            SuffixErrorV1::Cancelled,
            SuffixErrorV1::Unavailable("local budget"),
        ] {
            assert!(matches!(suffix_error(error), Error::ArtifactsUnavailable));
        }
        for error in [
            SuffixErrorV1::Bound("steps"),
            SuffixErrorV1::Rejected("sigma"),
        ] {
            assert!(matches!(suffix_error(error), Error::Proof));
        }
        for error in [
            ArtifactError::Unavailable,
            ArtifactError::Entropy,
            ArtifactError::Cancelled,
        ] {
            assert!(matches!(artifact_error(error), Error::ArtifactsUnavailable));
        }
        for error in [
            ArtifactError::Authority,
            ArtifactError::RuntimeBinding,
            ArtifactError::Inventory,
            ArtifactError::Profile,
            ArtifactError::Proof,
        ] {
            assert!(matches!(artifact_error(error), Error::Proof));
        }
    }
}
