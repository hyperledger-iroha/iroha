//! Narrow access to non-shipping native release evidence fixtures.
use super::{FcmpNativeErrorV1, FcmpOutputCommitmentOpeningV1, FcmpProverInputV1, FcmpTreeRootV1};
/// Construct the native retained FCMP release fixture.
pub fn fcmp_release_fixture_v1(
    maximum: bool,
) -> Result<
    (
        Vec<FcmpProverInputV1>,
        Vec<FcmpOutputCommitmentOpeningV1>,
        FcmpTreeRootV1,
    ),
    FcmpNativeErrorV1,
> {
    super::prover::fcmp_release_fixture_v1(maximum)
}
/// Construct the native release fixture with one invalid retained path.
pub fn fcmp_release_invalid_path_fixture_v1() -> Result<
    (
        Vec<FcmpProverInputV1>,
        Vec<FcmpOutputCommitmentOpeningV1>,
        FcmpTreeRootV1,
    ),
    FcmpNativeErrorV1,
> {
    super::prover::fcmp_release_invalid_path_fixture_v1()
}
