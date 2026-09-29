//! Target `open(2)` flags for symlink-safe SoraFS node filesystem access.
//!
//! Linux and Android take the per-architecture bits from `rustix`; the Apple and BSD values are
//! architecture-independent. Off Unix the helpers are no-ops.
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("SoraFS node filesystem flags are not qualified for this Unix target");

/// Refuse to follow a final-component symlink when `options` opens a path.
pub(crate) fn set_no_follow_flag(options: &mut OpenOptions) {
    #[cfg(unix)]
    options.custom_flags(platform_no_follow_flag());
    #[cfg(not(unix))]
    let _ = options;
}

/// Open only a directory, and never through a final-component symlink.
#[cfg(unix)]
pub(crate) fn set_directory_no_follow_flags(options: &mut OpenOptions) {
    options.custom_flags(platform_no_follow_flag() | platform_directory_only_flag());
}

/// Target `O_NOFOLLOW` bit.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn platform_no_follow_flag() -> i32 {
    rustix::fs::OFlags::NOFOLLOW.bits() as i32
}
/// Target `O_NOFOLLOW` bit.
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
pub(crate) const fn platform_no_follow_flag() -> i32 {
    0x100
}

/// Target `O_DIRECTORY` bit.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn platform_directory_only_flag() -> i32 {
    rustix::fs::OFlags::DIRECTORY.bits() as i32
}
/// Target `O_DIRECTORY` bit.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) const fn platform_directory_only_flag() -> i32 {
    0x0010_0000
}
/// Target `O_DIRECTORY` bit.
#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
pub(crate) const fn platform_directory_only_flag() -> i32 {
    0x0002_0000
}
/// Target `O_DIRECTORY` bit.
#[cfg(target_os = "dragonfly")]
pub(crate) const fn platform_directory_only_flag() -> i32 {
    0x0800_0000
}
/// Target `O_DIRECTORY` bit.
#[cfg(target_os = "netbsd")]
pub(crate) const fn platform_directory_only_flag() -> i32 {
    0x0020_0000
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pin the ABI value of every qualified target so neither a dependency change nor an edited
    // constant can silently weaken the symlink and directory checks.
    #[cfg(all(
        target_os = "linux",
        any(
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )
    ))]
    #[test]
    fn linux_open_flags_match_low_flag_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x8000);
        assert_eq!(platform_directory_only_flag(), 0x4000);
    }
    #[cfg(all(
        target_os = "linux",
        not(any(
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        ))
    ))]
    #[test]
    fn linux_open_flags_match_generic_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
        assert_eq!(platform_directory_only_flag(), 0x10000);
    }
    #[cfg(all(
        target_os = "android",
        any(target_arch = "aarch64", target_arch = "arm")
    ))]
    #[test]
    fn android_arm_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x8000);
        assert_eq!(platform_directory_only_flag(), 0x4000);
    }
    #[cfg(all(
        target_os = "android",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    #[test]
    fn android_x86_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
        assert_eq!(platform_directory_only_flag(), 0x10000);
    }
    #[cfg(all(target_os = "android", target_arch = "riscv64"))]
    #[test]
    fn android_riscv64_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x400000);
        assert_eq!(platform_directory_only_flag(), 0x200000);
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn apple_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
        assert_eq!(platform_directory_only_flag(), 0x0010_0000);
    }
    #[cfg(target_os = "freebsd")]
    #[test]
    fn freebsd_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
        assert_eq!(platform_directory_only_flag(), 0x0002_0000);
    }
    #[cfg(target_os = "dragonfly")]
    #[test]
    fn dragonfly_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
        assert_eq!(platform_directory_only_flag(), 0x0800_0000);
    }
    #[cfg(target_os = "openbsd")]
    #[test]
    fn openbsd_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
        assert_eq!(platform_directory_only_flag(), 0x0002_0000);
    }
    #[cfg(target_os = "netbsd")]
    #[test]
    fn netbsd_open_flags_match_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
        assert_eq!(platform_directory_only_flag(), 0x0020_0000);
    }
}
