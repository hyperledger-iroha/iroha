//! Helpers for rendering and writing local application bootstrap files.
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write as _},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
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
#[derive(Clone, PartialEq, Eq)]
pub struct BootstrapInputs {
    /// Base HTTP address used by explorer-style API requests.
    pub api_base: String,
    /// Torii base URL used for transaction/query submissions.
    pub torii_url: String,
    /// Native MCP endpoint exposed by the local Torii node.
    pub mcp_url: Option<String>,
    /// Chain identifier exposed by the local network.
    pub chain_id: String,
    /// Optional account identifier for the preferred dev signer.
    pub account_id: Option<String>,
    /// Optional private key for the preferred dev signer.
    pub private_key: Option<String>,
}
impl std::fmt::Debug for BootstrapInputs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BootstrapInputs")
            .field("api_base", &self.api_base)
            .field("torii_url", &self.torii_url)
            .field("mcp_url", &self.mcp_url)
            .field("chain_id", &self.chain_id)
            .field("account_id", &self.account_id)
            .field(
                "private_key",
                &self.private_key.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
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
        if let Some(mcp_url) = self.mcp_url.as_deref() {
            lines.push(format!(
                "export IROHA_MCP_URL={}",
                shell_quote(&ensure_http_base(mcp_url))
            ));
        }
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
        if let Some(mcp_url) = self.mcp_url.as_deref() {
            lines.push("IROHA_MCP_URL=".to_owned() + &dotenv_quote(&ensure_http_base(mcp_url)));
        }
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
    /// An artifact path was absolute or attempted to leave the workspace root.
    #[error("bootstrap artifact path must be a non-empty relative path: {path}")]
    InvalidPath { path: PathBuf },
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
    let mut destinations = Vec::with_capacity(bundle.artifacts.len());
    for artifact in &bundle.artifacts {
        if artifact.relative_path.as_os_str().is_empty()
            || !artifact
                .relative_path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(BootstrapWriteError::InvalidPath {
                path: artifact.relative_path.clone(),
            });
        }
        let destination = artifact.path_in(workspace_root);
        if !replace_existing {
            match fs::symlink_metadata(&destination) {
                Ok(_) => return Err(BootstrapWriteError::AlreadyExists { path: destination }),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        destinations.push(destination);
    }

    let mut written = Vec::with_capacity(bundle.artifacts.len());
    for (artifact, destination) in bundle.artifacts.iter().zip(destinations) {
        let parent = ensure_artifact_parent(workspace_root, &artifact.relative_path)?;
        let private = artifact.relative_path == Path::new(ENV_LOCAL_FILE);
        let (tmp_path, mut file) = create_artifact_temp(&parent, &destination, private)?;
        let write_result = (|| -> io::Result<()> {
            file.write_all(artifact.contents.as_bytes())?;
            file.sync_all()
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&tmp_path);
            return Err(error.into());
        }
        let publish_result = if replace_existing {
            fs::rename(&tmp_path, &destination)
        } else {
            fs::hard_link(&tmp_path, &destination)
        };
        if let Err(error) = publish_result {
            let _ = fs::remove_file(&tmp_path);
            if !replace_existing && error.kind() == io::ErrorKind::AlreadyExists {
                return Err(BootstrapWriteError::AlreadyExists { path: destination });
            }
            return Err(error.into());
        }
        if !replace_existing {
            fs::remove_file(&tmp_path)?;
        }
        #[cfg(unix)]
        File::open(&parent)?.sync_all()?;
        written.push(destination);
    }
    Ok(written)
}
fn ensure_artifact_parent(workspace_root: &Path, relative_path: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(workspace_root)?;
    let mut current = workspace_root.to_path_buf();
    let parent = relative_path.parent().unwrap_or_else(|| Path::new(""));
    for component in parent.components() {
        let Component::Normal(name) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bootstrap artifact parent must stay under the workspace root",
            ));
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "bootstrap artifact parent `{}` must be a real directory",
                        current.display()
                    ),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(current)
}
fn create_artifact_temp(
    parent: &Path,
    destination: &Path,
    private: bool,
) -> io::Result<(PathBuf, File)> {
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("bootstrap");
    for _ in 0..32 {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let tmp_path = parent.join(format!(
            ".{file_name}.mochi-tmp.{}.{id}",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(if private { 0o600 } else { 0o644 });
        #[cfg(not(unix))]
        let _ = private;
        match options.open(&tmp_path) {
            Ok(file) => return Ok((tmp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique bootstrap temporary file",
    ))
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
  mcpUrl?: string;
  chainId: string;
  accountId?: string;
  privateKey?: string;
}};

export const irohaLocalDefaults: IrohaLocalConfig = {{
  apiBase: process.env.IROHA_API_BASE ?? "{api_base}",
  toriiUrl: process.env.IROHA_TORII_URL ?? "{torii_url}",
  mcpUrl: process.env.IROHA_MCP_URL ?? {mcp_url},
  chainId: process.env.IROHA_CHAIN_ID ?? "{chain_id}",
  accountId: process.env.IROHA_ACCOUNT_ID ?? {account_id},
  privateKey: process.env.IROHA_PRIVATE_KEY,
}};
"#,
        api_base = ensure_http_base(&inputs.api_base),
        torii_url = ensure_http_base(&inputs.torii_url),
        mcp_url = render_ts_optional(inputs.mcp_url.as_deref().map(ensure_http_base).as_deref(),),
        chain_id = inputs.chain_id,
        account_id = render_ts_optional(inputs.account_id.as_deref()),
    )
}
fn render_rust_sample(inputs: &BootstrapInputs) -> String {
    format!(
        r#"#[derive(Debug, Clone)]
pub struct IrohaLocalConfig {{
    pub api_base: String,
    pub torii_url: String,
    pub mcp_url: Option<String>,
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
            mcp_url: std::env::var("IROHA_MCP_URL").ok().or_else(|| {mcp_url}),
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
        mcp_url = render_rust_optional(inputs.mcp_url.as_deref().map(ensure_http_base).as_deref(),),
        chain_id = inputs.chain_id,
    )
}
fn render_kotlin_sample(inputs: &BootstrapInputs) -> String {
    format!(
        r#"data class IrohaLocalConfig(
    val apiBase: String,
    val toriiUrl: String,
    val mcpUrl: String?,
    val chainId: String,
    val accountId: String?,
    val privateKey: String?
)

fun irohaLocalConfig(env: Map<String, String> = System.getenv()): IrohaLocalConfig =
    IrohaLocalConfig(
        apiBase = env["IROHA_API_BASE"] ?: "{api_base}",
        toriiUrl = env["IROHA_TORII_URL"] ?: "{torii_url}",
        mcpUrl = env["IROHA_MCP_URL"] ?: {mcp_url},
        chainId = env["IROHA_CHAIN_ID"] ?: "{chain_id}",
        accountId = env["IROHA_ACCOUNT_ID"],
        privateKey = env["IROHA_PRIVATE_KEY"],
    )
"#,
        api_base = ensure_http_base(&inputs.api_base),
        torii_url = ensure_http_base(&inputs.torii_url),
        mcp_url =
            render_kotlin_optional(inputs.mcp_url.as_deref().map(ensure_http_base).as_deref(),),
        chain_id = inputs.chain_id,
    )
}
fn render_rust_optional(value: Option<&str>) -> String {
    match value {
        Some(value) => format!(
            "Some(\"{}\".to_owned())",
            value.replace('\\', "\\\\").replace('"', "\\\"")
        ),
        None => "None".to_owned(),
    }
}
fn render_kotlin_optional(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")),
        None => "null".to_owned(),
    }
}
fn render_ts_optional(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")),
        None => "undefined".to_owned(),
    }
}
#[cfg(test)]
mod tests {
    use super::{
        BootstrapBundle, BootstrapInputs, BootstrapWriteError, ENV_LOCAL_FILE, KOTLIN_SAMPLE_FILE,
        RUST_SAMPLE_FILE, TYPESCRIPT_SAMPLE_FILE, ensure_http_base, shell_quote,
        write_bootstrap_bundle,
    };
    use tempfile::TempDir;
    fn sample_inputs() -> BootstrapInputs {
        BootstrapInputs {
            api_base: "127.0.0.1:8080".to_owned(),
            torii_url: "http://127.0.0.1:8080".to_owned(),
            mcp_url: Some("http://127.0.0.1:8080/v1/mcp".to_owned()),
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
        let inputs = sample_inputs();
        let debug = format!("{inputs:?}");
        assert!(!debug.contains("private key value"));
        assert!(debug.contains("[REDACTED]"));
        let bundle = BootstrapBundle::render(&inputs);
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
        assert!(
            bundle.artifacts[0]
                .contents
                .contains("IROHA_MCP_URL=http://127.0.0.1:8080/v1/mcp")
        );
        let typescript = &bundle.artifacts[1].contents;
        assert!(typescript.contains("privateKey: process.env.IROHA_PRIVATE_KEY"));
        assert!(
            !typescript.contains("private key value"),
            "generated source must not embed signer secrets"
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            let mode = fs::metadata(temp.path().join(ENV_LOCAL_FILE))
                .expect("env metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode & 0o077, 0, ".env.local must be owner-only");
        }
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
    #[test]
    fn write_bootstrap_bundle_rejects_paths_outside_workspace() {
        let temp = TempDir::new().expect("temp dir");
        let bundle = BootstrapBundle {
            artifacts: vec![super::BootstrapArtifact {
                relative_path: PathBuf::from("../outside"),
                contents: "nope".to_owned(),
            }],
        };
        let error = write_bootstrap_bundle(temp.path(), &bundle, false)
            .expect_err("parent traversal must be rejected");
        assert!(matches!(error, BootstrapWriteError::InvalidPath { .. }));
    }
    #[cfg(unix)]
    #[test]
    fn write_bootstrap_bundle_rejects_symlinked_child_directory() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temp dir");
        let outside = TempDir::new().expect("outside temp dir");
        let mochi_dir = temp.path().join(".mochi");
        fs::create_dir(&mochi_dir).expect("create .mochi");
        symlink(outside.path(), mochi_dir.join("generated")).expect("link generated directory");
        let error = write_bootstrap_bundle(temp.path(), &BootstrapBundle::render(&sample_inputs()), false)
            .expect_err("symlinked artifact parent must be rejected");
        assert!(matches!(error, BootstrapWriteError::Io(_)));
        assert!(!outside.path().join("typescript/connect.ts").exists());
    }
}
