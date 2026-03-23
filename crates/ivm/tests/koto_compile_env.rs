use std::{env, fs, path::PathBuf};

#[test]
fn emit_contract_artifact_from_env() {
    let input = match env::var("KOTO_COMPILE_INPUT") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return,
    };
    let output = env::var("KOTO_COMPILE_OUT").expect("KOTO_COMPILE_OUT must be set");
    let manifest_output =
        env::var("KOTO_COMPILE_MANIFEST_OUT").expect("KOTO_COMPILE_MANIFEST_OUT must be set");
    let abi_version = env::var("KOTO_COMPILE_ABI")
        .ok()
        .and_then(|raw| raw.parse::<u8>().ok())
        .unwrap_or(1);

    let source = fs::read_to_string(&input).expect("read contract source");
    let compiler =
        ivm::KotodamaCompiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
            abi_version,
            ..Default::default()
        });
    let (artifact, manifest, _) = compiler
        .compile_source_with_manifest_and_diagnostics(&source)
        .expect("compile contract");

    if let Some(parent) = PathBuf::from(&output).parent() {
        fs::create_dir_all(parent).expect("create artifact output directory");
    }
    if let Some(parent) = PathBuf::from(&manifest_output).parent() {
        fs::create_dir_all(parent).expect("create manifest output directory");
    }

    fs::write(&output, &artifact).expect("write artifact");
    let manifest_json = norito::json::to_json_pretty(&manifest).expect("serialize manifest");
    fs::write(&manifest_output, manifest_json).expect("write manifest");

    let parsed = ivm::ProgramMetadata::parse(&artifact).expect("parse artifact metadata");
    assert_eq!(parsed.metadata.abi_version, abi_version);
    assert!(
        parsed.contract_interface.is_some(),
        "compiled contract must embed a CNTR section",
    );
}
