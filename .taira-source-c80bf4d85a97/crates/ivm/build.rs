use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-changed=spec/syscalls.toml");
    println!("cargo:rerun-if-env-changed=IVM_CUDA_NVCC");
    println!("cargo:rerun-if-env-changed=IVM_CUDA_GENCODE");
    println!("cargo:rerun-if-env-changed=IVM_CUDA_NVCC_EXTRA");
    println!("cargo:rerun-if-env-changed=NVCC");
    println!("cargo:rerun-if-env-changed=CXX");
    println!("cargo:rerun-if-env-changed=HOST_CXX");
    if let Ok(target) = env::var("TARGET") {
        println!("cargo:rerun-if-env-changed=CXX_{target}");
        println!(
            "cargo:rerun-if-env-changed=CXX_{}",
            target.replace('-', "_")
        );
    }
    if env::var_os("CARGO_FEATURE_CUDA").is_some()
        && let Err(err) = build_cuda_artifacts()
    {
        panic!("ivm cuda build failed: {err}");
    }
    if let Err(err) = generate_syscall_signatures() {
        panic!("ivm syscall signature generation failed: {err}");
    }
    dump_dep_env();
}

fn generate_syscall_signatures() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let specification = fs::read_to_string(manifest_dir.join("spec/syscalls.toml"))?;
    let mut current_number = None;
    let mut current_argument_count = None;
    let mut signatures = Vec::new();
    for line in specification.lines() {
        if let Some(raw) = line
            .strip_prefix("number = \"")
            .and_then(|raw| raw.strip_suffix('"'))
        {
            let number = u32::from_str_radix(
                raw.strip_prefix("0x")
                    .ok_or("syscall number must use a 0x prefix")?,
                16,
            )?;
            if current_number.replace(number).is_some() || current_argument_count.is_some() {
                return Err("syscall number missing an args or ret declaration".into());
            }
            continue;
        }
        if let Some(arguments) = line.strip_prefix("args = \"") {
            let number = current_number.ok_or("syscall args missing number")?;
            if current_argument_count.is_some() {
                return Err(format!("syscall {number:#x} has duplicate args declarations").into());
            }
            current_argument_count = Some(register_window_len(number, "args", arguments, false)?);
            continue;
        }
        let Some(returns) = line.strip_prefix("ret = \"") else {
            continue;
        };
        let number = current_number.take().ok_or("syscall ret missing number")?;
        let argument_count = current_argument_count
            .take()
            .ok_or("syscall ret missing args declaration")?;
        let return_count = register_window_len(number, "ret", returns, true)?;
        signatures.push((number, argument_count, return_count));
    }
    if current_number.is_some() || current_argument_count.is_some() {
        return Err("final syscall number missing an args or ret declaration".into());
    }
    signatures.sort_unstable();
    if signatures.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err("duplicate syscall number in spec/syscalls.toml".into());
    }

    let mut generated = String::from(
        "// Generated from spec/syscalls.toml; do not edit.\n\
         /// Return the exact public input-register window for an ABI syscall.\n\
         pub(crate) fn syscall_public_input_registers(number: u32) -> &'static [usize] {\n\
             match number {\n",
    );
    for &(number, count, _) in &signatures {
        generated.push_str(&format!("        {number:#08x} => SYSCALL_ARGS_{count},\n"));
    }
    generated.push_str(
        "        _ => SYSCALL_ARGS_5,\n\
         }\n\
         }\n\
         /// Return the exact public output-register window for an ABI syscall.\n\
         pub(crate) fn syscall_public_output_registers(number: u32) -> &'static [usize] {\n\
             match number {\n",
    );
    for (number, _, count) in signatures {
        generated.push_str(&format!("        {number:#08x} => SYSCALL_ARGS_{count},\n"));
    }
    generated.push_str(
        "        _ => SYSCALL_ARGS_0,\n\
         }\n\
         }\n",
    );
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    fs::write(out_dir.join("syscall_signatures.rs"), generated)?;
    Ok(())
}

fn register_window_len(
    number: u32,
    field: &str,
    declaration: &str,
    implicit_r10: bool,
) -> Result<usize, Box<dyn Error>> {
    let mut registers = declared_registers(declaration)?;
    let declaration = declaration.strip_suffix('"').unwrap_or(declaration);
    if implicit_r10 && declaration != "-" && registers.is_empty() {
        registers.push(10);
    }
    registers.sort_unstable();
    registers.dedup();
    if registers
        .iter()
        .enumerate()
        .any(|(offset, &register)| register != 10 + offset)
    {
        return Err(format!(
            "syscall {number:#x} {field} must use a contiguous V1 public register window starting at r10"
        )
        .into());
    }
    Ok(registers.len())
}

fn declared_registers(declaration: &str) -> Result<Vec<usize>, Box<dyn Error>> {
    let bytes = declaration.as_bytes();
    let mut registers = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let starts_register = bytes[index] == b'r'
            && bytes.get(index + 1).is_some_and(u8::is_ascii_digit)
            && (index == 0
                || (!bytes[index - 1].is_ascii_alphanumeric() && bytes[index - 1] != b'_'));
        if !starts_register {
            index += 1;
            continue;
        }

        let start = index + 1;
        let mut end = start;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        let register = declaration[start..end].parse::<usize>()?;
        if !(10..=14).contains(&register) {
            return Err(format!(
                "syscall register r{register} exceeds the V1 public window r10..r14"
            )
            .into());
        }
        registers.push(register);
        index = end;
    }
    Ok(registers)
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
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let host_compiler = select_cuda_host_compiler(&target_os);
    if let Some(host_compiler) = &host_compiler {
        println!(
            "cargo:warning=ivm cuda build: using CUDA host compiler {}",
            host_compiler.display()
        );
    }

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
        if let Some(host_compiler) = &host_compiler {
            cmd.arg(format!("-ccbin={}", host_compiler.display()));
        }
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
            return Err(format!(
                "no real PTX available for {stem}: nvcc failed and {} does not exist",
                cuda_dir.join(format!("{stem}.ptx")).display()
            )
            .into());
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
