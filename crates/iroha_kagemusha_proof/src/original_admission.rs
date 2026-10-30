//! Explicit original-admission dispatch shared by typed native source owners.
//!
//! Existing import APIs always select strict source reconstruction. A built-in
//! member is an opaque, closed-profile capability, never caller-provided pins.
//! It attests only the exact source compiled into that member. Constructing the
//! supplied circuit preserves typed preflight checks. By default it does not
//! compare its actual tables with the member, because every later proof rebuild
//! fingerprints its actual source and rejects a mismatch. A member selected from
//! a source-verifying profile makes that same comparison here, so a build or
//! installation check refuses a drifted circuit before any device proves it.
//! The installation owner must bind every plan and route to the complete
//! selected profile before granting a wallet owner.

use iroha_pasta::{CancellationToken, PastaCurve};
use iroha_plonk::{
    DescriptorBinding,
    frontend::Circuit,
    keys::{
        BuiltinSourceMemberV1, SourceBoundVerifyingKeyV2, VerifyingKey,
        check_source_v2_cancellable,
        pk::artifact::{Error, ReadConfig},
    },
    pcs::ipa::PinnedParams,
};

/// Closed original-source admission selected by the owning native operation.
#[derive(Clone, Copy)]
pub enum OriginalAdmission {
    /// Reconstruct the supplied source and compare its actual commitments.
    Strict,
    /// Use the exact compiled source member selected by the installation owner.
    /// A source-verifying member also checks the supplied circuit's actual tables.
    Builtin(BuiltinSourceMemberV1),
}

impl OriginalAdmission {
    // Preserve the original source factory and all owner checks; only the expensive
    // witnessless synthesis/commitment pass differs for an exact closed member, and
    // a source-verifying member keeps the synthesis without the commitments.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn admit<C: PastaCurve, T: Circuit<C::ScalarExt>>(
        self,
        original: &[u8],
        binding: &DescriptorBinding,
        installed_vk: &VerifyingKey<C>,
        params: &PinnedParams<C>,
        circuit: &T,
        config: ReadConfig,
        cancellation: Option<&CancellationToken>,
    ) -> Result<SourceBoundVerifyingKeyV2<C>, Error> {
        match self {
            Self::Strict => SourceBoundVerifyingKeyV2::from_masked_artifact_v2_cancellable(
                original,
                binding,
                params,
                circuit,
                config,
                cancellation,
            ),
            Self::Builtin(member) => {
                let admitted = member
                    .admit_originals_with_metadata_cancellable::<C>(
                        binding.encoded(),
                        installed_vk.to_bytes(),
                        original,
                        binding,
                        installed_vk,
                        params,
                        config,
                        cancellation,
                    )
                    .map_err(|error| {
                        if error.is_cancelled() {
                            Error::Cancelled
                        } else {
                            Error::Source
                        }
                    })?;
                if member.verifies_source() {
                    check_source_v2_cancellable(params, circuit, &admitted.view(), cancellation)
                        .map_err(|error| {
                            if error.is_cancelled() {
                                Error::Cancelled
                            } else {
                                Error::Source
                            }
                        })?;
                }
                Ok(admitted)
            }
        }
    }
}
