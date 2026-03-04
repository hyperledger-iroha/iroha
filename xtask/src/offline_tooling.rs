use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use eyre::{Context as _, Result, eyre};
use iroha::data_model::{
    account::AccountId, asset::AssetId, metadata::Metadata, name::Name, prelude::Numeric,
};
use iroha_crypto::{Algorithm, Hash, PrivateKey, PublicKey};
use iroha_primitives::json::Json;
use norito::{
    codec::Encode,
    json::{self as serde_json, JsonSerialize, Value as JsonValue},
    to_bytes,
};

pub(crate) fn parse_private_key(spec: &str) -> Result<PrivateKey> {
    let (algorithm, payload) = split_key_spec(spec)?;
    PrivateKey::from_hex(algorithm, payload).map_err(|err| eyre!(err))
}

pub(crate) fn parse_public_key(spec: &str) -> Result<PublicKey> {
    if let Ok(key) = PublicKey::from_str(spec) {
        return Ok(key);
    }
    let (algorithm, payload) = split_key_spec(spec)?;
    PublicKey::from_hex(algorithm, payload).map_err(|err| eyre!(err))
}

fn split_key_spec(spec: &str) -> Result<(Algorithm, &str)> {
    let (algo, payload) = spec
        .split_once(':')
        .ok_or_else(|| eyre!("keys must be formatted as `<algorithm>:<hex>`"))?;
    let algorithm = Algorithm::from_str(algo)
        .map_err(|_| eyre!("unrecognised algorithm `{algo}` in key spec"))?;
    Ok((algorithm, payload))
}

pub(crate) fn build_metadata(
    inline: Option<&JsonValue>,
    file: Option<PathBuf>,
) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    if let Some(value) = inline {
        merge_metadata_value(&mut metadata, value)?;
    }
    if let Some(path) = file {
        let bytes = fs::read(&path)
            .wrap_err_with(|| format!("failed to read metadata file {}", path.display()))?;
        let parsed: JsonValue = serde_json::from_slice(&bytes)
            .wrap_err_with(|| format!("metadata file {} is not valid JSON", path.display()))?;
        merge_metadata_value(&mut metadata, &parsed)?;
    }
    Ok(metadata)
}

fn merge_metadata_value(metadata: &mut Metadata, value: &JsonValue) -> Result<()> {
    let obj = value
        .as_object()
        .ok_or_else(|| eyre!("metadata entries must be JSON objects (received {value:?})"))?;
    for (key, val) in obj {
        let name = Name::from_str(key)
            .map_err(|err| eyre!("invalid metadata key `{key}`: {}", err.reason()))?;
        let serialized = serde_json::to_string(val)?;
        let json = Json::from_str(&serialized)
            .map_err(|e| eyre!("failed to serialise metadata entry `{key}`: {e}"))?;
        metadata.insert(name, json);
    }
    Ok(())
}

pub(crate) fn write_norito_bytes<T>(path: &Path, value: &T) -> Result<()>
where
    T: Encode,
{
    let bytes = to_bytes(value)?;
    fs::write(path, bytes).wrap_err_with(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub(crate) fn write_json<T>(path: &Path, value: &T) -> Result<()>
where
    T: ?Sized + JsonSerialize,
{
    let mut rendered = serde_json::to_json_pretty(value)?;
    rendered.push('\n');
    fs::write(path, rendered).wrap_err_with(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub(crate) fn write_norito_json<T>(path: &Path, value: &T) -> Result<()>
where
    T: ?Sized + JsonSerialize,
{
    let mut rendered = serde_json::to_json_pretty(value)?;
    rendered.push('\n');
    fs::write(path, rendered).wrap_err_with(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub(crate) fn parse_account_id(spec: &str) -> Result<AccountId> {
    AccountId::from_str(spec).map_err(|err| eyre!("invalid account id `{spec}`: {err}"))
}

pub(crate) fn parse_asset_id(spec: &str) -> Result<AssetId> {
    AssetId::from_str(spec).map_err(|err| eyre!("invalid asset id `{spec}`: {err}"))
}

pub(crate) fn parse_numeric(input: &str) -> Result<Numeric> {
    Numeric::from_str(input).map_err(|err| eyre!(err))
}

pub(crate) fn parse_hash_hex(field: &str, value: &str) -> Result<Hash> {
    Hash::from_str(value).map_err(|err| eyre!("invalid {field} `{value}`: {err}"))
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn split_key_spec_parses_algorithm() {
        let (algo, payload) = split_key_spec("ed25519:abcd").expect("parsed");
        assert_eq!(algo, Algorithm::Ed25519);
        assert_eq!(payload, "abcd");
    }

    #[test]
    fn build_metadata_merges_sources() {
        let inline = norito::json!({"ios.app_attest.team_id":"XYZ"});
        let temp = NamedTempFile::new().expect("temp file");
        fs::write(
            temp.path(),
            br#"{"android.attestation.require_strongbox":true}"#,
        )
        .expect("write metadata file");
        let metadata = build_metadata(Some(&inline), Some(temp.path().into())).expect("metadata");
        let keys: Vec<_> = metadata.iter().map(|(k, _)| k.to_string()).collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"ios.app_attest.team_id".to_string()));
        assert!(keys.contains(&"android.attestation.require_strongbox".to_string()));
    }
}
