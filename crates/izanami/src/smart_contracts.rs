//! Helpers for loading and compiling sample smart-contract programs used by Izanami.

use std::{
    collections::HashMap,
    path::{Component, Path},
    sync::{Mutex, OnceLock},
};

use color_eyre::{Result, eyre::eyre};
use iroha_data_model::transaction::IvmBytecode;
use iroha_test_network::repo_root;
use ivm::KotodamaCompiler;

/// Compile a Kotodama sample from `crates/ivm/src/kotodama/samples` into bytecode.
///
/// Results are cached in-memory so repeated calls do not recompile.
pub fn kotodama_program(name: &str) -> Result<IvmBytecode> {
    static CACHE: OnceLock<Mutex<HashMap<String, IvmBytecode>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    let validated_name = validate_name(name, "Kotodama sample")?;

    if let Some(existing) = cache
        .lock()
        .expect("cache poisoned")
        .get(&validated_name)
        .cloned()
    {
        return Ok(existing);
    }

    let mut path = repo_root();
    path.push("crates/ivm/src/kotodama/samples");
    let mut file_name = validated_name.clone();
    file_name.push_str(".ko");
    path.push(file_name);

    let compiler = KotodamaCompiler::new();
    let bytecode = compiler
        .compile_file(&path)
        .map_err(|msg| eyre!("failed to compile Kotodama sample `{validated_name}`: {msg}"))?;

    let program = IvmBytecode::from_compiled(bytecode);
    cache
        .lock()
        .expect("cache poisoned")
        .insert(validated_name, program.clone());
    Ok(program)
}

/// Load a precompiled IVM bytecode artifact from the fixtures directory.
///
/// Files live under `crates/ivm/tests/fixtures/predecoder/mixed/artifacts`.
pub fn ivm_artifact(name: &str) -> Result<IvmBytecode> {
    let validated_name = validate_name(name, "IVM artifact")?;

    let mut path = repo_root();
    path.push("crates/ivm/tests/fixtures/predecoder/mixed/artifacts");
    let mut file_name = validated_name.clone();
    file_name.push_str(".to");
    path.push(file_name);

    let blob = std::fs::read(&path).map_err(|err| {
        eyre!(
            "failed to load IVM artifact `{validated_name}` from {}: {err}",
            path.display()
        )
    })?;
    Ok(IvmBytecode::from_compiled(blob))
}

fn validate_name(name: &str, context: &str) -> Result<String> {
    let path = Path::new(name);

    if path.is_absolute() {
        return Err(eyre!(
            "{context} name `{name}` must not be an absolute path"
        ));
    }

    let mut components = path.components();
    let Some(component) = components.next() else {
        return Err(eyre!("{context} name must not be empty"));
    };

    if components.next().is_some() {
        return Err(eyre!(
            "{context} name `{name}` must not contain path separators"
        ));
    }

    match component {
        Component::Normal(part) => Ok(part
            .to_str()
            .ok_or_else(|| eyre!("{context} name `{name}` contains invalid UTF-8"))?
            .to_string()),
        Component::CurDir | Component::ParentDir => Err(eyre!(
            "{context} name `{name}` must not contain relative components"
        )),
        Component::Prefix(_) | Component::RootDir => Err(eyre!(
            "{context} name `{name}` must not be an absolute path"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kotodama_program_compiles() {
        let program = kotodama_program("asset_ops").expect("compilation succeeds");
        assert!(program.size_bytes() > 0);
    }

    #[test]
    fn ivm_artifact_loads() {
        let artifact = ivm_artifact("artifact_v1_7_mode00_vlen0_cycles0_abi1")
            .expect("artifact should be readable");
        assert!(artifact.size_bytes() > 0);
    }

    #[test]
    fn kotodama_program_rejects_invalid_names() {
        for invalid in ["", ".", "..", "../secret", "secret/../../", "/etc/passwd"] {
            assert!(
                kotodama_program(invalid).is_err(),
                "expected `{invalid}` to fail"
            );
        }
    }

    #[test]
    fn ivm_artifact_rejects_invalid_names() {
        for invalid in ["", ".", "..", "artifact/..", "../artifact", "C:/absolute"] {
            assert!(
                ivm_artifact(invalid).is_err(),
                "expected `{invalid}` to fail"
            );
        }
    }
}
