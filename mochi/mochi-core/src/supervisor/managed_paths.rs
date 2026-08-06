//! Managed peer configuration invariants that temporary overlays cannot redirect.

use std::{collections::BTreeMap, path::Path};

use iroha_config::parameters::actual;
use iroha_crypto::PublicKey;
use iroha_primitives::addr::SocketAddr;

use super::{PeerSpec, Result, SupervisorError};

const SORACLOUD_RUNTIME_TABLE: &str = "soracloud_runtime";
const SORACLOUD_RUNTIME_STATE_DIR: &str = "state_dir";
const SORACLOUD_RUNTIME_STORAGE_CHILD: &str = "soracloud_runtime";

fn merge_nested_table(target: &mut toml::Table, overlay: &toml::Table) {
    for (key, value) in overlay {
        match (target.get_mut(key), value) {
            (Some(toml::Value::Table(target)), toml::Value::Table(overlay)) => {
                merge_nested_table(target, overlay);
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

fn nested_value<'a>(table: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Value> {
    let (last, parents) = path.split_last()?;
    let mut current = table;
    for key in parents {
        current = current.get(*key)?.as_table()?;
    }
    current.get(*last)
}

pub(super) fn merge_temporary_peer_overlays(
    root: &mut toml::Table,
    overlays: &[toml::Table],
) -> Result<()> {
    let managed_network = root
        .get("network")
        .and_then(toml::Value::as_table)
        .cloned()
        .ok_or_else(|| SupervisorError::Config("generated network must be a table".to_owned()))?;
    let mut restored_network = managed_network.clone();
    for overlay in overlays {
        if let Some(network) = overlay.get("network") {
            let network = network
                .as_table()
                .ok_or_else(|| SupervisorError::Config("network must be a table".to_owned()))?;
            merge_nested_table(&mut restored_network, network);
        }
        super::merge_table(root, overlay);
    }

    for field in ["address", "public_address"] {
        if restored_network.get(field) != managed_network.get(field) {
            return Err(SupervisorError::Config(format!(
                "temporary config overlays must preserve Mochi's managed network.{field}"
            )));
        }
    }
    let pow_path = ["soranet_handshake", "pow"];
    if let Some(expected_pow) = nested_value(&managed_network, &pow_path)
        && nested_value(&restored_network, &pow_path) != Some(expected_pow)
    {
        return Err(SupervisorError::Config(
            "temporary config overlays must preserve Mochi's full managed local network.soranet_handshake.pow configuration"
                .to_owned(),
        ));
    }
    root.insert("network".into(), toml::Value::Table(restored_network));
    Ok(())
}

fn expected_soracloud_runtime_state_dir(storage_dir: &Path) -> Result<std::path::PathBuf> {
    Ok(storage_dir
        .canonicalize()?
        .join(SORACLOUD_RUNTIME_STORAGE_CHILD))
}

pub(super) fn bind_soracloud_runtime_state_dir(
    root: &mut toml::Table,
    storage_dir: &Path,
) -> Result<()> {
    let expected = expected_soracloud_runtime_state_dir(storage_dir)?;
    let expected_literal = expected.display().to_string();
    let runtime = root
        .entry(SORACLOUD_RUNTIME_TABLE)
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| SupervisorError::Config("soracloud_runtime must be a table".to_owned()))?;
    if let Some(configured) = runtime.get(SORACLOUD_RUNTIME_STATE_DIR)
        && configured.as_str() != Some(expected_literal.as_str())
    {
        return Err(SupervisorError::Config(format!(
            "temporary config overlays must preserve Mochi's managed Soracloud runtime state root `{expected_literal}`"
        )));
    }
    runtime.insert(
        SORACLOUD_RUNTIME_STATE_DIR.into(),
        toml::Value::String(expected_literal),
    );
    Ok(())
}

pub(super) fn validate_soracloud_runtime_state_dir(
    config: &actual::Root,
    config_path: &Path,
    storage_dir: &Path,
) -> Result<()> {
    let expected = expected_soracloud_runtime_state_dir(storage_dir)?;
    let configured = &config.soracloud_runtime.state_dir;
    if configured != &expected {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate peer config `{}` redirects Mochi-managed `soracloud_runtime.state_dir` from `{}` to `{}`",
            config_path.display(),
            expected.display(),
            configured.display()
        )));
    }
    Ok(())
}

fn candidate_socket_addr(value: &str, config_path: &Path, field: &str) -> Result<SocketAddr> {
    value.parse::<SocketAddr>().map_err(|error| {
        SupervisorError::GenerationValidation(format!(
            "candidate peer config `{}` has invalid expected `{field}` `{value}`: {error}",
            config_path.display()
        ))
    })
}

pub(super) fn validate_candidate_peer_topology(
    config: &actual::Root,
    peer: &PeerSpec,
    peers: &[PeerSpec],
    expected_roster: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<()> {
    let config_path = &peer.config_path;
    let expected_bind = candidate_socket_addr(&peer.p2p_bind, config_path, "network.address")?;
    let expected_public =
        candidate_socket_addr(&peer.p2p_public, config_path, "network.public_address")?;
    if config.network.address.value() != &expected_bind {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate peer config `{}` redirects managed `network.address` from `{expected_bind}` to `{}`",
            config_path.display(),
            config.network.address.value()
        )));
    }
    if config.network.public_address.value() != &expected_public {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate peer config `{}` redirects managed `network.public_address` from `{expected_public}` to `{}`",
            config_path.display(),
            config.network.public_address.value()
        )));
    }

    let trusted = config.common.trusted_peers.value();
    if config.common.key_pair.public_key() != &peer.keys.public_key
        || config.common.peer.id().public_key() != &peer.keys.public_key
        || config.common.peer.address() != &expected_bind
        || trusted.myself.id().public_key() != &peer.keys.public_key
        || trusted.myself.address() != &expected_bind
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate peer config `{}` identity or self address differs from its candidate PeerSpec",
            config_path.display()
        )));
    }
    if &trusted.pops != expected_roster {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate peer config `{}` PoP roster differs from signed genesis",
            config_path.display()
        )));
    }

    let expected_others = peers
        .iter()
        .filter(|candidate| candidate.keys.public_key != peer.keys.public_key)
        .map(|candidate| {
            candidate_socket_addr(&candidate.p2p_public, config_path, "trusted_peers address")
                .map(|address| (candidate.keys.public_key.clone(), address))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let configured_others = trusted
        .others
        .iter()
        .map(|candidate| {
            (
                candidate.id().public_key().clone(),
                candidate.address().clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if configured_others != expected_others {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate peer config `{}` trusted PeerId/address topology differs from the candidate peers",
            config_path.display()
        )));
    }
    Ok(())
}
