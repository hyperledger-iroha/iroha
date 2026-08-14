/// Exact evidence-viewer message class presented to the runtime signer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EvidenceViewerSigningPurposeV1 {
    /// Domain-prefixed evidence-view receipt message.
    Receipt = 1,
    /// Domain-separated checkpoint-store record digest.
    CheckpointStoreRecord = 2,
    /// Domain-prefixed public checkpoint-anchor message.
    CheckpointAnchor = 3,
    /// Domain-separated compaction-archive head digest.
    CompactionArchive = 4,
}
impl EvidenceViewerSigningPurposeV1 {
    /// Immutable V1 wire identifier.
    #[must_use]
    pub const fn wire_id(self) -> u8 {
        self as u8
    }
    /// Decode one immutable V1 wire identifier without aliases.
    #[must_use]
    pub const fn try_from_wire_id(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Receipt),
            2 => Some(Self::CheckpointStoreRecord),
            3 => Some(Self::CheckpointAnchor),
            4 => Some(Self::CompactionArchive),
            _ => None,
        }
    }
}
/// Payload-free failure while validating one evidence-viewer signing message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceViewerSigningMessageErrorV1;
/// Validate the exact V1 message shape admitted for one signing purpose.
///
/// The checkpoint-store-record and compaction-archive messages are already
/// domain-separated 32-byte digests. Their semantic recomputation remains at
/// the qualified evidence-viewer caller boundary; this function binds their
/// exact purpose and width so neither can be substituted for another purpose.
pub fn validate_evidence_viewer_signing_message_v1(
    purpose: EvidenceViewerSigningPurposeV1,
    message: &[u8],
    expected_signer_handle: &str,
    expected_public_key: [u8; 32],
) -> Result<(), EvidenceViewerSigningMessageErrorV1> {
    let valid = match purpose {
        EvidenceViewerSigningPurposeV1::Receipt => message
            .strip_prefix(RECEIPT_SIGNATURE_DOMAIN_V1)
            .is_some_and(exact_nonzero_evidence_signing_digest),
        EvidenceViewerSigningPurposeV1::CheckpointStoreRecord
        | EvidenceViewerSigningPurposeV1::CompactionArchive => {
            exact_nonzero_evidence_signing_digest(message)
        }
        EvidenceViewerSigningPurposeV1::CheckpointAnchor => {
            validate_checkpoint_anchor_signing_message(
                message,
                expected_signer_handle,
                expected_public_key,
            )
        }
    };
    valid
        .then_some(())
        .ok_or(EvidenceViewerSigningMessageErrorV1)
}
fn exact_nonzero_evidence_signing_digest(bytes: &[u8]) -> bool {
    bytes.len() == 32 && bytes.iter().any(|byte| *byte != 0)
}
fn take_evidence_signing_bytes<'a>(input: &mut &'a [u8], count: usize) -> Option<&'a [u8]> {
    if input.len() < count {
        return None;
    }
    let (value, remaining) = input.split_at(count);
    *input = remaining;
    Some(value)
}
fn take_evidence_signing_u64(input: &mut &[u8]) -> Option<u64> {
    Some(u64::from_le_bytes(
        take_evidence_signing_bytes(input, 8)?.try_into().ok()?,
    ))
}
fn take_evidence_signing_optional_digest(input: &mut &[u8]) -> Option<Option<[u8; 32]>> {
    match *take_evidence_signing_bytes(input, 1)?.first()? {
        0 => Some(None),
        1 => Some(Some(
            take_evidence_signing_bytes(input, 32)?.try_into().ok()?,
        )),
        _ => None,
    }
}
fn take_evidence_signing_text(input: &mut &[u8]) -> Option<String> {
    let length = usize::try_from(take_evidence_signing_u64(input)?).ok()?;
    if length == 0 || length > EVIDENCE_VIEWER_RUNTIME_PROVIDER_HANDLE_MAX_BYTES_V1 {
        return None;
    }
    let value = std::str::from_utf8(take_evidence_signing_bytes(input, length)?).ok()?;
    if value.trim() != value || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.to_owned())
}
fn validate_checkpoint_anchor_signing_message(
    message: &[u8],
    expected_signer_handle: &str,
    expected_public_key: [u8; 32],
) -> bool {
    let Some(mut input) = message.strip_prefix(CHECKPOINT_SIGNATURE_DOMAIN_V1) else {
        return false;
    };
    let Some(version) =
        take_evidence_signing_bytes(&mut input, 2).and_then(|bytes| bytes.try_into().ok())
    else {
        return false;
    };
    let Some(generation) = take_evidence_signing_u64(&mut input) else {
        return false;
    };
    let Some(predecessor_revision) = take_evidence_signing_optional_digest(&mut input) else {
        return false;
    };
    let Some(predecessor_digest) = take_evidence_signing_optional_digest(&mut input) else {
        return false;
    };
    if u16::from_le_bytes(version) != EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1
        || generation == 0
        || (generation == 1) != predecessor_revision.is_none()
        || predecessor_revision.is_none() != predecessor_digest.is_none()
        || take_evidence_signing_bytes(&mut input, 32)
            .is_none_or(|digest| digest.iter().all(|byte| *byte == 0))
    {
        return false;
    }
    let Some(receipt_count) = take_evidence_signing_u64(&mut input) else {
        return false;
    };
    let chain_head_valid = match take_evidence_signing_bytes(&mut input, 1)
        .and_then(|bytes| bytes.first())
        .copied()
    {
        Some(0) => receipt_count == 0,
        Some(1) => {
            take_evidence_signing_u64(&mut input)
                .is_some_and(|sequence| sequence == receipt_count && sequence != 0)
                && take_evidence_signing_bytes(&mut input, 32)
                    .is_some_and(|digest| digest.iter().any(|byte| *byte != 0))
        }
        _ => false,
    };
    let Some(archive_digest) = take_evidence_signing_optional_digest(&mut input) else {
        return false;
    };
    let Some(checkpoint_store_handle) = take_evidence_signing_text(&mut input) else {
        return false;
    };
    if !chain_head_valid
        || archive_digest.is_some_and(|digest| digest == [0; 32])
        || !is_production_runtime_handle(&checkpoint_store_handle)
        || take_evidence_signing_u64(&mut input).is_none_or(|revision| revision == 0)
        || take_evidence_signing_bytes(&mut input, 32)
            .is_none_or(|digest| digest.iter().all(|byte| *byte == 0))
    {
        return false;
    }
    let Some(signer_handle) = take_evidence_signing_text(&mut input) else {
        return false;
    };
    let Some(signer_key) = take_evidence_signing_bytes(&mut input, 32) else {
        return false;
    };
    signer_handle == expected_signer_handle && signer_key == expected_public_key && input.is_empty()
}
/// Runtime-only Ed25519 receipt signer.
pub trait EvidenceViewerReceiptSignerV1: EvidenceViewerRuntimeProviderV1 {
    /// Exact Ed25519 public key.
    fn public_key(&self) -> [u8; 32];
    /// Sign one exact canonical message for the declared purpose.
    fn sign(
        &self,
        purpose: EvidenceViewerSigningPurposeV1,
        message: &[u8],
    ) -> Result<[u8; 64], EvidenceViewerExternalErrorV1>;
}
struct QualifiedEvidenceViewerReceiptSignerV1 {
    inner: QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerReceiptSignerV1>,
    public_key: [u8; 32],
}
impl QualifiedEvidenceViewerReceiptSignerV1 {
    fn try_new(
        expected_handle: &str,
        expected_qualification: EvidenceViewerRuntimeProviderQualificationV1,
        expected_public_key: [u8; 32],
        provider: Arc<dyn EvidenceViewerReceiptSignerV1>,
    ) -> Result<Self, EvidenceViewerRuntimeProviderQualificationErrorV1> {
        let inner = QualifiedEvidenceViewerProviderV1::try_new(
            expected_handle,
            expected_qualification,
            provider,
        )?;
        let public_key = Self::read_qualified_public_key(&inner)?;
        if public_key != expected_public_key {
            return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::SignerPublicKeyChanged);
        }
        Ok(Self {
            inner,
            public_key: expected_public_key,
        })
    }
    fn read_qualified_public_key(
        inner: &QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerReceiptSignerV1>,
    ) -> Result<[u8; 32], EvidenceViewerRuntimeProviderQualificationErrorV1> {
        inner.revalidate()?;
        let public_key = inner.provider.public_key();
        inner.revalidate()?;
        Ok(public_key)
    }
    fn sign(
        &self,
        purpose: EvidenceViewerSigningPurposeV1,
        message: &[u8],
    ) -> Result<[u8; 64], EvidenceViewerExternalErrorV1> {
        let public_key_before = Self::read_qualified_public_key(&self.inner)
            .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
        if public_key_before != self.public_key {
            return Err(EvidenceViewerExternalErrorV1::Unavailable);
        }
        let result = self.inner.provider.sign(purpose, message);
        let public_key_after = Self::read_qualified_public_key(&self.inner)
            .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
        if public_key_after != self.public_key {
            return Err(EvidenceViewerExternalErrorV1::Unavailable);
        }
        result
    }
}
impl fmt::Debug for QualifiedEvidenceViewerReceiptSignerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedEvidenceViewerReceiptSignerV1")
            .field("inner", &self.inner)
            .field("public_key", &self.public_key)
            .finish()
    }
}
