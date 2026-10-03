//! Explicit SHA3 device readiness, independent public KAT and permanent quarantine.
use crate::{
    Digest384GpuBackendV1 as Backend, Error, Result,
    keccak_batch::{self, Job},
};
use fastpq_isi::keccak256::Sha3_256V1;
use std::sync::Mutex;
#[cfg(test)]
use zeroize::Zeroizing;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Readiness {
    Unchecked,
    Ready,
    Quarantined,
}
static METAL: Mutex<Readiness> = Mutex::new(Readiness::Unchecked);
/// Public probe counts are charged cold even if readiness is already cached.
pub const KAT_HASHES: usize = 8;
const KATS: [(usize, usize, [u8; 32]); KAT_HASHES] = [
    (
        0,
        0,
        [
            0xa7, 0xff, 0xc6, 0xf8, 0xbf, 0x1e, 0xd7, 0x66, 0x51, 0xc1, 0x47, 0x56, 0xa0, 0x61,
            0xd6, 0x62, 0xf5, 0x80, 0xff, 0x4d, 0xe4, 0x3b, 0x49, 0xfa, 0x82, 0xd8, 0x0a, 0x4b,
            0x80, 0xf8, 0x43, 0x4a,
        ],
    ),
    (
        1,
        0,
        [
            0x27, 0x67, 0xf1, 0x5c, 0x8a, 0xf2, 0xf2, 0xc7, 0x22, 0x5d, 0x52, 0x73, 0xfd, 0xd6,
            0x83, 0xed, 0xc7, 0x14, 0x11, 0x0a, 0x98, 0x7d, 0x10, 0x54, 0x69, 0x7c, 0x34, 0x8a,
            0xed, 0x4e, 0x6c, 0xc7,
        ],
    ),
    (
        135,
        7,
        [
            0xfe, 0x62, 0x29, 0x33, 0xe1, 0x8e, 0xd7, 0x12, 0x6d, 0x12, 0x01, 0xc4, 0xd7, 0xa6,
            0x28, 0x38, 0x60, 0x25, 0x08, 0xcb, 0x20, 0x26, 0x03, 0x9e, 0x6e, 0xa5, 0xb3, 0x0b,
            0x8c, 0xba, 0x6e, 0x81,
        ],
    ),
    (
        136,
        135,
        [
            0x71, 0x92, 0x49, 0xbe, 0xa3, 0xbb, 0xaa, 0xcb, 0xa4, 0x88, 0x35, 0xae, 0xbc, 0x7e,
            0xf4, 0x85, 0xa4, 0x25, 0x9c, 0x0c, 0x36, 0xed, 0x19, 0x5f, 0xc0, 0x3c, 0x04, 0x2c,
            0xfa, 0x6b, 0x0c, 0x5d,
        ],
    ),
    (
        137,
        136,
        [
            0x7a, 0xa8, 0xb7, 0x96, 0x11, 0xd7, 0xfb, 0x30, 0xfa, 0xc4, 0x5d, 0xdd, 0xe3, 0x9b,
            0x73, 0x7b, 0xe5, 0x34, 0x6c, 0x7a, 0xf6, 0xbb, 0x36, 0xbc, 0x95, 0x8f, 0xcf, 0x21,
            0x73, 0x7a, 0xe7, 0x76,
        ],
    ),
    (
        272,
        137,
        [
            0x80, 0x12, 0xbc, 0x39, 0x15, 0xa1, 0xdf, 0x25, 0xf2, 0xa6, 0x74, 0x7b, 0xc4, 0x4f,
            0x0a, 0x5f, 0xf8, 0x0d, 0xc3, 0x9f, 0x5d, 0xa3, 0x50, 0x8f, 0x35, 0x94, 0xf8, 0x86,
            0x73, 0xf9, 0x2d, 0x5c,
        ],
    ),
    (
        4096,
        271,
        [
            0x51, 0x7d, 0xd3, 0xd0, 0xf9, 0x4d, 0x89, 0xad, 0x85, 0x82, 0x6e, 0x35, 0x1d, 0x3c,
            0xb1, 0x7a, 0x1b, 0x81, 0x23, 0xb3, 0x9d, 0xa5, 0x1a, 0xb6, 0x4d, 0x25, 0xa2, 0x4d,
            0x22, 0xa3, 0xd8, 0xd5,
        ],
    ),
    (
        8328,
        136,
        [
            0xd3, 0x3a, 0xb0, 0xf1, 0x84, 0xab, 0xa8, 0xe0, 0xdc, 0xe7, 0x7e, 0x27, 0x14, 0x06,
            0xce, 0xc3, 0x39, 0x45, 0x16, 0x66, 0xdc, 0x12, 0x7a, 0xfe, 0x59, 0xf1, 0xc0, 0x75,
            0xe1, 0x3b, 0x52, 0xf2,
        ],
    ),
];
fn error(details: &'static str) -> Error {
    Error::NativeDigestExecution {
        details: details.into(),
    }
}
fn readiness(backend: Backend) -> Result<&'static Mutex<Readiness>> {
    match backend {
        Backend::Metal => Ok(&METAL),
        Backend::Cuda => Err(error("required SHA3 CUDA continuation is unavailable")),
    }
}
fn dispatch(backend: Backend, jobs: &[Job<'_>], output: &mut [[u8; 32]]) -> Result<()> {
    match backend {
        Backend::Metal => {
            #[cfg(target_os = "macos")]
            {
                crate::metal::keccak256::hash(jobs, output).map_err(|e| {
                    Error::NativeDigestExecution {
                        details: e.to_string(),
                    }
                })
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (jobs, output);
                Err(error("required SHA3 Metal continuation is unavailable"))
            }
        }
        Backend::Cuda => Err(error("required SHA3 CUDA continuation is unavailable")),
    }
}
impl Readiness {
    fn preflight(
        &mut self,
        execute: &mut impl FnMut(&[Job<'_>], &mut [[u8; 32]]) -> Result<()>,
    ) -> Result<()> {
        match self {
            Self::Ready => return Ok(()),
            Self::Quarantined => return Err(error("SHA3 device remains quarantined")),
            Self::Unchecked => {}
        }
        // Only these fixed public messages are inspected before readiness. The
        // previous typed witness/source callbacks have not been invoked.
        let messages = KATS.map(|(len, _, _)| {
            (0..len)
                .map(|i| (i * 73 + len).to_le_bytes()[0])
                .collect::<Vec<_>>()
        });
        let prefixes = core::array::from_fn::<_, KAT_HASHES, _>(|index| {
            let mut hash = Sha3_256V1::new();
            hash.update(&messages[index][..KATS[index].1]);
            hash
        });
        let jobs = core::array::from_fn::<_, KAT_HASHES, _>(|index| {
            Job::new(&prefixes[index], &messages[index][KATS[index].1..])
        });
        let mut output = [[0; 32]; KAT_HASHES];
        // Close before device execution; errors or unwinding cannot reopen it.
        *self = Self::Quarantined;
        execute(&jobs, &mut output)?;
        if output
            .iter()
            .zip(KATS)
            .any(|(actual, (_, _, expected))| *actual != expected)
        {
            return Err(error(
                "SHA3 device failed its independent FIPS202 known answers",
            ));
        }
        *self = Self::Ready;
        Ok(())
    }
    fn execute(
        &mut self,
        jobs: &[Job<'_>],
        output: &mut [[u8; 32]],
        dispatch: &mut impl FnMut(&[Job<'_>], &mut [[u8; 32]]) -> Result<()>,
    ) -> Result<()> {
        if *self != Self::Ready {
            return Err(error("SHA3 request requires completed public readiness"));
        }
        *self = Self::Quarantined;
        dispatch(jobs, output)?;
        *self = Self::Ready;
        Ok(())
    }
}
/// Call before entropy/private callbacks. It never receives a private job.
pub fn preflight(backend: Backend) -> Result<()> {
    if crate::gpu::transform_completion_uncertain_v1() {
        return Err(error(
            "uncertain device completion closes SHA3 private staging",
        ));
    }
    let mut state = readiness(backend)?
        .lock()
        .map_err(|_| error("SHA3 readiness lock poisoned"))?;
    state.preflight(&mut |jobs, out| dispatch(backend, jobs, out))
}
/// Admission was public; successful return exposes only completed exact outputs.
pub fn hash(backend: Backend, jobs: &[Job<'_>], output: &mut [[u8; 32]]) -> Result<()> {
    keccak_batch::validate(jobs, output.len())?;
    if crate::gpu::transform_completion_uncertain_v1() {
        return Err(error("uncertain device completion closes SHA3 dispatch"));
    }
    let mut state = readiness(backend)?
        .lock()
        .map_err(|_| error("SHA3 readiness lock poisoned"))?;
    // A caller cannot bypass public readiness by calling this lower executor.
    state.execute(jobs, output, &mut |jobs, out| dispatch(backend, jobs, out))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_answers_are_exact_scalar_outputs_and_preflight_runs_once() {
        let mut state = Readiness::Unchecked;
        let mut calls = 0;
        for _ in 0..2 {
            state
                .preflight(&mut |jobs, out| {
                    calls += 1;
                    for (job, out) in jobs.iter().zip(out) {
                        *out = job.scalar().into_bytes();
                    }
                    Ok(())
                })
                .unwrap();
        }
        assert_eq!(calls, 1);
        assert_eq!(state, Readiness::Ready);
    }
    #[test]
    fn kat_failure_error_and_unwind_permanently_close_readiness() {
        for kind in 0..3 {
            let mut state = Readiness::Unchecked;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                state.preflight(&mut |_, out| {
                    if kind == 0 {
                        out.fill([0; 32]);
                        Ok(())
                    } else if kind == 1 {
                        Err(error("injected"))
                    } else {
                        panic!("injected")
                    }
                })
            }));
            assert!(result.is_err() || result.unwrap().is_err());
            assert_eq!(state, Readiness::Quarantined);
            assert!(
                state
                    .preflight(&mut |_, _| panic!("must not redispatch"))
                    .is_err()
            );
        }
    }
    #[test]
    fn request_requires_ready_and_failure_never_returns_to_ready() {
        let hash = Sha3_256V1::new();
        let jobs = [Job::new(&hash, b"secret fixture")];
        let mut output = Zeroizing::new([[0; 32]; 1]);
        for initial in [Readiness::Unchecked, Readiness::Quarantined] {
            let mut state = initial;
            assert!(
                state
                    .execute(&jobs, &mut *output, &mut |_, _| panic!("no readiness"))
                    .is_err()
            );
        }
        let mut state = Readiness::Ready;
        assert!(
            state
                .execute(&jobs, &mut *output, &mut |_, _| Err(error("injected")))
                .is_err()
        );
        assert_eq!(state, Readiness::Quarantined);
    }
    #[test]
    #[ignore = "requires actual Metal SHA3 KAT and private continuation completion"]
    fn actual_metal_continuations_match_all_scalar_boundaries() {
        let _lane = crate::backend::acquire_gpu_lane();
        preflight(Backend::Metal).unwrap();
        let bytes = (0..MAX_BODY_FOR_TEST)
            .map(|i| (i * 73).to_le_bytes()[0])
            .collect::<Vec<_>>();
        for prefix_length in [0, 1, 7, 135, 136, 137, 271] {
            let mut prefix = Sha3_256V1::new();
            prefix.update(&bytes[..prefix_length]);
            for length in [0, 1, 7, 8, 135, 136, 137, 272, 8192] {
                let jobs = [Job::new(&prefix, &bytes[..length])];
                let mut output = [[0; 32]; 1];
                hash(Backend::Metal, &jobs, &mut output).unwrap();
                assert_eq!(output[0], jobs[0].scalar().into_bytes());
            }
        }
    }
    const MAX_BODY_FOR_TEST: usize = 8192;
}
