//! Server-owned bootstrap retention and nonce-bound freshness issuance.
//!
//! One configured authority owns one durable store for a policy/network/asset/reserve scope.
//! All of that deployment's freshness signing passes through this owner; independent replica
//! directories are not a distributed compare-and-swap authority. Private keys remain runtime
//! inputs. Only the existing signed bootstrap package is retained, never freshness replies.
//! This server store is not a handset hardware counter or wallet antirollback mechanism.

use std::{path::Path, sync::Mutex};

use iroha_crypto::{Hash, KeyPair, SignatureOf};
use iroha_data_model::{
    NetworkId,
    kagemusha::{
        KAGEMUSHA_ASSET_SCALE_MAX_V1, KagemushaMobileBootstrapFreshnessApprovalV1,
        KagemushaMobileBootstrapFreshnessPackageV1, KagemushaMobileBootstrapFreshnessPinsV1,
        KagemushaMobileBootstrapFreshnessStatementV1, KagemushaMobileBootstrapPackageV1,
        KagemushaMobileBootstrapPinsV1, KagemushaMobileBootstrapReplayPinV1,
        KagemushaMobileBootstrapScopeV1, KagemushaReleaseAuthorityPolicyV1,
    },
};

mod store;
use store::HighWaterStore;

type Result<T> = std::result::Result<T, String>;

/// Independent authority scope; release changes do not create a new retained sequence floor.
#[derive(Clone, Debug)]
pub(crate) struct KagemushaBootstrapIssuerAuthorityV1 {
    /// Independently configured threshold policy.
    pub policy: KagemushaReleaseAuthorityPolicyV1,
    /// Exact deployment network.
    pub network_id: NetworkId,
    /// Exact asset incarnation, scale and reserve scope.
    pub scope: KagemushaMobileBootstrapScopeV1,
}

/// Independently configured current release selected by the native server caller.
#[derive(Clone, Copy, Debug)]
pub(crate) struct KagemushaBootstrapIssuerReleaseV1 {
    /// Current release identity, never selected from the request.
    pub release_id: [u8; 32],
    /// Current release-attestation identity, never selected from the request.
    pub release_attestation_digest: [u8; 32],
}

/// UTC bounds already qualified by the native server's independent clock authority.
///
/// The caller obtains this interval after receiving the request nonce. Its uncertainty must
/// cover the trusted source's error; NTS dispersion alone is not such an error bound. This
/// component neither reads the host wall clock nor accepts timestamp fields from HTTP clients.
#[derive(Clone, Copy, Debug)]
pub(crate) struct KagemushaBootstrapIssuerTimeIntervalV1 {
    /// Inclusive UTC lower bound, in milliseconds since the Unix epoch.
    pub lower_ms: u64,
    /// Inclusive UTC upper bound for the same source observation.
    pub upper_ms: u64,
}

struct IssuerState {
    store: HighWaterStore,
    retained: KagemushaMobileBootstrapReplayPinV1,
    archive: Vec<u8>,
}

/// Process-owned authority that retains a checkpoint durably before signing any reply.
///
/// Create requires a valid initial checkpoint; recovery never creates missing state. Keys
/// and state are deliberately not exposed through a wire codec, `Clone`, or `Debug`.
pub(crate) struct KagemushaMobileBootstrapIssuerV1 {
    process_id: u32,
    authority: KagemushaBootstrapIssuerAuthorityV1,
    signers: Vec<KeyPair>,
    state: Mutex<IssuerState>,
    #[cfg(test)]
    signatures_created: std::sync::atomic::AtomicUsize,
}

impl KagemushaMobileBootstrapIssuerV1 {
    /// Exclusively create and seed a new authority store with an authenticated checkpoint.
    ///
    /// # Errors
    /// Rejects an existing directory, invalid independent pins/keys/time, or any durability error.
    pub(crate) fn create(
        directory: &Path,
        authority: KagemushaBootstrapIssuerAuthorityV1,
        signers: Vec<KeyPair>,
        initial_archive: &[u8],
        release: KagemushaBootstrapIssuerReleaseV1,
        time: KagemushaBootstrapIssuerTimeIntervalV1,
    ) -> Result<Self> {
        let signers = validate_authority_signers(&authority, signers)?;
        let package = KagemushaMobileBootstrapPackageV1::decode_canonical_exact(initial_archive)?;
        let retained = authenticate_current(&authority, &package, release, time, None)?;
        let store = HighWaterStore::create(directory, initial_archive)?;
        Ok(Self::from_parts(
            authority,
            signers,
            store,
            retained,
            initial_archive.to_vec(),
        ))
    }

    /// Recover the exact historical floor without authorizing that release for new issuance.
    ///
    /// Historical authentication verifies signatures and structure at the record's own issuance
    /// time solely to recover retention. Every new reply separately requires independent current
    /// release pins and qualified current UTC bounds supplied to `issue`.
    ///
    /// # Errors
    /// Rejects missing/corrupt state, another live writer, changed authority scope or invalid keys.
    pub(crate) fn recover(
        directory: &Path,
        authority: KagemushaBootstrapIssuerAuthorityV1,
        signers: Vec<KeyPair>,
    ) -> Result<Self> {
        let signers = validate_authority_signers(&authority, signers)?;
        let (mut store, archive) = HighWaterStore::recover(directory)?;
        let package = KagemushaMobileBootstrapPackageV1::decode_canonical_exact(&archive)?;
        let checkpoint = package.checkpoint;
        // This is archival signature/structure validation, never a current-time admission.
        let digest = package.authenticate(&KagemushaMobileBootstrapPinsV1 {
            authority_policy: &authority.policy,
            network_id: authority.network_id,
            scope: authority.scope,
            release_id: checkpoint.release_id,
            release_attestation_digest: checkpoint.release_attestation_digest,
            minimum_sequence: 1,
            previous: None,
            trusted_now_ms: checkpoint.issued_at_ms,
        })?;
        let retained = KagemushaMobileBootstrapReplayPinV1 {
            sequence: checkpoint.sequence,
            checkpoint_digest: digest,
        };
        // An interrupted, unpublished temporary file may be removed only after the committed
        // record has authenticated. An invalid head never triggers a write during recovery.
        store.finish_recovery()?;
        Ok(Self::from_parts(
            authority, signers, store, retained, archive,
        ))
    }

    fn from_parts(
        authority: KagemushaBootstrapIssuerAuthorityV1,
        signers: Vec<KeyPair>,
        store: HighWaterStore,
        retained: KagemushaMobileBootstrapReplayPinV1,
        archive: Vec<u8>,
    ) -> Self {
        Self {
            process_id: std::process::id(),
            authority,
            signers,
            state: Mutex::new(IssuerState {
                store,
                retained,
                archive,
            }),
            #[cfg(test)]
            signatures_created: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Authenticate, atomically retain, then threshold-sign one native nonce-bound response.
    ///
    /// `release` and `time` belong to the independently configured native server; they are not
    /// accepted from the request body. The process mutex spans retention and signing so a reply
    /// never races another local floor update. Exact retries issue a fresh nonce-bound response.
    ///
    /// # Errors
    /// Rejects forked ownership before locking, substituted/expired requests, sequence regression,
    /// same-sequence equivocation, uncertain storage, and unavailable/invalid signatures.
    pub(crate) fn issue(
        &self,
        bootstrap_archive: &[u8],
        request_nonce: [u8; 32],
        release: KagemushaBootstrapIssuerReleaseV1,
        time: KagemushaBootstrapIssuerTimeIntervalV1,
    ) -> Result<KagemushaMobileBootstrapFreshnessPackageV1> {
        if self.process_id != std::process::id() {
            return Err("bootstrap issuer belongs to another process".to_owned());
        }
        if request_nonce == [0; 32] {
            return Err("bootstrap issuer request nonce is zero".to_owned());
        }
        let package = KagemushaMobileBootstrapPackageV1::decode_canonical_exact(bootstrap_archive)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "bootstrap issuer is poisoned".to_owned())?;
        let retained = authenticate_current(
            &self.authority,
            &package,
            release,
            time,
            Some(state.retained),
        )?;
        let IssuerState { store, archive, .. } = &mut *state;
        store.require_current(archive)?;
        if retained != state.retained {
            state.store.replace(bootstrap_archive)?;
            state.retained = retained;
            state.archive = bootstrap_archive.to_vec();
        }
        let statement = KagemushaMobileBootstrapFreshnessStatementV1 {
            version: 1,
            request_nonce,
            authority_policy_digest: self
                .authority
                .policy
                .canonical_digest()
                .map_err(|_| "bootstrap issuer policy is invalid".to_owned())?,
            network_id: self.authority.network_id,
            scope: self.authority.scope,
            checkpoint_digest: retained.checkpoint_digest,
            retained_sequence: retained.sequence,
            authority_time_lower_ms: time.lower_ms,
            authority_time_upper_ms: time.upper_ms,
        };
        let mut approvals = Vec::with_capacity(self.signers.len());
        for signer in &self.signers {
            #[cfg(test)]
            self.signatures_created
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let approval = KagemushaMobileBootstrapFreshnessApprovalV1 {
                public_key: signer.public_key().clone(),
                signature: SignatureOf::try_new(
                    signer.private_key(),
                    &statement.approval_payload(),
                )
                .map_err(|_| "bootstrap issuer signing failed".to_owned())?,
            };
            approval.verify(&statement, &self.authority.policy)?;
            approvals.push(approval);
        }
        let reply = KagemushaMobileBootstrapFreshnessPackageV1 {
            statement,
            approvals,
        };
        reply.authenticate(&KagemushaMobileBootstrapFreshnessPinsV1 {
            authority_policy: &self.authority.policy,
            network_id: self.authority.network_id,
            scope: self.authority.scope,
            release_id: release.release_id,
            release_attestation_digest: release.release_attestation_digest,
            minimum_sequence: retained.sequence,
            previous: Some(retained),
            checkpoint: &package.checkpoint,
            request_nonce,
            native_elapsed_ms: 0,
        })?;
        let IssuerState { store, archive, .. } = &mut *state;
        store.require_current(archive)?;
        Ok(reply)
    }
}

fn validate_authority_signers(
    authority: &KagemushaBootstrapIssuerAuthorityV1,
    mut signers: Vec<KeyPair>,
) -> Result<Vec<KeyPair>> {
    authority
        .policy
        .canonical_digest()
        .map_err(|_| "bootstrap issuer policy is invalid".to_owned())?;
    let scope = authority.scope;
    if authority.network_id.as_bytes() == &[0; 32]
        || authority.network_id.as_bytes() == Hash::prehashed([0; 32]).as_ref()
        || scope.asset_identity_digest == [0; 32]
        || scope.asset_incarnation == [0; 32]
        || scope.liability_pool_id == [0; 32]
        || scope.asset_identity_digest == scope.liability_pool_id
        || scope.asset_scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
        || signers.len() < usize::from(authority.policy.threshold)
        || signers.len() > authority.policy.authorized_signers.len()
    {
        return Err("bootstrap issuer authority scope or signer count is invalid".to_owned());
    }
    signers.sort_by(|a, b| a.public_key().cmp(b.public_key()));
    if !signers
        .windows(2)
        .all(|pair| pair[0].public_key() < pair[1].public_key())
        || signers.iter().any(|key| {
            authority
                .policy
                .authorized_signers
                .binary_search(key.public_key())
                .is_err()
        })
    {
        return Err("bootstrap issuer keys are duplicated or outside its pinned policy".to_owned());
    }
    Ok(signers)
}

fn authenticate_current(
    authority: &KagemushaBootstrapIssuerAuthorityV1,
    package: &KagemushaMobileBootstrapPackageV1,
    release: KagemushaBootstrapIssuerReleaseV1,
    time: KagemushaBootstrapIssuerTimeIntervalV1,
    previous: Option<KagemushaMobileBootstrapReplayPinV1>,
) -> Result<KagemushaMobileBootstrapReplayPinV1> {
    if time.lower_ms == 0 || time.lower_ms > time.upper_ms {
        return Err("bootstrap issuer requires qualified ordered UTC bounds".to_owned());
    }
    let mut pins = KagemushaMobileBootstrapPinsV1 {
        authority_policy: &authority.policy,
        network_id: authority.network_id,
        scope: authority.scope,
        release_id: release.release_id,
        release_attestation_digest: release.release_attestation_digest,
        minimum_sequence: previous.map_or(1, |pin| pin.sequence),
        previous,
        trusted_now_ms: time.lower_ms,
    };
    let checkpoint_digest = package.authenticate(&pins)?;
    pins.trusted_now_ms = time.upper_ms;
    package.checkpoint.validate_pins(&pins)?;
    Ok(KagemushaMobileBootstrapReplayPinV1 {
        sequence: package.checkpoint.sequence,
        checkpoint_digest,
    })
}

#[cfg(all(test, unix))]
mod tests;
