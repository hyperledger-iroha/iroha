//! Build the fail-closed v1 contract-subject migration manifest from an exhaustive finalized
//! smart-contract event export.

use std::{collections::BTreeSet, fs, path::PathBuf};

use clap::Parser;
use eyre::{Result, WrapErr as _, bail};
use iroha_core::smartcontracts::code::LegacyContractSubjectMigrationManifest;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{ChainId, block::BlockHeader, smart_contract::ContractAddress};
use norito::derive::{JsonDeserialize, JsonSerialize};

#[derive(Parser, Debug)]
struct Args {
    /// Exhaustive finalized block/event export to audit.
    #[arg(long)]
    input: PathBuf,
    /// Destination for the canonical migration manifest.
    #[arg(long)]
    output: PathBuf,
}

#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
struct FinalizedContractEventExport {
    schema_version: u8,
    chain_id: ChainId,
    blocks: Vec<FinalizedBlockContractEvents>,
}

#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
struct FinalizedBlockContractEvents {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    finalized: bool,
    contract_events_complete: bool,
    activations: Vec<ContractAddress>,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize)]
struct AuditReceipt {
    source_export_hash: Hash,
    audited_through_height: u64,
    activation_event_count: u64,
    unique_contract_address_count: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let source = fs::read(&args.input)
        .wrap_err_with(|| format!("failed to read `{}`", args.input.display()))?;
    let (manifest, receipt) = audit_source(&source)?;
    let encoded = encode_manifest(&manifest)?;
    fs::write(&args.output, encoded.as_bytes())
        .wrap_err_with(|| format!("failed to write `{}`", args.output.display()))?;

    println!("{}", norito::json::to_json(&receipt)?);
    Ok(())
}

fn audit_source(source: &[u8]) -> Result<(LegacyContractSubjectMigrationManifest, AuditReceipt)> {
    let source_text = core::str::from_utf8(source).wrap_err("event export is not UTF-8")?;
    let export: FinalizedContractEventExport =
        norito::json::from_json(source_text).wrap_err("invalid finalized event export")?;
    audit_export(export, Hash::new(source))
}

fn audit_export(
    export: FinalizedContractEventExport,
    source_export_hash: Hash,
) -> Result<(LegacyContractSubjectMigrationManifest, AuditReceipt)> {
    if export.schema_version != 1 {
        bail!(
            "unsupported finalized event export schema {}",
            export.schema_version
        );
    }

    let mut expected_height = 1_u64;
    let mut historical = BTreeSet::new();
    let mut activation_event_count = 0_u64;
    for block in &export.blocks {
        if block.height != expected_height {
            bail!(
                "event export coverage gap: expected height {expected_height}, found {}",
                block.height
            );
        }
        if !block.finalized {
            bail!("block {} is not attested finalized", block.height);
        }
        if !block.contract_events_complete {
            bail!(
                "block {} does not attest a complete contract-event stream",
                block.height
            );
        }
        activation_event_count = activation_event_count
            .checked_add(u64::try_from(block.activations.len())?)
            .ok_or_else(|| eyre::eyre!("activation event count overflow"))?;
        historical.extend(block.activations.iter().cloned());
        expected_height = expected_height
            .checked_add(1)
            .ok_or_else(|| eyre::eyre!("block height overflow"))?;
    }

    let audited_through_height = export.blocks.last().map_or(0, |block| block.height);
    let audited_tip_hash = export.blocks.last().map(|block| block.block_hash);
    let historical_contract_addresses: Vec<_> = historical.into_iter().collect();
    let manifest = LegacyContractSubjectMigrationManifest {
        schema_version: 1,
        chain_id: export.chain_id,
        audited_through_height,
        audited_tip_hash,
        complete_finalized_contract_event_export: true,
        source_export_hash,
        activation_event_count,
        historical_contract_addresses,
    };

    let receipt = AuditReceipt {
        source_export_hash,
        audited_through_height,
        activation_event_count,
        unique_contract_address_count: u64::try_from(manifest.historical_contract_addresses.len())?,
    };
    Ok((manifest, receipt))
}

fn encode_manifest(manifest: &LegacyContractSubjectMigrationManifest) -> Result<String> {
    let mut encoded = norito::json::to_json(manifest).wrap_err("encode migration manifest")?;
    encoded.push('\n');
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT_A: &str = "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7";
    const CONTRACT_B: &str = "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8";

    fn address(literal: &str) -> ContractAddress {
        literal.parse().expect("valid contract address fixture")
    }

    fn block(height: u64, seed: u8) -> FinalizedBlockContractEvents {
        FinalizedBlockContractEvents {
            height,
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([seed; Hash::LENGTH])),
            finalized: true,
            contract_events_complete: true,
            activations: Vec::new(),
        }
    }

    fn export(blocks: Vec<FinalizedBlockContractEvents>) -> FinalizedContractEventExport {
        FinalizedContractEventExport {
            schema_version: 1,
            chain_id: ChainId::from("migration-audit-test"),
            blocks,
        }
    }

    fn audit_error(export: FinalizedContractEventExport) -> String {
        audit_export(export, Hash::new(b"test-export"))
            .expect_err("export must fail closed")
            .to_string()
    }

    #[test]
    fn malformed_event_export_is_rejected() {
        let malformed = audit_source(b"{").expect_err("malformed JSON must be rejected");
        assert!(
            malformed
                .to_string()
                .contains("invalid finalized event export")
        );

        let non_utf8 = audit_source(&[0xff]).expect_err("non-UTF-8 input must be rejected");
        assert!(non_utf8.to_string().contains("event export is not UTF-8"));
    }

    #[test]
    fn non_finalized_block_is_rejected() {
        let mut candidate = block(1, 1);
        candidate.finalized = false;
        assert_eq!(
            audit_error(export(vec![candidate])),
            "block 1 is not attested finalized"
        );
    }

    #[test]
    fn coverage_gap_is_rejected() {
        assert_eq!(
            audit_error(export(vec![block(1, 1), block(3, 3)])),
            "event export coverage gap: expected height 2, found 3"
        );
    }

    #[test]
    fn incomplete_contract_event_stream_is_rejected() {
        let mut candidate = block(1, 1);
        candidate.contract_events_complete = false;
        assert_eq!(
            audit_error(export(vec![candidate])),
            "block 1 does not attest a complete contract-event stream"
        );
    }

    #[test]
    fn manifest_and_receipt_are_deterministic_sorted_and_deduplicated() {
        let contract_a = address(CONTRACT_A);
        let contract_b = address(CONTRACT_B);
        let mut first_block = block(1, 1);
        first_block.activations = vec![contract_b.clone(), contract_a.clone(), contract_b.clone()];
        let mut second_block = block(2, 2);
        second_block.activations = vec![contract_a.clone()];
        let source = norito::json::to_json(&export(vec![first_block, second_block]))
            .expect("encode event export fixture");

        let first = audit_source(source.as_bytes()).expect("audit valid event export");
        let second = audit_source(source.as_bytes()).expect("repeat identical audit");
        assert_eq!(first, second);
        assert_eq!(
            norito::json::to_json(&first.1).expect("encode receipt"),
            norito::json::to_json(&second.1).expect("repeat receipt encoding")
        );

        let (manifest, receipt) = first;
        let mut expected_addresses = vec![contract_a, contract_b];
        expected_addresses.sort();
        assert_eq!(manifest.historical_contract_addresses, expected_addresses);
        assert_eq!(manifest.activation_event_count, 4);
        assert_eq!(manifest.audited_through_height, 2);
        assert_eq!(manifest.audited_tip_hash, Some(block(2, 2).block_hash));
        assert!(manifest.complete_finalized_contract_event_export);
        assert_eq!(manifest.source_export_hash, Hash::new(source.as_bytes()));
        assert_eq!(receipt.source_export_hash, manifest.source_export_hash);
        assert_eq!(receipt.audited_through_height, 2);
        assert_eq!(receipt.activation_event_count, 4);
        assert_eq!(receipt.unique_contract_address_count, 2);

        let first_encoding = encode_manifest(&manifest).expect("encode manifest");
        let second_encoding = encode_manifest(&manifest).expect("repeat manifest encoding");
        assert_eq!(first_encoding, second_encoding);
        assert!(first_encoding.ends_with('\n'));
    }
}
