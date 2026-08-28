//! GPU build helper for the FASTPQ prover.
//!
//! Metal kernels are compiled into an offline library when the toolchain is
//! available, with runtime source compilation retained as a fallback. This
//! script also supports the static CUDA path when `fastpq-gpu` is enabled.
// SPDX-License-Identifier: Apache-2.0
use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const METAL_TOOLCHAIN_REMEDIATION: &str = "verify that `xcode-select -p` or `DEVELOPER_DIR` selects a full Xcode installation and accept any pending Xcode license, then run `xcodebuild -downloadComponent MetalToolchain` manually; set `FASTPQ_SKIP_GPU_BUILD=1` only to opt out and use runtime Metal source compilation";

fn main() {
    println!("cargo:rerun-if-env-changed=FASTPQ_SKIP_GPU_BUILD");
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");
    println!("cargo:rerun-if-env-changed=SDKROOT");
    println!("cargo:rerun-if-env-changed=TOOLCHAINS");
    println!("cargo:rerun-if-env-changed=PATH");
    println!("cargo:rerun-if-changed=cuda/fastpq_cuda.cu");
    println!("cargo:rerun-if-changed=metal/include/params.h");
    println!("cargo:rerun-if-changed=metal/kernels/field.metal");
    println!("cargo:rerun-if-changed=metal/kernels/ntt_stage.metal");
    println!("cargo:rerun-if-changed=metal/kernels/poseidon.metal");
    println!("cargo:rerun-if-changed=metal/kernels/bn254.metal");
    let fastpq_gpu_feature = env::var_os("CARGO_FEATURE_FASTPQ_GPU").is_some();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let skip_gpu_build = env::var_os("FASTPQ_SKIP_GPU_BUILD").is_some();
    if fastpq_gpu_feature && target_os == "macos" {
        if skip_gpu_build {
            println!(
                "cargo:warning=FASTPQ_SKIP_GPU_BUILD set; skipping offline Metal shader build and using runtime source compilation"
            );
            println!("cargo:rustc-env=FASTPQ_METAL_LIB=");
        } else if let Err(error) = ensure_metal_toolchain().and_then(|()| compile_metal_shaders()) {
            println!("cargo:warning={error}; falling back to runtime Metal source compilation");
            println!("cargo:rustc-env=FASTPQ_METAL_LIB=");
        }
    }
    if !fastpq_gpu_feature {
        println!("cargo:rustc-cfg=fastpq_cuda_unavailable");
        return;
    }
    if target_os == "macos" {
        // Metal hosts skip the static CUDA path; the runtime will fall back to the Metal
        // backend without surfacing an unnecessary warning.
        println!("cargo:rustc-cfg=fastpq_cuda_unavailable");
        return;
    }
    if skip_gpu_build {
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
fn ensure_metal_toolchain() -> Result<(), String> {
    metal_toolchain_status().map_err(|problem| {
        format!("Metal compiler/linker is unavailable: {problem}; {METAL_TOOLCHAIN_REMEDIATION}")
    })
}

fn metal_toolchain_status() -> Result<(), String> {
    let metal = find_xcrun_tool("metal")
        .map_err(|error| format!("failed to locate the `metal` compiler: {error}"))?;
    probe_metal_tool("metal", &metal)?;
    let metallib = find_metallib_tool(&metal)?;
    probe_metal_tool("metallib", &metallib)
}

fn probe_metal_tool(name: &str, path: &Path) -> Result<(), String> {
    let output = Command::new(path).arg("-v").output().map_err(|error| {
        format!(
            "found `{name}` at {}, but could not execute it: {error}",
            path.display()
        )
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "`{}` failed its `-v` probe with {}: {}",
            path.display(),
            output.status,
            command_diagnostic(&output)
        ))
    }
}

fn compile_metal_shaders() -> Result<(), String> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);
    let kernels = [
        ("ntt_stage", Path::new("metal/kernels/ntt_stage.metal")),
        ("poseidon", Path::new("metal/kernels/poseidon.metal")),
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
    let metal_exe = find_xcrun_tool("metal")?;
    let mut air_paths = Vec::new();
    for (name, source) in &kernels {
        let air_path = out_dir.join(format!("{name}.air"));
        remove_stale_output("Metal AIR object", &air_path)?;
        let status = Command::new(&metal_exe)
            .arg("-std=macos-metal2.4")
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
            .map_err(|error| {
                format!(
                    "failed to launch Metal compiler `{}` for {}: {error}",
                    metal_exe.display(),
                    source.display()
                )
            })?;
        if !status.status.success() {
            return Err(format!(
                "failed to compile Metal shader {}: {}",
                source.display(),
                command_diagnostic(&status)
            ));
        }
        ensure_nonempty_output("Metal AIR object", &air_path)?;
        air_paths.push(air_path);
    }
    let metallib_exe = find_metallib_tool(&metal_exe)?;
    remove_stale_output("Metal library", &metallib_path)?;
    let mut link_cmd = Command::new(&metallib_exe);
    for air in &air_paths {
        link_cmd.arg(air);
    }
    let link = link_cmd
        .arg("-o")
        .arg(&metallib_path)
        .output()
        .map_err(|error| {
            format!(
                "failed to launch Metal linker `{}`: {error}",
                metallib_exe.display()
            )
        })?;
    if !link.status.success() {
        return Err(format!(
            "failed to link Metal library: {}",
            command_diagnostic(&link)
        ));
    }
    ensure_nonempty_output("Metal library", &metallib_path)?;
    println!(
        "cargo:rustc-env=FASTPQ_METAL_LIB={}",
        metallib_path.display()
    );
    println!("cargo:rustc-cfg=fastpq_metal_available");
    Ok(())
}

fn find_metallib_tool(metal: &Path) -> Result<PathBuf, String> {
    find_xcrun_tool("metallib").or_else(|xcrun_error| {
        let candidate = metal.with_file_name("metallib");
        candidate.is_file().then_some(candidate.clone()).ok_or_else(|| {
            format!(
                "failed to locate the `metallib` linker ({xcrun_error}); sibling candidate is missing: {}",
                candidate.display()
            )
        })
    })
}

fn find_xcrun_tool(tool: &str) -> Result<PathBuf, String> {
    find_xcrun_tool_with_args(&["-sdk", "macosx", "--find", tool]).or_else(|sdk_error| {
        find_xcrun_tool_with_args(&["--find", tool]).map_err(|fallback_error| {
            format!(
                "SDK lookup failed ({sdk_error}); default lookup also failed ({fallback_error})"
            )
        })
    })
}
fn find_xcrun_tool_with_args(args: &[&str]) -> Result<PathBuf, String> {
    let invocation = format!("xcrun {}", args.join(" "));
    let output = Command::new("xcrun")
        .args(args)
        .output()
        .map_err(|error| format!("failed to launch `{invocation}`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`{invocation}` exited with {}: {}",
            output.status,
            command_diagnostic(&output)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let reported_path = stdout.trim();
    if reported_path.is_empty() {
        return Err(format!("`{invocation}` returned an empty tool path"));
    }
    let path = PathBuf::from(reported_path);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "`{invocation}` returned a missing or non-file tool path: {}",
            path.display()
        ))
    }
}

fn command_diagnostic(output: &Output) -> String {
    let stderr = compact_output(&output.stderr);
    let stdout = compact_output(&output.stdout);
    match (stderr.is_empty(), stdout.is_empty()) {
        (false, false) => format!("stderr: {stderr}; stdout: {stdout}"),
        (false, true) => format!("stderr: {stderr}"),
        (true, false) => format!("stdout: {stdout}"),
        (true, true) => "no diagnostic output".to_owned(),
    }
}

fn compact_output(bytes: &[u8]) -> String {
    const MAX_DIAGNOSTIC_CHARS: usize = 2_000;

    let compact = String::from_utf8_lossy(bytes)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if compact.chars().count() > MAX_DIAGNOSTIC_CHARS {
        format!(
            "{}…",
            compact
                .chars()
                .take(MAX_DIAGNOSTIC_CHARS)
                .collect::<String>()
        )
    } else {
        compact
    }
}

fn ensure_nonempty_output(label: &str, path: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("{label} was not produced at {}: {error}", path.display()))?;
    if metadata.is_file() && metadata.len() > 0 {
        Ok(())
    } else {
        Err(format!(
            "{label} at {} is not a non-empty regular file",
            path.display()
        ))
    }
}

fn remove_stale_output(label: &str, path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale {label} at {}: {error}",
            path.display()
        )),
    }
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
