//! Helpers for rendering and writing local application bootstrap files.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Relative path for the generated environment file.
pub const ENV_LOCAL_FILE: &str = ".env.local";
/// Relative path for the generated TypeScript sample.
pub const TYPESCRIPT_SAMPLE_FILE: &str = ".mochi/generated/typescript/connect.ts";
/// Relative path for the generated Rust sample.
pub const RUST_SAMPLE_FILE: &str = ".mochi/generated/rust/connect.rs";
/// Relative path for the generated Kotlin sample.
pub const KOTLIN_SAMPLE_FILE: &str = ".mochi/generated/kotlin/MochiConnect.kt";

/// Inputs shared across generated bootstrap artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapInputs {
    /// Base HTTP address used by explorer-style API requests.
    pub api_base: String,
    /// Torii base URL used for transaction/query submissions.
    pub torii_url: String,
    /// Chain identifier exposed by the local network.
    pub chain_id: String,
    /// Optional account identifier for the preferred dev signer.
    pub account_id: Option<String>,
    /// Optional private key for the preferred dev signer.
    pub private_key: Option<String>,
}

impl BootstrapInputs {
    /// Render shell `export` lines for copy/paste-friendly local development.
    #[must_use]
    pub fn render_shell_exports(&self) -> String {
        let mut lines = vec![
            "# local dev only; rename variables to match your app".to_owned(),
            format!(
                "export IROHA_API_BASE={}",
                shell_quote(&ensure_http_base(&self.api_base))
            ),
            format!(
                "export IROHA_TORII_URL={}",
                shell_quote(&ensure_http_base(&self.torii_url))
            ),
            format!("export IROHA_CHAIN_ID={}", shell_quote(&self.chain_id)),
        ];
        if let Some(account_id) = self.account_id.as_deref() {
            lines.push(format!(
                "export IROHA_ACCOUNT_ID={}",
                shell_quote(account_id)
            ));
        }
        if let Some(private_key) = self.private_key.as_deref() {
            lines.push(format!(
                "export IROHA_PRIVATE_KEY={}",
                shell_quote(private_key)
            ));
        }
        lines.join("\n")
    }

    /// Render a dotenv-style `.env.local` file.
    #[must_use]
    pub fn render_env_local(&self) -> String {
        let mut lines = vec![
            "IROHA_API_BASE=".to_owned() + &dotenv_quote(&ensure_http_base(&self.api_base)),
            "IROHA_TORII_URL=".to_owned() + &dotenv_quote(&ensure_http_base(&self.torii_url)),
            "IROHA_CHAIN_ID=".to_owned() + &dotenv_quote(&self.chain_id),
        ];
        if let Some(account_id) = self.account_id.as_deref() {
            lines.push("IROHA_ACCOUNT_ID=".to_owned() + &dotenv_quote(account_id));
        }
        if let Some(private_key) = self.private_key.as_deref() {
            lines.push("IROHA_PRIVATE_KEY=".to_owned() + &dotenv_quote(private_key));
        }
        lines.join("\n") + "\n"
    }
}

/// A generated bootstrap file and its relative destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapArtifact {
    /// Relative path from the selected workspace root.
    pub relative_path: PathBuf,
    /// File contents ready to write.
    pub contents: String,
}

impl BootstrapArtifact {
    /// Join the artifact path onto a workspace root.
    #[must_use]
    pub fn path_in(&self, workspace_root: &Path) -> PathBuf {
        workspace_root.join(&self.relative_path)
    }
}

/// The full bootstrap bundle Mochi can write into a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapBundle {
    /// Generated artifacts in write order.
    pub artifacts: Vec<BootstrapArtifact>,
}

impl BootstrapBundle {
    /// Build the standard local-development bundle.
    #[must_use]
    pub fn render(inputs: &BootstrapInputs) -> Self {
        Self {
            artifacts: vec![
                BootstrapArtifact {
                    relative_path: PathBuf::from(ENV_LOCAL_FILE),
                    contents: inputs.render_env_local(),
                },
                BootstrapArtifact {
                    relative_path: PathBuf::from(TYPESCRIPT_SAMPLE_FILE),
                    contents: render_typescript_sample(inputs),
                },
                BootstrapArtifact {
                    relative_path: PathBuf::from(RUST_SAMPLE_FILE),
                    contents: render_rust_sample(inputs),
                },
                BootstrapArtifact {
                    relative_path: PathBuf::from(KOTLIN_SAMPLE_FILE),
                    contents: render_kotlin_sample(inputs),
                },
            ],
        }
    }
}

/// Errors raised while writing bootstrap artifacts.
#[derive(Debug, thiserror::Error)]
pub enum BootstrapWriteError {
    /// One of the target files already exists and overwrite was not requested.
    #[error("bootstrap file already exists: {path}")]
    AlreadyExists { path: PathBuf },
    /// Filesystem operation failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Write a bundle to the target workspace.
pub fn write_bootstrap_bundle(
    workspace_root: &Path,
    bundle: &BootstrapBundle,
    replace_existing: bool,
) -> Result<Vec<PathBuf>, BootstrapWriteError> {
    let mut written = Vec::with_capacity(bundle.artifacts.len());
    for artifact in &bundle.artifacts {
        let destination = artifact.path_in(workspace_root);
        if destination.exists() && !replace_existing {
            return Err(BootstrapWriteError::AlreadyExists { path: destination });
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&destination, &artifact.contents)?;
        written.push(destination);
    }
    Ok(written)
}

/// Quote a shell value conservatively for copy/paste recipes.
#[must_use]
pub fn shell_quote(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "''".to_owned();
    }
    if trimmed.chars().all(|ch| {
        matches!(
            ch,
            'A'..='Z' | 'a'..='z' | '0'..='9' | '/' | '.' | '_' | '-' | ':' | '@'
        )
    }) {
        trimmed.to_owned()
    } else {
        format!("'{}'", trimmed.replace('\'', "'\"'\"'"))
    }
}

/// Ensure a host:port-ish value is rooted under `http://`.
#[must_use]
pub fn ensure_http_base(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_owned()
    } else {
        format!("http://{trimmed}")
    }
}

fn dotenv_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| !matches!(ch, '\n' | '\r' | '"' | '\'' | ' ' | '\t'))
    {
        value.to_owned()
    } else {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

fn render_typescript_sample(inputs: &BootstrapInputs) -> String {
    format!(
        r#"export type IrohaLocalConfig = {{
  apiBase: string;
  toriiUrl: string;
  chainId: string;
  accountId?: string;
  privateKey?: string;
}};

export const irohaLocalDefaults: IrohaLocalConfig = {{
  apiBase: process.env.IROHA_API_BASE ?? "{api_base}",
  toriiUrl: process.env.IROHA_TORII_URL ?? "{torii_url}",
  chainId: process.env.IROHA_CHAIN_ID ?? "{chain_id}",
  accountId: process.env.IROHA_ACCOUNT_ID ?? {account_id},
  privateKey: process.env.IROHA_PRIVATE_KEY ?? {private_key},
}};
"#,
        api_base = ensure_http_base(&inputs.api_base),
        torii_url = ensure_http_base(&inputs.torii_url),
        chain_id = inputs.chain_id,
        account_id = render_ts_optional(inputs.account_id.as_deref()),
        private_key = render_ts_optional(inputs.private_key.as_deref()),
    )
}

fn render_rust_sample(inputs: &BootstrapInputs) -> String {
    format!(
        r#"#[derive(Debug, Clone)]
pub struct IrohaLocalConfig {{
    pub api_base: String,
    pub torii_url: String,
    pub chain_id: String,
    pub account_id: Option<String>,
    pub private_key: Option<String>,
}}

impl IrohaLocalConfig {{
    pub fn from_env() -> Self {{
        Self {{
            api_base: std::env::var("IROHA_API_BASE")
                .unwrap_or_else(|_| "{api_base}".to_owned()),
            torii_url: std::env::var("IROHA_TORII_URL")
                .unwrap_or_else(|_| "{torii_url}".to_owned()),
            chain_id: std::env::var("IROHA_CHAIN_ID")
                .unwrap_or_else(|_| "{chain_id}".to_owned()),
            account_id: std::env::var("IROHA_ACCOUNT_ID").ok(),
            private_key: std::env::var("IROHA_PRIVATE_KEY").ok(),
        }}
    }}
}}
"#,
        api_base = ensure_http_base(&inputs.api_base),
        torii_url = ensure_http_base(&inputs.torii_url),
        chain_id = inputs.chain_id,
    )
}

fn render_kotlin_sample(inputs: &BootstrapInputs) -> String {
    format!(
        r#"data class IrohaLocalConfig(
    val apiBase: String,
    val toriiUrl: String,
    val chainId: String,
    val accountId: String?,
    val privateKey: String?
)

fun irohaLocalConfig(env: Map<String, String> = System.getenv()): IrohaLocalConfig =
    IrohaLocalConfig(
        apiBase = env["IROHA_API_BASE"] ?: "{api_base}",
        toriiUrl = env["IROHA_TORII_URL"] ?: "{torii_url}",
        chainId = env["IROHA_CHAIN_ID"] ?: "{chain_id}",
        accountId = env["IROHA_ACCOUNT_ID"],
        privateKey = env["IROHA_PRIVATE_KEY"],
    )
"#,
        api_base = ensure_http_base(&inputs.api_base),
        torii_url = ensure_http_base(&inputs.torii_url),
        chain_id = inputs.chain_id,
    )
}

fn render_ts_optional(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")),
        None => "undefined".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{
        BootstrapBundle, BootstrapInputs, BootstrapWriteError, ENV_LOCAL_FILE, KOTLIN_SAMPLE_FILE,
        RUST_SAMPLE_FILE, TYPESCRIPT_SAMPLE_FILE, ensure_http_base, shell_quote,
        write_bootstrap_bundle,
    };

    fn sample_inputs() -> BootstrapInputs {
        BootstrapInputs {
            api_base: "127.0.0.1:8080".to_owned(),
            torii_url: "http://127.0.0.1:8080".to_owned(),
            chain_id: "mochi-local".to_owned(),
            account_id: Some("alice@wonderland".to_owned()),
            private_key: Some("private key value".to_owned()),
        }
    }

    #[test]
    fn shell_quote_handles_spaces_and_single_quotes() {
        assert_eq!(shell_quote("mochi-local"), "mochi-local");
        assert_eq!(shell_quote("/tmp/mochi data"), "'/tmp/mochi data'");
        assert_eq!(shell_quote("alice's sandbox"), "'alice'\"'\"'s sandbox'");
    }

    #[test]
    fn ensure_http_base_adds_scheme_once() {
        assert_eq!(ensure_http_base("127.0.0.1:8080"), "http://127.0.0.1:8080");
        assert_eq!(
            ensure_http_base("http://127.0.0.1:8080/"),
            "http://127.0.0.1:8080"
        );
    }

    #[test]
    fn bootstrap_bundle_renders_expected_files() {
        let bundle = BootstrapBundle::render(&sample_inputs());
        let paths = bundle
            .artifacts
            .iter()
            .map(|artifact| artifact.relative_path.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                ENV_LOCAL_FILE.to_owned(),
                TYPESCRIPT_SAMPLE_FILE.to_owned(),
                RUST_SAMPLE_FILE.to_owned(),
                KOTLIN_SAMPLE_FILE.to_owned(),
            ]
        );
        assert!(
            bundle.artifacts[0]
                .contents
                .contains("IROHA_PRIVATE_KEY=\"private key value\"")
        );
    }

    #[test]
    fn write_bootstrap_bundle_creates_files() {
        let temp = TempDir::new().expect("temp dir");
        let bundle = BootstrapBundle::render(&sample_inputs());

        let written =
            write_bootstrap_bundle(temp.path(), &bundle, false).expect("bundle should write");

        assert_eq!(written.len(), 4);
        assert!(temp.path().join(ENV_LOCAL_FILE).exists());
        assert!(temp.path().join(TYPESCRIPT_SAMPLE_FILE).exists());
        assert!(temp.path().join(RUST_SAMPLE_FILE).exists());
        assert!(temp.path().join(KOTLIN_SAMPLE_FILE).exists());
    }

    #[test]
    fn write_bootstrap_bundle_rejects_existing_files_without_replace() {
        let temp = TempDir::new().expect("temp dir");
        let bundle = BootstrapBundle::render(&sample_inputs());
        write_bootstrap_bundle(temp.path(), &bundle, false).expect("first write");

        let err = write_bootstrap_bundle(temp.path(), &bundle, false).expect_err("should fail");
        assert!(matches!(err, BootstrapWriteError::AlreadyExists { .. }));
    }

    #[test]
    fn write_bootstrap_bundle_replaces_existing_files_when_requested() {
        let temp = TempDir::new().expect("temp dir");
        let bundle = BootstrapBundle::render(&sample_inputs());
        write_bootstrap_bundle(temp.path(), &bundle, false).expect("first write");

        let updated = BootstrapBundle::render(&BootstrapInputs {
            chain_id: "updated-chain".to_owned(),
            ..sample_inputs()
        });
        write_bootstrap_bundle(temp.path(), &updated, true).expect("second write");

        let contents =
            std::fs::read_to_string(temp.path().join(ENV_LOCAL_FILE)).expect("read env file");
        assert!(contents.contains("IROHA_CHAIN_ID=updated-chain"));
    }
}
