//! Profile-aware genesis verification entrypoint.

use std::{
    collections::HashSet,
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::Parser;
use color_eyre::eyre::{Result, WrapErr as _, eyre};
use iroha_data_model::{
    parameter::{
        custom::CustomParameterId,
        system::{SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameters},
    },
    prelude::PeerId,
};
use iroha_genesis::RawGenesisTransaction;

use crate::{
    Outcome, RunArgs,
    genesis::{
        GenesisProfile, ProfileDefaults, parse_vrf_seed_hex, profile_defaults,
        profile_requires_npos, resolve_vrf_seed,
    },
    tui,
};

/// Verify a genesis manifest against a known profile (chain id, DA/RBC, collectors, VRF seed, PoPs).
#[derive(Debug, Parser, Clone)]
pub struct Args {
    /// Profile to verify against (`iroha3-dev`, `iroha3-taira`, `iroha3-nexus`).
    #[clap(long, value_enum)]
    profile: GenesisProfile,
    /// Path to the genesis manifest (JSON).
    #[clap(long, value_name = "PATH")]
    genesis: PathBuf,
    /// Optional VRF seed (hex, 32 bytes). Required for NPoS taira/nexus manifests.
    #[clap(long, value_name = "HEX")]
    vrf_seed_hex: Option<String>,
}

#[derive(Debug)]
struct VerificationReport {
    chain_id: String,
    fingerprint: String,
    collectors_k: u16,
    collectors_r: u8,
    vrf_seed_hex: String,
    peer_count: usize,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        tui::status("Verifying genesis manifest against profile");
        let manifest =
            RawGenesisTransaction::from_path(&self.genesis).wrap_err("failed to load genesis")?;
        let vrf_seed_override = self
            .vrf_seed_hex
            .as_deref()
            .map(parse_vrf_seed_hex)
            .transpose()
            .wrap_err("invalid --vrf-seed-hex")?;

        let report =
            verify_manifest(&manifest, self.profile, vrf_seed_override).wrap_err_with(|| {
                format!(
                    "profile verification failed for {:?} using {}",
                    self.profile,
                    self.genesis.display()
                )
            })?;

        writeln!(writer, "profile: {:?}", self.profile)?;
        writeln!(writer, "chain_id: {}", report.chain_id)?;
        writeln!(
            writer,
            "collectors: k={} r={}",
            report.collectors_k, report.collectors_r
        )?;
        writeln!(writer, "vrf_seed: {}", report.vrf_seed_hex)?;
        writeln!(writer, "peers_with_pop: {}", report.peer_count)?;
        writeln!(writer, "consensus_fingerprint: {}", report.fingerprint)?;
        writeln!(writer, "kagami_version: {}", env!("CARGO_PKG_VERSION"))?;

        tui::success("Genesis manifest verified");
        Ok(())
    }
}

fn verify_manifest(
    manifest: &RawGenesisTransaction,
    profile: GenesisProfile,
    vrf_seed_override: Option<[u8; 32]>,
) -> Result<VerificationReport> {
    let defaults = profile_defaults(profile);
    ensure_chain_id(manifest, &defaults)?;

    let normalized = manifest.clone().with_consensus_meta();
    let params = normalized.effective_parameters();
    let sumeragi: SumeragiParameters = params.sumeragi().clone();

    let mode = enforce_mode(profile, &normalized)?;
    enforce_da_enabled(&sumeragi)?;
    enforce_collectors(&sumeragi, &defaults)?;
    enforce_gas_limit(&params)?;

    let next_mode = sumeragi.next_mode();
    let activation_height = sumeragi.mode_activation_height();
    if next_mode.is_some() || activation_height.is_some() {
        return Err(eyre!(
            "staged cutovers are not supported for Iroha3 profiles; remove next_mode/mode_activation_height"
        ));
    }
    let wants_npos = matches!(mode, SumeragiConsensusMode::Npos)
        || matches!(next_mode, Some(SumeragiConsensusMode::Npos));
    if !wants_npos && vrf_seed_override.is_some() {
        return Err(eyre!(
            "`--vrf-seed-hex` applies only to NPoS consensus manifests"
        ));
    }
    let seed_hex = if wants_npos {
        let npos_params = resolve_npos_params(&params)?;
        let expected_seed = resolve_vrf_seed(profile, manifest.chain_id(), vrf_seed_override)?;
        let seed_hex = hex::encode_upper(expected_seed);
        if npos_params.epoch_seed() != expected_seed {
            return Err(eyre!(
                "VRF seed mismatch: expected {} but manifest carries {}",
                seed_hex,
                hex::encode_upper(npos_params.epoch_seed())
            ));
        }
        seed_hex
    } else {
        "n/a".to_owned()
    };

    let peers_with_pops = collect_topology(manifest)?;
    let unique_peers: HashSet<_> = peers_with_pops.iter().collect();
    if unique_peers.len() < defaults.min_peers {
        return Err(eyre!(
            "profile {:?} requires at least {} topology entries with PoP (saw {})",
            profile,
            defaults.min_peers,
            unique_peers.len()
        ));
    }

    let fingerprint = normalized
        .consensus_fingerprint()
        .ok_or_else(|| eyre!("consensus fingerprint missing after normalization"))?
        .to_string();
    if let Some(raw_fp) = manifest.consensus_fingerprint()
        && raw_fp != fingerprint
    {
        return Err(eyre!(
            "consensus_fingerprint mismatch: manifest advertises {raw_fp} but recomputation yielded {fingerprint}"
        ));
    }

    Ok(VerificationReport {
        chain_id: manifest.chain_id().as_str().to_owned(),
        fingerprint,
        collectors_k: sumeragi.collectors_k(),
        collectors_r: sumeragi.collectors_redundant_send_r(),
        vrf_seed_hex: seed_hex,
        peer_count: unique_peers.len(),
    })
}

fn ensure_chain_id(manifest: &RawGenesisTransaction, defaults: &ProfileDefaults) -> Result<()> {
    if manifest.chain_id() != &defaults.chain_id {
        return Err(eyre!(
            "chain id mismatch: expected `{}`, found `{}`",
            defaults.chain_id,
            manifest.chain_id().as_str()
        ));
    }
    Ok(())
}

fn enforce_mode(
    profile: GenesisProfile,
    manifest: &RawGenesisTransaction,
) -> Result<SumeragiConsensusMode> {
    let mode = manifest.consensus_mode().ok_or_else(|| {
        eyre!("consensus_mode missing; call with_consensus_meta() during generation")
    })?;
    if profile_requires_npos(profile) && mode != SumeragiConsensusMode::Npos {
        return Err(eyre!(
            "profile {:?} targets the public dataspace; expected NPoS but manifest advertises {:?}",
            profile,
            mode
        ));
    }
    Ok(mode)
}

fn enforce_da_enabled(params: &SumeragiParameters) -> Result<()> {
    if !params.da_enabled() {
        return Err(eyre!(
            "manifest must enable DA (da_enabled={})",
            params.da_enabled()
        ));
    }
    Ok(())
}

fn enforce_collectors(params: &SumeragiParameters, defaults: &ProfileDefaults) -> Result<()> {
    if params.collectors_k() != defaults.collectors_k
        || params.collectors_redundant_send_r() != defaults.collectors_redundant_send_r
    {
        return Err(eyre!(
            "collector settings mismatch: expected k={} r={}, saw k={} r={}",
            defaults.collectors_k,
            defaults.collectors_redundant_send_r,
            params.collectors_k(),
            params.collectors_redundant_send_r()
        ));
    }
    Ok(())
}

fn enforce_gas_limit(params: &iroha_data_model::parameter::Parameters) -> Result<()> {
    let gas_param_id = CustomParameterId::new("ivm_gas_limit_per_block".parse()?);
    let Some(custom) = params.custom().get(&gas_param_id) else {
        return Err(eyre!(
            "`ivm_gas_limit_per_block` parameter missing; profiles pin it to 1_680_000"
        ));
    };
    let Some(limit) = custom.payload().try_into_any::<u64>().ok() else {
        return Err(eyre!(
            "`ivm_gas_limit_per_block` payload must be an integer (JSON u64)"
        ));
    };
    if limit != 1_680_000 {
        return Err(eyre!(
            "`ivm_gas_limit_per_block` mismatch: expected 1_680_000, saw {}",
            limit
        ));
    }
    Ok(())
}

fn resolve_npos_params(
    params: &iroha_data_model::parameter::Parameters,
) -> Result<SumeragiNposParameters> {
    let npos_param_id = SumeragiNposParameters::parameter_id();
    params
        .custom()
        .get(&npos_param_id)
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .ok_or_else(|| eyre!("missing `sumeragi_npos_parameters` in manifest"))
}

fn collect_topology(manifest: &RawGenesisTransaction) -> Result<Vec<PeerId>> {
    let mut peers_with_pop = Vec::new();
    for (tx_idx, tx) in manifest.transactions().iter().enumerate() {
        for entry in tx.topology() {
            let Some(_pop) = entry.pop_bytes().map_err(|err| {
                eyre!(
                    "transaction {tx_idx} has invalid `pop_hex` for peer {}: {err}",
                    entry.peer.public_key()
                )
            })?
            else {
                return Err(eyre!(
                    "transaction {tx_idx} missing `pop_hex` for peer {}",
                    entry.peer.public_key()
                ));
            };
            peers_with_pop.push(entry.peer.clone());
        }
    }
    Ok(peers_with_pop)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, bls_normal_pop_prove};
    use iroha_data_model::{
        parameter::system::SumeragiConsensusMode,
        prelude::{ChainId, PeerId, PublicKey},
    };
    use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry, RawGenesisTransaction};
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;
    use iroha_version::BuildLine;
    use tempfile::NamedTempFile;

    use super::*;
    use crate::genesis::profile::derive_vrf_seed_from_chain;

    fn build_manifest_with_profile(
        profile: GenesisProfile,
        consensus_mode: SumeragiConsensusMode,
        vrf_seed: [u8; 32],
        peers: &[(PublicKey, Vec<u8>)],
    ) -> RawGenesisTransaction {
        let defaults = profile_defaults(profile);
        let builder =
            GenesisBuilder::new_without_executor(defaults.chain_id.clone(), PathBuf::from("."));
        let manifest = crate::genesis::generate_default(
            builder,
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            consensus_mode,
            None,
            None,
            Some(&defaults),
            Some(vrf_seed),
            BuildLine::Iroha3,
        )
        .expect("generate profile manifest");

        manifest
            .into_builder()
            .next_transaction()
            .set_topology(
                peers
                    .iter()
                    .map(|(pk, pop)| {
                        GenesisTopologyEntry::new(PeerId::new(pk.clone()), pop.clone())
                    })
                    .collect(),
            )
            .build_raw()
    }

    fn generate_peer_pop() -> (PublicKey, Vec<u8>) {
        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let pop = bls_normal_pop_prove(kp.private_key()).expect("generate PoP");
        (kp.public_key().clone(), pop)
    }

    #[test]
    fn verify_accepts_dev_profile_manifest() {
        let seed = derive_vrf_seed_from_chain(&ChainId::from("iroha3-dev.local"));
        let peer = generate_peer_pop();
        let manifest = build_manifest_with_profile(
            GenesisProfile::Iroha3Dev,
            SumeragiConsensusMode::Npos,
            seed,
            std::slice::from_ref(&peer),
        );

        let report =
            verify_manifest(&manifest, GenesisProfile::Iroha3Dev, None).expect("verify manifest");
        assert_eq!(report.peer_count, 1);
        assert_eq!(report.vrf_seed_hex, hex::encode_upper(seed));
    }

    #[test]
    fn verify_allows_permissioned_dev_profile() {
        let seed = derive_vrf_seed_from_chain(&ChainId::from("iroha3-dev.local"));
        let peer = generate_peer_pop();
        let manifest = build_manifest_with_profile(
            GenesisProfile::Iroha3Dev,
            SumeragiConsensusMode::Permissioned,
            seed,
            std::slice::from_ref(&peer),
        );

        let report =
            verify_manifest(&manifest, GenesisProfile::Iroha3Dev, None).expect("verify manifest");
        assert_eq!(report.peer_count, 1);
        assert_eq!(report.vrf_seed_hex, "n/a");
    }

    #[test]
    fn verify_requires_seed_for_taira_profile() {
        let seed = [7u8; 32];
        let peers = (0..4).map(|_| generate_peer_pop()).collect::<Vec<_>>();
        let manifest = build_manifest_with_profile(
            GenesisProfile::Iroha3Taira,
            SumeragiConsensusMode::Npos,
            seed,
            &peers,
        );

        let err = verify_manifest(&manifest, GenesisProfile::Iroha3Taira, None)
            .expect_err("seed required");
        assert!(
            err.to_string().contains("vrf-seed-hex"),
            "unexpected error: {err}"
        );

        let ok = verify_manifest(&manifest, GenesisProfile::Iroha3Taira, Some(seed));
        assert!(
            ok.is_ok(),
            "explicit seed should satisfy verification: {ok:?}"
        );
    }

    #[test]
    fn verify_rejects_missing_pop() {
        let defaults = profile_defaults(GenesisProfile::Iroha3Dev);
        let seed = derive_vrf_seed_from_chain(&defaults.chain_id);
        let builder =
            GenesisBuilder::new_without_executor(defaults.chain_id.clone(), PathBuf::from("."));
        let manifest = crate::genesis::generate_default(
            builder,
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Npos,
            None,
            None,
            Some(&defaults),
            Some(seed),
            BuildLine::Iroha3,
        )
        .expect("generate profile manifest")
        .into_builder()
        .next_transaction()
        .set_topology(vec![GenesisTopologyEntry::from(PeerId::new(
            generate_peer_pop().0,
        ))])
        .build_raw();

        let err = verify_manifest(&manifest, GenesisProfile::Iroha3Dev, Some(seed))
            .expect_err("missing PoP should fail");
        assert!(
            err.to_string().contains("missing `pop_hex`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn verify_rejects_staged_cutover() {
        let defaults = profile_defaults(GenesisProfile::Iroha3Dev);
        let seed = derive_vrf_seed_from_chain(&defaults.chain_id);
        let peer = generate_peer_pop();
        let builder =
            GenesisBuilder::new_without_executor(defaults.chain_id.clone(), PathBuf::from("."));
        let manifest = crate::genesis::generate_default(
            builder,
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Npos,
            Some(SumeragiConsensusMode::Npos),
            Some(10),
            Some(&defaults),
            Some(seed),
            BuildLine::Iroha3,
        )
        .expect("generate staged manifest")
        .into_builder()
        .next_transaction()
        .set_topology(vec![GenesisTopologyEntry::new(PeerId::new(peer.0), peer.1)])
        .build_raw();

        let err = verify_manifest(&manifest, GenesisProfile::Iroha3Dev, Some(seed))
            .expect_err("staged cutover should be rejected");
        assert!(
            err.to_string()
                .contains("staged cutovers are not supported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn verify_command_outputs_report_for_dev_profile() {
        let seed = derive_vrf_seed_from_chain(&ChainId::from("iroha3-dev.local"));
        let peer = generate_peer_pop();
        let manifest = build_manifest_with_profile(
            GenesisProfile::Iroha3Dev,
            SumeragiConsensusMode::Npos,
            seed,
            std::slice::from_ref(&peer),
        );

        let mut file = NamedTempFile::new().expect("create temp file");
        let json = norito::json::to_json_pretty(&manifest).expect("serialize genesis");
        file.write_all(json.as_bytes()).expect("write genesis");

        let args = Args {
            profile: GenesisProfile::Iroha3Dev,
            genesis: file.path().to_path_buf(),
            vrf_seed_hex: None,
        };
        let mut writer = std::io::BufWriter::new(Vec::new());
        args.run(&mut writer)
            .expect("verify command should succeed");
        writer.flush().expect("flush buffer");
        let output =
            String::from_utf8(writer.into_inner().expect("collect output")).expect("utf8 output");

        assert!(
            output.contains("chain_id: iroha3-dev.local"),
            "report should include chain id: {output}"
        );
        assert!(
            output.contains("vrf_seed:"),
            "report should include VRF seed: {output}"
        );
    }
}
