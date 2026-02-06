#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cloned_instead_of_copied,
    clippy::clone_on_copy,
    clippy::collapsible_if,
    clippy::implicit_clone,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::map_unwrap_or,
    clippy::match_wildcard_for_single_variants,
    clippy::redundant_pub_crate,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::vec_init_then_push
)]
//! Izanami chaosnet tool for orchestrating fault-injection scenarios on local Iroha clusters.

mod chaos;
mod config;
mod faults;
mod instructions;
mod persistence;
mod smart_contracts;
mod tui;

use clap::{ArgMatches, CommandFactory, FromArgMatches, parser::ValueSource};
use color_eyre::Result;

use crate::config::IzanamiArgs;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let command = config::IzanamiArgs::command();
    let matches = command.get_matches();

    let args = config::IzanamiArgs::from_arg_matches(&matches)
        .expect("command-generated matches should parse");

    if args.tui {
        let defaults = config::IzanamiArgs::defaults();
        let persisted = persistence::load_args()?.unwrap_or_else(|| defaults.clone());
        let initial = merge_with_overrides(persisted, &args, &matches);
        match tui::launch(initial)? {
            Some((config, mut updated_args)) => {
                updated_args.tui = false;
                persistence::store_args(&updated_args)?;
                config::init_tracing_with_filter(&config.log_filter);
                persistence::store_config(&config)?;
                chaos::IzanamiRunner::new(config).await?.run().await
            }
            None => Ok(()),
        }
    } else {
        let config = config::ChaosConfig::try_from(args.clone())?;
        persistence::store_config(&config)?;
        config::init_tracing_with_filter(&config.log_filter);
        chaos::IzanamiRunner::new(config).await?.run().await
    }
}

fn merge_with_overrides(
    mut base: IzanamiArgs,
    overrides: &IzanamiArgs,
    matches: &ArgMatches,
) -> IzanamiArgs {
    if is_cli_source(matches, "allow_net") {
        base.allow_net = overrides.allow_net;
    }
    if is_cli_source(matches, "peers") {
        base.peers = overrides.peers;
    }
    if is_cli_source(matches, "faulty") {
        base.faulty = overrides.faulty;
    }
    if is_cli_source(matches, "duration") {
        base.duration = overrides.duration;
    }
    if is_cli_source(matches, "pipeline_time") {
        base.pipeline_time = overrides.pipeline_time;
    }
    if is_cli_source(matches, "target_blocks") {
        base.target_blocks = overrides.target_blocks;
    }
    if is_cli_source(matches, "progress_interval") {
        base.progress_interval = overrides.progress_interval;
    }
    if is_cli_source(matches, "progress_timeout") {
        base.progress_timeout = overrides.progress_timeout;
    }
    if is_cli_source(matches, "seed") {
        base.seed = overrides.seed;
    }
    if is_cli_source(matches, "tps") {
        base.tps = overrides.tps;
    }
    if is_cli_source(matches, "max_inflight") {
        base.max_inflight = overrides.max_inflight;
    }
    if is_cli_source(matches, "workload_profile") {
        base.workload_profile = overrides.workload_profile;
    }
    if is_cli_source(matches, "allow_contract_deploy_in_stable") {
        base.allow_contract_deploy_in_stable = overrides.allow_contract_deploy_in_stable;
    }
    if is_cli_source(matches, "log_filter") {
        base.log_filter.clone_from(&overrides.log_filter);
    }
    if is_cli_source(matches, "fault_interval_min") {
        base.fault_interval_min = overrides.fault_interval_min;
    }
    if is_cli_source(matches, "fault_interval_max") {
        base.fault_interval_max = overrides.fault_interval_max;
    }
    if is_cli_source(matches, "network_latency") {
        base.faults.network_latency = overrides.faults.network_latency;
    }
    if is_cli_source(matches, "network_partition") {
        base.faults.network_partition = overrides.faults.network_partition;
    }
    if is_cli_source(matches, "cpu_stress") {
        base.faults.cpu_stress = overrides.faults.cpu_stress;
    }
    if is_cli_source(matches, "disk_saturation") {
        base.faults.disk_saturation = overrides.faults.disk_saturation;
    }
    if is_cli_source(matches, "nexus") {
        base.nexus = overrides.nexus;
    }
    base.tui = overrides.tui;
    base
}

fn is_cli_source(matches: &ArgMatches, id: &str) -> bool {
    use std::borrow::Cow;

    let mut variants: Vec<Cow<'_, str>> = vec![Cow::Borrowed(id)];
    if id.contains('-') {
        variants.push(Cow::Owned(id.replace('-', "_")));
    }
    if id.contains('_') {
        variants.push(Cow::Owned(id.replace('_', "-")));
    }

    for candidate in variants {
        let needle = candidate.as_ref();
        if matches.ids().any(|existing| existing.as_str() == needle)
            && matches
                .value_source(needle)
                .is_some_and(|source| source == ValueSource::CommandLine)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use clap::{CommandFactory, FromArgMatches};

    use super::*;
    use crate::config;

    fn parse_cli_arguments(args: Vec<String>) -> (IzanamiArgs, ArgMatches) {
        let command = config::IzanamiArgs::command();
        let matches = command
            .try_get_matches_from(args)
            .expect("cli arguments should parse");
        let parsed = config::IzanamiArgs::from_arg_matches(&matches)
            .expect("matches produced by clap should be valid");
        (parsed, matches)
    }

    #[test]
    fn tui_cli_overrides_persisted_peers_even_when_default() {
        let defaults = config::IzanamiArgs::defaults();
        let mut persisted = defaults.clone();
        persisted.tui = false;
        persisted.peers = defaults.peers + 5;
        persisted.faulty = defaults.faulty + 1;
        persisted.duration = Duration::from_secs(300);
        persisted.pipeline_time = Some(Duration::from_secs(4));
        persisted.target_blocks = Some(256);
        persisted.progress_interval = Duration::from_secs(9);
        persisted.progress_timeout = Duration::from_secs(90);
        persisted.seed = Some(13);
        persisted.tps = defaults.tps + 1.0;
        persisted.max_inflight = defaults.max_inflight + 10;
        persisted.log_filter = "debug".to_string();
        persisted.fault_interval_min = Duration::from_secs(1);
        persisted.fault_interval_max = Duration::from_secs(2);

        let peers_arg = defaults.peers.to_string();
        let (cli_args, matches) = parse_cli_arguments(vec![
            "izanami".to_string(),
            "--tui".to_string(),
            "--peers".to_string(),
            peers_arg,
        ]);

        let merged = merge_with_overrides(persisted.clone(), &cli_args, &matches);

        assert!(merged.tui, "cli --tui flag should be respected");
        assert_eq!(merged.peers, defaults.peers);
        assert_eq!(merged.faulty, persisted.faulty);
        assert_eq!(merged.duration, persisted.duration);
        assert_eq!(merged.pipeline_time, persisted.pipeline_time);
        assert_eq!(merged.target_blocks, persisted.target_blocks);
        assert_eq!(merged.progress_interval, persisted.progress_interval);
        assert_eq!(merged.progress_timeout, persisted.progress_timeout);
        assert_eq!(merged.seed, persisted.seed);
        assert!((merged.tps - persisted.tps).abs() <= f64::EPSILON);
        assert_eq!(merged.max_inflight, persisted.max_inflight);
        assert_eq!(merged.log_filter, persisted.log_filter);
        assert_eq!(merged.fault_interval_min, persisted.fault_interval_min);
        assert_eq!(merged.fault_interval_max, persisted.fault_interval_max);
    }

    #[test]
    fn cli_value_matching_default_replaces_persisted_setting() {
        let defaults = config::IzanamiArgs::defaults();
        let mut persisted = defaults.clone();
        persisted.max_inflight = defaults.max_inflight * 2;
        persisted.pipeline_time = Some(Duration::from_secs(11));
        persisted.target_blocks = Some(42);
        persisted.progress_interval = Duration::from_secs(5);
        persisted.progress_timeout = Duration::from_secs(50);

        let max_inflight_arg = defaults.max_inflight.to_string();
        let (cli_args, matches) = parse_cli_arguments(vec![
            "izanami".to_string(),
            "--tui".to_string(),
            "--max-inflight".to_string(),
            max_inflight_arg,
        ]);

        let merged = merge_with_overrides(persisted.clone(), &cli_args, &matches);

        assert_eq!(merged.max_inflight, defaults.max_inflight);
        assert_eq!(merged.peers, persisted.peers);
        assert_eq!(merged.pipeline_time, persisted.pipeline_time);
        assert_eq!(merged.target_blocks, persisted.target_blocks);
        assert_eq!(merged.progress_interval, persisted.progress_interval);
        assert_eq!(merged.progress_timeout, persisted.progress_timeout);
    }

    #[test]
    fn cli_overrides_allow_contract_deploy_in_stable() {
        let defaults = config::IzanamiArgs::defaults();
        let mut persisted = defaults.clone();
        persisted.allow_contract_deploy_in_stable = false;

        let (cli_args, matches) = parse_cli_arguments(vec![
            "izanami".to_string(),
            "--allow-contract-deploy-in-stable".to_string(),
        ]);

        let merged = merge_with_overrides(persisted, &cli_args, &matches);

        assert!(
            merged.allow_contract_deploy_in_stable,
            "cli flag should enable contract deploys in stable runs"
        );
    }

    #[test]
    fn tui_cli_overrides_fault_toggles() {
        let defaults = config::IzanamiArgs::defaults();
        let mut persisted = defaults.clone();
        persisted.tui = false;
        persisted.faults = config::FaultArgs {
            network_latency: false,
            network_partition: false,
            cpu_stress: false,
            disk_saturation: false,
        };

        let (cli_args, matches) = parse_cli_arguments(vec![
            "izanami".to_string(),
            "--tui".to_string(),
            "--fault-enable-network-latency".to_string(),
            "--fault-enable-cpu-stress".to_string(),
        ]);

        let merged = merge_with_overrides(persisted, &cli_args, &matches);

        assert!(merged.faults.network_latency);
        assert!(merged.faults.cpu_stress);
        assert!(!merged.faults.network_partition);
        assert!(!merged.faults.disk_saturation);
    }
}
