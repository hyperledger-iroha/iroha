//! Validated runtime construction for `SoraNet` handshake configuration.

use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use iroha_config::parameters::actual::{
    SoranetHandshake as ActualSoranetHandshake, SoranetPow as ActualSoranetPow,
};
use iroha_crypto::soranet::{
    pow::{Parameters as PowParameters, TicketRevocationStore, TicketRevocationStoreLimits},
    puzzle,
};
use soranet_pq::MlDsaSuite;

use crate::{Error, peer::SoranetHandshakeConfig, puzzle_work_admission::process_wide_admission};

fn absolute_replay_state_path(path: &Path) -> std::io::Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}

pub fn runtime_from_handshake(
    handshake: ActualSoranetHandshake,
) -> Result<Arc<SoranetHandshakeConfig>, Error> {
    let ActualSoranetHandshake {
        descriptor_commit,
        client_capabilities,
        relay_capabilities,
        trust_gossip,
        kem_id,
        sig_id,
        resume_hash,
        pow,
    } = handshake;
    let ActualSoranetPow {
        required,
        difficulty,
        max_future_skew,
        min_ticket_ttl,
        ticket_ttl,
        outbound_mint_capacity,
        inbound_verify_capacity,
        revocation_store_capacity,
        revocation_max_ttl,
        revocation_store_path,
        puzzle,
        signed_ticket_public_key,
    } = pow;
    validate_revocation_window(max_future_skew, revocation_max_ttl)?;
    validate_puzzle_work_capacities(outbound_mint_capacity, inbound_verify_capacity)?;
    let puzzle_work_admission =
        process_wide_admission(outbound_mint_capacity, inbound_verify_capacity)
            .map_err(Error::HandshakeSoranet)?;
    let pow_params =
        PowParameters::try_new(difficulty, max_future_skew, min_ticket_ttl).map_err(|err| {
            Error::HandshakeSoranet(format!("invalid soranet PoW configuration: {err}"))
        })?;
    let puzzle_params = puzzle
        .map(|cfg| {
            puzzle::Parameters::try_new(
                cfg.memory_kib,
                cfg.time_cost,
                cfg.lanes,
                difficulty,
                max_future_skew,
                min_ticket_ttl,
            )
            .map_err(|err| {
                Error::HandshakeSoranet(format!("invalid soranet puzzle configuration: {err}"))
            })
        })
        .transpose()?;
    if puzzle_params.is_some() && ticket_ttl <= min_ticket_ttl {
        return Err(Error::HandshakeSoranet(format!(
            "invalid soranet puzzle ticket timing: ticket_ttl {ticket_ttl:?} must exceed min_ticket_ttl {min_ticket_ttl:?}"
        )));
    }
    let signed_ticket_public_key = validate_signed_ticket_public_key(signed_ticket_public_key)?;
    let revocation_limits =
        TicketRevocationStoreLimits::new(revocation_store_capacity, revocation_max_ttl).map_err(
            |err| {
                Error::HandshakeSoranet(format!("invalid soranet revocation configuration: {err}"))
            },
        )?;
    let revocation_store = if required {
        let replay_path = absolute_replay_state_path(Path::new(revocation_store_path.as_ref()))
            .map_err(|err| {
                Error::HandshakeSoranet(format!(
                    "failed to resolve soranet revocation store path {revocation_store_path}: {err}"
                ))
            })?;
        TicketRevocationStore::load(&replay_path, revocation_limits, SystemTime::now()).map_err(
            |err| {
                Error::HandshakeSoranet(format!(
                    "failed to load soranet revocation store at {}: {err}",
                    replay_path.display()
                ))
            },
        )?
    } else {
        // Production configuration fixes `required = true`. Programmatic test
        // configurations that disable admission use only in-memory replay state.
        TicketRevocationStore::in_memory(revocation_limits).map_err(|err| {
            Error::HandshakeSoranet(format!("invalid soranet revocation configuration: {err}"))
        })?
    };
    let config = SoranetHandshakeConfig::new(
        descriptor_commit.into_value(),
        client_capabilities.into_value(),
        relay_capabilities.into_value(),
        trust_gossip,
        kem_id,
        sig_id,
        resume_hash.map(iroha_config::base::WithOrigin::into_value),
        required,
        pow_params,
        puzzle_params,
        ticket_ttl,
        signed_ticket_public_key,
        Arc::new(Mutex::new(revocation_store)),
    )?
    .with_puzzle_work_admission(puzzle_work_admission);
    Ok(Arc::new(config))
}

fn validate_revocation_window(
    max_future_skew: std::time::Duration,
    revocation_max_ttl: std::time::Duration,
) -> Result<(), Error> {
    if revocation_max_ttl < max_future_skew {
        return Err(Error::HandshakeSoranet(format!(
            "invalid soranet revocation configuration: revocation_store_ttl {revocation_max_ttl:?} must cover max_future_skew {max_future_skew:?}"
        )));
    }
    Ok(())
}

fn validate_puzzle_work_capacities(
    outbound_mint_capacity: NonZeroUsize,
    inbound_verify_capacity: NonZeroUsize,
) -> Result<(), Error> {
    for (name, capacity) in [
        ("outbound_mint_capacity", outbound_mint_capacity),
        ("inbound_verify_capacity", inbound_verify_capacity),
    ] {
        if capacity.get() > ActualSoranetPow::MAX_PUZZLE_WORK_CAPACITY_PER_DIRECTION {
            return Err(Error::HandshakeSoranet(format!(
                "invalid soranet puzzle work configuration: {name} {capacity} exceeds the per-direction maximum {}",
                ActualSoranetPow::MAX_PUZZLE_WORK_CAPACITY_PER_DIRECTION
            )));
        }
    }
    Ok(())
}

fn validate_signed_ticket_public_key(key: Option<Vec<u8>>) -> Result<Option<Vec<u8>>, Error> {
    key.map(|key| {
        MlDsaSuite::MlDsa44
            .validate_public_key(&key)
            .map_err(|error| {
                Error::HandshakeSoranet(format!(
                    "invalid soranet signed_ticket_public_key_hex (ML-DSA-44): {error}"
                ))
            })?;
        Ok(key)
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use soranet_pq::generate_mldsa_keypair_from_os;

    use super::*;

    #[test]
    fn revocation_window_must_cover_every_accepted_ticket() {
        let max_future_skew = std::time::Duration::from_secs(300);
        let error =
            validate_revocation_window(max_future_skew, std::time::Duration::from_secs(299))
                .expect_err(
                    "a store that cannot retain every accepted ticket must fail at startup",
                );
        assert!(matches!(
            error,
            Error::HandshakeSoranet(message)
                if message.contains("revocation_store_ttl 299s must cover max_future_skew 300s")
        ));
        validate_revocation_window(max_future_skew, max_future_skew)
            .expect("an equal revocation and acceptance window is sufficient");
    }

    #[test]
    fn signed_ticket_public_key_rejects_inert_material() {
        let inert = vec![0_u8; MlDsaSuite::MlDsa44.public_key_len()];
        assert!(validate_signed_ticket_public_key(Some(inert)).is_err());
    }

    #[test]
    fn signed_ticket_public_key_accepts_generated_material() {
        let keypair =
            generate_mldsa_keypair_from_os(MlDsaSuite::MlDsa44).expect("generate ML-DSA keypair");
        let expected = keypair.public_key().to_vec();
        let validated = validate_signed_ticket_public_key(Some(expected.clone()))
            .expect("validate generated public key");
        assert_eq!(validated.as_deref(), Some(expected.as_slice()));
    }
}
