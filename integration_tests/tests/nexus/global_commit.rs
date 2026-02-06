#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Regression tests for Nexus lane commitment fixtures.

use std::{fs, path::PathBuf};

use eyre::{Result, WrapErr, ensure};
use iroha_data_model::block::consensus::LaneBlockCommitment;
use norito::{core::NoritoDeserialize as _, json};

struct CommitmentFixture {
    name: String,
    json_path: PathBuf,
    to_path: PathBuf,
    commitment: LaneBlockCommitment,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn lane_commitment_dir() -> PathBuf {
    repo_root().join("fixtures/nexus/lane_commitments")
}

fn load_lane_commitments() -> Result<Vec<CommitmentFixture>> {
    let dir = lane_commitment_dir();
    let mut fixtures = Vec::new();
    for entry in fs::read_dir(&dir).wrap_err_with(|| format!("read {}", dir.display()))? {
        let entry = entry.wrap_err("read fixture entry")?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let file_stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| eyre::eyre!("invalid fixture name {:?}", path))?;
        let raw = fs::read_to_string(&path)
            .wrap_err_with(|| format!("read lane commitment JSON {}", path.display()))?;
        let commitment: LaneBlockCommitment = json::from_str(&raw)
            .wrap_err_with(|| format!("decode lane commitment JSON fixture {}", path.display()))?;
        let to_path = dir.join(format!("{file_stem}.to"));
        fixtures.push(CommitmentFixture {
            name: file_stem.to_string(),
            json_path: path.clone(),
            to_path,
            commitment,
        });
    }
    fixtures.sort_by(|a, b| a.name.cmp(&b.name));
    ensure!(
        !fixtures.is_empty(),
        "no lane commitment fixtures found under {}",
        dir.display()
    );
    Ok(fixtures)
}

#[test]
fn lane_commitment_json_matches_norito_payloads() -> Result<()> {
    for fixture in load_lane_commitments()? {
        let bytes = fs::read(&fixture.to_path).wrap_err_with(|| {
            format!(
                "read lane commitment Norito payload for {}",
                fixture.to_path.display()
            )
        })?;
        let decoded = norito::from_bytes::<LaneBlockCommitment>(&bytes)
            .and_then(LaneBlockCommitment::try_deserialize)
            .map_err(|err| {
                eyre::eyre!(
                    "failed to decode {} for fixture {}: {err}",
                    fixture.to_path.display(),
                    fixture.name
                )
            })?;
        assert_eq!(
            decoded,
            fixture.commitment,
            "Norito payload mismatch for fixture {} ({})",
            fixture.name,
            fixture.json_path.display()
        );
    }
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)] // fixture validation requires exhaustive assertions
fn lane_commitment_receipt_totals_are_consistent() -> Result<()> {
    for fixture in load_lane_commitments()? {
        let sum_local: u128 = fixture
            .commitment
            .receipts
            .iter()
            .map(|receipt| receipt.local_amount_micro)
            .sum();
        ensure!(
            sum_local == fixture.commitment.total_local_micro,
            "local totals mismatch for {} (expected {}, got {})",
            fixture.name,
            fixture.commitment.total_local_micro,
            sum_local
        );

        let sum_due: u128 = fixture
            .commitment
            .receipts
            .iter()
            .map(|receipt| receipt.xor_due_micro)
            .sum();
        ensure!(
            sum_due == fixture.commitment.total_xor_due_micro,
            "xor_due totals mismatch for {} (expected {}, got {})",
            fixture.name,
            fixture.commitment.total_xor_due_micro,
            sum_due
        );

        let sum_after_haircut: u128 = fixture
            .commitment
            .receipts
            .iter()
            .map(|receipt| receipt.xor_after_haircut_micro)
            .sum();
        ensure!(
            sum_after_haircut == fixture.commitment.total_xor_after_haircut_micro,
            "xor_after_haircut totals mismatch for {} (expected {}, got {})",
            fixture.name,
            fixture.commitment.total_xor_after_haircut_micro,
            sum_after_haircut
        );

        ensure!(
            fixture.commitment.total_xor_due_micro
                >= fixture.commitment.total_xor_after_haircut_micro,
            "variance underflow for {}: due {} < after haircut {}",
            fixture.name,
            fixture.commitment.total_xor_due_micro,
            fixture.commitment.total_xor_after_haircut_micro
        );
        let expected_variance = fixture.commitment.total_xor_due_micro
            - fixture.commitment.total_xor_after_haircut_micro;
        ensure!(
            expected_variance == fixture.commitment.total_xor_variance_micro,
            "variance mismatch for {} (expected {}, got {})",
            fixture.name,
            fixture.commitment.total_xor_variance_micro,
            expected_variance
        );
        let summed_variance: u128 = fixture
            .commitment
            .receipts
            .iter()
            .map(|receipt| receipt.xor_variance_micro)
            .sum();
        ensure!(
            summed_variance == fixture.commitment.total_xor_variance_micro,
            "per-receipt variance mismatch for {} (expected {}, got {})",
            fixture.name,
            fixture.commitment.total_xor_variance_micro,
            summed_variance
        );

        let receipt_count = u64::try_from(fixture.commitment.receipts.len())
            .expect("receipt vector length fits in u64");
        ensure!(
            fixture.commitment.tx_count == receipt_count,
            "tx_count mismatch for {} (expected {}, got {})",
            fixture.name,
            fixture.commitment.tx_count,
            receipt_count
        );

        if let Some(metadata) = &fixture.commitment.swap_metadata {
            ensure!(
                metadata.epsilon_bps > 0,
                "epsilon must be > 0 for {}",
                fixture.name
            );
            ensure!(
                metadata.twap_window_seconds > 0,
                "twap window must be > 0 for {}",
                fixture.name
            );
            let twap = metadata.twap_local_per_xor.trim();
            ensure!(
                !twap.is_empty(),
                "twap_local_per_xor must be populated for {}",
                fixture.name
            );
            let parsed_twap: f64 = twap.parse().wrap_err_with(|| {
                format!(
                    "parse twap_local_per_xor '{}' for {}",
                    metadata.twap_local_per_xor, fixture.name
                )
            })?;
            ensure!(
                parsed_twap.is_finite() && parsed_twap > 0.0,
                "twap_local_per_xor must be positive for {}",
                fixture.name
            );
        }

        for (idx, receipt) in fixture.commitment.receipts.iter().enumerate() {
            ensure!(
                receipt.xor_due_micro >= receipt.xor_after_haircut_micro,
                "receipt {} in {} has xor_due_micro < xor_after_haircut_micro",
                idx,
                fixture.name
            );
            ensure!(
                receipt.xor_variance_micro
                    == receipt.xor_due_micro - receipt.xor_after_haircut_micro,
                "receipt {} in {} has inconsistent variance field",
                idx,
                fixture.name
            );
            if idx > 0 {
                let prev = &fixture.commitment.receipts[idx - 1];
                ensure!(
                    receipt.timestamp_ms >= prev.timestamp_ms,
                    "receipt timestamps must be non-decreasing in {}",
                    fixture.name
                );
            }
        }
    }
    Ok(())
}
