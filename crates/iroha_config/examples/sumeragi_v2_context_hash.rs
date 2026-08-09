//! Print the canonical Sumeragi v2 Nexus/AMX context commitment.
//!
//! The optional first argument is a peer configuration TOML. The optional
//! second argument is canonical Norito containing the staged active public-lane
//! validator records. Omitting either argument selects repository defaults or
//! an empty active-validator set, respectively. Pass `--consensus-sections`
//! before a template config to parse only its `nexus` and `pipeline` tables;
//! this is useful for deployment templates whose runtime secrets are redacted.

use std::{
    env, fs,
    path::{Path, PathBuf},
    str::FromStr as _,
};

use iroha_config::{
    base::toml::TomlSource,
    parameters::actual::{Nexus, Pipeline, Root, sumeragi_v2_nexus_amx_context_hash},
};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, bls_normal_pop_prove};
use iroha_data_model::block::consensus_v2::GenesisActiveNexusLaneRecord;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let first = args.next();
    let consensus_sections = first.as_deref() == Some(std::ffi::OsStr::new("--consensus-sections"));
    let config_path = if consensus_sections {
        args.next().map(PathBuf::from)
    } else {
        first.map(PathBuf::from)
    };
    let active_records_path = args.next().map(PathBuf::from);
    if args.next().is_some() {
        return Err("usage: sumeragi_v2_context_hash [--consensus-sections] [config.toml] [active-records.norito]".into());
    }

    let (nexus, pipeline) = if let Some(path) = config_path {
        let root = if consensus_sections {
            load_consensus_sections(&path)?
        } else {
            Root::from_toml_source(TomlSource::from_file(&path)?)
                .map_err(|error| format!("failed to parse {}: {error}", path.display()))?
        };
        (root.nexus, root.pipeline)
    } else {
        (Nexus::default(), Pipeline::default())
    };
    let active_records = if let Some(path) = active_records_path {
        let bytes = fs::read(&path)?;
        norito::decode_from_bytes::<Vec<GenesisActiveNexusLaneRecord>>(&bytes)
            .map_err(|error| format!("failed to decode {}: {error}", path.display()))?
    } else {
        Vec::new()
    };

    let hash = sumeragi_v2_nexus_amx_context_hash(&nexus, &pipeline, &active_records, &[]);
    println!("{}", hex::encode(hash.as_ref()));
    println!("{}", norito::json::to_json(&<[u8; 32]>::from(hash))?);
    Ok(())
}

fn load_consensus_sections(path: &Path) -> Result<Root, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let header = || {
        source
            .lines()
            .take_while(|line| !line.trim_start().starts_with('['))
    };
    let chain = header()
        .find(|line| line.trim_start().starts_with("chain ="))
        .ok_or_else(|| format!("{} has no top-level chain", path.display()))?;
    let chain_discriminant = header()
        .find(|line| line.trim_start().starts_with("chain_discriminant ="))
        .unwrap_or("");
    let validator = KeyPair::try_from_seed(
        b"sumeragi-v2-context-projection-validator".to_vec(),
        Algorithm::BlsNormal,
    )?;
    let validator_pop = bls_normal_pop_prove(validator.private_key())?;
    let genesis = KeyPair::try_from_seed(
        b"sumeragi-v2-context-projection-genesis".to_vec(),
        Algorithm::Ed25519,
    )?;
    let streaming = KeyPair::try_from_seed(
        b"sumeragi-v2-context-projection-streaming".to_vec(),
        Algorithm::Ed25519,
    )?;
    let soranet_transport = KeyPair::try_from_seed(
        b"sumeragi-v2-context-projection-soranet-transport-v1".to_vec(),
        Algorithm::Ed25519,
    )?;
    let mut projected = format!(
        r#"{chain}
{chain_discriminant}
public_key = "{validator_public}"
private_key = "{validator_private}"
soranet_transport_public_key = "{soranet_transport_public}"
soranet_transport_private_key = "{soranet_transport_private}"
trusted_peers_pop = [
  {{ public_key = "{validator_public}", pop_hex = "{validator_pop}" }}
]

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[genesis]
public_key = "{genesis_public}"
expected_hash = "0000000000000000000000000000000000000000000000000000000000000001"

[streaming]
identity_public_key = "{streaming_public}"
identity_private_key = "{streaming_private}"
"#,
        validator_public = validator.public_key(),
        validator_private = ExposedPrivateKey(validator.private_key().clone()),
        validator_pop = hex::encode(validator_pop),
        soranet_transport_public = soranet_transport.public_key(),
        soranet_transport_private = ExposedPrivateKey(soranet_transport.private_key().clone()),
        genesis_public = genesis.public_key(),
        streaming_public = streaming.public_key(),
        streaming_private = ExposedPrivateKey(streaming.private_key().clone()),
    );
    let mut include = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            let section = trimmed.trim_start_matches('[');
            include = section.starts_with("nexus") || section.starts_with("pipeline");
        }
        if include {
            projected.push_str(line);
            projected.push('\n');
        }
    }
    let table = toml::Table::from_str(&projected)
        .map_err(|error| format!("failed to project {}: {error}", path.display()))?;
    Root::from_toml_source(TomlSource::inline(table)).map_err(|error| {
        format!(
            "failed to parse projection from {}: {error}",
            path.display()
        )
        .into()
    })
}
