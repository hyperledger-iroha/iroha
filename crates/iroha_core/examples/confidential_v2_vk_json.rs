//! Print operator JSON fields for TAIRA confidential v2 verifying-key updates.

use base64::Engine as _;
use iroha_core::zk::confidential_v2;

fn print_record(kind: &str, version: u32) -> Result<(), String> {
    let (name, mut record) = match kind {
        "transfer" => (
            "vk_transfer",
            confidential_v2::confidential_transfer_v2_vk_record("vk_transfer", version)?,
        ),
        "unshield" => (
            "vk_unshield",
            confidential_v2::confidential_unshield_v2_vk_record("vk_unshield", version)?,
        ),
        "unshield-v3" => (
            "vk_unshield",
            confidential_v2::confidential_unshield_v3_vk_record("vk_unshield", version)?,
        ),
        _ => {
            return Err(
                "usage: confidential_v2_vk_json <transfer|unshield|unshield-v3> <version>".into(),
            );
        }
    };
    if kind == "unshield-v3" {
        iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE.clone_into(&mut record.namespace);
    }
    let key = record
        .key
        .as_ref()
        .ok_or_else(|| "confidential v2 record did not include vk bytes".to_owned())?;
    let record_norito = norito::to_bytes(&record)
        .map_err(|err| format!("failed to encode verifier record as Norito: {err}"))?;
    println!(
        concat!(
            "{{\n",
            "  \"backend\": \"halo2/ipa\",\n",
            "  \"name\": \"{}\",\n",
            "  \"version\": {},\n",
            "  \"circuit_id\": \"{}\",\n",
            "  \"public_inputs_schema_hash_hex\": \"{}\",\n",
            "  \"curve\": \"{}\",\n",
            "  \"gas_schedule_id\": \"{}\",\n",
            "  \"vk_len\": {},\n",
            "  \"max_proof_bytes\": {},\n",
            "  \"vk_bytes\": \"{}\",\n",
            "  \"record_norito_base64\": \"{}\"\n",
            "}}"
        ),
        name,
        record.version,
        record.circuit_id,
        hex::encode_upper(record.public_inputs_schema_hash),
        record.curve,
        record.gas_schedule_id.as_deref().unwrap_or("halo2_default"),
        record.vk_len,
        record.max_proof_bytes,
        base64::engine::general_purpose::STANDARD.encode(&key.bytes),
        base64::engine::general_purpose::STANDARD.encode(record_norito),
    );
    Ok(())
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let result = match args.as_slice() {
        [kind, version] => version
            .parse::<u32>()
            .map_err(|err| format!("invalid version: {err}"))
            .and_then(|version| print_record(kind, version)),
        _ => Err("usage: confidential_v2_vk_json <transfer|unshield|unshield-v3> <version>".into()),
    };
    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(2);
    }
}
