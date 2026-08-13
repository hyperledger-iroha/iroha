/// Lossless four-word projection of one 256-bit digest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionDigest256Projection {
    pub(crate) word0: u64,
    pub(crate) word1: u64,
    pub(crate) word2: u64,
    pub(crate) word3: u64,
}

/// Versioned authentication record for one checked first-release transition.
///
/// Production attaches this record after the dependency-free composed checker
/// accepts the exact projection. The two digests cover the canonical fixed-width
/// encodings of the complete abstract pre/post states; `source_identity` binds
/// the checked relation to the reviewed TLA+ action source. Keeping the witness
/// inside the move-only checked token makes its lifetime end at the same
/// mutation boundary as the accepted projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProductionInFlightFirstReleaseTransitionWitnessV1 {
    pub(crate) schema_version: u16,
    pub(crate) action: u8,
    pub(crate) actor: u128,
    pub(crate) target: u128,
    pub(crate) before_state_digest: ProductionDigest256Projection,
    pub(crate) after_state_digest: ProductionDigest256Projection,
    pub(crate) source_identity: ProductionDigest256Projection,
}

macro_rules! production_in_flight_first_release_witness_binding_body {
    ($projection:expr, $witness:expr) => {{
        $witness.schema_version == 1u16
            && $witness.action == $projection.action
            && $witness.actor == $projection.actor
            && $witness.target == $projection.target
            && $witness.source_identity.word0 == 0x9b9babea9e018b44u64
            && $witness.source_identity.word1 == 0xfb739f96b2690f17u64
            && $witness.source_identity.word2 == 0xe1f8d08aa23a38f4u64
            && $witness.source_identity.word3 == 0x2a16ecef1e858f7du64
    }};
}

/// Reverse ownership classification for a terminal Commit or release state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)] // Consumed by the verification harness and refinement tests.
pub(crate) struct ProductionInFlightFirstReleaseTerminalOwnerProjection {
    pub(crate) ordinary_fifo_owner: bool,
    pub(crate) canonical_wsv_owner: bool,
    pub(crate) commit_terminal: bool,
    pub(crate) release_terminal: bool,
}

/// Opaque evidence that one production transition gate accepted a projection.
///
/// The field is private so callers cannot manufacture authorization from a
/// projection they already hold. Every constructor below evaluates the
/// executable kernel and returns `None` on rejection; consumers must acquire
/// this token before crossing their state-changing linearization point.
#[must_use = "checked transition evidence must be consumed at the authorized mutation boundary"]
#[derive(Debug, PartialEq, Eq)]
pub struct CheckedProductionTransition<P> {
    projection: P,
    first_release_witness: Option<ProductionInFlightFirstReleaseTransitionWitnessV1>,
}

impl<P> CheckedProductionTransition<P> {
    const fn unwitnessed(projection: P) -> Self {
        Self {
            projection,
            first_release_witness: None,
        }
    }

    /// Borrow the exact accepted projection without consuming its authority.
    ///
    /// This supports deterministic composition checks while retaining the
    /// move-only token for the authorized mutation boundary.
    #[must_use]
    pub(crate) const fn accepted_projection(&self) -> &P {
        &self.projection
    }

    /// Bind the production-authenticated first-release witness to this token.
    ///
    /// This is crate-private so only the production wrapper around the shared
    /// executable checker can attach a witness. Test and Verus instantiations of
    /// the dependency-free checker deliberately produce an unwitnessed token.
    #[must_use]
    pub(super) fn with_first_release_witness(
        mut self,
        witness: ProductionInFlightFirstReleaseTransitionWitnessV1,
    ) -> Self {
        self.first_release_witness = Some(witness);
        self
    }

    /// Borrow the versioned witness attached by the production checker.
    #[must_use]
    pub(crate) const fn first_release_witness(
        &self,
    ) -> Option<&ProductionInFlightFirstReleaseTransitionWitnessV1> {
        self.first_release_witness.as_ref()
    }

    /// Consume the checked token and recover the exact accepted projection.
    #[must_use]
    pub fn into_projection(self) -> P {
        self.projection
    }
}
