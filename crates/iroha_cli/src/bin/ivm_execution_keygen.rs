#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![doc = "Generate canonical Halo2 IPA IVM execution verifier and prover key artifacts."]

use std::{env, fs, path::PathBuf};

use base64::Engine as _;

fn take_arg(args: &mut Vec<String>, name: &str) -> Result<String, String> {
    let Some(index) = args.iter().position(|arg| arg == name) else {
        return Err(format!("missing required argument {name}"));
    };
    if index + 1 >= args.len() {
        return Err(format!("{name} requires a value"));
    }
    let value = args.remove(index + 1);
    args.remove(index);
    Ok(value)
}

fn print_help(program: &str) {
    eprintln!(
        "Usage: {program} --vk-out <PATH> --pk-out <PATH> --template-out <PATH> [--name <VK_NAME>]"
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().collect::<Vec<_>>();
    let program = args.remove(0);
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help(&program);
        return Ok(());
    }

    let vk_out = PathBuf::from(take_arg(&mut args, "--vk-out")?);
    let pk_out = PathBuf::from(take_arg(&mut args, "--pk-out")?);
    let template_out = PathBuf::from(take_arg(&mut args, "--template-out")?);
    let name = if args.iter().any(|arg| arg == "--name") {
        take_arg(&mut args, "--name")?
    } else {
        "ivm_execution".to_owned()
    };
    if !args.is_empty() {
        return Err(format!("unexpected arguments: {}", args.join(" ")).into());
    }

    let vk_box = iroha_core::zk::halo2_ipa_ivm_execution_vk_box()
        .map_err(|err| format!("failed to build ivm-execution-v1 verifying key: {err}"))?;
    let pk = iroha_core::zk::derive_halo2_ipa_ivm_execution_proving_key_bytes(&vk_box)
        .map_err(|err| format!("failed to derive ivm-execution-v1 proving key: {err}"))?;
    let record = iroha_core::zk::halo2_ipa_ivm_execution_vk_record("core", 1)
        .map_err(|err| format!("failed to build ivm-execution-v1 VK record: {err}"))?;

    if let Some(parent) = vk_out.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = pk_out.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = template_out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&vk_out, &vk_box.bytes)?;
    fs::write(&pk_out, &pk)?;

    let vk_bytes_b64 = base64::engine::general_purpose::STANDARD.encode(&vk_box.bytes);
    let template = format!(
        concat!(
            "{{\n",
            "  \"authority\": \"REPLACE_WITH_AUTHORITY_ACCOUNT\",\n",
            "  \"private_key\": \"REPLACE_WITH_AUTHORITY_PRIVATE_KEY\",\n",
            "  \"backend\": \"{}\",\n",
            "  \"name\": \"{}\",\n",
            "  \"version\": {},\n",
            "  \"circuit_id\": \"{}\",\n",
            "  \"public_inputs_schema_hex\": \"{}\",\n",
            "  \"curve\": \"pallas\",\n",
            "  \"gas_schedule_id\": \"{}\",\n",
            "  \"vk_len\": {},\n",
            "  \"max_proof_bytes\": {},\n",
            "  \"commitment_hex\": \"{}\",\n",
            "  \"vk_bytes\": \"{}\",\n",
            "  \"status\": \"Active\"\n",
            "}}\n"
        ),
        vk_box.backend,
        name,
        record.version,
        record.circuit_id,
        hex::encode(record.public_inputs_schema_hash),
        record.gas_schedule_id.as_deref().unwrap_or("halo2_default"),
        record.vk_len,
        record.max_proof_bytes,
        hex::encode(record.commitment),
        vk_bytes_b64,
    );
    fs::write(&template_out, template)?;

    println!(
        "wrote vk={} pk={} template={}",
        vk_out.display(),
        pk_out.display(),
        template_out.display()
    );
    println!("vk_len={}", vk_box.bytes.len());
    println!("pk_len={}", pk.len());
    println!("commitment_hex={}", hex::encode(record.commitment));
    println!(
        "public_inputs_schema_hex={}",
        hex::encode(record.public_inputs_schema_hash)
    );
    Ok(())
}
