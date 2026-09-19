//! A committed catalog expansion must survive full Kura replay before signed-snapshot recovery.
//! The native beacon-custody scenario supplies all four peers with their retained
//! startup lane configuration, keys, genesis and block history.
//! Snapshot writes begin only after the retained replay, before geometry compaction is permitted.
use super::*;

#[path = "catalog_native_execution.rs"]
mod native_execution;
use native_execution::authenticated_native_execution;

#[path = "catalog_recovery.rs"]
pub(super) mod real_custody;
use iroha_core::queue::{RoutingDecision, RoutingPlan};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader,
        consensus_v2::{ConsensusMode, ExecutionCommitment, ValidatorPower},
        decode_framed_signed_block,
    },
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    isi::{Grant, Register, Revoke, SetParameter},
    nexus::{
        DataSpaceCatalog, DataSpaceMetadata, LaneConfig, LaneLifecycleStatusV1, LaneVisibility,
        NexusCatalogTransitionV1, NexusRuntimeCatalogV1, RuntimeDataSpaceAdditionV1,
        RuntimeLaneManifestV1, dataspace_catalog_hash,
    },
    parameter::Parameter,
    permission::Permission,
    role::Role,
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanDelegateAccountAliasResolution, CanResolveAccountAlias,
};
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_primitives::json::Json;
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
struct AppliedEvidence {
    transaction: SignedTransaction,
    height: u64,
    lane: LaneId,
    dataspace: DataSpaceId,
    canonical_block: Vec<u8>,
}

#[derive(Clone)]
struct FixtureFinality {
    network_id: NetworkId,
    genesis_hash: HashOf<BlockHeader>,
    roster: Vec<ValidatorPower>,
    validator_pops: Vec<Vec<u8>>,
    peers: BTreeMap<PeerId, Arc<Mutex<VerifiedPeerFinality>>>,
}

struct VerifiedPeerFinality {
    network_id: NetworkId,
    genesis_hash: HashOf<BlockHeader>,
    peer: PeerId,
    verifier: Option<BridgeFinalityVerifier>,
    proofs: BTreeMap<u64, BridgeFinalityProof>,
}

impl FixtureFinality {
    fn validate_fixture_roster(&self, proof: &BridgeFinalityProof) -> Result<()> {
        let artifact = &proof.finality_artifact;
        ensure!(
            artifact.height_context.network_id == self.network_id
                && artifact.height_context.mode == ConsensusMode::Npos
                && artifact.height_context.roster == self.roster
                && artifact.validator_set_pops == self.validator_pops
                && artifact.height_context.snapshot_bootstrap.is_none(),
            "finality proof differs from the exact generated fixture validator authority"
        );
        Ok(())
    }

    fn execution_commitment(
        &self,
        peer: &PeerId,
        client: &iroha::client::Client,
        height: u64,
        expected_block_hash: HashOf<BlockHeader>,
    ) -> Result<ExecutionCommitment> {
        ensure!(
            (1..=128).contains(&height),
            "catalog fixture exceeded its bounded finality history"
        );
        let cache = self
            .peers
            .get(peer)
            .ok_or_else(|| eyre!("finality reader is not a fixture peer"))?;
        // This synchronous lock is held only on the peer's dedicated blocking reader. Other
        // peers have independent caches, and no asynchronous network work holds this lock.
        let mut cache = cache
            .lock()
            .map_err(|_| eyre!("fixture finality cache was poisoned"))?;
        ensure!(
            cache.network_id == self.network_id
                && cache.genesis_hash == self.genesis_hash
                && cache.peer == *peer,
            "finality cache belongs to another network, genesis or peer"
        );
        if cache.verifier.is_none() {
            ensure!(
                cache.proofs.is_empty(),
                "unanchored cache contains finality evidence"
            );
            let (proof, hash) = client.get_bridge_finality_anchor(
                NonZeroU64::new(1).expect("genesis height"),
                self.network_id,
            )?;
            ensure!(
                hash == self.genesis_hash && proof.block_header.hash() == self.genesis_hash,
                "finality anchor is not the fixture's exact signed genesis"
            );
            self.validate_fixture_roster(&proof)?;
            // Only the externally known genesis hash and complete fixture roster/PoPs can
            // authorize this context. A self-consistent proof-controlled roster is insufficient.
            let mut verifier = BridgeFinalityVerifier::with_context(
                self.network_id,
                proof.finality_artifact.context_id(),
            );
            verifier.verify(&proof)?;
            cache.proofs.insert(1, proof);
            cache.verifier = Some(verifier);
        }
        while cache
            .proofs
            .last_key_value()
            .map_or(0, |(height, _)| *height)
            < height
        {
            let next = cache
                .proofs
                .last_key_value()
                .expect("anchored proof cache")
                .0
                + 1;
            let mut verifier = cache.verifier.clone().expect("anchored native verifier");
            let proof = client.get_next_bridge_finality_proof(
                NonZeroU64::new(next).expect("successor height"),
                &mut verifier,
            )?;
            self.validate_fixture_roster(&proof)?;
            cache.proofs.insert(next, proof);
            cache.verifier = Some(verifier);
        }
        let proof = cache
            .proofs
            .get(&height)
            .ok_or_else(|| eyre!("verified finality history has a gap"))?;
        ensure!(
            proof.block_header.hash() == expected_block_hash
                && proof.finality_artifact.block_hash == expected_block_hash,
            "transaction carrier differs from the independently verified finality chain"
        );
        Ok(proof.finality_artifact.commit_qc.execution_commitment)
    }
}

const PERMISSION_FIXTURE_LIMIT: u64 = 500;

fn complete_permission_page(
    response: &iroha::http::Response<Vec<u8>>,
) -> Result<BTreeSet<Permission>> {
    #[derive(norito::derive::JsonDeserialize)]
    #[norito(deny_unknown_fields)]
    struct Page {
        items: Vec<Permission>,
        total: u64,
    }

    ensure!(
        response.status().as_u16() == 200,
        "permission read failed at offset 0, limit {PERMISSION_FIXTURE_LIMIT}: {}; body: {}",
        response.status(),
        String::from_utf8_lossy(&response.body()[..response.body().len().min(2048)])
    );
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    };
    ensure!(
        header("content-type").is_some_and(|value| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case("application/json")
        }) && header("x-iroha-account-permission-semantics") == Some("effective-v1"),
        "permission response omitted its canonical media type or effective semantics"
    );
    let counter = |name: &str| -> Result<u64> {
        header(name)
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| eyre!("permission response omitted {name}"))
    };
    let attempted = counter("x-iroha-fanout-routes-attempted")?;
    ensure!(
        attempted > 0
            && counter("x-iroha-fanout-routes-succeeded")? == attempted
            && counter("x-iroha-fanout-routes-failed")? == 0
            && counter("x-iroha-fanout-routes-denied")? == 0
            && counter("x-iroha-fanout-routes-unavailable")? == 0
            && counter("x-iroha-fanout-routes-not-found")? == 0,
        "permission read returned incomplete fanout"
    );
    let page: Page = json::from_slice(response.body())?;
    ensure!(
        page.total == u64::try_from(page.items.len())?,
        "permission page count mismatch"
    );
    // Each route returns unique permissions, and fanout merges their union.
    // A union smaller than the requested limit proves every route was short.
    // `total` counts only this merged page; it is not a global row count.
    // At the fetch-budget boundary there is no safe next-page exhaustion probe.
    ensure!(
        page.total < PERMISSION_FIXTURE_LIMIT,
        "permission fixture cannot prove exhaustion within its {PERMISSION_FIXTURE_LIMIT}-row fetch budget"
    );
    let permissions: BTreeSet<_> = page.items.into_iter().collect();
    ensure!(
        u64::try_from(permissions.len())? == page.total,
        "permission fanout returned duplicate items"
    );
    Ok(permissions)
}

fn effective_permissions(
    client: &iroha::client::Client,
    account_id: &AccountId,
) -> Result<BTreeSet<Permission>> {
    let response =
        client.get_account_permissions_page_response(account_id, PERMISSION_FIXTURE_LIMIT, 0)?;
    complete_permission_page(&response)
}

#[cfg(test)]
mod permission_page_tests {
    use super::*;
    use iroha::http::Response;

    const ADDED_DATASPACE: DataSpaceId = DataSpaceId::new(3);

    fn resolution_permission() -> Permission {
        CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(ADDED_DATASPACE),
        }
        .into()
    }

    fn resolution_delegation_permission() -> Permission {
        CanDelegateAccountAliasResolution {
            scope: AccountAliasPermissionScope::Dataspace(ADDED_DATASPACE),
        }
        .into()
    }

    fn response(items: Vec<Permission>) -> Response<Vec<u8>> {
        let total = items.len();
        let body = json::to_vec(&norito::json!({"total": total, "items": items})).unwrap();
        Response::builder()
            .status(200)
            .header("content-type", "application/json; charset=utf-8")
            .header("x-iroha-account-permission-semantics", "effective-v1")
            .header("x-iroha-fanout-routes-attempted", "4")
            .header("x-iroha-fanout-routes-succeeded", "4")
            .header("x-iroha-fanout-routes-failed", "0")
            .header("x-iroha-fanout-routes-denied", "0")
            .header("x-iroha-fanout-routes-unavailable", "0")
            .header("x-iroha-fanout-routes-not-found", "0")
            .body(body)
            .unwrap()
    }

    #[test]
    fn permission_page_requires_complete_short_fanout() {
        let items = vec![resolution_permission(), resolution_delegation_permission()];
        assert_eq!(
            complete_permission_page(&response(items.clone())).unwrap(),
            items.into_iter().collect()
        );
        assert!(
            complete_permission_page(&response(Vec::new()))
                .unwrap()
                .is_empty()
        );
        for name in [
            "x-iroha-fanout-routes-attempted",
            "x-iroha-fanout-routes-succeeded",
            "x-iroha-fanout-routes-failed",
            "x-iroha-fanout-routes-denied",
            "x-iroha-fanout-routes-unavailable",
            "x-iroha-fanout-routes-not-found",
        ] {
            let mut page = response(vec![resolution_permission()]);
            page.headers_mut().insert(name, "1".parse().unwrap());
            assert!(
                complete_permission_page(&page).is_err(),
                "accepted changed {name}"
            );
            page.headers_mut().remove(name);
            assert!(
                complete_permission_page(&page).is_err(),
                "accepted absent {name}"
            );
        }
    }

    #[test]
    fn permission_page_rejects_saturation_and_duplicate_items() {
        for size in [500, 501] {
            let items = (0..size)
                .map(|value| Permission::new(format!("fixture{value}"), Json::default()))
                .collect();
            assert!(
                complete_permission_page(&response(items))
                    .unwrap_err()
                    .to_string()
                    .contains("cannot prove exhaustion")
            );
        }
        let duplicate = response(vec![resolution_permission(), resolution_permission()]);
        assert!(
            complete_permission_page(&duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
    }

    #[test]
    fn permission_page_preserves_failure_context_and_rejects_invalid_metadata() {
        let failed = Response::builder()
            .status(400)
            .body(b"invalid_pagination: fetch budget exceeded".to_vec())
            .unwrap();
        let error = complete_permission_page(&failed).unwrap_err().to_string();
        assert!(error.contains("offset 0, limit 500"));
        assert!(error.contains("400 Bad Request"));
        assert!(error.contains("invalid_pagination"));
        for name in ["content-type", "x-iroha-account-permission-semantics"] {
            let mut page = response(Vec::new());
            page.headers_mut().remove(name);
            assert!(complete_permission_page(&page).is_err());
        }
        let mut mismatch = response(vec![resolution_permission()]);
        *mismatch.body_mut() = br#"{"items":[],"total":1}"#.to_vec();
        assert!(
            complete_permission_page(&mismatch)
                .unwrap_err()
                .to_string()
                .contains("count mismatch")
        );
    }
}
