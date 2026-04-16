//! Print operator JSON fields for TAIRA confidential v2 verifying-key updates.

use base64::Engine as _;
use iroha_core::zk::confidential_v2;

fn print_record(kind: &str, version: u32) -> Result<(), String> {
    let (name, record) = match kind {
        "transfer" => (
            "vk_transfer",
            confidential_v2::confidential_transfer_v2_vk_record("vk_transfer", version)?,
        ),
        "unshield" => (
            "vk_unshield",
            confidential_v2::confidential_unshield_v2_vk_record("vk_unshield", version)?,
        ),
        _ => return Err("usage: confidential_v2_vk_json <transfer|unshield> <version>".into()),
    };
    let key = record
        .key
        .as_ref()
        .ok_or_else(|| "confidential v2 record did not include vk bytes".to_owned())?;
    println!(
        concat!(
            "{{\n",
            "  \"backend\": \"halo2/ipa\",\n",
            "  \"name\": \"{}\",\n",
            "  \"version\": {},\n",
            "  \"circuit_id\": \"{}\",\n",
            "  \"public_inputs_schema_hex\": \"{}\",\n",
            "  \"curve\": \"{}\",\n",
            "  \"gas_schedule_id\": \"{}\",\n",
            "  \"vk_len\": {},\n",
            "  \"max_proof_bytes\": {},\n",
            "  \"vk_bytes\": \"{}\"\n",
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
        _ => Err("usage: confidential_v2_vk_json <transfer|unshield> <version>".into()),
    };
    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(2);
    }
}
