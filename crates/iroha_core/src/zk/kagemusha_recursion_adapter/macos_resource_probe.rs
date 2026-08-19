// Native macOS resource queries for the Kagemusha generation-memory guard.

#[cfg(any(target_os = "macos", test))]
fn validate_kagemusha_macos_native_resource_value_v4(
    value: u64,
    returned_size: usize,
    expected_size: usize,
    label: &str,
) -> Result<u64, String> {
    if returned_size != expected_size || value == 0 {
        return Err(format!(
            "native macOS {label} query returned an invalid value"
        ));
    }
    Ok(value)
}

/// Exact public Darwin `rusage_info_v0` prefix consumed by `proc_pid_rusage`.
#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Default)]
struct KagemushaMacosRusageInfoV0 {
    uuid: [u8; 16],
    user_time: u64,
    system_time: u64,
    package_idle_wakeups: u64,
    interrupt_wakeups: u64,
    pageins: u64,
    wired_size: u64,
    resident_size: u64,
    physical_footprint: u64,
    process_start_absolute_time: u64,
    process_exit_absolute_time: u64,
}

#[cfg(target_os = "macos")]
const _: [(); 96] = [(); std::mem::size_of::<KagemushaMacosRusageInfoV0>()];

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "the public Darwin sysctlbyname ABI has no safe standard-library wrapper"
)]
fn kagemusha_physical_memory_bytes_v4() -> Result<u64, String> {
    use std::ffi::{CString, c_void};

    unsafe extern "C" {
        fn sysctlbyname(
            name: *const i8,
            old_value: *mut c_void,
            old_length: *mut usize,
            new_value: *mut c_void,
            new_length: usize,
        ) -> i32;
    }

    let name = CString::new("hw.memsize").expect("static sysctl name contains no NUL");
    let mut bytes = 0u64;
    let mut length = std::mem::size_of::<u64>();
    let result = unsafe {
        sysctlbyname(
            name.as_ptr(),
            (&raw mut bytes).cast(),
            &raw mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return Err(format!(
            "native macOS physical-memory query failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    validate_kagemusha_macos_native_resource_value_v4(
        bytes,
        length,
        std::mem::size_of::<u64>(),
        "physical-memory",
    )
}

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "the public Darwin proc_pid_rusage ABI has no safe standard-library wrapper"
)]
fn kagemusha_process_physical_footprint_bytes_v4() -> Result<u64, String> {
    unsafe extern "C" {
        fn proc_pid_rusage(
            process_id: i32,
            flavor: i32,
            buffer: *mut KagemushaMacosRusageInfoV0,
        ) -> i32;
    }

    const RUSAGE_INFO_V0: i32 = 0;
    let process_id = i32::try_from(std::process::id())
        .map_err(|_| "macOS process identifier does not fit i32".to_owned())?;
    let mut usage = KagemushaMacosRusageInfoV0::default();
    if unsafe { proc_pid_rusage(process_id, RUSAGE_INFO_V0, &raw mut usage) } != 0 {
        return Err(format!(
            "native macOS physical-footprint query failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    validate_kagemusha_macos_native_resource_value_v4(
        usage.physical_footprint,
        std::mem::size_of_val(&usage),
        96,
        "physical-footprint",
    )
}
