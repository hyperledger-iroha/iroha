//! Deterministic suffix receipt/completion assembly behind the existing Advance boundary.
//! This owner does no signing itself. Fresh callers must possess the genuine Native review;
//! recovery may rebuild it only for the provider-selected exact frozen transition.
use super::native_frames::{NativeCapsuleV1, NativeCompletionV1};
use super::*;
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::{
    KagemushaWalletSuffixHeaderV1, KagemushaWalletSuffixUnloadV1, SuffixCompletionRecordV1,
    SuffixRecoveryCapsuleV1,
};

fn model<T>(
    read: impl FnOnce() -> Result<T, KagemushaWalletValidationErrorV1>,
) -> Result<T, ProviderError> {
    crate::kagemusha_wallet_advance_v1::decode_resource::read_model(read, "suffix owner")
}

pub(in crate::kagemusha_wallet_state_v1) struct SuffixTransitionOwnerV1 {
    credential: KagemushaWalletCredentialV1,
    certificates: KagemushaWalletCertificateSetV1,
}
impl SuffixTransitionOwnerV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn new(
        credential: KagemushaWalletCredentialV1,
        certificates: KagemushaWalletCertificateSetV1,
    ) -> Self {
        Self {
            credential,
            certificates,
        }
    }

    fn require(
        &self,
        capsule: &SuffixRecoveryCapsuleV1,
        digest: &[u8; 32],
    ) -> Result<(), ProviderError> {
        let body = capsule.body();
        model(|| body.validate())?;
        model(|| self.credential.validate())?;
        model(|| {
            body.successor_state
                .validate_for_credential(&self.credential)
        })?;
        model(|| body.statement.validate_for_credential(&self.credential))?;
        model(|| self.certificates.validate())?;
        if capsule.custody_digest() != *digest || body.wallet_id != self.credential.body.wallet_id {
            return Err(ProviderError::Invalid {
                field: "suffix owner capsule binding",
            });
        }
        let header = model(|| {
            KagemushaWalletSuffixHeaderV1::decode_canonical(&body.authority_header, &body.scheme_id)
        })?;
        if model(|| kagemusha_wallet_account_digest_v1(&header.account))?
            != self.credential.body.account_digest
        {
            return Err(ProviderError::Invalid {
                field: "suffix owner destination",
            });
        }
        let mut expected = vec![(
            self.credential.body.issuer_certificate,
            KagemushaWalletSignerRoleV1::Enrollment,
        )];
        if let Some(quote) = header.charge.quote() {
            expected.push((
                quote.body.signer_certificate,
                KagemushaWalletSignerRoleV1::RegulatoryPolicy,
            ));
        }
        let mut digests: Vec<_> = expected.iter().map(|(digest, _)| *digest).collect();
        digests.sort_unstable();
        digests.dedup();
        if self.certificates.digests() != digests {
            return Err(ProviderError::Invalid {
                field: "suffix owner exact certificates",
            });
        }
        for (digest, role) in expected {
            if model(|| self.certificates.certificate(&digest, role))?
                .body
                .scheme_id
                != body.scheme_id
            {
                return Err(ProviderError::Invalid {
                    field: "suffix owner certificate scheme",
                });
            }
        }
        Ok(())
    }
}
impl KagemushaWalletTransitionOwnerV1<SuffixRecoveryCapsuleV1, SuffixCompletionRecordV1>
    for SuffixTransitionOwnerV1
{
    fn receipt_body(
        &self,
        capsule: &SuffixRecoveryCapsuleV1,
        digest: &[u8; 32],
    ) -> Result<Vec<u8>, ProviderError> {
        self.require(capsule, digest)?;
        let signer = model(|| KagemushaWalletReceiptSignerV1::from_credential(&self.credential))?;
        let body = model(|| {
            KagemushaWalletReceiptBodyV1::derive(
                &signer,
                &capsule.body().statement,
                &capsule.body().authority_digest,
                capsule.receipt_commitment(),
                [0; 32],
            )
        })?;
        Ok(body.transcript())
    }
    fn require_completion(
        &self,
        capsule: &SuffixRecoveryCapsuleV1,
        record: &SuffixCompletionRecordV1,
    ) -> Result<(), ProviderError> {
        self.require(capsule, &capsule.custody_digest())?;
        model(|| record.verify(&self.credential, capsule))?;
        let output = model(|| {
            KagemushaWalletSuffixUnloadV1::decode_canonical(
                &record.output,
                &capsule.body().scheme_id,
            )
        })?;
        if output.certificates != self.certificates {
            return Err(ProviderError::Invalid {
                field: "suffix owner completion certificates",
            });
        }
        Ok(())
    }
    fn assemble(
        &self,
        capsule: &SuffixRecoveryCapsuleV1,
        digest: &[u8; 32],
        signature: &KagemushaDeviceSignatureV1,
    ) -> Result<SuffixCompletionRecordV1, ProviderError> {
        self.require(capsule, digest)?;
        let receipt = KagemushaWalletReceiptV1 {
            version: 1,
            operation_id: capsule.body().operation_id,
            capsule_commitment: capsule.receipt_commitment(),
            payment_digest: [0; 32],
            signature: *signature,
        };
        let output = KagemushaWalletSuffixUnloadV1 {
            version: 1,
            credential: self.credential,
            certificates: self.certificates.clone(),
            after: capsule.body().successor_state,
            receipt,
            map_openings: capsule.body().map_openings.clone(),
        };
        let record = SuffixCompletionRecordV1 {
            version: 1,
            discriminator: 12,
            wallet_id: capsule.body().wallet_id,
            operation_id: capsule.body().operation_id,
            capsule_digest: *digest,
            receipt,
            output: model(|| output.to_canonical_bytes())?,
        };
        self.require_completion(capsule, &record)?;
        Ok(record)
    }
}

pub(in crate::kagemusha_wallet_state_v1) enum NativeTransitionOwnerV1<'a> {
    Ordinary(&'a TransitionOwner),
    Suffix(&'a SuffixTransitionOwnerV1),
}
fn wrong_kind() -> ProviderError {
    ProviderError::Invalid {
        field: "native transition owner schema",
    }
}
impl KagemushaWalletTransitionOwnerV1<NativeCapsuleV1, NativeCompletionV1>
    for NativeTransitionOwnerV1<'_>
{
    fn receipt_body(
        &self,
        capsule: &NativeCapsuleV1,
        digest: &[u8; 32],
    ) -> Result<Vec<u8>, ProviderError> {
        match (self, capsule) {
            (Self::Ordinary(owner), NativeCapsuleV1::Ordinary(value)) => {
                owner.receipt_body(value, digest)
            }
            (Self::Suffix(owner), NativeCapsuleV1::Suffix(value)) => {
                owner.receipt_body(value, digest)
            }
            _ => Err(wrong_kind()),
        }
    }
    fn require_completion(
        &self,
        capsule: &NativeCapsuleV1,
        completion: &NativeCompletionV1,
    ) -> Result<(), ProviderError> {
        match (self, capsule, completion) {
            (
                Self::Ordinary(owner),
                NativeCapsuleV1::Ordinary(value),
                NativeCompletionV1::Ordinary(record),
            ) => owner.require_completion(value, record),
            (
                Self::Suffix(owner),
                NativeCapsuleV1::Suffix(value),
                NativeCompletionV1::Suffix(record),
            ) => owner.require_completion(value, record),
            _ => Err(wrong_kind()),
        }
    }
    fn assemble(
        &self,
        capsule: &NativeCapsuleV1,
        digest: &[u8; 32],
        signature: &KagemushaDeviceSignatureV1,
    ) -> Result<NativeCompletionV1, ProviderError> {
        match (self, capsule) {
            (Self::Ordinary(owner), NativeCapsuleV1::Ordinary(value)) => owner
                .assemble(value, digest, signature)
                .map(NativeCompletionV1::Ordinary),
            (Self::Suffix(owner), NativeCapsuleV1::Suffix(value)) => owner
                .assemble(value, digest, signature)
                .map(NativeCompletionV1::Suffix),
            _ => Err(wrong_kind()),
        }
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1> AdvanceHandle<F, P> {
    pub(in crate::kagemusha_wallet_state_v1) fn advance_suffix(
        &mut self,
        owner: &SuffixTransitionOwnerV1,
        request: &AdvanceRequest<SuffixRecoveryCapsuleV1>,
    ) -> Result<AdvanceOutcome<SuffixCompletionRecordV1>, ProviderError> {
        let request = AdvanceRequest {
            expected: request.expected,
            operation_id: request.operation_id,
            new_head: request.new_head,
            proof_digest: request.proof_digest,
            capsule: NativeCapsuleV1::Suffix(request.capsule.clone()),
            growth_bytes: request.growth_bytes,
        };
        let result = self.lock()?.advance(
            &self.slot,
            &NativeTransitionOwnerV1::Suffix(owner),
            &request,
        )?;
        super::native_frames::map_outcome(result, NativeCompletionV1::into_suffix)
    }
}
