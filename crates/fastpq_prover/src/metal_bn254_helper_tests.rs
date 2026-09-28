//! Existing Metal bn254 helper tests qualification controls.

use super::*;
use metal::Device;
#[test]
fn upload_bn254_twiddles_rejects_non_limb_multiple() {
    if Device::system_default().is_none() {
        return;
    }
    let device = Device::system_default().expect("device");
    let err = upload_bn254_twiddles(&device, &[1u64, 2, 3]).expect_err("expected invalid");
    assert!(matches!(err, GpuError::InvalidInput(_)));
}
#[test]
fn upload_bn254_twiddles_rejects_an_empty_metal_buffer() {
    let Some(device) = Device::system_default() else {
        return;
    };
    let err = upload_bn254_twiddles(&device, &[]).expect_err("expected empty rejection");
    assert!(matches!(err, GpuError::InvalidInput(_)));
}
#[test]
fn flatten_bn254_twiddles_concatenates_limbs() {
    let inputs = [[1u64, 2, 3, 4], [5, 6, 7, 8]];
    let flat = super::flatten_bn254_twiddles(&inputs).expect("flatten twiddles");
    assert_eq!(flat, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}
#[test]
fn upload_bn254_coset_requires_four_limbs() {
    if Device::system_default().is_none() {
        return;
    }
    let device = Device::system_default().expect("device");
    let err = upload_bn254_coset(&device, &[1u64, 2, 3]).expect_err("expected invalid");
    assert!(matches!(err, GpuError::InvalidInput(_)));
}
#[test]
fn validate_bn254_twiddles_shape_checks_length() {
    let ok = super::validate_bn254_twiddles_shape(2, &[[0u64; 4]; 3]).is_ok();
    assert!(ok, "expected shape to be valid");
    let err =
        super::validate_bn254_twiddles_shape(2, &[[0u64; 4]; 4]).expect_err("expected shape error");
    assert!(matches!(err, GpuError::InvalidInput(_)));
}
#[test]
fn bn254_twiddle_len_helpers_match_shape() {
    assert_eq!(super::bn254_fft_twiddle_len(2).unwrap(), 3);
    assert!(super::bn254_fft_twiddle_len(0).is_err());
    assert_eq!(super::bn254_lde_twiddle_len(2, 1).unwrap(), 7);
    assert!(super::bn254_lde_twiddle_len(0, 1).is_err());
    assert!(super::bn254_lde_twiddle_len(2, 0).is_err());
}
#[test]
fn bn254_twiddle_len_helpers_reject_oversized_logs_without_panicking() {
    assert!(matches!(
        super::bn254_fft_twiddle_len(u32::MAX),
        Err(GpuError::InvalidInput(_))
    ));
    assert!(matches!(
        super::bn254_lde_twiddle_len(u32::MAX, 1),
        Err(GpuError::InvalidInput(_))
    ));
    assert!(matches!(
        super::bn254_lde_twiddle_len(1, u32::MAX),
        Err(GpuError::InvalidInput(_))
    ));
}
#[test]
fn stage_bn254_twiddles_rejects_zero_log() {
    if Device::system_default().is_none() {
        return;
    }
    let device = Device::system_default().expect("device");
    let err = super::stage_bn254_twiddles(&device, 0).expect_err("expected log_size rejection");
    assert!(matches!(err, GpuError::InvalidInput(_)));
}
#[test]
fn stage_bn254_twiddles_matches_expected_size() {
    if Device::system_default().is_none() {
        return;
    }
    let device = Device::system_default().expect("device");
    let log_size = 3;
    let buffer = super::stage_bn254_twiddles(&device, log_size).expect("twiddles");
    let expected_twiddles = super::bn254_fft_twiddle_len(log_size).unwrap();
    let expected_bytes = expected_twiddles * BN254_LIMBS * std::mem::size_of::<u64>();
    assert_eq!(buffer.length() as usize, expected_bytes);
}
#[test]
fn bn254_status_runs_smoke_checks() {
    if Device::system_default().is_none() {
        return;
    }
    let _gpu_lane = crate::backend::acquire_gpu_lane();
    match super::bn254_status() {
        Ok(()) => {}
        Err(GpuError::Unsupported(_)) => return,
        Err(err) => panic!("BN254 status smoke test failed: {err}"),
    }
}
