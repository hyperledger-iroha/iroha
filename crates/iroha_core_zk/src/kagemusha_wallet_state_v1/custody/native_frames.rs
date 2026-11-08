//! In-memory Native frame dispatch. Each variant encodes the original frame, never an enum.
//! Declared schemas select exactly one decoder. A failed decoder is never retried as another.
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletAdvanceCapsuleV1, KagemushaWalletCapacityClassV1,
    KagemushaWalletCompletionFrameV1, KagemushaWalletFrozenFrameV1, KagemushaWalletRetainedV1,
};
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::{
    SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1, SuffixCompletionRecordV1, SuffixRecoveryCapsuleV1,
};

/// Mixed-history Native provider. The generic provider's original defaults remain unchanged.
pub type NativeProviderV1<F, P> =
    KagemushaWalletProviderV1<F, P, NativeCapsuleV1, NativeCompletionV1>;

/// Exact validated capsule owner; has no Norito enum representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCapsuleV1 {
    /// Original ordinary monetary capsule.
    Ordinary(KagemushaWalletRecoveryCapsuleV1),
    /// Independently tagged Native suffix capsule.
    Suffix(SuffixRecoveryCapsuleV1),
}
/// Exact validated completion owner; has no Norito enum representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCompletionV1 {
    /// Original ordinary completion and exact output.
    Ordinary(KagemushaWalletCompletionRecordV1),
    /// Independently tagged suffix completion and exact Finalize original.
    Suffix(SuffixCompletionRecordV1),
}

fn schema(bytes: &[u8]) -> Option<[u8; 16]> {
    norito::core::Header::read(bytes)
        .ok()
        .map(|header| header.schema)
}
fn invalid_schema() -> KagemushaWalletValidationErrorV1 {
    KagemushaWalletValidationErrorV1::InvalidField {
        field: "native custody frame schema",
    }
}
impl KagemushaWalletFrozenFrameV1 for NativeCapsuleV1 {
    const OBJECT: &'static str = "capsule";
    const MAX_FRAME_BYTES: usize =
        if KAGEMUSHA_WALLET_CAPSULE_MAX_BYTES_V1 > SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1 {
            KAGEMUSHA_WALLET_CAPSULE_MAX_BYTES_V1
        } else {
            SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1
        };
    type Binding = [u8; 32];
    fn encode_frame(&self) -> Result<Vec<u8>, KagemushaWalletValidationErrorV1> {
        match self {
            Self::Ordinary(value) => value.encode_frame(),
            Self::Suffix(value) => Ok(value.to_canonical_bytes()),
        }
    }
    fn decode_frame(
        bytes: &[u8],
        binding: &[u8; 32],
    ) -> Result<Self, KagemushaWalletValidationErrorV1> {
        match schema(bytes) {
            Some(value)
                if value
                    == norito::schema::identity::frame_hash::<KagemushaWalletRecoveryCapsuleV1>(
                    ) =>
            {
                KagemushaWalletRecoveryCapsuleV1::decode_canonical(bytes, binding)
                    .map(Self::Ordinary)
            }
            Some(value) if value == SuffixRecoveryCapsuleV1::frame_schema() => {
                SuffixRecoveryCapsuleV1::decode_canonical(bytes, binding).map(Self::Suffix)
            }
            _ => Err(invalid_schema()),
        }
    }
    fn frame_digest(bytes: &[u8]) -> [u8; 32] {
        match schema(bytes) {
            Some(value)
                if value
                    == norito::schema::identity::frame_hash::<KagemushaWalletRecoveryCapsuleV1>(
                    ) =>
            {
                <KagemushaWalletRecoveryCapsuleV1 as KagemushaWalletFrozenFrameV1>::frame_digest(
                    bytes,
                )
            }
            Some(value) if value == SuffixRecoveryCapsuleV1::frame_schema() => {
                SuffixRecoveryCapsuleV1::frame_digest(bytes)
            }
            // No valid selected marker has this digest. Decode independently rejects it too.
            _ => [0; 32],
        }
    }
}
impl KagemushaWalletAdvanceCapsuleV1 for NativeCapsuleV1 {
    fn marker_binding(marker: &KagemushaWalletMarkerV1) -> [u8; 32] {
        marker.scheme_id
    }
    fn operation_id(&self) -> [u8; 32] {
        match self {
            Self::Ordinary(v) => v.body().operation_id,
            Self::Suffix(v) => v.body().operation_id,
        }
    }
    fn head_marker_state(&self) -> Result<KagemushaWalletMarkerStateV1, ProviderError> {
        Ok(match self {
            Self::Ordinary(v) => v.head_marker_state(),
            Self::Suffix(v) => v.head_marker_state(),
        })
    }
    fn predecessor_commitment(&self) -> KagemushaWalletStateCommitmentV1 {
        match self {
            Self::Ordinary(v) => v.body().statement.predecessor,
            Self::Suffix(v) => v.body().statement.predecessor,
        }
    }
    fn bound_proof_digest(&self) -> Result<[u8; 32], ProviderError> {
        match self {
            Self::Ordinary(v) => v.bound_proof_digest(),
            Self::Suffix(v) => Ok(v.body().authority_digest),
        }
    }
    fn bootstrap_enrollment_marker(&self) -> Option<[u8; 32]> {
        match self {
            Self::Ordinary(v) => v.bootstrap_enrollment_marker(),
            Self::Suffix(_) => None,
        }
    }
    fn capacity_class(&self) -> KagemushaWalletCapacityClassV1 {
        match self {
            Self::Ordinary(v) => v.capacity_class(),
            Self::Suffix(_) => KagemushaWalletCapacityClassV1::Growth,
        }
    }
    fn require_marker(&self, selected: &KagemushaWalletMarkerV1) -> Result<(), ProviderError> {
        match self {
            Self::Ordinary(v) => v.require_marker(selected),
            Self::Suffix(v) => {
                invalid(selected.validate())?;
                crate::kagemusha_wallet_advance_v1::decode_resource::read_model(
                    || v.body().validate(),
                    "suffix capsule marker",
                )?;
                if selected.scheme_id != v.body().scheme_id
                    || selected.wallet_id != v.body().wallet_id
                    || selected.state != v.head_marker_state()
                {
                    return Err(ProviderError::Invalid {
                        field: "suffix capsule marker",
                    });
                }
                Ok(())
            }
        }
    }
}
impl KagemushaWalletFrozenFrameV1 for NativeCompletionV1 {
    const OBJECT: &'static str = "completion";
    const MAX_FRAME_BYTES: usize =
        if KAGEMUSHA_WALLET_COMPLETION_RECORD_MAX_BYTES_V1 > SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1 {
            KAGEMUSHA_WALLET_COMPLETION_RECORD_MAX_BYTES_V1
        } else {
            SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1
        };
    type Binding = [u8; 32];
    fn encode_frame(&self) -> Result<Vec<u8>, KagemushaWalletValidationErrorV1> {
        match self {
            Self::Ordinary(v) => v.to_canonical_bytes(),
            Self::Suffix(v) => v.to_canonical_bytes(),
        }
    }
    fn decode_frame(
        bytes: &[u8],
        binding: &[u8; 32],
    ) -> Result<Self, KagemushaWalletValidationErrorV1> {
        match schema(bytes) {
            Some(value)
                if value
                    == norito::schema::identity::frame_hash::<KagemushaWalletCompletionRecordV1>(
                    ) =>
            {
                KagemushaWalletCompletionRecordV1::decode_canonical(bytes, binding)
                    .map(Self::Ordinary)
            }
            Some(value)
                if value == norito::schema::identity::frame_hash::<SuffixCompletionRecordV1>() =>
            {
                SuffixCompletionRecordV1::decode_canonical(bytes, binding).map(Self::Suffix)
            }
            _ => Err(invalid_schema()),
        }
    }
    fn frame_digest(bytes: &[u8]) -> [u8; 32] {
        match schema(bytes) {
            Some(value)
                if value
                    == norito::schema::identity::frame_hash::<KagemushaWalletCompletionRecordV1>(
                    ) =>
            {
                <KagemushaWalletCompletionRecordV1 as KagemushaWalletFrozenFrameV1>::frame_digest(
                    bytes,
                )
            }
            Some(value)
                if value == norito::schema::identity::frame_hash::<SuffixCompletionRecordV1>() =>
            {
                SuffixCompletionRecordV1::frame_digest(bytes)
            }
            _ => [0; 32],
        }
    }
}
impl KagemushaWalletCompletionFrameV1 for NativeCompletionV1 {
    fn marker_binding(marker: &KagemushaWalletMarkerV1) -> [u8; 32] {
        marker.wallet_id
    }
    fn operation_id(&self) -> [u8; 32] {
        match self {
            Self::Ordinary(v) => v.operation_id,
            Self::Suffix(v) => v.operation_id,
        }
    }
    fn capsule_digest(&self) -> [u8; 32] {
        match self {
            Self::Ordinary(v) => v.capsule_digest,
            Self::Suffix(v) => v.capsule_digest,
        }
    }
}

/// Preserve the provider's exact retained frame and every marker binding during dispatch.
pub(in crate::kagemusha_wallet_state_v1) fn map_retained<R, T>(
    value: KagemushaWalletRetainedV1<R>,
    map: impl FnOnce(R) -> Result<T, ProviderError>,
) -> Result<KagemushaWalletRetainedV1<T>, ProviderError> {
    Ok(KagemushaWalletRetainedV1 {
        operation_id: value.operation_id,
        capsule_digest: value.capsule_digest,
        selected_generation: value.selected_generation,
        completion_digest: value.completion_digest,
        frame: value.frame,
        record: map(value.record)?,
    })
}
pub(in crate::kagemusha_wallet_state_v1) fn map_lookup<R, T>(
    value: Lookup<R>,
    map: impl FnOnce(R) -> Result<T, ProviderError>,
) -> Result<Lookup<T>, ProviderError> {
    Ok(match value {
        Lookup::Retained(value) => Lookup::Retained(Box::new(map_retained(*value, map)?)),
        Lookup::SelectedUnsigned { capsule_digest } => Lookup::SelectedUnsigned { capsule_digest },
        Lookup::Archived(value) => Lookup::Archived(value),
        Lookup::DeliveryDataLoss => Lookup::DeliveryDataLoss,
        Lookup::Unknown => Lookup::Unknown,
    })
}
pub(in crate::kagemusha_wallet_state_v1) fn map_outcome<R, T>(
    value: AdvanceOutcome<R>,
    map: impl FnOnce(R) -> Result<T, ProviderError>,
) -> Result<AdvanceOutcome<T>, ProviderError> {
    Ok(match value {
        AdvanceOutcome::Released { retained, resumed } => AdvanceOutcome::Released {
            retained: Box::new(map_retained(*retained, map)?),
            resumed,
        },
        AdvanceOutcome::Pending { operation_id } => AdvanceOutcome::Pending { operation_id },
        AdvanceOutcome::NotPerformed(reason) => AdvanceOutcome::NotPerformed(reason),
        AdvanceOutcome::Archived(value) => AdvanceOutcome::Archived(value),
        AdvanceOutcome::DeliveryDataLoss { operation_id } => {
            AdvanceOutcome::DeliveryDataLoss { operation_id }
        }
    })
}

impl KagemushaWalletFrozenFrameV1 for SuffixRecoveryCapsuleV1 {
    const OBJECT: &'static str = "capsule";
    const MAX_FRAME_BYTES: usize = SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1;
    type Binding = [u8; 32];
    fn encode_frame(&self) -> Result<Vec<u8>, KagemushaWalletValidationErrorV1> {
        Ok(self.to_canonical_bytes())
    }
    fn decode_frame(
        bytes: &[u8],
        binding: &[u8; 32],
    ) -> Result<Self, KagemushaWalletValidationErrorV1> {
        Self::decode_canonical(bytes, binding)
    }
    fn frame_digest(bytes: &[u8]) -> [u8; 32] {
        Self::frame_digest(bytes)
    }
}
impl KagemushaWalletCompletionFrameV1 for SuffixCompletionRecordV1 {
    fn marker_binding(marker: &KagemushaWalletMarkerV1) -> [u8; 32] {
        marker.wallet_id
    }
    fn operation_id(&self) -> [u8; 32] {
        self.operation_id
    }
    fn capsule_digest(&self) -> [u8; 32] {
        self.capsule_digest
    }
}
impl KagemushaWalletFrozenFrameV1 for SuffixCompletionRecordV1 {
    const OBJECT: &'static str = "completion";
    const MAX_FRAME_BYTES: usize = SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1;
    type Binding = [u8; 32];
    fn encode_frame(&self) -> Result<Vec<u8>, KagemushaWalletValidationErrorV1> {
        self.to_canonical_bytes()
    }
    fn decode_frame(
        bytes: &[u8],
        binding: &[u8; 32],
    ) -> Result<Self, KagemushaWalletValidationErrorV1> {
        Self::decode_canonical(bytes, binding)
    }
    fn frame_digest(bytes: &[u8]) -> [u8; 32] {
        Self::frame_digest(bytes)
    }
}
impl NativeCompletionV1 {
    pub(in crate::kagemusha_wallet_state_v1) fn into_ordinary(
        self,
    ) -> Result<KagemushaWalletCompletionRecordV1, ProviderError> {
        match self {
            Self::Ordinary(value) => Ok(value),
            Self::Suffix(_) => Err(ProviderError::Invalid {
                field: "ordinary completion required",
            }),
        }
    }
    pub(in crate::kagemusha_wallet_state_v1) fn into_suffix(
        self,
    ) -> Result<SuffixCompletionRecordV1, ProviderError> {
        match self {
            Self::Suffix(value) => Ok(value),
            Self::Ordinary(_) => Err(ProviderError::Invalid {
                field: "suffix completion required",
            }),
        }
    }
}
