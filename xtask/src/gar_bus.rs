//! File-based “bus” for distributing GAR CDN policy payloads to PoPs.

use std::{
    fmt::Write as FmtWrite,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use eyre::{Result, WrapErr};
use iroha_data_model::sorafs::gar::GarCdnPolicyV1;
use norito::json::{self, Map, Number, Value};

/// Options controlling GAR CDN policy publication.
#[derive(Debug, Clone)]
pub struct GarBusOptions {
    pub policy_path: PathBuf,
    pub output_dir: PathBuf,
    pub pop: Option<String>,
}

/// Load a GAR CDN policy document and emit a bus event bundle.
pub fn publish_policy(options: GarBusOptions) -> Result<Value> {
    let bytes = fs::read(&options.policy_path).wrap_err_with(|| {
        format!(
            "failed to read CDN policy `{}`",
            options.policy_path.display()
        )
    })?;
    let policy: GarCdnPolicyV1 = json::from_slice(&bytes).wrap_err_with(|| {
        format!(
            "failed to parse CDN policy `{}`",
            options.policy_path.display()
        )
    })?;

    fs::create_dir_all(&options.output_dir).wrap_err_with(|| {
        format!(
            "failed to create output directory `{}`",
            options.output_dir.display()
        )
    })?;

    let policy_value: Value = json::from_slice(&json::to_vec(&policy)?)
        .wrap_err("failed to convert CDN policy to JSON value")?;

    let mut root = Map::new();
    root.insert(
        "kind".into(),
        Value::String("soranet_gar_cdn_policy".to_string()),
    );
    if let Some(pop) = &options.pop {
        root.insert("pop".into(), Value::String(pop.clone()));
    }
    root.insert(
        "policy_path".into(),
        Value::String(options.policy_path.display().to_string()),
    );
    let published_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs();
    root.insert(
        "published_at_unix".into(),
        Value::Number(Number::from(published_at)),
    );
    root.insert("policy".into(), policy_value);

    let value = Value::Object(root);
    let json_path = options.output_dir.join("gar_cdn_policy_event.json");
    fs::write(&json_path, json::to_string_pretty(&value)?)
        .wrap_err_with(|| format!("failed to write `{}`", json_path.display()))?;

    let markdown = render_markdown(&value);
    let markdown_path = options.output_dir.join("gar_cdn_policy_event.md");
    fs::write(&markdown_path, markdown)
        .wrap_err_with(|| format!("failed to write `{}`", markdown_path.display()))?;

    Ok(value)
}

fn render_markdown(value: &Value) -> String {
    let mut out = String::new();
    if let Some(pop) = value.get("pop").and_then(Value::as_str) {
        let _ = writeln!(&mut out, "# GAR CDN Policy ({pop})");
    } else {
        let _ = writeln!(&mut out, "# GAR CDN Policy");
    }
    if let Some(ts) = value.get("published_at_unix").and_then(Value::as_u64) {
        let _ = writeln!(&mut out, "- published_at_unix: {ts}");
    }
    if let Some(path) = value.get("policy_path").and_then(Value::as_str) {
        let _ = writeln!(&mut out, "- policy_path: {path}");
    }
    if let Some(policy) = value.get("policy") {
        let policy_str = json::to_string_pretty(policy)
            .unwrap_or_else(|_| "unable to render policy".to_string());
        let _ = writeln!(&mut out, "\n## Policy\n```\n{policy_str}\n```");
    }
    out
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn publishes_policy_bundle() -> Result<()> {
        let temp = TempDir::new()?;
        let policy_path = temp.path().join("policy.json");
        let policy = GarCdnPolicyV1 {
            ttl_override_secs: Some(30),
            purge_tags: vec!["hotfix".to_string()],
            moderation_slugs: vec!["global-block".to_string()],
            rate_ceiling_rps: Some(25),
            allow_regions: vec!["EU".to_string()],
            deny_regions: vec!["US".to_string()],
            legal_hold: false,
        };
        let bytes = json::to_vec_pretty(&policy)?;
        fs::write(&policy_path, bytes)?;

        let options = GarBusOptions {
            policy_path: policy_path.clone(),
            output_dir: temp.path().join("out"),
            pop: Some("soranet-pop-m1".to_string()),
        };
        let value = publish_policy(options.clone())?;
        assert_eq!(
            value
                .get("policy_path")
                .and_then(Value::as_str)
                .map(PathBuf::from),
            Some(policy_path)
        );
        assert!(
            options
                .output_dir
                .join("gar_cdn_policy_event.json")
                .exists()
        );
        assert!(options.output_dir.join("gar_cdn_policy_event.md").exists());
        Ok(())
    }
}
