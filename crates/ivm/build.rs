use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
const DEFAULT_CUDA_GENCODE: &str = "arch=compute_86,code=sm_86";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CudaPtxMode {
    Bundled,
    Generate,
    Check,
}
fn main() {
    println!("cargo:rerun-if-changed=spec/syscalls.toml");
    println!("cargo:rerun-if-env-changed=IVM_CUDA_PTX_MODE");
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
    println!("cargo:rerun-if-changed={}", cuda_dir.display());
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    fs::create_dir_all(&out_dir)?;
    let mode = cuda_ptx_mode()?;
    let mut sources = Vec::new();
    for entry in fs::read_dir(&cuda_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension() == Some(OsStr::new("cu")) {
            sources.push(path);
        }
    }
    sources.sort();
    if sources.is_empty() {
        return Err(format!("no CUDA sources found in {}", cuda_dir.display()).into());
    }
    let mut artifacts = Vec::with_capacity(sources.len());
    for path in sources {
        println!("cargo:rerun-if-changed={}", path.display());
        let stem = path
            .file_stem()
            .ok_or_else(|| format!("CUDA source has no file stem: {}", path.display()))?
            .to_string_lossy()
            .into_owned();
        let bundled = cuda_dir.join(format!("{stem}.ptx"));
        println!("cargo:rerun-if-changed={}", bundled.display());
        artifacts.push((path, bundled, out_dir.join(format!("{stem}.ptx")), stem));
    }
    if mode != CudaPtxMode::Generate {
        // TODO: Check in all 11 reproducibly generated PTX files plus their
        // signed provenance manifest. Until then, ordinary CUDA builds must
        // fail here instead of compiling host-specific artifacts implicitly.
        let missing: Vec<_> = artifacts
            .iter()
            .filter(|(_, bundled, _, _)| !bundled.is_file())
            .map(|(_, bundled, _, _)| bundled.display().to_string())
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "missing checked-in PTX artifacts required by {mode:?} mode: {}",
                missing.join(", ")
            )
            .into());
        }
    }
    match mode {
        CudaPtxMode::Bundled => {
            for (_, bundled, target, _) in artifacts {
                install_bundled_ptx(&bundled, &target)?;
            }
        }
        CudaPtxMode::Generate | CudaPtxMode::Check => {
            let nvcc = NvccConfig::from_env();
            if let Some(host_compiler) = &nvcc.host_compiler {
                println!(
                    "cargo:warning=ivm cuda build: using CUDA host compiler {}",
                    host_compiler.display()
                );
            }
            for (source, bundled, target, stem) in artifacts {
                if mode == CudaPtxMode::Generate {
                    compile_cuda_source(&cuda_dir, &source, &target, &nvcc)?;
                    continue;
                }
                let generated = out_dir.join(format!("{stem}.generated.ptx"));
                compile_cuda_source(&cuda_dir, &source, &generated, &nvcc)?;
                verify_bundled_ptx(&bundled, &generated)?;
                install_bundled_ptx(&bundled, &target)?;
            }
        }
    }
    Ok(())
}
fn cuda_ptx_mode() -> Result<CudaPtxMode, Box<dyn Error>> {
    match env::var("IVM_CUDA_PTX_MODE") {
        Ok(value) => parse_cuda_ptx_mode(&value).map_err(Into::into),
        Err(env::VarError::NotPresent) => Ok(CudaPtxMode::Bundled),
        Err(err) => Err(format!("invalid IVM_CUDA_PTX_MODE: {err}").into()),
    }
}
fn parse_cuda_ptx_mode(value: &str) -> Result<CudaPtxMode, String> {
    match value {
        "bundled" => Ok(CudaPtxMode::Bundled),
        "generate" => Ok(CudaPtxMode::Generate),
        "check" => Ok(CudaPtxMode::Check),
        _ => Err(format!(
            "IVM_CUDA_PTX_MODE must be one of bundled, generate, or check; got {value:?}"
        )),
    }
}
struct NvccConfig {
    executable: String,
    host_compiler: Option<PathBuf>,
    gencode: String,
    extra_flags: Vec<String>,
}
impl NvccConfig {
    fn from_env() -> Self {
        let executable = env::var("IVM_CUDA_NVCC")
            .or_else(|_| env::var("NVCC"))
            .unwrap_or_else(|_| "nvcc".to_string());
        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        let host_compiler = select_cuda_host_compiler(&target_os);
        let gencode =
            env::var("IVM_CUDA_GENCODE").unwrap_or_else(|_| DEFAULT_CUDA_GENCODE.to_string());
        let extra_flags = env::var("IVM_CUDA_NVCC_EXTRA")
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        Self {
            executable,
            host_compiler,
            gencode,
            extra_flags,
        }
    }
}
fn compile_cuda_source(
    cuda_dir: &Path,
    source: &Path,
    target: &Path,
    nvcc: &NvccConfig,
) -> Result<(), Box<dyn Error>> {
    if target.exists() {
        fs::remove_file(target)?;
    }
    let file_name = source
        .file_name()
        .ok_or_else(|| format!("CUDA source has no file name: {}", source.display()))?;
    let mut cmd = Command::new(&nvcc.executable);
    cmd.current_dir(cuda_dir);
    cmd.arg("-ptx");
    cmd.arg(file_name);
    cmd.arg("-o");
    cmd.arg(target);
    cmd.arg("-std=c++14");
    if let Some(host_compiler) = &nvcc.host_compiler {
        cmd.arg(format!("-ccbin={}", host_compiler.display()));
    }
    if !nvcc.gencode.trim().is_empty() {
        cmd.args(["-gencode", &nvcc.gencode]);
    }
    for flag in &nvcc.extra_flags {
        cmd.arg(flag);
    }
    let status = cmd.status().map_err(|err| {
        format!(
            "failed to spawn {} for {}: {err}",
            nvcc.executable,
            source.display()
        )
    })?;
    if !status.success() {
        return Err(format!(
            "{} exited with status {status} for {}",
            nvcc.executable,
            source.display()
        )
        .into());
    }
    let bytes = fs::read(target)?;
    validate_ptx_bytes(target, &bytes)
}
fn install_bundled_ptx(bundled: &Path, target: &Path) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(bundled)
        .map_err(|err| format!("failed to read checked-in PTX {}: {err}", bundled.display()))?;
    validate_ptx_bytes(bundled, &bytes)?;
    fs::write(target, bytes)?;
    Ok(())
}
fn verify_bundled_ptx(bundled: &Path, generated: &Path) -> Result<(), Box<dyn Error>> {
    let expected = fs::read(bundled)?;
    let actual = fs::read(generated)?;
    validate_ptx_bytes(bundled, &expected)?;
    validate_ptx_bytes(generated, &actual)?;
    if expected != actual {
        return Err(format!(
            "generated PTX differs from checked-in artifact {} (expected {} bytes, generated {} bytes)",
            bundled.display(),
            expected.len(),
            actual.len()
        )
        .into());
    }
    Ok(())
}
fn validate_ptx_bytes(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let text = std::str::from_utf8(bytes)
        .map_err(|err| format!("PTX {} is not UTF-8 text: {err}", path.display()))?;
    for directive in [".version", ".target", ".address_size", ".entry"] {
        if !text
            .split_ascii_whitespace()
            .any(|token| token == directive)
        {
            return Err(format!(
                "PTX {} is missing required {directive} directive",
                path.display()
            )
            .into());
        }
    }
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cuda_ptx_mode_parser_is_strict() {
        assert_eq!(parse_cuda_ptx_mode("bundled"), Ok(CudaPtxMode::Bundled));
        assert_eq!(parse_cuda_ptx_mode("generate"), Ok(CudaPtxMode::Generate));
        assert_eq!(parse_cuda_ptx_mode("check"), Ok(CudaPtxMode::Check));
        assert!(parse_cuda_ptx_mode("fallback").is_err());
        assert!(parse_cuda_ptx_mode("BUNDLED").is_err());
        assert!(parse_cuda_ptx_mode("").is_err());
    }
    #[test]
    fn ptx_validator_rejects_comment_only_placeholders() {
        let path = Path::new("placeholder.ptx");
        assert!(validate_ptx_bytes(path, b"// Placeholder PTX; CUDA stays disabled.\n").is_err());
    }
    #[test]
    fn ptx_validator_accepts_required_directives_and_entry() {
        let path = Path::new("kernel.ptx");
        let ptx = b".version 7.8\n\
                    .target sm_86\n\
                    .address_size 64\n\
                    .visible .entry kernel() { ret; }\n";
        assert!(validate_ptx_bytes(path, ptx).is_ok());
    }
}
