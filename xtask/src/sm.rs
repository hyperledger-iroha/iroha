use std::{
    error::Error,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hex::encode_upper;
use iroha_crypto::{
    Algorithm, PrivateKey, PublicKey,
    sm::{
        Sm2PrivateKey, Sm2PublicKey, encode_sm2_private_key_payload, encode_sm2_public_key_payload,
    },
};
use norito::json::{self, Value};
use rand_core_06::OsRng;
use zeroize::Zeroize;

#[derive(Clone)]
pub enum OutputTarget {
    Stdout,
    File(PathBuf),
}

impl OutputTarget {
    pub fn file(path: PathBuf) -> Self {
        OutputTarget::File(path)
    }
}

#[derive(Default)]
pub struct SmOperatorSnippetOptions {
    pub distid: Option<String>,
    pub seed_hex: Option<String>,
    pub json_out: Option<OutputTarget>,
    pub snippet_out: Option<OutputTarget>,
}

/// Source for Wycheproof SM2 fixtures.
pub enum WycheproofInput {
    File(PathBuf),
    Url(String),
    Official,
}

/// Options for importing/sanitizing Wycheproof SM2 fixtures.
pub struct WycheproofSyncOptions {
    pub input: WycheproofInput,
    pub output: PathBuf,
    pub generator_version: Option<String>,
    pub pretty: bool,
}

pub fn generate_sm_operator_snippet(
    options: SmOperatorSnippetOptions,
) -> Result<(), Box<dyn Error>> {
    let distid = options.distid.unwrap_or_else(Sm2PublicKey::default_distid);
    let private = match options.seed_hex {
        Some(seed_hex) => {
            let seed = decode_hex(&seed_hex)?;
            Sm2PrivateKey::from_seed(distid.clone(), &seed)?
        }
        None => {
            let mut rng = OsRng;
            Sm2PrivateKey::random(distid.clone(), &mut rng)
        }
    };

    let material = SmOperatorKeyMaterial::new(&private)?;

    let json_target = options
        .json_out
        .unwrap_or_else(|| OutputTarget::file(Path::new("sm2-key.json").to_path_buf()));
    let snippet_target = options
        .snippet_out
        .unwrap_or_else(|| OutputTarget::file(Path::new("client-sm2.toml").to_path_buf()));

    let json_payload = material.json_pretty();
    write_output(&json_target, json_payload.as_bytes())?;

    let snippet_payload = material.snippet().to_string();
    write_output(&snippet_target, snippet_payload.as_bytes())?;

    Ok(())
}

const WYCHEPROOF_SM2_OFFICIAL_URL: &str =
    "https://raw.githubusercontent.com/google/wycheproof/main/testvectors/sm2_sign_test.json";

pub fn sync_wycheproof(options: WycheproofSyncOptions) -> Result<(), Box<dyn Error>> {
    let WycheproofSyncOptions {
        input,
        output,
        generator_version,
        pretty,
    } = options;

    let raw = match input {
        WycheproofInput::File(path) => fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?,
        WycheproofInput::Url(url) => fetch_wycheproof_url(&url)?,
        WycheproofInput::Official => fetch_wycheproof_url(WYCHEPROOF_SM2_OFFICIAL_URL)?,
    };
    let parsed: Value =
        norito::json::from_str(&raw).map_err(|err| format!("parse Wycheproof SM2 JSON: {err}"))?;

    let mut root = parsed
        .as_object()
        .cloned()
        .ok_or("Wycheproof SM2 payload must be a JSON object")?;

    let algorithm = root
        .get("algorithm")
        .and_then(Value::as_str)
        .unwrap_or("SM2")
        .to_string();

    let mut sanitized_groups = Vec::new();
    let mut total_tests: u64 = 0;

    let groups = root
        .remove("testGroups")
        .ok_or("Wycheproof SM2 payload missing testGroups")?;
    let groups = groups
        .as_array()
        .cloned()
        .ok_or("Wycheproof SM2 testGroups must be an array")?;

    for group in groups {
        let mut group_obj = group
            .as_object()
            .cloned()
            .ok_or("Wycheproof SM2 test group must be an object")?;

        let distid = group_obj
            .remove("distid")
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "1234567812345678".to_string());

        let group_id = group_obj
            .get("groupId")
            .and_then(Value::as_u64)
            .map(Value::from);

        let key_value = group_obj
            .remove("key")
            .ok_or("Wycheproof SM2 group missing key")?;
        let key_obj = key_value
            .as_object()
            .ok_or("Wycheproof SM2 key must be an object")?;
        let uncompressed = key_obj
            .get("uncompressed")
            .and_then(Value::as_str)
            .ok_or("Wycheproof SM2 key missing uncompressed SEC1 point")?
            .to_string();

        let tests_value = group_obj
            .remove("tests")
            .ok_or("Wycheproof SM2 group missing tests array")?;
        let tests = tests_value
            .as_array()
            .cloned()
            .ok_or("Wycheproof SM2 tests must be an array")?;

        let mut sanitized_tests = Vec::with_capacity(tests.len());
        for test in tests {
            let test_obj = test
                .as_object()
                .ok_or("Wycheproof SM2 test case must be an object")?;

            let tc_id = test_obj
                .get("tcId")
                .and_then(Value::as_u64)
                .ok_or("Wycheproof SM2 test missing tcId")?;
            let comment = test_obj
                .get("comment")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let message = test_obj
                .get("msg")
                .and_then(Value::as_str)
                .ok_or("Wycheproof SM2 test missing msg")?
                .to_string();
            let signature = test_obj
                .get("sig")
                .and_then(Value::as_str)
                .ok_or("Wycheproof SM2 test missing sig")?
                .to_string();
            let result = test_obj
                .get("result")
                .and_then(Value::as_str)
                .ok_or("Wycheproof SM2 test missing result")?
                .to_string();

            let mut flag_values = Vec::new();
            if let Some(flags) = test_obj.get("flags").and_then(Value::as_array) {
                for flag in flags {
                    if let Some(text) = flag.as_str() {
                        flag_values.push(Value::from(text.to_string()));
                    }
                }
            }

            let mut test_map = norito::json::Map::new();
            test_map.insert("tcId".to_string(), Value::from(tc_id));
            test_map.insert("comment".to_string(), Value::from(comment));
            test_map.insert("flags".to_string(), Value::Array(flag_values));
            test_map.insert("msg".to_string(), Value::from(message));
            test_map.insert("result".to_string(), Value::from(result));
            test_map.insert("sig".to_string(), Value::from(signature));

            sanitized_tests.push(Value::Object(test_map));
        }

        let tests_count = sanitized_tests.len() as u64;
        total_tests = total_tests
            .checked_add(tests_count)
            .ok_or("Wycheproof SM2 test count overflow")?;

        let mut sanitized_group = norito::json::Map::new();
        sanitized_group.insert("distid".to_string(), Value::from(distid));
        if let Some(id_value) = group_id {
            sanitized_group.insert("groupId".to_string(), id_value);
        }
        let mut key_map = norito::json::Map::new();
        key_map.insert("uncompressed".to_string(), Value::from(uncompressed));
        sanitized_group.insert("key".to_string(), Value::Object(key_map));
        if let Some(comment) = group_obj
            .get("comment")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            sanitized_group.insert("comment".to_string(), Value::from(comment.to_string()));
        }
        sanitized_group.insert("tests".to_string(), Value::Array(sanitized_tests));

        sanitized_groups.push(Value::Object(sanitized_group));
    }

    let mut sanitized_root = norito::json::Map::new();
    sanitized_root.insert("algorithm".to_string(), Value::from(algorithm));

    let generator_version = generator_version
        .or_else(|| {
            root.remove("generatorVersion")
                .and_then(|value| value.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| "iroha-sm2-fixture".to_string());
    sanitized_root.insert(
        "generatorVersion".to_string(),
        Value::from(generator_version),
    );

    sanitized_root.insert("numberOfTests".to_string(), Value::from(total_tests));
    sanitized_root.insert("testGroups".to_string(), Value::Array(sanitized_groups));

    let value = Value::Object(sanitized_root);
    let output_json = if pretty {
        norito::json::to_json_pretty(&value)?
    } else {
        norito::json::to_json(&value)?
    };

    write_parent(&output)?;
    fs::write(&output, output_json.as_bytes())
        .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
    println!("updated {} ({} tests)", output.display(), total_tests);

    Ok(())
}

fn fetch_wycheproof_url(url: &str) -> Result<String, Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("iroha-xtask/wycheproof-sync (rust)")
        .build()
        .map_err(|err| format!("build HTTP client: {err}"))?;
    let response = client
        .get(url)
        .send()
        .map_err(|err| format!("fetch {url}: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("fetch {url}: unexpected status {}", response.status()).into());
    }
    response
        .text()
        .map_err(|err| format!("read response body from {url}: {err}").into())
}

fn write_parent(path: &Path) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn write_output(target: &OutputTarget, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    match target {
        OutputTarget::Stdout => {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(bytes)?;
        }
        OutputTarget::File(path) => {
            write_parent(path)?;
            fs::write(path, bytes)?;
            println!("wrote {}", path.display());
        }
    }
    Ok(())
}

fn decode_hex(raw: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let trimmed = raw.trim();
    let without_prefix = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    Ok(hex::decode(without_prefix)?)
}

struct SmOperatorKeyMaterial {
    json: Value,
    snippet: String,
}

impl SmOperatorKeyMaterial {
    fn new(private: &Sm2PrivateKey) -> Result<Self, Box<dyn Error>> {
        let artifacts = collect_sm2_artifacts(private)?;
        let json = json::object([
            ("algorithm", Value::from("sm2")),
            ("distid", Value::from(artifacts.distid.clone())),
            (
                "private_key_hex",
                Value::from(artifacts.private_key_hex.clone()),
            ),
            (
                "private_key_b64",
                Value::from(artifacts.private_key_b64.clone()),
            ),
            (
                "private_key_config",
                Value::from(artifacts.private_key_config.clone()),
            ),
            (
                "private_key_pem",
                Value::from(artifacts.private_key_pem.clone()),
            ),
            (
                "public_key_sec1_hex",
                Value::from(artifacts.public_key_hex.clone()),
            ),
            (
                "public_key_sec1_compressed_hex",
                Value::from(artifacts.public_key_compressed_hex.clone()),
            ),
            (
                "public_key_b64",
                Value::from(artifacts.public_key_b64.clone()),
            ),
            (
                "public_key_config",
                Value::from(artifacts.public_key_config.clone()),
            ),
            (
                "public_key_pem",
                Value::from(artifacts.public_key_pem.clone()),
            ),
        ])
        .expect("static SM2 key payload");

        let snippet = render_snippet(&artifacts);

        Ok(Self { json, snippet })
    }

    fn json_pretty(&self) -> String {
        norito::json::to_json_pretty(&self.json).expect("SM2 payload serializes")
    }

    fn snippet(&self) -> &str {
        &self.snippet
    }
}

struct Sm2Artifacts {
    distid: String,
    private_key_hex: String,
    private_key_b64: String,
    private_key_config: String,
    private_key_pem: String,
    public_key_hex: String,
    public_key_b64: String,
    public_key_compressed_hex: String,
    public_key_config: String,
    public_key_pem: String,
}

fn collect_sm2_artifacts(private: &Sm2PrivateKey) -> Result<Sm2Artifacts, Box<dyn Error>> {
    let distid = private.distid().to_string();

    let mut secret = private.secret_bytes();
    let private_key_hex = encode_upper(secret);
    let private_key_b64 = BASE64.encode(secret);
    let private_payload = encode_sm2_private_key_payload(&distid, &secret)?;
    let private_key_config = PrivateKey::from_bytes(Algorithm::Sm2, &private_payload)?.to_string();
    let private_key_pem = private.to_pkcs8_pem()?;

    let public = private.public_key();
    let public_bytes = public.to_sec1_bytes(false);
    let public_compressed = public.to_sec1_bytes(true);
    let public_key_hex = encode_upper(&public_bytes);
    let public_key_b64 = BASE64.encode(&public_bytes);
    let public_key_compressed_hex = encode_upper(&public_compressed);
    let public_payload = encode_sm2_public_key_payload(&distid, &public_bytes)?;
    let public_key_config = PublicKey::from_bytes(Algorithm::Sm2, &public_payload)?.to_string();
    let public_key_pem = public.to_public_key_pem()?;

    secret.zeroize();

    Ok(Sm2Artifacts {
        distid,
        private_key_hex,
        private_key_b64,
        private_key_config,
        private_key_pem,
        public_key_hex,
        public_key_b64,
        public_key_compressed_hex,
        public_key_config,
        public_key_pem,
    })
}

fn render_snippet(artifacts: &Sm2Artifacts) -> String {
    format!(
        r#"# Account key material
public_key = "{public_key}"
private_key = "{private_key}"
# public_key_pem = \"\"\"\
{public_key_pem}\"\"\"
# private_key_pem = \"\"\"\
{private_key_pem}\"\"\"

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]  # remove "sm2" to stay in verify-only mode
sm2_distid_default = "{distid}"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
"#,
        public_key = artifacts.public_key_config,
        private_key = artifacts.private_key_hex,
        public_key_pem = artifacts.public_key_pem.trim_end(),
        private_key_pem = artifacts.private_key_pem.trim_end(),
        distid = artifacts.distid
    )
}
