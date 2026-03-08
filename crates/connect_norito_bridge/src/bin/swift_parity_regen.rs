//! Regenerate Swift parity fixtures from payload JSON using the current encoder.

use std::{
    collections::BTreeMap, fs, num::NonZeroU32, path::PathBuf, str::FromStr, time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use iroha_data_model::{
    account::{AccountId, address},
    asset::{AssetId, id::AssetDefinitionId},
    isi::{Burn, InstructionBox, Mint, Transfer},
    metadata::Metadata,
    name::Name,
    transaction::{
        Executable, IvmBytecode, SignedTransaction, TransactionBuilder, signed::TransactionPayload,
    },
};
use iroha_primitives::{json::Json, numeric::Numeric};
use norito::{
    codec::Encode,
    json::{self, Map, Number, Value},
};

const DEFAULT_FIXTURES_PATH: &str = "IrohaSwift/Fixtures/swift_parity_payloads.json";
const DEFAULT_OUT_DIR: &str = "IrohaSwift/Fixtures";
const DEFAULT_MANIFEST_NAME: &str = "swift_parity_manifest.json";
const PAYLOAD_SCHEMA_NAME: &str = "iroha.android.transaction.Payload.v1";
const SIGNED_SCHEMA_NAME: &str = "iroha.transaction.SignedTransaction.v1";
const SIGNING_SEED_HEX: &str = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";
const DEFAULT_CHAIN_DISCRIMINANT: u16 = 0x02F1;

#[derive(Debug, norito::json::JsonDeserialize)]
struct PayloadFileEntry {
    name: String,
    payload: PayloadSpec,
}

#[derive(Debug, norito::json::JsonDeserialize)]
struct PayloadSpec {
    chain: String,
    authority: String,
    creation_time_ms: u64,
    executable: Value,
    time_to_live_ms: Option<u64>,
    nonce: Option<u32>,
    metadata: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, norito::json::JsonDeserialize, norito::json::JsonSerialize)]
struct InstructionSpec {
    kind: String,
    arguments: BTreeMap<String, String>,
}

struct FixtureOutput {
    name: String,
    payload_bytes: Vec<u8>,
    signed_bytes: Vec<u8>,
    chain: String,
    authority: String,
    ttl_ms: Option<u64>,
    nonce: Option<u32>,
    payload_base64: String,
    signed_base64: String,
    payload_hash: String,
    signed_hash: String,
}

struct ChainDiscriminantReset(u16);

impl ChainDiscriminantReset {
    fn new(discriminant: u16) -> Self {
        let previous = address::set_chain_discriminant(discriminant);
        Self(previous)
    }
}

impl Drop for ChainDiscriminantReset {
    fn drop(&mut self) {
        address::set_chain_discriminant(self.0);
    }
}

impl PayloadSpec {
    fn to_builder(&self) -> Result<TransactionBuilder, String> {
        let chain_id = self
            .chain
            .parse()
            .map_err(|_| format!("invalid chain id '{}'", self.chain))?;
        let authority = AccountId::parse_encoded(&self.authority)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .map_err(|_| format!("invalid authority id '{}'", self.authority))?;
        let mut builder = TransactionBuilder::new(chain_id, authority.clone());
        builder.set_creation_time(Duration::from_millis(self.creation_time_ms));
        if let Some(ttl_ms) = self.time_to_live_ms {
            builder.set_ttl(Duration::from_millis(ttl_ms));
        }
        if let Some(nonce) = self.nonce {
            let nz = NonZeroU32::new(nonce).ok_or_else(|| "nonce must be > 0".to_string())?;
            builder.set_nonce(nz);
        }

        let mut metadata = Metadata::default();
        if let Some(entries) = &self.metadata {
            for (key, value) in entries {
                let name =
                    Name::from_str(key).map_err(|_| format!("invalid metadata key '{key}'"))?;
                metadata.insert(name, Json::new(value.clone()));
            }
        }
        builder = builder.with_metadata(metadata);

        let executable = self
            .executable
            .as_object()
            .ok_or_else(|| "executable must be a JSON object".to_string())?;
        if let Some(value) = executable.get("Instructions") {
            let raws: Vec<InstructionSpec> =
                json::from_value(value.clone()).map_err(|err| err.to_string())?;
            let mut instructions = Vec::with_capacity(raws.len());
            for raw in raws {
                instructions.push(raw.to_instruction(&authority)?);
            }
            builder = builder.with_instructions(instructions);
        } else if let Some(value) = executable.get("Ivm") {
            let base64 = value
                .as_str()
                .ok_or_else(|| "Ivm executable must be a base64 string".to_string())?;
            let bytes = BASE64
                .decode(base64.as_bytes())
                .map_err(|err| format!("invalid IVM base64 payload: {err}"))?;
            builder = builder.with_executable(Executable::Ivm(IvmBytecode::from_compiled(bytes)));
        } else {
            return Err("unsupported executable variant".to_string());
        }

        Ok(builder)
    }
}

impl InstructionSpec {
    fn to_instruction(&self, authority: &AccountId) -> Result<InstructionBox, String> {
        let action = self
            .arguments
            .get("action")
            .ok_or_else(|| "missing 'action' argument".to_string())?;
        match action.as_str() {
            "TransferAsset" => {
                if self.kind != "Transfer" {
                    return Err(format!(
                        "expected Transfer kind for TransferAsset, got '{}'",
                        self.kind
                    ));
                }
                let asset_definition = self
                    .arguments
                    .get("asset_definition_id")
                    .ok_or_else(|| "missing 'asset_definition_id' argument".to_string())?;
                let quantity = self
                    .arguments
                    .get("quantity")
                    .ok_or_else(|| "missing 'quantity' argument".to_string())?;
                let destination = self
                    .arguments
                    .get("destination")
                    .ok_or_else(|| "missing 'destination' argument".to_string())?;
                let asset_definition: AssetDefinitionId =
                    asset_definition.parse().map_err(|err| {
                        format!("invalid asset definition '{asset_definition}': {err}")
                    })?;
                let quantity: Numeric = quantity
                    .parse()
                    .map_err(|err| format!("invalid quantity '{quantity}': {err}"))?;
                let destination: AccountId = AccountId::parse_encoded(destination)
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                    .map_err(|_| format!("invalid destination account '{destination}'"))?;
                let asset_id = AssetId::new(asset_definition, authority.clone());
                Ok(Transfer::asset_numeric(asset_id, quantity, destination).into())
            }
            "MintAsset" => {
                if self.kind != "Mint" {
                    return Err(format!(
                        "expected Mint kind for MintAsset, got '{}'",
                        self.kind
                    ));
                }
                let asset_definition = self
                    .arguments
                    .get("asset_definition_id")
                    .ok_or_else(|| "missing 'asset_definition_id' argument".to_string())?;
                let quantity = self
                    .arguments
                    .get("quantity")
                    .ok_or_else(|| "missing 'quantity' argument".to_string())?;
                let destination = self
                    .arguments
                    .get("destination")
                    .ok_or_else(|| "missing 'destination' argument".to_string())?;
                let asset_definition: AssetDefinitionId =
                    asset_definition.parse().map_err(|err| {
                        format!("invalid asset definition '{asset_definition}': {err}")
                    })?;
                let quantity: Numeric = quantity
                    .parse()
                    .map_err(|err| format!("invalid quantity '{quantity}': {err}"))?;
                let destination: AccountId = AccountId::parse_encoded(destination)
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                    .map_err(|_| format!("invalid destination account '{destination}'"))?;
                let asset_id = AssetId::new(asset_definition, destination);
                Ok(Mint::asset_numeric(quantity, asset_id).into())
            }
            "BurnAsset" => {
                if self.kind != "Burn" {
                    return Err(format!(
                        "expected Burn kind for BurnAsset, got '{}'",
                        self.kind
                    ));
                }
                let asset_definition = self
                    .arguments
                    .get("asset_definition_id")
                    .ok_or_else(|| "missing 'asset_definition_id' argument".to_string())?;
                let quantity = self
                    .arguments
                    .get("quantity")
                    .ok_or_else(|| "missing 'quantity' argument".to_string())?;
                let destination = self
                    .arguments
                    .get("destination")
                    .ok_or_else(|| "missing 'destination' argument".to_string())?;
                let asset_definition: AssetDefinitionId =
                    asset_definition.parse().map_err(|err| {
                        format!("invalid asset definition '{asset_definition}': {err}")
                    })?;
                let quantity: Numeric = quantity
                    .parse()
                    .map_err(|err| format!("invalid quantity '{quantity}': {err}"))?;
                let destination: AccountId = AccountId::parse_encoded(destination)
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                    .map_err(|_| format!("invalid destination account '{destination}'"))?;
                let asset_id = AssetId::new(asset_definition, destination);
                Ok(Burn::asset_numeric(quantity, asset_id).into())
            }
            other => Err(format!("unsupported instruction action '{other}'")),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(err) = run() {
        return Err(std::io::Error::other(err).into());
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let fixtures_path = PathBuf::from(DEFAULT_FIXTURES_PATH);
    let out_dir = PathBuf::from(DEFAULT_OUT_DIR);
    let manifest_path = out_dir.join(DEFAULT_MANIFEST_NAME);
    let _chain_guard = ChainDiscriminantReset::new(DEFAULT_CHAIN_DISCRIMINANT);

    let payload_bytes = fs::read(&fixtures_path)
        .map_err(|err| format!("failed to read {}: {err}", fixtures_path.display()))?;
    let entries: Vec<PayloadFileEntry> = norito::json::from_slice(&payload_bytes)
        .map_err(|err| format!("invalid payload JSON: {err}"))?;

    let seed = hex::decode(SIGNING_SEED_HEX).map_err(|err| err.to_string())?;
    let keypair = KeyPair::from_seed(seed, Algorithm::Ed25519);
    let (_, public_key_bytes) = keypair.public_key().to_bytes();
    let public_key_hex = hex::encode(public_key_bytes);

    fs::create_dir_all(&out_dir)
        .map_err(|err| format!("failed to create {}: {err}", out_dir.display()))?;

    let mut fixtures = Vec::with_capacity(entries.len());
    for entry in entries {
        let builder = entry.payload.to_builder()?;
        let signed = builder.sign(keypair.private_key());
        let payload = signed.payload().clone();
        let payload_bytes = payload.encode();
        let signed_bytes = signed.encode();
        let payload_base64 = BASE64.encode(&payload_bytes);
        let signed_base64 = BASE64.encode(&signed_bytes);
        let payload_hash = format!("{}", HashOf::<TransactionPayload>::new(&payload));
        let signed_hash = format!("{}", HashOf::<SignedTransaction>::new(&signed));

        let fixture = FixtureOutput {
            name: entry.name,
            payload_bytes,
            signed_bytes,
            chain: payload.chain.to_string(),
            authority: payload.authority.to_string(),
            ttl_ms: payload.time_to_live_ms.map(|v| v.get()),
            nonce: payload.nonce.map(|v| v.get()),
            payload_base64,
            signed_base64,
            payload_hash,
            signed_hash,
        };

        let norito_path = out_dir.join(format!("{}.norito", fixture.name));
        fs::write(&norito_path, &fixture.payload_bytes)
            .map_err(|err| format!("failed to write {}: {err}", norito_path.display()))?;
        fixtures.push(fixture);
    }

    // Preserve the previous timestamp to avoid adding a new time dependency here.
    let generated_at = fs::read(&manifest_path)
        .ok()
        .and_then(|bytes| json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| value.as_object().cloned())
        .and_then(|map| map.get("generated_at").cloned())
        .and_then(|value| value.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
    let entries: Vec<Value> = fixtures
        .iter()
        .map(|fixture| {
            let mut map = Map::new();
            map.insert("name".into(), Value::String(fixture.name.clone()));
            map.insert(
                "payload_base64".into(),
                Value::String(fixture.payload_base64.clone()),
            );
            map.insert(
                "signed_base64".into(),
                Value::String(fixture.signed_base64.clone()),
            );
            map.insert(
                "payload_hash".into(),
                Value::String(fixture.payload_hash.clone()),
            );
            map.insert(
                "signed_hash".into(),
                Value::String(fixture.signed_hash.clone()),
            );
            map.insert("chain".into(), Value::String(fixture.chain.clone()));
            map.insert("authority".into(), Value::String(fixture.authority.clone()));
            map.insert(
                "time_to_live_ms".into(),
                fixture
                    .ttl_ms
                    .map(|value| Value::Number(Number::U64(value)))
                    .unwrap_or(Value::Null),
            );
            map.insert(
                "nonce".into(),
                fixture
                    .nonce
                    .map(|value| Value::Number(Number::U64(value as u64)))
                    .unwrap_or(Value::Null),
            );
            map.insert(
                "encoded_file".into(),
                Value::String(format!("{}.norito", fixture.name)),
            );
            map.insert(
                "encoded_len".into(),
                Value::Number(Number::U64(fixture.payload_bytes.len() as u64)),
            );
            map.insert(
                "signed_len".into(),
                Value::Number(Number::U64(fixture.signed_bytes.len() as u64)),
            );
            Value::Object(map)
        })
        .collect();

    let mut schema = Map::new();
    schema.insert(
        "payload".into(),
        Value::String(PAYLOAD_SCHEMA_NAME.to_string()),
    );
    schema.insert(
        "signed".into(),
        Value::String(SIGNED_SCHEMA_NAME.to_string()),
    );

    let mut signing_key = Map::new();
    signing_key.insert("algorithm".into(), Value::String("ed25519".into()));
    signing_key.insert("seed_hex".into(), Value::String(SIGNING_SEED_HEX.into()));
    signing_key.insert("public_key_hex".into(), Value::String(public_key_hex));

    let mut root = Map::new();
    root.insert("generated_at".into(), Value::String(generated_at));
    root.insert("schema".into(), Value::Object(schema));
    root.insert("signing_key".into(), Value::Object(signing_key));
    root.insert("fixtures".into(), Value::Array(entries));
    let manifest_json = json::to_json_pretty(&Value::Object(root))
        .map_err(|err| format!("failed to serialize manifest: {err}"))?;
    fs::write(&manifest_path, format!("{manifest_json}\n"))
        .map_err(|err| format!("failed to write {}: {err}", manifest_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use iroha_data_model::domain::DomainId;

    use super::*;

    fn account_literal(account: &AccountId) -> String {
        account.to_string()
    }

    #[test]
    fn instruction_builder_rejects_legacy_asset_literal_argument() {
        let keypair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let authority = AccountId::new(domain.clone(), keypair.public_key().clone());
        let destination = AccountId::new(domain, keypair.public_key().clone());

        let mut args = BTreeMap::new();
        args.insert("action".into(), "TransferAsset".into());
        args.insert("asset".into(), "rose#wonderland#alice@wonderland".into());
        args.insert("quantity".into(), "1.2500".into());
        args.insert("destination".into(), account_literal(&destination));
        let instruction = InstructionSpec {
            kind: "Transfer".into(),
            arguments: args,
        };

        let err = instruction
            .to_instruction(&authority)
            .expect_err("legacy asset literal argument should fail");
        assert!(
            err.contains("missing 'asset_definition_id' argument"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn payload_builder_sets_nonce_and_ttl() {
        let keypair = KeyPair::from_seed(vec![0xCD; 32], Algorithm::Ed25519);
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let authority = AccountId::new(domain.clone(), keypair.public_key().clone());
        let destination = AccountId::new(domain, keypair.public_key().clone());
        let mut args = BTreeMap::new();
        args.insert("action".into(), "TransferAsset".into());
        args.insert("asset_definition_id".into(), "rose#wonderland".into());
        args.insert("quantity".into(), "1.2500".into());
        args.insert("destination".into(), account_literal(&destination));
        let payload = PayloadSpec {
            chain: "00000042".into(),
            authority: account_literal(&authority),
            creation_time_ms: 123,
            executable: {
                let mut exec = Map::new();
                let instructions = json::to_value(&vec![InstructionSpec {
                    kind: "Transfer".into(),
                    arguments: args,
                }])
                .expect("instruction to json");
                exec.insert("Instructions".into(), instructions);
                Value::Object(exec)
            },
            time_to_live_ms: Some(3500),
            nonce: Some(17),
            metadata: None,
        };
        let builder = payload.to_builder().expect("builder");
        let signed = builder.sign(keypair.private_key());
        assert_eq!(signed.payload().nonce.map(|v| v.get()), Some(17));
        assert_eq!(
            signed.payload().time_to_live_ms.map(|v| v.get()),
            Some(3500)
        );
        match signed.instructions() {
            Executable::Instructions(instructions) => assert_eq!(instructions.len(), 1),
            _ => panic!("expected instruction executable"),
        }
    }
}
