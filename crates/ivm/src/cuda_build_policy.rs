//! Pure CUDA artifact-mode policy shared by the IVM build and its focused tests.

/// Explicit CUDA PTX artifact mode selected by a build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CudaPtxMode {
    Bundled,
    Generate,
    Check,
}

/// Parse only the three reviewed CUDA artifact modes.
pub(crate) fn parse_cuda_ptx_mode(value: &str) -> Result<CudaPtxMode, String> {
    match value {
        "bundled" => Ok(CudaPtxMode::Bundled),
        "generate" => Ok(CudaPtxMode::Generate),
        "check" => Ok(CudaPtxMode::Check),
        _ => Err(format!(
            "IVM_CUDA_PTX_MODE must be one of bundled, generate, or check; got {value:?}"
        )),
    }
}

/// Keep unsigned local generation out of every non-debug shipping profile.
pub(crate) fn reject_generated_release_ptx(mode: CudaPtxMode, profile: &str) -> Result<(), String> {
    if mode == CudaPtxMode::Generate && profile != "debug" {
        return Err("non-debug CUDA builds must use signed bundled or checked PTX".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_ptx_mode_parser_is_strict() {
        assert_eq!(parse_cuda_ptx_mode("bundled"), Ok(CudaPtxMode::Bundled));
        assert_eq!(parse_cuda_ptx_mode("generate"), Ok(CudaPtxMode::Generate));
        assert_eq!(parse_cuda_ptx_mode("check"), Ok(CudaPtxMode::Check));
        assert!(parse_cuda_ptx_mode("fallback").is_err());
        assert!(parse_cuda_ptx_mode("BUNDLED").is_err());
        assert!(parse_cuda_ptx_mode("").is_err());
    }

    #[test]
    fn non_debug_profiles_reject_unsigned_generated_ptx() {
        for profile in ["release", "deploy", "local-release", "profiling", ""] {
            assert!(reject_generated_release_ptx(CudaPtxMode::Generate, profile).is_err());
            assert!(reject_generated_release_ptx(CudaPtxMode::Check, profile).is_ok());
            assert!(reject_generated_release_ptx(CudaPtxMode::Bundled, profile).is_ok());
        }
        assert!(reject_generated_release_ptx(CudaPtxMode::Generate, "debug").is_ok());
    }
}
