use std::{env, path::PathBuf, process::Command};

fn main() {
    // Teach rustc about our custom cfg so check-cfg doesn't flag it.
    println!("cargo::rustc-check-cfg=cfg(crc64_cuda_available)");
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=JSONSTAGE1_CUDA_ARCH");
    println!("cargo:rerun-if-env-changed=JSONSTAGE1_CUDA_SKIP_BUILD");
    println!("cargo:rerun-if-changed=src/cuda_crc64.cu");

    let feature_enabled = env::var_os("CARGO_FEATURE_CUDA_KERNEL").is_some();
    if !feature_enabled {
        // Feature not requested; keep the Rust fallback only.
        return;
    }

    if env::var_os("JSONSTAGE1_CUDA_SKIP_BUILD").is_some() {
        println!("cargo:warning=JSONSTAGE1_CUDA_SKIP_BUILD set; skipping CUDA kernel compilation.");
        return;
    }

    if !nvcc_available() {
        println!("cargo:warning=nvcc not found; building jsonstage1_cuda without GPU kernels.");
        return;
    }

    if let Some(dir) = locate_cuda_lib_dir() {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }

    let mut build = cc::Build::new();
    build.cuda(true);
    build.file("src/cuda_crc64.cu");
    build.flag("-std=c++17");
    build.flag("-O3");
    build.flag("-lineinfo");
    build.flag("-Xcompiler=-fPIC");

    if let Some(arch_flag) = env::var_os("JSONSTAGE1_CUDA_ARCH") {
        build.flag(
            arch_flag
                .to_str()
                .expect("JSONSTAGE1_CUDA_ARCH must be valid UTF-8"),
        );
    }

    build.compile("jsonstage1_cuda_kernels");
    println!("cargo:rustc-link-lib=cudart");
    println!("cargo:rustc-link-lib=stdc++");
    println!("cargo:rustc-cfg=crc64_cuda_available");
}

fn nvcc_available() -> bool {
    Command::new("nvcc")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn locate_cuda_lib_dir() -> Option<PathBuf> {
    let root = env::var_os("CUDA_HOME")
        .or_else(|| env::var_os("CUDA_PATH"))
        .map(PathBuf::from)?;
    for candidate in ["lib64", "lib"].iter() {
        let joined = root.join(candidate);
        if joined.exists() {
            return Some(joined);
        }
    }
    None
}
