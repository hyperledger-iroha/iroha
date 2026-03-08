//! Generate per-client Iroha CLI configs from a base client.toml.

use std::{
    collections::BTreeSet,
    fmt::Write as _,
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use clap::Args as ClapArgs;
use color_eyre::eyre::{Result, WrapErr as _, eyre};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};

use crate::{Outcome, RunArgs, tui};

const DEFAULT_TTL_MS: u64 = 120_000;
const DEFAULT_STATUS_TIMEOUT_MS: u64 = 120_000;

#[derive(Debug, Clone)]
struct BaseConfig {
    chain: String,
    torii_url: String,
    basic_auth: Option<BasicAuth>,
}

#[derive(Debug, Clone)]
struct BasicAuth {
    web_login: String,
    password: String,
}

/// Generate per-client CLI configs from a base client.toml.
#[derive(ClapArgs, Debug, Clone)]
pub struct Args {
    /// Base client config to copy `chain`, `torii_url`, and `basic_auth` from.
    #[arg(long, value_name = "PATH")]
    base_config: PathBuf,
    /// Output directory for generated client configs (default: <base-config-dir>/clients).
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,
    /// Account domain for generated client configs.
    #[arg(long, default_value = "acme", value_name = "DOMAIN")]
    domain: String,
    /// Seed prefix for deterministic key generation (`<prefix>-<name>`).
    #[arg(long, default_value = "demo", value_name = "SEED")]
    seed_prefix: String,
    /// Comma-separated list of client names.
    #[arg(long, value_delimiter = ',', required = true, value_name = "NAME")]
    names: Vec<String>,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let base = load_base_config(&self.base_config)?;
        let out_dir = resolve_out_dir(&self.base_config, self.out_dir)?;
        let names = normalize_names(self.names)?;
        fs::create_dir_all(&out_dir)
            .wrap_err_with(|| format!("failed to create {}", out_dir.display()))?;

        tui::status(format!(
            "Generating {} client configs in {}",
            names.len(),
            out_dir.display()
        ));

        for name in names {
            let seed = format!("{}-{}", self.seed_prefix, name);
            let key_pair = KeyPair::from_seed(seed.as_bytes().to_vec(), Algorithm::Ed25519);
            let rendered = render_client_config(&base, &self.domain, &key_pair);
            let path = out_dir.join(format!("{name}.toml"));
            fs::write(&path, rendered)
                .wrap_err_with(|| format!("failed to write {}", path.display()))?;
            writeln!(writer, "wrote {}", path.display())?;
        }

        tui::success("Client configs ready");
        Ok(())
    }
}

fn load_base_config(path: &Path) -> Result<BaseConfig> {
    let raw =
        fs::read_to_string(path).wrap_err_with(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&raw).wrap_err_with(|| format!("invalid TOML in {}", path.display()))?;

    let chain = value
        .get("chain")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("base config is missing `chain`"))?
        .to_owned();
    let torii_url = value
        .get("torii_url")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("base config is missing `torii_url`"))?
        .to_owned();

    let basic_auth = match value.get("basic_auth").and_then(toml::Value::as_table) {
        Some(table) => {
            let web_login = table
                .get("web_login")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| eyre!("base config `basic_auth.web_login` is missing"))?
                .to_owned();
            let password = table
                .get("password")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| eyre!("base config `basic_auth.password` is missing"))?
                .to_owned();
            Some(BasicAuth {
                web_login,
                password,
            })
        }
        None => None,
    };

    Ok(BaseConfig {
        chain,
        torii_url,
        basic_auth,
    })
}

fn resolve_out_dir(base_config: &Path, out_dir: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(out_dir) = out_dir {
        return Ok(out_dir);
    }
    let parent = base_config
        .parent()
        .ok_or_else(|| eyre!("base config has no parent directory"))?;
    Ok(parent.join("clients"))
}

fn normalize_names(raw: Vec<String>) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();

    for name in raw {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(eyre!("client name cannot be empty"));
        }
        if trimmed.contains('/') || trimmed.contains('\\') {
            return Err(eyre!(
                "client name `{}` must not contain path separators",
                trimmed
            ));
        }
        if !seen.insert(trimmed.to_owned()) {
            return Err(eyre!("duplicate client name `{}`", trimmed));
        }
        names.push(trimmed.to_owned());
    }

    if names.is_empty() {
        return Err(eyre!("no client names provided"));
    }

    Ok(names)
}

fn render_client_config(base: &BaseConfig, domain: &str, key_pair: &KeyPair) -> String {
    let public_key = key_pair.public_key().to_string();
    let private_key = ExposedPrivateKey(key_pair.private_key().clone()).to_string();

    let mut rendered = format!(
        concat!(
            "chain = \"{chain}\"\n",
            "torii_url = \"{torii_url}\"\n",
            "\n",
            "[transaction]\n",
            "time_to_live_ms = {ttl}\n",
            "status_timeout_ms = {status}\n",
            "nonce = false\n",
            "\n",
            "[account]\n",
            "domain = \"{domain}\"\n",
            "private_key = \"{private_key}\"\n",
            "public_key  = \"{public_key}\"\n",
        ),
        chain = base.chain,
        torii_url = base.torii_url,
        ttl = DEFAULT_TTL_MS,
        status = DEFAULT_STATUS_TIMEOUT_MS,
        domain = domain,
        private_key = private_key,
        public_key = public_key,
    );

    if let Some(auth) = &base.basic_auth {
        rendered.push_str("\n[basic_auth]\n");
        let _ = writeln!(rendered, "password  = \"{}\"", auth.password);
        let _ = writeln!(rendered, "web_login = \"{}\"", auth.web_login);
    }

    rendered
}

#[cfg(test)]
mod tests {
    use std::{fs, io::BufWriter};

    use super::*;

    fn write_base_config(path: &Path) {
        let payload = r#"
chain = "demo-chain"
torii_url = "http://127.0.0.1:8080/"

[basic_auth]
password = "secret"
web_login = "demo"
"#;
        fs::write(path, payload).expect("write base config");
    }

    #[test]
    fn load_base_config_reads_fields() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("client.toml");
        write_base_config(&path);

        let base = load_base_config(&path).expect("load base config");
        assert_eq!(base.chain, "demo-chain");
        assert_eq!(base.torii_url, "http://127.0.0.1:8080/");
        let auth = base.basic_auth.expect("basic auth present");
        assert_eq!(auth.web_login, "demo");
        assert_eq!(auth.password, "secret");
    }

    #[test]
    fn resolve_out_dir_defaults_to_clients_dir() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("client.toml");
        let out_dir = resolve_out_dir(&path, None).expect("resolve out dir");
        assert_eq!(out_dir, temp.path().join("clients"));
    }

    #[test]
    fn normalize_names_trims_and_rejects_duplicates() {
        let names = normalize_names(vec![" admin1 ".into(), "admin2".into()]).expect("names ok");
        assert_eq!(names, vec!["admin1".to_owned(), "admin2".to_owned()]);

        let err = normalize_names(vec!["admin1".into(), "admin1".into()])
            .expect_err("duplicate rejected");
        assert!(format!("{err}").contains("duplicate client name"));
    }

    #[test]
    fn render_client_config_contains_expected_fields() {
        let base = BaseConfig {
            chain: "demo-chain".to_owned(),
            torii_url: "http://127.0.0.1:8080/".to_owned(),
            basic_auth: Some(BasicAuth {
                web_login: "demo".to_owned(),
                password: "secret".to_owned(),
            }),
        };
        let key_pair = KeyPair::from_seed(b"demo-admin1".to_vec(), Algorithm::Ed25519);
        let rendered = render_client_config(&base, "acme", &key_pair);
        let value: toml::Value = toml::from_str(&rendered).expect("parse rendered config");

        assert_eq!(
            value.get("chain").and_then(toml::Value::as_str),
            Some("demo-chain")
        );
        assert_eq!(
            value.get("torii_url").and_then(toml::Value::as_str),
            Some("http://127.0.0.1:8080/")
        );

        let account = value
            .get("account")
            .and_then(toml::Value::as_table)
            .expect("account");
        assert_eq!(
            account.get("domain").and_then(toml::Value::as_str),
            Some("acme")
        );
        let expected_public = key_pair.public_key().to_string();
        let expected_private = ExposedPrivateKey(key_pair.private_key().clone()).to_string();
        assert_eq!(
            account.get("public_key").and_then(toml::Value::as_str),
            Some(expected_public.as_str())
        );
        assert_eq!(
            account.get("private_key").and_then(toml::Value::as_str),
            Some(expected_private.as_str())
        );

        let transaction = value
            .get("transaction")
            .and_then(toml::Value::as_table)
            .expect("transaction");
        assert_eq!(
            transaction
                .get("time_to_live_ms")
                .and_then(toml::Value::as_integer),
            Some(i64::try_from(DEFAULT_TTL_MS).expect("DEFAULT_TTL_MS fits i64"))
        );
        assert_eq!(
            transaction
                .get("status_timeout_ms")
                .and_then(toml::Value::as_integer),
            Some(
                i64::try_from(DEFAULT_STATUS_TIMEOUT_MS)
                    .expect("DEFAULT_STATUS_TIMEOUT_MS fits i64"),
            )
        );
        assert_eq!(
            transaction.get("nonce").and_then(toml::Value::as_bool),
            Some(false)
        );

        let basic_auth = value
            .get("basic_auth")
            .and_then(toml::Value::as_table)
            .expect("basic_auth");
        assert_eq!(
            basic_auth.get("web_login").and_then(toml::Value::as_str),
            Some("demo")
        );
        assert_eq!(
            basic_auth.get("password").and_then(toml::Value::as_str),
            Some("secret")
        );
    }

    #[test]
    fn run_writes_client_configs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let base_path = temp.path().join("client.toml");
        write_base_config(&base_path);
        let out_dir = temp.path().join("clients");

        let args = Args {
            base_config: base_path.clone(),
            out_dir: Some(out_dir.clone()),
            domain: "acme".to_owned(),
            seed_prefix: "demo".to_owned(),
            names: vec!["admin1".to_owned()],
        };

        let mut writer = BufWriter::new(Vec::new());
        args.run(&mut writer).expect("run client configs");

        let config_path = out_dir.join("admin1.toml");
        assert!(config_path.exists());
    }
}
