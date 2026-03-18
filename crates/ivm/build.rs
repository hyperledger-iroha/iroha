use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    if env::var_os("CARGO_FEATURE_CUDA").is_some()
        && let Err(err) = build_cuda_artifacts()
    {
        println!("cargo:warning=ivm cuda build: {err}");
    }
    dump_dep_env();
}

fn build_cuda_artifacts() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let cuda_dir = manifest_dir.join("cuda");
    if !cuda_dir.exists() {
        return Ok(());
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    fs::create_dir_all(&out_dir)?;

    let nvcc = env::var("IVM_CUDA_NVCC")
        .or_else(|_| env::var("NVCC"))
        .unwrap_or_else(|_| "nvcc".to_string());

    let gencode =
        env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| "arch=compute_61,code=sm_61".to_string());
    let extra_flags: Vec<String> = env::var("IVM_CUDA_NVCC_EXTRA")
        .unwrap_or_default()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    for entry in fs::read_dir(&cuda_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension() != Some(OsStr::new("cu")) {
            continue;
        }

        println!("cargo:rerun-if-changed={}", path.display());

        let stem = match path.file_stem() {
            Some(stem) => stem.to_string_lossy(),
            None => continue,
        };

        let target = out_dir.join(format!("{stem}.ptx"));

        let mut cmd = Command::new(&nvcc);
        cmd.current_dir(&cuda_dir);
        cmd.arg("-ptx");
        cmd.arg(path.file_name().expect("file_name present"));
        cmd.arg("-o");
        cmd.arg(&target);
        cmd.arg("-std=c++14");
        if !gencode.trim().is_empty() {
            cmd.args(["-gencode", &gencode]);
        }
        for flag in &extra_flags {
            cmd.arg(flag);
        }

        match cmd.status() {
            Ok(status) if status.success() => continue,
            Ok(status) => {
                println!(
                    "cargo:warning=ivm cuda build: nvcc exited with status {status} for {stem}"
                );
            }
            Err(err) => {
                println!("cargo:warning=ivm cuda build: failed to spawn nvcc ({err}) for {stem}");
            }
        }

        if !fallback_copy(&cuda_dir, &target, &stem)? {
            write_stub_ptx(&target, &stem)?;
        }
    }

    Ok(())
}

fn fallback_copy(cuda_dir: &Path, target: &Path, stem: &str) -> Result<bool, Box<dyn Error>> {
    let fallback = cuda_dir.join(format!("{stem}.ptx"));
    if fallback.exists() {
        fs::copy(&fallback, target)?;
        return Ok(true);
    }
    Ok(false)
}

fn write_stub_ptx(target: &Path, stem: &str) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(target)?;
    writeln!(
        file,
        "// Placeholder PTX for {stem}. No GPU kernels were built; CUDA runtime will stay disabled."
    )?;
    Ok(())
}

fn dump_dep_env() {
    let mut report = String::new();
    for (key, value) in env::vars() {
        if key.starts_with("DEP_") {
            report.push_str(&key);
            report.push('=');
            report.push_str(&value);
            report.push('\n');
        }
    }
    if let Some(out_dir) = env::var_os("OUT_DIR") {
        let mut path = PathBuf::from(out_dir);
        path.push("dep_env.txt");
        let _ = fs::write(path, report);
    }
}
