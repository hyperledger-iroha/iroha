//! Native developer bootstrap through one installed origin and quorum-certified state.
//!
//! The selected HTTPS origin, or explicit numeric-loopback development origin, supplies the
//! initial exact network identity. Signed genesis and native
//! epoch-boundary certificates authenticate the parent; no separately published checkpoint or
//! validator HTTP inventory is needed. A returned checkpoint proves its recorded decision,
//! not a fresh observation from every current validator.

use std::time::{Duration, Instant};

use iroha::http::PublicHttpClient;
use iroha_data_model::{
    NetworkId,
    block::consensus::SumeragiRootScope,
    sumeragi_finality::{
        FinalityValidator, MAX_FINALITY_BLOCK_BYTES, SumeragiCommitCertificateV1,
        SumeragiCommitVerifierV1, SumeragiFinalityProof, SumeragiFinalityVerifier,
        authenticated_genesis,
    },
};
use iroha_torii_shared::{
    account_capabilities::AccountCapabilitiesV1, account_faucet_policy::AccountFaucetAdvertisement,
    route_catalog,
};
use url::Url;

use super::{BootstrapError, InstalledNetworkProfile, Result};
use crate::verify::finality::{FinalityVerifier, MAX_ADVANCE_BYTES, MAX_ADVANCE_PROOFS};

// The full proof adds only a header and bounded committee to its canonical block frame.
const MAX_PROOF_RESPONSE_BYTES: usize = MAX_FINALITY_BLOCK_BYTES + 64 * 1024;

/// Exact network, native certified parent checkpoint and independently discovered faucet policy.
#[derive(Debug)]
pub struct NativeBootstrap {
    /// Genesis identity discovered from the installed origin and checked against genesis.
    pub(super) network_id: NetworkId,
    /// Native checkpoint authenticated by genesis and exact-quorum epoch certificates.
    pub(super) verifier: FinalityVerifier,
    /// Strict account bootstrap policy, including the admitted signing algorithms.
    pub(super) capabilities: AccountCapabilitiesV1,
    /// Public faucet policy independently bound to the discovered network and address profile.
    pub(super) faucet: AccountFaucetAdvertisement,
}

/// Discover a selected network and authenticate a compact native parent checkpoint.
///
/// Work is proportional to epoch changes plus three recent blocks, rather than total height.
/// All requests share the original deadline; credentials and validator endpoint guesses are absent.
/// Callers must retain the resulting identity and reject changes against their existing context.
///
/// # Errors
/// Strict discovery/schema errors, identity mismatches, missing or invalid native certificates,
/// an uncertified genesis-only chain, finite work/response limits, or the original deadline.
pub fn discover_native_bootstrap(
    profile: &InstalledNetworkProfile,
    deadline: Instant,
) -> Result<NativeBootstrap> {
    discover_with(
        profile,
        deadline,
        &PublicHttpClient::new(),
        |timeout| {
            iroha::blocking::account_bootstrap::Client::new(profile.torii_root().clone(), timeout)
                .and_then(|client| client.capabilities())
                .map_err(|_| {
                    BootstrapError::Invalid(
                        "native account discovery failed at /v1/accounts/capabilities; check connectivity and the current API schema",
                    )
                })
        },
        |network_id, timeout| {
            iroha::blocking::account_bootstrap::Client::new(profile.torii_root().clone(), timeout)
                .and_then(|client| {
                    client.faucet_policy(network_id, profile.account_chain_discriminant())
                })
                .map_err(|_| BootstrapError::Invalid("native faucet policy discovery failed"))
        },
    )
}

fn discover_with(
    profile: &InstalledNetworkProfile,
    deadline: Instant,
    client: &PublicHttpClient,
    capabilities: impl FnOnce(Duration) -> Result<AccountCapabilitiesV1>,
    faucet: impl FnOnce(NetworkId, Duration) -> Result<AccountFaucetAdvertisement>,
) -> Result<NativeBootstrap> {
    let capabilities = capabilities(remaining(deadline)?)?;
    if capabilities.schema_version != 1
        || capabilities.network_prefix != profile.account_chain_discriminant()
    {
        return Err(BootstrapError::Invalid(
            "native account capabilities differ from the installed network profile",
        ));
    }
    if !capabilities
        .allowed_signing
        .iter()
        .any(|algorithm| algorithm == "ed25519")
    {
        return Err(BootstrapError::Invalid(
            "selected network does not admit the managed Ed25519 owner",
        ));
    }
    let network_id = capabilities.network_id;
    let verifier = read_parent_checkpoint(profile, network_id, deadline, client)?;
    let faucet = faucet(network_id, remaining(deadline)?)?;
    // The SDK validates the full advertisement. Retain its exact network binding at this
    // boundary as well, before returning material that the caller will persist independently.
    if faucet.schema_version != 1
        || faucet.network_id != network_id
        || faucet.network_prefix != profile.account_chain_discriminant()
    {
        return Err(BootstrapError::Invalid(
            "native faucet policy differs from the discovered network",
        ));
    }
    remaining(deadline)?;
    Ok(NativeBootstrap {
        network_id,
        verifier,
        capabilities,
        faucet,
    })
}

fn read_parent_checkpoint(
    profile: &InstalledNetworkProfile,
    network_id: NetworkId,
    deadline: Instant,
    client: &PublicHttpClient,
) -> Result<FinalityVerifier> {
    let mut budget = ProofBudget {
        proofs: MAX_ADVANCE_PROOFS,
        bytes: MAX_ADVANCE_BYTES,
    };
    let genesis_proof = read_proof(profile, client, 1, deadline, &mut budget)?;
    let genesis = iroha_genesis::decode_signed_genesis(&genesis_proof.block_wire)
        .map_err(|_| BootstrapError::Invalid("native bootstrap genesis is not canonical"))?;
    if NetworkId::from_genesis_hash(genesis.hash()) != network_id {
        return Err(BootstrapError::Invalid(
            "native bootstrap genesis differs from the discovered network",
        ));
    }
    let signed = authenticated_genesis(&genesis)
        .map_err(|_| BootstrapError::Invalid("native bootstrap genesis signature is invalid"))?;
    let (epoch, scope) = signed.into_parts();
    if scope != SumeragiRootScope::Global {
        return Err(BootstrapError::Invalid(
            "native bootstrap parent must be a global network",
        ));
    }
    let validators = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect();
    let mut native = SumeragiFinalityVerifier::new(&genesis, profile.chain_id(), validators)
        .map_err(|_| BootstrapError::Invalid("native bootstrap genesis authority is invalid"))?;
    native
        .verify(&genesis_proof)
        .map_err(|_| BootstrapError::Invalid("native bootstrap genesis proof is invalid"))?;
    let genesis_checkpoint = native
        .export_checkpoint(&genesis_proof)
        .map_err(|_| BootstrapError::Invalid("cannot retain native bootstrap genesis"))?;
    let mut compact = SumeragiCommitVerifierV1::new(&native)
        .map_err(|_| BootstrapError::Invalid("cannot initialize native bootstrap certificates"))?;
    let tip = read_tip(profile, client, deadline)?;
    let mut boundary_height = epoch.authorization.last_height;
    let mut last_boundary = None;
    while boundary_height < tip {
        let proof = read_proof(profile, client, boundary_height, deadline, &mut budget)?;
        let certificate = SumeragiCommitCertificateV1::from_proof(&proof)
            .map_err(|_| BootstrapError::Invalid("native epoch boundary proof is invalid"))?;
        let boundary = compact
            .verify_epoch_boundary(&certificate)
            .map_err(|_| BootstrapError::Invalid("native epoch boundary authority is invalid"))?;
        let next = boundary.selected_epoch().authorization.last_height;
        if next <= boundary_height {
            return Err(BootstrapError::Invalid(
                "native epoch boundary does not advance authority",
            ));
        }
        boundary_height = next;
        last_boundary = Some(proof);
        remaining(deadline)?;
    }
    let mut proofs = Vec::with_capacity(3);
    for height in tip.saturating_sub(2).max(1)..=tip {
        let proof = if height == 1 {
            genesis_proof.clone()
        } else if last_boundary
            .as_ref()
            .is_some_and(|proof| proof.height() == height)
        {
            last_boundary.take().expect("matching retained boundary")
        } else {
            read_proof(profile, client, height, deadline, &mut budget)?
        };
        proofs.push(proof);
    }
    let checkpoint = compact
        .checkpoint_from_proofs(&genesis_checkpoint, &proofs)
        .map_err(|_| {
            BootstrapError::Invalid("native parent checkpoint certificates are invalid")
        })?;
    remaining(deadline)?;
    FinalityVerifier::from_checkpoint(checkpoint, network_id, profile.chain_id())
        .map_err(Into::into)
}

struct ProofBudget {
    proofs: usize,
    bytes: usize,
}

fn read_proof(
    profile: &InstalledNetworkProfile,
    client: &PublicHttpClient,
    height: u64,
    deadline: Instant,
    budget: &mut ProofBudget,
) -> Result<SumeragiFinalityProof> {
    remaining(deadline)?;
    if height == 0 || budget.proofs == 0 || budget.bytes == 0 {
        return Err(BootstrapError::Invalid(
            "native bootstrap proof budget is exhausted",
        ));
    }
    let route = route_catalog::sumeragi::BRIDGE_FINALITY
        .path()
        .replace("{height}", &height.to_string());
    let maximum = MAX_PROOF_RESPONSE_BYTES.min(budget.bytes);
    let bytes = client
        .get_norito_bytes_blocking(&endpoint(profile, &route)?, deadline, maximum)
        .map_err(|_| BootstrapError::Invalid("native bootstrap proof retrieval failed"))?;
    budget.proofs -= 1;
    budget.bytes -= bytes.len();
    let proof: SumeragiFinalityProof =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| BootstrapError::Invalid("native bootstrap proof is not canonical"))?;
    if proof.height() != height {
        return Err(BootstrapError::Invalid(
            "native bootstrap proof differs from the requested height",
        ));
    }
    remaining(deadline)?;
    Ok(proof)
}

fn read_tip(
    profile: &InstalledNetworkProfile,
    client: &PublicHttpClient,
    deadline: Instant,
) -> Result<u64> {
    let bytes = client
        .get_bytes_blocking(
            &endpoint(profile, route_catalog::diagnostic::STATUS_BLOCKS.path())?,
            deadline,
            32,
        )
        .map_err(|_| BootstrapError::Invalid("native parent height discovery failed"))?;
    let height: u64 = norito::json::from_slice(&bytes)
        .map_err(|_| BootstrapError::Invalid("native parent height is invalid"))?;
    if height < 2 {
        return Err(BootstrapError::Invalid(
            "native parent has no quorum-certified successor to genesis",
        ));
    }
    remaining(deadline)?;
    // This number selects an original certificate. Only native verification authenticates it.
    Ok(height)
}

fn endpoint(profile: &InstalledNetworkProfile, route: &str) -> Result<Url> {
    profile
        .torii_root()
        .join(route.trim_start_matches('/'))
        .map_err(|_| BootstrapError::Invalid("invalid native bootstrap route"))
}

fn remaining(deadline: Instant) -> Result<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(BootstrapError::Invalid("native bootstrap deadline expired"));
    }
    Ok(remaining)
}

#[cfg(test)]
mod tests;
