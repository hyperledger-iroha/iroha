//! Source-backed authenticated Kagemusha artifact access.
//!
//! Production runtimes retain pinned artifact handles instead of materializing
//! the complete Eq/Ep inventory.  A source can only lend a seekable framed
//! reader: descriptor selection, release binding, both digest passes, exact
//! payload consumption, and trailing-byte rejection remain owned by core.

use std::{
    fmt,
    io::{Read, Seek},
    sync::Arc,
};

use iroha_data_model::offline::{
    KagemushaAuthenticatedReleaseV4, KagemushaPastaCycleArtifactKindV4,
    KagemushaPastaCycleParityV1, KagemushaStepCircuitParamsV4,
};

use super::kagemusha_artifact_v4::{
    KagemushaRecursiveSpendPastaCycleArtifactHeaderV4, kagemusha_artifact_descriptor_v4,
    with_kagemusha_pasta_cycle_artifact_payload_v4,
};

/// Object-safe seekable reader lent by an authenticated artifact source.
///
/// The source must keep the underlying handle pinned for the complete callback.
/// Core rewinds and authenticates it twice while the callback is active.
pub trait KagemushaArtifactReadSeekV4: Read + Seek {}

impl<T: Read + Seek + ?Sized> KagemushaArtifactReadSeekV4 for T {}

/// Immutable source for one separately authenticated production release.
///
/// Implementations do not expose unframed bytes.  They lend exactly one raw
/// framed reader for the requested role and must propagate the callback result.
/// [`with_kagemusha_authenticated_artifact_payload_from_source_v4`] also checks
/// callback cardinality and preserves callback failures, so a faulty source
/// cannot bypass either rule by swallowing an error.
pub trait KagemushaAuthenticatedArtifactSourceV4: Send + Sync {
    /// The signed release selecting every descriptor this source can open.
    fn authenticated_release(&self) -> &KagemushaAuthenticatedReleaseV4;

    /// Lend the pinned complete KRV4 file for one exact parity and role.
    fn with_framed_artifact(
        &self,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
        consume: &mut dyn FnMut(&mut dyn KagemushaArtifactReadSeekV4) -> Result<(), String>,
    ) -> Result<(), String>;
}

/// Lightweight, release-bound semantic identity for one qualified parity.
///
/// This deliberately retains no Halo2 parameters or keys.  The raw SHA-256
/// identifies the signed `SerdeFormat::Processed` artifact payload; the
/// verifier-key commitment applies Iroha's `iroha:zk:v1:vk` domain separation
/// with the exact `halo2/ipa` backend and payload length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaQualifiedParityMetadataV4 {
    parity: KagemushaPastaCycleParityV1,
    circuit_params: KagemushaStepCircuitParamsV4,
    compiled_protocol_identity_sha256: [u8; 32],
    processed_verifying_key_len: u64,
    processed_verifying_key_sha256: [u8; 32],
    verifying_key_commitment: [u8; 32],
}

impl KagemushaQualifiedParityMetadataV4 {
    pub(crate) fn new(
        parity: KagemushaPastaCycleParityV1,
        circuit_params: KagemushaStepCircuitParamsV4,
        compiled_protocol_identity_sha256: [u8; 32],
        processed_verifying_key_len: u64,
        processed_verifying_key_sha256: [u8; 32],
        verifying_key_commitment: [u8; 32],
    ) -> Result<Self, String> {
        circuit_params.validate().map_err(|error| {
            format!("invalid qualified Kagemusha V4 circuit parameters: {error}")
        })?;
        if compiled_protocol_identity_sha256 == [0; 32]
            || processed_verifying_key_len == 0
            || processed_verifying_key_sha256 == [0; 32]
            || verifying_key_commitment == [0; 32]
        {
            return Err("Kagemusha V4 qualified parity identity is empty".to_owned());
        }
        Ok(Self {
            parity,
            circuit_params,
            compiled_protocol_identity_sha256,
            processed_verifying_key_len,
            processed_verifying_key_sha256,
            verifying_key_commitment,
        })
    }

    /// Pasta parity selected by this metadata.
    #[must_use]
    pub const fn parity(&self) -> KagemushaPastaCycleParityV1 {
        self.parity
    }

    /// Authenticated circuit parameters for this parity.
    #[must_use]
    pub fn circuit_params(&self) -> &KagemushaStepCircuitParamsV4 {
        &self.circuit_params
    }

    /// Identity of the fully compiled final protocol.
    #[must_use]
    pub const fn compiled_protocol_identity_sha256(&self) -> [u8; 32] {
        self.compiled_protocol_identity_sha256
    }

    /// Exact byte length of the canonical processed verifying key.
    #[must_use]
    pub const fn processed_verifying_key_len(&self) -> u64 {
        self.processed_verifying_key_len
    }

    /// Raw SHA-256 of the canonical processed verifying key artifact payload.
    #[must_use]
    pub const fn processed_verifying_key_sha256(&self) -> [u8; 32] {
        self.processed_verifying_key_sha256
    }

    /// Iroha domain-separated commitment to the processed verifying key.
    #[must_use]
    pub const fn verifying_key_commitment(&self) -> [u8; 32] {
        self.verifying_key_commitment
    }
}

/// Core-owned proof that all eight signed roles passed semantic qualification.
///
/// Construction is private to core.  The returned object inseparably retains
/// the original pinned file source and a clone of the authenticated release as
/// it existed before qualification.  Its source implementation always returns
/// that clone, so a source with mutable release metadata cannot introduce a
/// release/file time-of-check/time-of-use split after qualification.
#[derive(Clone)]
pub struct KagemushaQualifiedArtifactSourceV4 {
    source: Arc<dyn KagemushaAuthenticatedArtifactSourceV4>,
    authenticated_release: KagemushaAuthenticatedReleaseV4,
    step_eq: KagemushaQualifiedParityMetadataV4,
    step_ep: KagemushaQualifiedParityMetadataV4,
}

impl fmt::Debug for KagemushaQualifiedArtifactSourceV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KagemushaQualifiedArtifactSourceV4")
            .field(
                "manifest_sha256",
                &self.authenticated_release.manifest_sha256(),
            )
            .field("step_eq", &self.step_eq)
            .field("step_ep", &self.step_ep)
            .finish_non_exhaustive()
    }
}

impl KagemushaQualifiedArtifactSourceV4 {
    pub(crate) fn new(
        source: Arc<dyn KagemushaAuthenticatedArtifactSourceV4>,
        authenticated_release: KagemushaAuthenticatedReleaseV4,
        step_eq: KagemushaQualifiedParityMetadataV4,
        step_ep: KagemushaQualifiedParityMetadataV4,
    ) -> Result<Self, String> {
        if step_eq.parity != KagemushaPastaCycleParityV1::StepEq
            || step_ep.parity != KagemushaPastaCycleParityV1::StepEp
        {
            return Err("Kagemusha V4 qualified parity order mismatch".to_owned());
        }
        if source.authenticated_release() != &authenticated_release {
            return Err(
                "Kagemusha V4 artifact source release changed during qualification".to_owned(),
            );
        }
        Ok(Self {
            source,
            authenticated_release,
            step_eq,
            step_ep,
        })
    }

    /// Authenticated release pinned before semantic qualification began.
    #[must_use]
    pub fn authenticated_release(&self) -> &KagemushaAuthenticatedReleaseV4 {
        &self.authenticated_release
    }

    /// Core-qualified Eq metadata.
    #[must_use]
    pub const fn step_eq(&self) -> &KagemushaQualifiedParityMetadataV4 {
        &self.step_eq
    }

    /// Core-qualified Ep metadata.
    #[must_use]
    pub const fn step_ep(&self) -> &KagemushaQualifiedParityMetadataV4 {
        &self.step_ep
    }

    /// Lend one authenticated processed verifying-key payload without copying it.
    ///
    /// Only a semantically qualified source exposes this projection.  Core
    /// authenticates the complete framed file before the callback and hashes
    /// the exact bytes consumed by it a second time.  The callback must consume
    /// all `processed_verifying_key_len` bytes.
    pub fn with_authenticated_processed_verifying_key<T, F>(
        &self,
        parity: KagemushaPastaCycleParityV1,
        consume: F,
    ) -> Result<T, String>
    where
        F: FnOnce(&mut dyn Read, u64) -> Result<T, String>,
    {
        let metadata = match parity {
            KagemushaPastaCycleParityV1::StepEq => &self.step_eq,
            KagemushaPastaCycleParityV1::StepEp => &self.step_ep,
        };
        with_kagemusha_authenticated_artifact_payload_from_source_v4(
            self,
            parity,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            |reader, header| {
                if header.payload_size_bytes != metadata.processed_verifying_key_len {
                    return Err(
                        "Kagemusha V4 qualified verifier-key projection length changed".to_owned(),
                    );
                }
                consume(reader, metadata.processed_verifying_key_len)
            },
        )
    }
}

impl KagemushaAuthenticatedArtifactSourceV4 for KagemushaQualifiedArtifactSourceV4 {
    fn authenticated_release(&self) -> &KagemushaAuthenticatedReleaseV4 {
        &self.authenticated_release
    }

    fn with_framed_artifact(
        &self,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
        consume: &mut dyn FnMut(&mut dyn KagemushaArtifactReadSeekV4) -> Result<(), String>,
    ) -> Result<(), String> {
        self.source.with_framed_artifact(parity, kind, consume)
    }
}

/// Semantically qualify all eight roles of one authenticated production source.
///
/// Qualification is performed inside core and returns an opaque, unforgeable
/// source wrapper.  Parameters and keys are loaded one parity at a time and
/// dropped before the opposite parity is opened.
pub fn qualify_kagemusha_authenticated_artifact_source_v4(
    source: Arc<dyn KagemushaAuthenticatedArtifactSourceV4>,
) -> Result<KagemushaQualifiedArtifactSourceV4, String> {
    let authenticated_release = source.authenticated_release().clone();
    super::kagemusha_recursion_adapter::qualify_kagemusha_authenticated_artifact_source_v4(
        source,
        authenticated_release,
    )
}

struct KagemushaSourceCallbackStateV4<T> {
    outcome: Option<Result<T, String>>,
    callback_count: u8,
}

impl<T> KagemushaSourceCallbackStateV4<T> {
    const fn new() -> Self {
        Self {
            outcome: None,
            callback_count: 0,
        }
    }

    fn enter(&mut self) -> Result<(), String> {
        self.callback_count = self.callback_count.saturating_add(1);
        if self.callback_count != 1 {
            return Err(
                "Kagemusha V4 authenticated artifact source invoked its callback more than once"
                    .to_owned(),
            );
        }
        Ok(())
    }

    fn record(&mut self, outcome: Result<T, String>) -> Result<(), String> {
        let callback_result = outcome
            .as_ref()
            .map(|_| ())
            .map_err(std::clone::Clone::clone);
        self.outcome = Some(outcome);
        callback_result
    }

    fn finish(self, source_result: Result<(), String>) -> Result<T, String> {
        if self.callback_count != 1 {
            return Err(format!(
                "Kagemusha V4 authenticated artifact source invoked its callback {} times instead of once",
                self.callback_count
            ));
        }
        match self.outcome {
            Some(Err(error)) => Err(error),
            Some(Ok(parsed)) => {
                source_result?;
                Ok(parsed)
            }
            None => {
                Err("Kagemusha V4 authenticated artifact source omitted its callback".to_owned())
            }
        }
    }
}

/// Authenticate and parse one exact payload from a source-backed release.
///
/// The callback sees a bounded hashing reader and must consume it completely.
/// The selected file is authenticated once before parsing and again over the
/// exact bytes consumed by `parse`.  The source callback must run exactly once.
pub fn with_kagemusha_authenticated_artifact_payload_from_source_v4<T, F>(
    source: &dyn KagemushaAuthenticatedArtifactSourceV4,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
    parse: F,
) -> Result<T, String>
where
    F: FnOnce(
        &mut dyn Read,
        &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
    ) -> Result<T, String>,
{
    let release = source.authenticated_release();
    let descriptor = kagemusha_artifact_descriptor_v4(release.manifest(), parity, kind)?;
    let mut parse = Some(parse);
    let mut state = KagemushaSourceCallbackStateV4::new();

    let source_result = source.with_framed_artifact(parity, kind, &mut |reader| {
        state.enter()?;
        let parse = parse.take().ok_or_else(|| {
            "Kagemusha V4 authenticated artifact source reused its parser".to_owned()
        })?;
        // `&mut dyn KagemushaArtifactReadSeekV4` is itself a sized Read + Seek
        // value, allowing the existing generic core authenticator to retain its
        // concrete-reader API without weakening any checks.
        let mut pinned_reader = reader;
        let parsed = with_kagemusha_pasta_cycle_artifact_payload_v4(
            &mut pinned_reader,
            release,
            descriptor,
            parse,
        );
        state.record(parsed)
    });
    state.finish(source_result)
}

#[cfg(test)]
mod tests {
    use super::KagemushaSourceCallbackStateV4;

    #[test]
    fn source_callback_state_requires_exactly_one_invocation() {
        let omitted = KagemushaSourceCallbackStateV4::<u8>::new()
            .finish(Ok(()))
            .expect_err("omitted callback must fail");
        assert!(omitted.contains("0 times"));

        let mut repeated = KagemushaSourceCallbackStateV4::new();
        repeated.enter().expect("first callback");
        repeated.record(Ok(7_u8)).expect("first result");
        assert!(repeated.enter().is_err(), "second callback must fail");
        let repeated = repeated
            .finish(Ok(()))
            .expect_err("repeated callback must remain failed if source swallows it");
        assert!(repeated.contains("2 times"));
    }

    #[test]
    fn source_callback_state_preserves_parser_and_source_errors() {
        let mut swallowed_parser = KagemushaSourceCallbackStateV4::<u8>::new();
        swallowed_parser.enter().expect("callback");
        assert!(
            swallowed_parser
                .record(Err("parse sentinel".to_owned()))
                .is_err()
        );
        assert_eq!(
            swallowed_parser.finish(Ok(())).expect_err("parse failure"),
            "parse sentinel"
        );

        let mut late_source = KagemushaSourceCallbackStateV4::new();
        late_source.enter().expect("callback");
        late_source.record(Ok(9_u8)).expect("parse");
        assert_eq!(
            late_source
                .finish(Err("source sentinel".to_owned()))
                .expect_err("late source failure"),
            "source sentinel"
        );
    }
}
