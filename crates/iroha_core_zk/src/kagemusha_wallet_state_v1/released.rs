//! Nonserialized dispatch over the two exact monetary recovery schemas.
//! Shared projections carry no proof authority and can never encode an ordinary capsule.

use super::*;
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::{
    KagemushaWalletSuffixUnloadV1, SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1, SuffixCompletionRecordV1,
    SuffixRecoveryCapsuleV1,
};

/// A suffix frozen transition keeps the complete independently tagged capsule original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuffixFrozenTransitionV1 {
    /// Current issuer-authenticated credential.
    pub credential: KagemushaWalletCredentialV1,
    /// Exact minimal issuer and optional charge-signer certificate set original.
    pub certificates: Vec<u8>,
    /// Exact suffix capsule, never converted into an ordinary capsule.
    pub capsule: SuffixRecoveryCapsuleV1,
}
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::SuffixFrozenTransitionV1")]
struct SuffixFrozenFrameV1 {
    credential: KagemushaWalletCredentialV1,
    certificates: Vec<u8>,
    capsule_original: Vec<u8>,
}
impl SuffixFrozenTransitionV1 {
    /// Check exact model and credential bindings, without granting Advance authority.
    pub fn validate(&self) -> Result<(), Error> {
        valid(self.credential.validate())?;
        if self.certificates.is_empty()
            || self.certificates.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
        {
            return Err(Error::Invalid("suffix certificate original"));
        }
        let _: KagemushaWalletCertificateSetV1 = archive::decode(&self.certificates)?;
        read_model_original(
            || self.capsule.body().validate(),
            Error::Invalid("suffix recovery body"),
        )?;
        valid(
            self.capsule
                .body()
                .successor_state
                .validate_for_credential(&self.credential),
        )?;
        valid(
            self.capsule
                .body()
                .statement
                .validate_for_credential(&self.credential),
        )?;
        if self.capsule.body().wallet_id != self.credential.body.wallet_id {
            return Err(Error::Invalid("suffix frozen wallet"));
        }
        Ok(())
    }
    /// Exact canonical frozen frame retained by the selected archive.
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        archive::encode(&SuffixFrozenFrameV1 {
            credential: self.credential,
            certificates: self.certificates.clone(),
            capsule_original: self.capsule.to_canonical_bytes(),
        })
    }
    /// Dispatch only by the explicit frozen frame schema, never trial decoding.
    pub fn decode_canonical(bytes: &[u8], scheme: &[u8; 32]) -> Result<Self, Error> {
        if bytes.len()
            > SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1
                + KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
                + KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
                + archive::METADATA_BOUND
        {
            return Err(Error::Invalid("suffix frozen bound"));
        }
        let frame: SuffixFrozenFrameV1 = archive::decode(bytes)?;
        let result = Self {
            credential: frame.credential,
            certificates: frame.certificates,
            capsule: read_model_original(
                || SuffixRecoveryCapsuleV1::decode_canonical(&frame.capsule_original, scheme),
                Error::Invalid("suffix recovery capsule"),
            )?,
        };
        result.validate()?;
        Ok(result)
    }
    /// Exact certificate original used to resume completion assembly.
    pub fn certificates(&self) -> &[u8] {
        &self.certificates
    }
    /// Explicit schema identity for source-selected recovery dispatch.
    pub fn frame_schema() -> [u8; 16] {
        norito::schema::identity::frame_hash::<SuffixFrozenFrameV1>()
    }
}

/// One authenticated released transition; no serialized union changes existing records.
#[derive(Debug, Clone)]
pub enum ReleasedStep {
    /// Ordinary unchanged capsule/completion schemas.
    Ordinary {
        /// Exact ordinary pre-Advance inputs.
        frozen: FrozenTransition,
        /// Exact provider-selected ordinary completion.
        retained: Retained<KagemushaWalletCompletionRecordV1>,
    },
    /// Explicit suffix capsule/completion schemas.
    Suffix {
        /// Exact suffix pre-Advance inputs, including original minimal certificates.
        frozen: SuffixFrozenTransitionV1,
        /// Exact provider-selected suffix completion.
        retained: Retained<SuffixCompletionRecordV1>,
        /// Nonserialized provenance from the selected provider/archive reconciliation.
        selected: SelectedSuffixReleaseV1,
    },
}

/// Opaque source-selection provenance for a released suffix; never serialized or imported.
/// Cloning or copying preserves only its exact capsule, completion and provider generation bindings.
#[derive(Debug, Clone, Copy)]
pub struct SelectedSuffixReleaseV1 {
    capsule: [u8; 32],
    completion: [u8; 32],
    generation: u128,
}

/// Read-only shared fields of an actual selected capsule, with its exact schema retained.
#[derive(Clone, Copy)]
pub enum ReleasedCapsuleRefV1<'a> {
    /// Existing monetary capsule.
    Ordinary(&'a KagemushaWalletRecoveryCapsuleV1),
    /// Explicit suffix capsule.
    Suffix(&'a SuffixRecoveryCapsuleV1),
}

/// A borrowed view, never an ordinary body and never a proof or serialization owner.
#[derive(Clone, Copy)]
pub struct ReleasedBodyRefV1<'a> {
    /// Exact installed scheme identity.
    pub scheme_id: [u8; 32],
    /// Exact enrolled wallet identity.
    pub wallet_id: [u8; 32],
    /// Statement-derived operation identity.
    pub operation_id: [u8; 32],
    /// Authenticated operation kind.
    pub kind: KagemushaWalletOperationKindV1,
    /// Exact selected predecessor capsule identity.
    pub predecessor_capsule_digest: [u8; 32],
    /// Actual released successor state.
    pub successor_state: KagemushaWalletStateV1,
    /// Exact released statement.
    pub statement: KagemushaWalletStatementV1,
    /// Exact installed step proof original.
    pub step_proof: &'a KagemushaWalletStepProofV1,
    /// Incoming Payment digest; zero for suffix Unload.
    pub payment_digest: [u8; 32],
    /// Exact retained map opening originals.
    pub map_openings: &'a Vec<Vec<u8>>,
    /// Ordinary operation inputs; suffix originals belong to the separate pinned namespace.
    pub retained_inputs: &'a Vec<KagemushaWalletRetainedInputV1>,
    /// Receipt-free output descriptor selected by the capsule.
    pub output: &'a KagemushaWalletOutputDescriptorV1,
    lineage: Option<&'a KagemushaWalletLineageV1>,
}
static NO_INPUTS: Vec<KagemushaWalletRetainedInputV1> = Vec::new();
impl<'a> ReleasedBodyRefV1<'a> {
    /// Only an ordinary capsule can carry its pre-Advance hard Omega original.
    pub fn predecessor_lineage(&self) -> Option<&'a KagemushaWalletLineageV1> {
        self.lineage
    }
}
impl<'a> ReleasedCapsuleRefV1<'a> {
    /// Borrow common fields without constructing or signing another monetary object.
    pub fn body(self) -> ReleasedBodyRefV1<'a> {
        match self {
            Self::Ordinary(c) => {
                let b = c.body();
                ReleasedBodyRefV1 {
                    scheme_id: b.scheme_id,
                    wallet_id: b.wallet_id,
                    operation_id: b.operation_id,
                    kind: b.kind,
                    predecessor_capsule_digest: b.predecessor_capsule_digest,
                    successor_state: b.successor_state,
                    statement: b.statement,
                    step_proof: &b.step_proof,
                    payment_digest: b.payment_digest,
                    map_openings: &b.map_openings,
                    retained_inputs: &b.retained_inputs,
                    output: &b.output,
                    lineage: b.predecessor_lineage(),
                }
            }
            Self::Suffix(c) => {
                let b = c.body();
                ReleasedBodyRefV1 {
                    scheme_id: b.scheme_id,
                    wallet_id: b.wallet_id,
                    operation_id: b.operation_id,
                    kind: KagemushaWalletOperationKindV1::Unload,
                    predecessor_capsule_digest: b.predecessor_capsule_digest,
                    successor_state: b.successor_state,
                    statement: b.statement,
                    step_proof: &b.step_proof,
                    payment_digest: [0; 32],
                    map_openings: &b.map_openings,
                    retained_inputs: &NO_INPUTS,
                    output: &b.output,
                    lineage: None,
                }
            }
        }
    }
    /// Exact private custody digest of the selected capsule.
    pub fn custody_digest(self) -> [u8; 32] {
        match self {
            Self::Ordinary(c) => c.custody_digest(),
            Self::Suffix(c) => c.custody_digest(),
        }
    }
    /// Independent capsule commitment signed by the receipt.
    pub fn receipt_commitment(self) -> [u8; 32] {
        match self {
            Self::Ordinary(c) => c.receipt_commitment(),
            Self::Suffix(c) => c.receipt_commitment(),
        }
    }
    /// Preserve exact original receipt authority. Suffix never substitutes a later Omega hash.
    pub fn proof_digest(self) -> Result<[u8; 32], KagemushaWalletValidationErrorV1> {
        match self {
            Self::Ordinary(c) => c.proof_digest(),
            Self::Suffix(c) => Ok(c.body().authority_digest),
        }
    }
}
impl ReleasedStep {
    // Only the selected archive/provider reconciliation owner calls this constructor.
    // Public Preparation callers cannot create provenance from transported DATA.
    pub(super) fn selected_suffix(
        frozen: SuffixFrozenTransitionV1,
        retained: Retained<SuffixCompletionRecordV1>,
    ) -> Result<Self, Error> {
        let selected = SelectedSuffixReleaseV1 {
            capsule: retained.capsule_digest,
            completion: retained.completion_digest,
            generation: retained.selected_generation,
        };
        let result = Self::Suffix {
            frozen,
            retained,
            selected,
        };
        result.verify_completion()?;
        Ok(result)
    }

    /// Borrow the exact selected capsule through a nonserializable view.
    pub fn capsule(&self) -> ReleasedCapsuleRefV1<'_> {
        match self {
            Self::Ordinary { frozen, .. } => ReleasedCapsuleRefV1::Ordinary(&frozen.capsule),
            Self::Suffix { frozen, .. } => ReleasedCapsuleRefV1::Suffix(&frozen.capsule),
        }
    }
    /// Credential retained with the selected successor.
    pub fn credential(&self) -> &KagemushaWalletCredentialV1 {
        match self {
            Self::Ordinary { frozen, .. } => &frozen.credential,
            Self::Suffix { frozen, .. } => &frozen.credential,
        }
    }
    /// Require the ordinary frozen schema for an ordinary operation.
    pub fn ordinary_frozen(&self) -> Result<&FrozenTransition, Error> {
        match self {
            Self::Ordinary { frozen, .. } => Ok(frozen),
            _ => Err(Error::Invalid("ordinary transition required")),
        }
    }
    /// Require the ordinary capsule schema for an ordinary operation.
    pub fn ordinary_capsule(&self) -> Result<&KagemushaWalletRecoveryCapsuleV1, Error> {
        Ok(&self.ordinary_frozen()?.capsule)
    }
    /// Borrow the exact suffix frozen transition and provider completion, if selected.
    pub fn suffix(
        &self,
    ) -> Option<(
        &SuffixFrozenTransitionV1,
        &Retained<SuffixCompletionRecordV1>,
    )> {
        match self {
            Self::Suffix {
                frozen, retained, ..
            } => Some((frozen, retained)),
            _ => None,
        }
    }
    /// Validate the selected schema and its credential bindings.
    pub fn validate_frozen(&self) -> Result<(), Error> {
        match self {
            Self::Ordinary { frozen, .. } => frozen.validate(),
            Self::Suffix { frozen, .. } => frozen.validate(),
        }
    }
    /// Exact provider-retained operation identity.
    pub fn operation_id(&self) -> [u8; 32] {
        match self {
            Self::Ordinary { retained, .. } => retained.operation_id,
            Self::Suffix { retained, .. } => retained.operation_id,
        }
    }
    /// Exact selected capsule identity.
    pub fn capsule_digest(&self) -> [u8; 32] {
        self.capsule().custody_digest()
    }
    /// Provider generation which selected the monetary transition.
    pub fn selected_generation(&self) -> u128 {
        match self {
            Self::Ordinary { retained, .. } => retained.selected_generation,
            Self::Suffix { retained, .. } => retained.selected_generation,
        }
    }
    /// Exact provider-retained completion digest.
    pub fn completion_digest(&self) -> [u8; 32] {
        match self {
            Self::Ordinary { retained, .. } => retained.completion_digest,
            Self::Suffix { retained, .. } => retained.completion_digest,
        }
    }
    /// Exact original retained completion bytes, unchanged on retry.
    pub fn completion_frame(&self) -> &[u8] {
        match self {
            Self::Ordinary { retained, .. } => &retained.frame,
            Self::Suffix { retained, .. } => &retained.frame,
        }
    }
    /// Original hardware receipt; never assembled or signed again.
    pub fn receipt(&self) -> &KagemushaWalletReceiptV1 {
        match self {
            Self::Ordinary { retained, .. } => &retained.record.receipt,
            Self::Suffix { retained, .. } => &retained.record.receipt,
        }
    }
    /// Exact original retained output bytes, unchanged on retry.
    pub fn output(&self) -> &Vec<u8> {
        match self {
            Self::Ordinary { retained, .. } => &retained.record.output,
            Self::Suffix { retained, .. } => &retained.record.output,
        }
    }
    /// Recheck every canonical original, provider identity and original receipt.
    pub fn verify_completion(&self) -> Result<(), Error> {
        self.validate_frozen()?;
        if let Self::Suffix {
            selected,
            retained,
            frozen,
        } = self
        {
            if selected.capsule != frozen.capsule.custody_digest()
                || selected.capsule != retained.capsule_digest
                || selected.completion != retained.completion_digest
                || selected.generation != retained.selected_generation
            {
                return Err(Error::WitnessLost("suffix selected release provenance"));
            }
        }

        let (operation, capsule, frame, digest) = match self {
            Self::Ordinary { frozen, retained } => {
                read_model_original(
                    || retained.record.verify(&frozen.credential, &frozen.capsule),
                    Error::WitnessLost("released completion original"),
                )?;
                (
                    retained.operation_id,
                    retained.capsule_digest,
                    valid(retained.record.to_canonical_bytes())?,
                    valid(retained.record.completion_digest())?,
                )
            }
            Self::Suffix {
                frozen, retained, ..
            } => {
                read_model_original(
                    || retained.record.verify(&frozen.credential, &frozen.capsule),
                    Error::WitnessLost("released completion original"),
                )?;
                let output = read_model_original(
                    || {
                        KagemushaWalletSuffixUnloadV1::decode_canonical(
                            &retained.record.output,
                            &frozen.capsule.body().scheme_id,
                        )
                    },
                    Error::WitnessLost("suffix completion output original"),
                )?;
                if archive::encode(&output.certificates)? != frozen.certificates {
                    return Err(Error::WitnessLost("suffix completion certificate original"));
                }
                let frame = valid(retained.record.to_canonical_bytes())?;
                let digest = SuffixCompletionRecordV1::frame_digest(&frame);
                (
                    retained.operation_id,
                    retained.capsule_digest,
                    frame,
                    digest,
                )
            }
        };
        if self.selected_generation() == 0
            || operation != self.capsule().body().operation_id
            || capsule != self.capsule_digest()
            || frame != self.completion_frame()
            || digest != self.completion_digest()
        {
            return Err(Error::WitnessLost("released completion binding"));
        }
        Ok(())
    }
}

/// Internal dispatch selected by the explicit archived frozen frame schema.
pub(super) enum RecordedTransitionV1 {
    Ordinary(FrozenTransition),
    Suffix(SuffixFrozenTransitionV1),
}
impl RecordedTransitionV1 {
    pub(super) fn capsule(&self) -> ReleasedCapsuleRefV1<'_> {
        match self {
            Self::Ordinary(v) => ReleasedCapsuleRefV1::Ordinary(&v.capsule),
            Self::Suffix(v) => ReleasedCapsuleRefV1::Suffix(&v.capsule),
        }
    }
    pub(super) fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Ordinary(v) => v.validate(),
            Self::Suffix(v) => v.validate(),
        }
    }
}

#[cfg(test)]
impl ReleasedStep {
    pub(crate) fn ordinary_parts(
        &self,
    ) -> (
        &FrozenTransition,
        &Retained<KagemushaWalletCompletionRecordV1>,
    ) {
        match self {
            Self::Ordinary { frozen, retained } => (frozen, retained),
            Self::Suffix { .. } => panic!("ordinary fixture"),
        }
    }
    pub(crate) fn ordinary_parts_mut(
        &mut self,
    ) -> (
        &mut FrozenTransition,
        &mut Retained<KagemushaWalletCompletionRecordV1>,
    ) {
        match self {
            Self::Ordinary { frozen, retained } => (frozen, retained),
            Self::Suffix { .. } => panic!("ordinary fixture"),
        }
    }
}

#[cfg(test)]
mod tests;
