extern crate cc;
extern crate glob;

use std::env;
use std::path::{Path, PathBuf};

macro_rules! build_clean {
    ($variant:expr) => {
        let internals_include_path = &std::env::var("DEP_PQCRYPTO_INTERNALS_INCLUDEPATH").unwrap();
        let common_dir = Path::new("pqclean/common");

        let mut builder = cc::Build::new();
        let target_dir: PathBuf = ["pqclean", "crypto_kem", $variant, "clean"]
            .iter()
            .collect();

        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
        if target_os == "wasi" {
            let wasi_sdk_path =
                &std::env::var("WASI_SDK_DIR").expect("missing environment variable: WASI_SDK_DIR");
            builder.flag(format!("--sysroot={}", wasi_sdk_path).as_str());
        }

        let scheme_files = glob::glob(target_dir.join("*.c").to_str().unwrap()).unwrap();

        builder
            .include(internals_include_path)
            .include(&common_dir)
            .include(target_dir)
            .files(
                scheme_files
                    .into_iter()
                    .map(|p| p.unwrap().to_string_lossy().into_owned()),
            );
        builder.compile(format!("{}_clean", $variant).as_str());
    };
}

macro_rules! build_avx2 {
    ($variant:expr) => {
        let internals_include_path = &std::env::var("DEP_PQCRYPTO_INTERNALS_INCLUDEPATH").unwrap();
        let common_dir = Path::new("pqclean/common");

        let mut builder = cc::Build::new();
        let target_dir: PathBuf = ["pqclean", "crypto_kem", $variant, "avx2"].iter().collect();

        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
        if target_os == "wasi" {
            let wasi_sdk_path =
                &std::env::var("WASI_SDK_DIR").expect("missing environment variable: WASI_SDK_DIR");
            builder.flag(format!("--sysroot={}", wasi_sdk_path).as_str());
        }

        let scheme_files = glob::glob(target_dir.join("*.[csS]").to_str().unwrap()).unwrap();
        let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();
        if target_env == "msvc" {
            builder.flag("/arch:AVX2");
        } else {
            builder
                .flag("-mavx2")
                .flag("-mbmi2")
                .flag("-mbmi")
                .flag("-maes")
                .flag("-mpopcnt")
                .flag("-mpclmul");
        }

        builder
            .include(internals_include_path)
            .include(&common_dir)
            .include(target_dir)
            .files(
                scheme_files
                    .into_iter()
                    .map(|p| p.unwrap().to_string_lossy().into_owned()),
            );
        builder.compile(format!("{}_avx2", $variant).as_str());
    };
}

macro_rules! build_aarch64 {
    ($variant:expr) => {
        let internals_include_path = &std::env::var("DEP_PQCRYPTO_INTERNALS_INCLUDEPATH").unwrap();
        let common_dir = Path::new("pqclean/common");

        let mut builder = cc::Build::new();
        let target_dir: PathBuf = ["pqclean", "crypto_kem", $variant, "aarch64"]
            .iter()
            .collect();

        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
        if target_os == "wasi" {
            let wasi_sdk_path =
                &std::env::var("WASI_SDK_DIR").expect("missing environment variable: WASI_SDK_DIR");
            builder.flag(format!("--sysroot={}", wasi_sdk_path).as_str());
        }

        let scheme_files = glob::glob(target_dir.join("*.[csS]").to_str().unwrap()).unwrap();
        builder.flag("-march=armv8-a");

        builder
            .include(internals_include_path)
            .include(&common_dir)
            .include(target_dir)
            .files(
                scheme_files
                    .into_iter()
                    .map(|p| p.unwrap().to_string_lossy().into_owned()),
            );
        builder.compile(format!("{}_aarch64", $variant).as_str());
    };
}

fn main() {
    #[allow(unused_variables)]
    let aes_enabled = env::var("CARGO_FEATURE_AES").is_ok();
    #[allow(unused_variables)]
    let avx2_enabled = env::var("CARGO_FEATURE_AVX2").is_ok();
    #[allow(unused_variables)]
    let neon_enabled = env::var("CARGO_FEATURE_NEON").is_ok();
    #[allow(unused_variables)]
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    #[allow(unused_variables)]
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    #[allow(unused_variables)]
    let is_windows = target_os == "windows";
    #[allow(unused_variables)]
    let is_macos = target_os == "macos";

    build_clean!("ml-kem-512");
    if target_arch == "x86_64" && avx2_enabled && !is_windows && !is_macos {
        build_avx2!("ml-kem-512");
    }
    if target_arch == "aarch64" && neon_enabled {
        build_aarch64!("ml-kem-512");
    }
    build_clean!("ml-kem-768");
    if target_arch == "x86_64" && avx2_enabled && !is_windows && !is_macos {
        build_avx2!("ml-kem-768");
    }
    if target_arch == "aarch64" && neon_enabled {
        build_aarch64!("ml-kem-768");
    }
    build_clean!("ml-kem-1024");
    if target_arch == "x86_64" && avx2_enabled && !is_windows && !is_macos {
        build_avx2!("ml-kem-1024");
    }
    if target_arch == "aarch64" && neon_enabled {
        build_aarch64!("ml-kem-1024");
    }

    println!("cargo::rustc-check-cfg=cfg(enable_x86_avx2)");
    if target_arch == "x86_64" && avx2_enabled && !is_windows && !is_macos {
        // Print enableing flag for AVX2 implementation
        println!("cargo:rustc-cfg=enable_x86_avx2");
    }
    println!("cargo::rustc-check-cfg=cfg(enable_aarch64_neon)");
    if target_arch == "aarch64" && neon_enabled {
        // Print enableing flag for AARCH64 implementation
        println!("cargo:rustc-cfg=enable_aarch64_neon");
    }
}
