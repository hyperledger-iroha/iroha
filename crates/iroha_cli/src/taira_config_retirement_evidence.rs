//! Host-independent validation of Linux validator configuration-retirement evidence.
//!
//! Receipts describe the remote Linux custody transition. Verification retains that exact
//! path and stamp contract on every client platform without performing host mutation.

use eyre::{Result, ensure, eyre};
use norito::json;

/// Maximum byte length of a validator configuration covered by a receipt.
pub(crate) const LIMIT: u64 = 1024 * 1024;
/// Schema prefix shared by the host mutation and its portable evidence verifier.
pub(crate) const PREFIX: &str = "taira.validator-config-retirement";
/// Ordered public Taira validator cohort used by the remote operation and its receipts.
pub(crate) const VALIDATOR_SLUGS: [&str; 4] = [
    "taira-validator-1",
    "taira-validator-2",
    "taira-validator-3",
    "taira-validator-4",
];

/// Read one required receipt field.
pub(crate) fn field<'a>(value: &'a json::Value, name: &str) -> Result<&'a json::Value> {
    value
        .get(name)
        .ok_or_else(|| eyre!("config retirement receipt field missing: {name}"))
}
/// Read one required receipt string without coercion.
pub(crate) fn text<'a>(value: &'a json::Value, name: &str) -> Result<&'a str> {
    field(value, name)?
        .as_str()
        .ok_or_else(|| eyre!("config retirement receipt text differs: {name}"))
}
/// Read the explicit ordered validator receipt array.
pub(crate) fn rows(value: &json::Value) -> Result<&Vec<json::Value>> {
    field(value, "rows")?
        .as_array()
        .ok_or_else(|| eyre!("config retirement rows differ"))
}
/// Require the complete expected field set, including absence of unknown fields.
pub(crate) fn exact_fields(value: &json::Value, names: &[&str]) -> Result<()> {
    let map = value
        .as_object()
        .ok_or_else(|| eyre!("config retirement object differs"))?;
    ensure!(
        map.len() == names.len() && names.iter().all(|name| map.contains_key(*name)),
        "config retirement fields differ"
    );
    Ok(())
}
/// Validate the bounded Linux original-file custody stamp recorded by the host.
pub(crate) fn stamp_value(value: &json::Value) -> Result<&Vec<json::Value>> {
    let row = value
        .as_array()
        .ok_or_else(|| eyre!("config retirement stamp differs"))?;
    ensure!(
        row.len() == 9 && row.iter().all(|v| v.as_u64().is_some()),
        "config retirement stamp differs"
    );
    ensure!(
        row[3].as_u64() == Some(0)
            && row[4].as_u64() == Some(0)
            && row[5].as_u64() == Some(1)
            && matches!(row[2].as_u64().unwrap() & 0o7777, 0o400 | 0o600)
            && (1..=LIMIT).contains(&row[6].as_u64().unwrap()),
        "config retirement stamp custody differs"
    );
    Ok(row)
}

/// Derive canonical remote Linux names without applying the verifier host's path rules.
pub(crate) fn config_paths(plan: &json::Value, role: &str) -> Result<(String, String, String)> {
    let deployment = field(plan, "deployment")?;
    let operation = text(plan, "operation")?;
    let release = text(deployment, "config_release")?;
    let filename = text(deployment, "config_filename")?;
    ensure!(
        text(deployment, "config_root")? == "/srv/taira"
            && release.len() == 40
            && release.bytes().all(|b| b.is_ascii_hexdigit())
            && matches!(filename, "config.toml" | "beacon.toml")
            && operation
                .strip_prefix("update-")
                .is_some_and(|x| x.len() == 32 && x.bytes().all(|b| b.is_ascii_hexdigit()))
            && VALIDATOR_SLUGS.contains(&role),
        "config retirement retained path differs"
    );
    // These are paths on the Linux validator host, even when evidence is verified on
    // Windows. Native PathBuf operations would change their canonical separators.
    let parent = format!("/srv/taira/{role}/releases/{release}/config");
    let path = format!("{parent}/{filename}");
    let next = format!("{parent}/.{filename}.{operation}.retirement-next");
    let original = format!("{parent}/.{filename}.{operation}.retirement-original");
    Ok((path, next, original))
}

/// Authenticate one operation receipt against its retained plan and ordered cohort.
pub(crate) fn validate_receipt(
    receipt: &json::Value,
    plan: &json::Value,
    kind: &str,
) -> Result<()> {
    exact_fields(
        receipt,
        &["schema", "operation", "source_commit", "network_id", "rows"],
    )?;
    ensure!(
        text(receipt, "schema")? == format!("{PREFIX}.{kind}.v1")
            && text(receipt, "operation")? == text(plan, "operation")?
            && text(receipt, "source_commit")? == text(plan, "commit")?
            && text(receipt, "network_id")? == text(plan, "network_id")?,
        "config retirement receipt binding differs"
    );
    let values = rows(receipt)?;
    ensure!(values.len() == 4, "config retirement cohort differs");
    for (row, role) in values.iter().zip(VALIDATOR_SLUGS) {
        let mut names = vec![
            "role",
            "source_path",
            "staged_path",
            "original_path",
            "changed",
            "source_sha256",
            "output_sha256",
            "before_stamp",
            "staged_stamp",
        ];
        if kind == "installed" {
            names.push("installed_stamp");
        }
        if kind == "restored" {
            names.push("restored_stamp");
        }
        exact_fields(row, &names)?;
        let (source, staged, original) = config_paths(plan, role)?;
        ensure!(
            text(row, "role")? == role
                && text(row, "source_path")? == source
                && text(row, "staged_path")? == staged
                && text(row, "original_path")? == original,
            "config retirement role or paths differ"
        );
        for key in ["source_sha256", "output_sha256"] {
            let digest = text(row, key)?;
            ensure!(
                digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
                "config retirement digest differs"
            );
        }
        let changed = field(row, "changed")?
            .as_bool()
            .ok_or_else(|| eyre!("config retirement change flag differs"))?;
        ensure!(
            changed == (text(row, "source_sha256")? != text(row, "output_sha256")?),
            "config retirement change digest differs"
        );
        stamp_value(field(row, "before_stamp")?)?;
        stamp_value(field(row, "staged_stamp")?)?;
        if kind == "installed" {
            let installed = field(row, "installed_stamp")?;
            stamp_value(installed)?;
            if !changed {
                ensure!(
                    installed == field(row, "before_stamp")?,
                    "no-op retirement changed config custody"
                );
            } else {
                ensure!(
                    same_renamed_file(installed, field(row, "staged_stamp")?)?,
                    "installed config is not the prepared sibling"
                );
            }
        }
        if kind == "restored" {
            stamp_value(field(row, "restored_stamp")?)?;
        }
    }
    Ok(())
}

/// Compare original Linux file stamps, permitting only rename's ctime transition.
pub(crate) fn same_renamed_file(a: &json::Value, b: &json::Value) -> Result<bool> {
    let a = stamp_value(a)?;
    let b = stamp_value(b)?;
    // Rename changes ctime, but neither inode nor data/permissions/mtime.
    Ok(a[..8] == b[..8])
}

/// Join the only supported config metadata transition without changing peer trust.
pub(crate) fn validate_retirement_records(
    prepared: &json::Value,
    installed: &json::Value,
    plan: &json::Value,
) -> Result<()> {
    validate_receipt(prepared, plan, "prepared")?;
    validate_receipt(installed, plan, "installed")?;
    for (old, new) in rows(prepared)?.iter().zip(rows(installed)?) {
        let mut projected = new.clone();
        projected.as_object_mut().unwrap().remove("installed_stamp");
        ensure!(
            &projected == old,
            "installed config receipt changed its preparation"
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "taira_config_retirement_evidence_tests.rs"]
mod tests;
