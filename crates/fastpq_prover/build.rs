//! CUDA build helper for the FASTPQ prover.
//!
//! The runtime kernels are generated via NVRTC/Metal in `gpu.rs`. This build
//! script remains to support the static CUDA path when `fastpq-gpu` is enabled.
// SPDX-License-Identifier: Apache-2.0

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-env-changed=FASTPQ_GPU");
    println!("cargo:rerun-if-env-changed=FASTPQ_SKIP_GPU_BUILD");
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-changed=cuda/fastpq_cuda.cu");
    println!("cargo:rerun-if-changed=metal/include/params.h");
    println!("cargo:rerun-if-changed=metal/kernels/field.metal");
    println!("cargo:rerun-if-changed=metal/kernels/ntt_stage.metal");
    println!("cargo:rerun-if-changed=metal/kernels/poseidon2.metal");

    let cuda_feature = env::var_os("CARGO_FEATURE_CUDA").is_some();
    let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if fastpq_gpu_feature
        && target_os == "macos"
        && let Err(error) = compile_metal_shaders()
    {
        println!("cargo:warning={error}");
        println!("cargo:rustc-env=FASTPQ_METAL_LIB=");
    }
    if !cuda_feature && !fastpq_gpu_feature {
        println!("cargo:rustc-cfg=fastpq_cuda_unavailable");
        return;
    }
    if target_os == "macos" && !cuda_feature {
        // Metal hosts skip the static CUDA path; the runtime will fall back to the Metal
        // backend without surfacing an unnecessary warning.
        println!("cargo:rustc-cfg=fastpq_cuda_unavailable");
        return;
    }
    if env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some() {
        println!("cargo:warning=FASTPQ_SKIP_GPU_BUILD set; CUDA backend disabled.");
        println!("cargo:rustc-cfg=fastpq_cuda_unavailable");
        return;
    }
    if !nvcc_available() {
        println!("cargo:warning=nvcc not found; CUDA backend disabled.");
        println!("cargo:rustc-cfg=fastpq_cuda_unavailable");
        return;
    }

    let cuda_root = locate_cuda_root();
    if let Some(root) = &cuda_root {
        let lib_dir = cuda_lib_dir(root);
        if let Some(path) = lib_dir {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
    }

    // Link against cudart; nvcc will add device runtime automatically.
    println!("cargo:rustc-link-lib=cudart");

    let mut build = cc::Build::new();
    build.cuda(true);
    build.debug(false);
    build
        .file("cuda/fastpq_cuda.cu")
        .flag("-std=c++17")
        .flag("-O3")
        .flag("-lineinfo")
        .flag("-arch=sm_80")
        .flag("-Xptxas=-O3")
        .flag("-Xptxas=-fmad=false")
        .flag("-Xcompiler=-fno-fast-math")
        .flag("-Xcudafe=--display_error_number");

    if let Some(root) = &cuda_root {
        let include_dir = root.join("include");
        if include_dir.exists() {
            build.include(include_dir);
        }
    }

    if let Some(host_compiler) = select_cuda_host_compiler(&target_os) {
        build.ccbin(false);
        build.flag(format!("-ccbin={}", host_compiler.display()));
    } else if target_os == "linux" && !explicit_cxx_configured() {
        build.ccbin(false);
    }

    build.compile("fastpq_cuda");
}

fn nvcc_available() -> bool {
    Command::new("nvcc")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn compile_metal_shaders() -> Result<(), String> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);
    let kernels = [
        ("ntt_stage", Path::new("metal/kernels/ntt_stage.metal")),
        ("poseidon2", Path::new("metal/kernels/poseidon2.metal")),
        ("bn254", Path::new("metal/kernels/bn254.metal")),
    ];
    let include_dir = Path::new("metal/include");
    let kernels_dir = Path::new("metal/kernels");
    for (_, path) in &kernels {
        if !path.exists() {
            return Err(format!("Metal shader source missing: {}", path.display()));
        }
    }

    let metallib_path = out_dir.join("fastpq.metallib");
    let modules_cache = out_dir.join("metal_modules");
    std::fs::create_dir_all(&modules_cache).map_err(|err| err.to_string())?;

    let mut air_paths = Vec::new();
    for (name, source) in &kernels {
        let air_path = out_dir.join(format!("{name}.air"));
        let status = Command::new("xcrun")
            .arg("metal")
            .arg("-std=metal3.0")
            .arg("-O3")
            .arg("-c")
            .arg(format!("-fmodules-cache-path={}", modules_cache.display()))
            .arg("-I")
            .arg(include_dir.display().to_string())
            .arg("-I")
            .arg(kernels_dir.display().to_string())
            .arg(source)
            .arg("-o")
            .arg(&air_path)
            .output()
            .map_err(|err| err.to_string())?;
        if !status.status.success() {
            let stderr = String::from_utf8_lossy(&status.stderr);
            return Err(format!(
                "failed to compile Metal shader {}: {}",
                source.display(),
                stderr.trim()
            ));
        }
        air_paths.push(air_path);
    }

    let metallib_exe = Command::new("xcrun")
        .arg("--find")
        .arg("metallib")
        .output()
        .ok()
        .and_then(|output| {
            output.status.success().then(|| {
                let path = String::from_utf8_lossy(&output.stdout);
                PathBuf::from(path.trim())
            })
        })
        .or_else(|| {
            Command::new("xcrun")
                .arg("--find")
                .arg("metal")
                .output()
                .ok()
                .and_then(|output| {
                    output.status.success().then(|| {
                        let path = String::from_utf8_lossy(&output.stdout);
                        let mut candidate = PathBuf::from(path.trim());
                        candidate.set_file_name("metallib");
                        candidate
                    })
                })
                .filter(|path| path.exists())
        })
        .ok_or_else(|| "failed to locate metallib binary".to_string())?;

    let mut link_cmd = Command::new(metallib_exe);
    for air in &air_paths {
        link_cmd.arg(air);
    }
    let link = link_cmd
        .arg("-o")
        .arg(&metallib_path)
        .output()
        .map_err(|err| err.to_string())?;
    if !link.status.success() {
        let stderr = String::from_utf8_lossy(&link.stderr);
        return Err(format!("failed to link Metal library: {}", stderr.trim()));
    }

    println!(
        "cargo:rustc-env=FASTPQ_METAL_LIB={}",
        metallib_path.display()
    );
    println!("cargo:rustc-cfg=fastpq_metal_available");
    Ok(())
}

fn locate_cuda_root() -> Option<PathBuf> {
    env::var_os("CUDA_HOME")
        .or_else(|| env::var_os("CUDA_PATH"))
        .map(PathBuf::from)
        .or_else(|| {
            let default = Path::new("/usr/local/cuda");
            if default.exists() {
                Some(default.to_path_buf())
            } else {
                None
            }
        })
}

fn cuda_lib_dir(root: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let candidate = root.join("lib").join("x64");
        candidate.exists().then_some(candidate)
    }
    #[cfg(not(windows))]
    {
        let candidate = root.join("lib64");
        if candidate.exists() {
            return Some(candidate);
        }
        let alt = root.join("lib");
        alt.exists().then_some(alt)
    }
}

fn select_cuda_host_compiler(target_os: &str) -> Option<PathBuf> {
    if target_os != "linux" || explicit_cxx_configured() {
        return None;
    }

    for candidate in [
        Path::new("/usr/bin/g++-12"),
        Path::new("/usr/local/bin/g++-12"),
        Path::new("/bin/g++-12"),
    ] {
        if candidate.exists() {
            return Some(candidate.to_path_buf());
        }
    }

    None
}

fn explicit_cxx_configured() -> bool {
    env::var_os("CXX").is_some()
        || env::var_os("HOST_CXX").is_some()
        || env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CXX_"))
}
