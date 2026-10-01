//! Shared Native signer custody and real four-node current-clock transport.
//! These owners do not admit a runtime inventory, FI customer or monetary state by themselves.

use super::*;
use crate::participant_enrollment_request::{
    ParticipantEnrollmentOperationV1, ParticipantEnrollmentRequestV1,
    VerifiedEnrollmentWalletSignatoryV1, VerifiedParticipantEnrollmentRequestV1,
};
use iroha_core_zk::kagemusha_v1_state::{
    KagemushaOrdinaryAppPossessionAttemptV1, KagemushaOrdinaryNativeClockOwnerV1,
    KagemushaOrdinaryRetailEnrollmentAttemptV1, KagemushaPendingAppIdentityV1,
};
use iroha_data_model::kagemusha::KagemushaOrdinaryRetailEnrollmentChallengeV1;
use iroha_primitives::time::NativeContinuousReading;
use std::sync::Mutex;

mod current_wallet;
mod inventory;
pub use inventory::{
    KagemushaAdmittedOrdinaryNativeInventoryV1, KagemushaNativeInstalledRuntimeAuthorityV1,
    KagemushaOrdinaryNativeArtifactResolverV1, KagemushaOrdinaryNativeInventoryV1,
    KagemushaOrdinaryNativeNodeTargetV1, KagemushaOrdinaryNativeOriginalDescriptorV1,
    assemble_kagemusha_ordinary_native_inventory_v1,
};

/// Complete untrusted current account cut returned by a Native original-data transport.
/// The full World snapshot and all four original signatures remain mandatory; this grants no root.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
pub struct KagemushaOrdinaryNativeCurrentWalletOriginalV1 {
    /// Exact same already retained canonical certified finality proof.
    pub proof: Vec<u8>,
    /// Complete current World snapshot bound by that certified applied cut.
    pub world_snapshot: iroha_data_model::sumeragi_finality::WorldStateSnapshotV1,
    /// Full exact current S account value, checked against complete World.
    pub signatory_value: iroha_data_model::account::AccountValue,
    /// Full exact current W account value, checked against complete World.
    pub wallet_value: iroha_data_model::account::AccountValue,
    /// Four exact same-request nonce-bound actual Native statements, in installed node order.
    pub statements: [iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation; 4],
}

/// Fresh actual Native account-startup read retained before any response; no decoder or clone.
pub struct KagemushaNativeCurrentWalletReadV1 {
    account: AccountClient,
    inventory: Arc<KagemushaAdmittedOrdinaryNativeInventoryV1>,
    clock: Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>,
    challenge: crate::participant_enrollment_request::EnrollmentWalletReadChallengeV1,
    signatory: AccountId,
    height: u64,
}
impl KagemushaNativeCurrentWalletReadV1 {
    /// Generate a finite current read for the actual retained Native account under admitted runtime.
    /// # Errors
    /// Refuses changed installed custody, unavailable fresh clock or invalid actual S/W/native key.
    pub fn reserve(
        account: AccountClient,
        inventory: Arc<KagemushaAdmittedOrdinaryNativeInventoryV1>,
        clock: Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>,
    ) -> Result<Self> {
        inventory.require_account_transport(&account)?;
        let signatory = AccountId::new(account.context.key_pair.public_key().clone());
        let challenge=crate::participant_enrollment_request::EnrollmentWalletReadChallengeV1::for_native_wallet_selection(
            *account.network_id(),&signatory,account.authority())?;
        let height = clock
            .lock()
            .map_err(|_| eyre!("Native clock owner unavailable"))?
            .current_certified_height()
            .map_err(|_| eyre!("Native current prefix unavailable"))?;
        Ok(Self {
            account,
            inventory,
            clock,
            challenge,
            signatory,
            height,
        })
    }
    /// Native-generated public transport nonce only; possession does not grant an account owner.
    #[must_use]
    pub fn nonce(&self) -> [u8; 32] {
        self.challenge.bytes()
    }
    /// Exact Native selected S for transport; it cannot replace the retained key.
    #[must_use]
    pub fn signatory(&self) -> &AccountId {
        &self.signatory
    }
    /// Exact Native selected W for transport; it cannot replace the retained account.
    #[must_use]
    pub fn wallet(&self) -> &AccountId {
        self.account.authority()
    }
    /// Authenticate complete original current cut under the same installed clock/finality owner.
    /// # Errors
    /// Refuses any current root/schema/node/nonce/account/value/key substitution or elapsed read.
    pub fn authenticate(self, original: &[u8]) -> Result<VerifiedEnrollmentWalletSignatoryV1> {
        const MAX: usize = 64 * 1024 * 1024;
        ensure!(
            !original.is_empty() && original.len() <= MAX,
            "Native current wallet original bound rejected"
        );
        self.inventory.recheck()?;
        let raw: KagemushaOrdinaryNativeCurrentWalletOriginalV1 =
            norito::decode_canonical_with_limits(original, norito::canonical_decode_limits(MAX))?;
        ensure!(
            norito::encode_canonical(&raw)? == original,
            "Native current wallet original changed canonical bytes"
        );
        self.authenticate_typed(raw)
    }

    fn authenticate_typed(
        self,
        raw: KagemushaOrdinaryNativeCurrentWalletOriginalV1,
    ) -> Result<VerifiedEnrollmentWalletSignatoryV1> {
        self.challenge.remaining_native_budget()?;
        self.inventory.recheck()?;
        let proof: iroha_data_model::sumeragi_finality::SumeragiFinalityProof =
            norito::decode_canonical_with_limits(
                &raw.proof,
                norito::canonical_decode_limits(raw.proof.len()),
            )?;
        ensure!(
            norito::encode_canonical(&proof)? == raw.proof,
            "Native current proof changed canonical bytes"
        );
        let verifier = self
            .clock
            .lock()
            .map_err(|_| eyre!("Native clock owner unavailable"))?
            .current_finality_verifier()
            .map_err(|_| eyre!("Native current finality custody rejected"))?;
        ensure!(
            proof.height() == self.height,
            "Native current wallet prefix changed after reservation"
        );
        let block = verifier.verify_retained_decision(&proof)?;
        let snapshot = raw.world_snapshot.authenticate(&block)?;
        let current = VerifiedEnrollmentWalletSignatoryV1::authenticate(
            self.challenge,
            *self.account.network_id(),
            &self.inventory.membership_nodes(),
            &verifier,
            &proof,
            snapshot,
            self.inventory.world_schema_hash(),
            &raw.statements,
            self.signatory,
            &raw.signatory_value,
            self.account.authority().clone(),
            &raw.wallet_value,
        )?;
        self.inventory.recheck()?;
        current.recheck()?;
        Ok(current)
    }
}

/// Actual immutable Native account signer joined to current certified S/W membership.
///
/// The account context retains the real private key inside Native; this holder has no decoder,
/// key export, signature-callback constructor or clone. A signature supplied by managed code
/// cannot manufacture it. The original membership observation remains finite and must be
/// refreshed with another genuine current cut before it expires. A Bridge session registry
/// separately serializes uses and invalidates the selected account after cancellation/switch.
pub struct KagemushaNativeAccountCustodyV1 {
    account: AccountClient,
    current: VerifiedEnrollmentWalletSignatoryV1,
}
impl KagemushaNativeAccountCustodyV1 {
    /// Retain the real native account key only after independently verified current membership.
    /// The original current read must authenticate the exact S key, W and network.
    /// # Errors
    /// Refuses expired evidence, foreign account/key/network or an unsupported W controller.
    pub fn from_current_wallet(
        account: AccountClient,
        current: VerifiedEnrollmentWalletSignatoryV1,
    ) -> Result<Self> {
        let this = Self { account, current };
        this.recheck()?;
        Ok(this)
    }

    /// Replace only the current certified observation for the same original Native S/W/key.
    /// This refresh creates neither a new account nor a renewed historical FI challenge.
    /// # Errors
    /// Refuses a foreign current account/key/network or expired new observation.
    pub fn refresh_current_wallet(
        &mut self,
        current: VerifiedEnrollmentWalletSignatoryV1,
    ) -> Result<()> {
        current.recheck()?;
        ensure!(
            current.signatory() == self.current.signatory()
                && current.wallet() == self.current.wallet(),
            "native wallet refresh changed S/W"
        );
        Self::require_native_key(&self.account, &current)?;
        self.current = current;
        self.recheck()
    }

    /// Recheck the same held current read and Native account/key relationships.
    /// This is identity custody only; FI revocation and current PI remain separate checks.
    /// # Errors
    /// Refuses expired original membership or substituted Native account/key/network.
    pub fn recheck(&self) -> Result<()> {
        self.current.recheck()?;
        Self::require_native_key(&self.account, &self.current)
    }

    fn require_native_key(
        account: &AccountClient,
        current: &VerifiedEnrollmentWalletSignatoryV1,
    ) -> Result<()> {
        current.recheck()?;
        ensure!(
            account.authority() == current.wallet()
                && account.network_id() == current.network_id()
                && account.signing_capability() == AccountSigningCapability::MultisigMember
                && account.context.key_pair.public_key().algorithm()
                    == iroha_crypto::Algorithm::Ed25519
                && current.signatory().try_signatory()
                    == Some(account.context.key_pair.public_key()),
            "native account holder differs from certified exact S/W/key/network"
        );
        Ok(())
    }

    /// Exact retained W; this data projection cannot recreate custody or grant money.
    #[must_use]
    pub fn wallet(&self) -> &AccountId {
        self.current.wallet()
    }
    /// Exact retained S for Native session identity; this grants no installation authority.
    #[must_use]
    pub fn signatory(&self) -> &AccountId {
        self.current.signatory()
    }

    /// Sign the sole actual C20 FI challenge after its genuine WAL invocation fence, then
    /// durably retain Ed64 before exposing it. Recovery returns only the same retained original.
    /// A crash after the fence leaves the original unknown and cannot invoke the wallet again.
    /// # Errors
    /// Refuses stale custody, wrong challenge/account/network, unknown fence, changed digest or
    /// failed exact original verification/durability. No caller message is signed directly.
    pub fn sign_retained_retail_enrollment(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
        attempt: &mut KagemushaOrdinaryRetailEnrollmentAttemptV1,
    ) -> Result<[u8; 64]> {
        self.recheck()?;
        let fields = attempt
            .preparation_fields(pending, possession)
            .map_err(|_| eyre!("native retail challenge custody rejected"))?;
        ensure!(
            fields.len() == 5 && !fields[1].is_empty() && fields[1].len() <= 40 * 1024,
            "native retail challenge shape rejected"
        );
        let challenge: KagemushaOrdinaryRetailEnrollmentChallengeV1 =
            norito::decode_canonical_with_limits(
                &fields[1],
                norito::canonical_decode_limits(40 * 1024),
            )
            .map_err(|_| eyre!("native retail challenge is not canonical"))?;
        ensure!(
            norito::encode_canonical(&challenge)? == fields[1]
                && challenge.owner.account_id == *self.current.wallet()
                && &challenge.owner.runtime.network_id == self.account.network_id(),
            "native retail challenge changed original W/network"
        );
        let message = challenge
            .account_signing_message()
            .map_err(|_| eyre!("native retail signing subject rejected"))?;
        ensure!(
            fields[2].as_slice() == message.as_slice(),
            "native retail signing digest changed"
        );
        self.recheck()?;
        let fence = attempt
            .fence(pending, possession)
            .map_err(|_| eyre!("native retail invocation is unavailable or uncertain"))?;
        let raw: [u8; 64] = match fence.as_slice() {
            [tag, empty] if tag.as_slice() == [1] && empty.is_empty() => {
                // Recheck after the fsynced fence. Refusal leaves uncertainty closed; it never
                // rewinds the fence or creates another wallet invocation.
                self.recheck()?;
                iroha_crypto::Signature::try_new(
                    self.account.context.key_pair.private_key(),
                    &message,
                )?
                .payload()
                .try_into()
                .map_err(|_| eyre!("native wallet Ed64 shape rejected"))?
            }
            [tag, original] if tag.as_slice() == [2] => original
                .as_slice()
                .try_into()
                .map_err(|_| eyre!("native retained wallet Ed64 shape rejected"))?,
            _ => return Err(eyre!("native retail invocation frame rejected")),
        };
        // The Native journal verifies the actual account controller over its exact HashOf
        // subject and platform possession. There is no caller signature/digest authority.
        attempt
            .retain_account_signature(pending, possession, raw)
            .map_err(|_| eyre!("native retail account original durability rejected"))?;
        self.recheck()?;
        Ok(raw)
    }

    /// Prepare the exact enrollment HTTP signing subject with the actual Native clock and
    /// fresh Native request entropy. Managed metadata supplies no time, network, S or W.
    /// The returned originals remain move-only and bound to this same actual account context.
    /// # Errors
    /// Refuses stale wallet/clock custody, malformed FI context or unavailable Native entropy.
    pub fn prepare_enrollment_request(
        &self,
        context: KagemushaNativeEnrollmentRequestContextV1,
        clock: Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>,
    ) -> Result<KagemushaNativePreparedEnrollmentRequestV1> {
        self.recheck()?;
        let timestamp_ms = {
            let mut owner = clock
                .lock()
                .map_err(|_| eyre!("native clock owner lock unavailable"))?;
            ensure!(
                &owner
                    .network_id()
                    .map_err(|_| eyre!("native clock custody rejected"))?
                    == self.account.network_id(),
                "native request clock changed installed network"
            );
            owner
                .current_native_time_interval()
                .map_err(|_| eyre!("native current clock unavailable"))?
                .lower_ms()
        };
        let entropy = rand::random::<[u8; 32]>();
        ensure!(entropy != [0; 32], "native request entropy unavailable");
        let prepared = KagemushaNativePreparedEnrollmentRequestV1 {
            account: self.account.context.clone(),
            clock,
            context,
            timestamp_ms,
            nonce: hex::encode(entropy),
            started: NativeContinuousReading::now()?,
            signatory: self.current.signatory().clone(),
            wallet: self.current.wallet().clone(),
        };
        prepared.request().signing_message()?;
        prepared.recheck(&self.account)?;
        self.recheck()?;
        Ok(prepared)
    }

    /// Sign the exact Native-prepared request with this real Native key, then consume only its
    /// matching current certified read. Another body/session/clock/account cannot reuse it.
    /// # Errors
    /// Refuses another Native holder or current read, expired request/clock or altered original.
    pub fn sign_current_enrollment_request(
        &self,
        prepared: KagemushaNativePreparedEnrollmentRequestV1,
        current: VerifiedEnrollmentWalletSignatoryV1,
    ) -> Result<(
        iroha_crypto::Signature,
        VerifiedParticipantEnrollmentRequestV1,
    )> {
        self.recheck()?;
        prepared.recheck(&self.account)?;
        Self::require_native_key(&self.account, &current)?;
        ensure!(
            current.signatory() == self.current.signatory()
                && current.wallet() == self.current.wallet(),
            "native request read changed original S/W"
        );
        let request = prepared.request();
        let message = request.signing_message()?;
        self.recheck()?;
        prepared.recheck(&self.account)?;
        let signature = iroha_crypto::Signature::try_new(
            self.account.context.key_pair.private_key(),
            &message,
        )?;
        let verified = current.verify_request(&request, &signature)?;
        prepared.recheck(&self.account)?;
        self.recheck()?;
        Ok((signature, verified))
    }
}

/// Public enrollment context only; clocks, native account/key and network are absent.
/// These bounded fields select the exact FI request purpose and original body, not authority.
#[derive(Debug)]
pub struct KagemushaNativeEnrollmentRequestContextV1 {
    /// Exact owner-admitted FI namespace.
    pub authentication_namespace: String,
    /// Exact currently authenticated FI customer/session actor.
    pub actor_id: String,
    /// Exact authenticated FI session digest; this grants no Native session by itself.
    pub session_sha256: [u8; 32],
    /// Original request identity retained through uncertain replies.
    pub request_id: String,
    /// Original immutable business retry identity.
    pub idempotency_key: String,
    /// Fixed first-release enrollment purpose.
    pub operation: ParticipantEnrollmentOperationV1,
    /// Actual independently selected complete HTTPS FI target.
    pub target: Url,
    /// Exact complete original request body, never reconstructed after signing.
    pub body: Vec<u8>,
}

/// Native-generated exact request, bound to the real account context and actual clock owner.
/// It has no decoder, caller timestamp constructor or clone. Its public request projection
/// provides transport data and the read challenge subject, without reconstituting this holder.
pub struct KagemushaNativePreparedEnrollmentRequestV1 {
    account: Arc<Client>,
    clock: Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>,
    context: KagemushaNativeEnrollmentRequestContextV1,
    timestamp_ms: u64,
    nonce: String,
    started: NativeContinuousReading,
    signatory: AccountId,
    wallet: AccountId,
}
impl KagemushaNativePreparedEnrollmentRequestV1 {
    /// Sole original data projection to create the exact current wallet-read challenge/HTTP wire.
    #[must_use]
    pub fn request(&self) -> ParticipantEnrollmentRequestV1<'_> {
        ParticipantEnrollmentRequestV1 {
            network_id: &self.account.network_id,
            authentication_namespace: &self.context.authentication_namespace,
            actor_id: &self.context.actor_id,
            session_sha256: self.context.session_sha256,
            signatory: &self.signatory,
            wallet: &self.wallet,
            request_id: &self.context.request_id,
            idempotency_key: &self.context.idempotency_key,
            operation: self.context.operation,
            target: &self.context.target,
            body: &self.context.body,
            timestamp_ms: self.timestamp_ms,
            nonce: &self.nonce,
        }
    }
    fn recheck(&self, account: &AccountClient) -> Result<()> {
        ensure!(
            Arc::ptr_eq(&self.account, &account.context)
                && self.started.elapsed()? < std::time::Duration::from_secs(10),
            "native enrollment original account or finite lifetime changed"
        );
        let mut clock = self
            .clock
            .lock()
            .map_err(|_| eyre!("native clock owner lock unavailable"))?;
        ensure!(
            &clock
                .network_id()
                .map_err(|_| eyre!("native clock custody rejected"))?
                == account.network_id(),
            "native enrollment clock network changed"
        );
        let interval = clock
            .current_native_time_interval()
            .map_err(|_| eyre!("native current clock unavailable"))?;
        let (low, high) = self.request().native_time_window()?;
        interval
            .require_validity(low, high)
            .map_err(|_| eyre!("native enrollment clock left original signing window"))?;
        Ok(())
    }
}

/// Closed progress result: this Native call fsynced genuine certified successors but could
/// not reach the current tip within its finite work/time budget. Retry through a new Native
/// request; the result carries no checkpoint, signed clock, session or monetary authority.
#[derive(Debug)]
pub struct KagemushaNativeClockCatchupRequiredV1 {
    /// Number of actually verified/fsynced immediate successors during this call.
    pub verified_successors: u64,
}
impl std::fmt::Display for KagemushaNativeClockCatchupRequiredV1 {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            out,
            "Native certified prefix advanced by {} successors; a fresh bounded read is required",
            self.verified_successors
        )
    }
}
impl std::error::Error for KagemushaNativeClockCatchupRequiredV1 {}

fn clock_catchup_step(distance: u64, advanced: u64) -> Result<u64> {
    let step = distance.min(256_u64.saturating_sub(advanced));
    if step == 0 {
        return Err(eyre!(KagemushaNativeClockCatchupRequiredV1 {
            verified_successors: advanced
        }));
    }
    Ok(step)
}
fn remaining_clock_budget(
    started: NativeContinuousReading,
    budget: std::time::Duration,
    advanced: u64,
) -> Result<std::time::Duration> {
    budget
        .checked_sub(started.elapsed()?)
        .filter(|v| !v.is_zero())
        .ok_or_else(|| {
            if advanced > 0 {
                eyre!(KagemushaNativeClockCatchupRequiredV1 {
                    verified_successors: advanced
                })
            } else {
                eyre!("Native clock refresh elapsed budget exhausted")
            }
        })
}

/// Real shared SDK transport over four retained immutable Native client contexts.
///
/// This selects no root from a reply and accepts no caller timestamp or transport callback.
/// The independently admitted runtime owner supplies the four contexts and retains their
/// authorization; the CoreZK clock owner supplies the exact current height, pins and nonce.
/// Every response is reverified and fsynced by that same actual clock owner before any time loan.
pub struct KagemushaNativeClockTransportV1 {
    nodes: [Client; 4],
}
impl KagemushaNativeClockTransportV1 {
    /// Retain four actual configured Native transports for the same independently selected network.
    /// This shape check does not authenticate SDK/runtime inventory or grant a session.
    /// # Errors
    /// Refuses a foreign network, invalid endpoint or non-HTTPS selected transport.
    pub fn from_native_clients(
        nodes: [Client; 4],
        clock: &Mutex<KagemushaOrdinaryNativeClockOwnerV1>,
    ) -> Result<Self> {
        let network = clock
            .lock()
            .map_err(|_| eyre!("native clock owner lock unavailable"))?
            .network_id()
            .map_err(|_| eyre!("native clock custody rejected"))?;
        for node in &nodes {
            node.validate_context_endpoint()?;
            ensure!(
                node.network_id == network && node.torii_url.scheme() == "https",
                "native clock transport changed installed network or HTTPS endpoint"
            );
        }
        Ok(Self { nodes })
    }

    /// Generate the real Native nonce, contact the four selected validators and consume only
    /// their exact signed originals within the one suspend-inclusive response budget.
    /// No HTTP/error body is a clock, and a failed read never reopens or renews its original nonce.
    /// # Errors
    /// Refuses unavailable clock custody, transport failure, node/root/nonce/runtime substitution,
    /// stale replies, skew/regression, account switch or failed durable admission.
    pub fn refresh_current_clock(
        &self,
        clock: &Mutex<KagemushaOrdinaryNativeClockOwnerV1>,
    ) -> Result<()> {
        let started = NativeContinuousReading::now()?;
        let overall_budget = std::time::Duration::from_secs(10);
        let remaining = |advanced: u64| remaining_clock_budget(started, overall_budget, advanced);
        let mut advances = 0_u64;
        loop {
            remaining(advances)?;
            let read = clock
                .lock()
                .map_err(|_| eyre!("native clock owner lock unavailable"))?
                .reserve_current_read()
                .map_err(|_| eyre!("native clock reservation rejected"))?;
            let mut originals = Vec::with_capacity(4);
            let mut retry = false;
            for (index, client) in self.nodes.iter().enumerate() {
                let (height, nodes, read_budget) = clock
                    .lock()
                    .map_err(|_| eyre!("native clock owner lock unavailable"))?
                    .current_read_targets(&read)
                    .map_err(|_| eyre!("native clock original read expired"))?;
                let budget = remaining(advances)?.min(read_budget);
                let deadline = std::time::Instant::now()
                    .checked_add(budget)
                    .ok_or_else(|| eyre!("native transport deadline overflow"))?;
                match client
                    .with_request_deadline(deadline)
                    .get_sumeragi_finality_attestation(height, read.nonce(), &nodes[index].peer_id)
                {
                    Ok(reply) => originals.push(norito::encode_canonical(&reply)?),
                    Err(error) => {
                        let Some(progress) =
                            error.downcast_ref::<BridgeFinalityAttestationTipMismatch>()
                        else {
                            return Err(error);
                        };
                        // The request-bound HTTP hint supplies routing only. Every admitted height
                        // is the immediate original certified successor under the retained verifier.
                        let target = progress.response().applied_height;
                        let distance = target
                            .checked_sub(height.get())
                            .filter(|distance| *distance > 0)
                            .ok_or_else(|| eyre!("native tip hint did not advance"))?;
                        let step = clock_catchup_step(distance, advances)?;
                        for offset in 1..=step {
                            let next = height
                                .get()
                                .checked_add(offset)
                                .and_then(std::num::NonZeroU64::new)
                                .ok_or_else(|| eyre!("native prefix height overflow"))?;
                            let deadline = std::time::Instant::now()
                                .checked_add(remaining(advances)?)
                                .ok_or_else(|| eyre!("native prefix deadline overflow"))?;
                            let proof = match client
                                .with_request_deadline(deadline)
                                .get_sumeragi_finality_proof(next)
                            {
                                Ok(proof) => proof,
                                Err(error) => {
                                    remaining(advances)?;
                                    return Err(error);
                                }
                            };
                            let original = norito::encode_canonical(&proof)?;
                            remaining(advances)?;
                            clock
                                .lock()
                                .map_err(|_| eyre!("native clock owner lock unavailable"))?
                                .advance_certified_prefix(&original)
                                .map_err(|_| eyre!("Native certified prefix successor rejected"))?;
                            advances += 1;
                            remaining(advances)?;
                        }
                        if step < distance {
                            return Err(eyre!(KagemushaNativeClockCatchupRequiredV1 {
                                verified_successors: advances
                            }));
                        }
                        retry = true;
                        break;
                    }
                }
                remaining(advances)?;
                clock
                    .lock()
                    .map_err(|_| eyre!("native clock owner lock unavailable"))?
                    .current_read_targets(&read)
                    .map_err(|_| eyre!("native clock original read expired"))?;
            }
            if retry {
                continue;
            } // Abandon the old nonce; a fresh Native read binds the new prefix.
            let originals: [Vec<u8>; 4] = originals
                .try_into()
                .map_err(|_| eyre!("native clock original count rejected"))?;
            remaining(advances)?;
            clock
                .lock()
                .map_err(|_| eyre!("native clock owner lock unavailable"))?
                .admit_current_read(read, originals)
                .map_err(|_| eyre!("native clock signed originals rejected"))?;
            remaining(advances)?;
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::participant_enrollment_request::NativeCustodyFixture;

    #[test]
    fn cold_clock_catchup_returns_closed_progress_instead_of_permanent_distance_refusal() {
        assert_eq!(clock_catchup_step(10_000, 0).unwrap(), 256);
        assert_eq!(clock_catchup_step(10_000, 250).unwrap(), 6);
        assert_eq!(clock_catchup_step(3, 250).unwrap(), 3);
        let error = clock_catchup_step(1, 256).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<KagemushaNativeClockCatchupRequiredV1>()
                .unwrap()
                .verified_successors,
            256
        );
        let reading = NativeContinuousReading::now().unwrap();
        let error = remaining_clock_budget(reading, std::time::Duration::ZERO, 17).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<KagemushaNativeClockCatchupRequiredV1>()
                .unwrap()
                .verified_successors,
            17
        );
        assert!(
            remaining_clock_budget(reading, std::time::Duration::ZERO, 0)
                .unwrap_err()
                .downcast_ref::<KagemushaNativeClockCatchupRequiredV1>()
                .is_none()
        );
        // A fresh actual call has another work budget and resumes the already durable prefix.
        assert_eq!(clock_catchup_step(9_744, 0).unwrap(), 256);
    }
    fn context(fixture: &NativeCustodyFixture) -> AccountClient {
        let mut client = super::super::evidence_http_tests::client_with_base_url(
            Url::parse("https://mock.local/").unwrap(),
        );
        client.key_pair = fixture.key().clone();
        client.account = fixture.wallet().clone();
        client.network_id = fixture.network();
        client.account_client().unwrap()
    }
    fn request_context() -> KagemushaNativeEnrollmentRequestContextV1 {
        KagemushaNativeEnrollmentRequestContextV1 {
            authentication_namespace: "leumi.is2".into(),
            actor_id: "fixture-retail-actor".into(),
            session_sha256: [7; 32],
            request_id: "fixture-request".into(),
            idempotency_key: "fixture-stable-attempt".into(),
            operation: ParticipantEnrollmentOperationV1::Prepare,
            target: Url::parse(
                "https://fi.example.invalid/leumi.is2/v1/offline/enrollment/ordinary/prepare",
            )
            .unwrap(),
            body: b"exact-native-prepared-json".to_vec(),
        }
    }
    fn initial_current(fixture: &NativeCustodyFixture) -> VerifiedEnrollmentWalletSignatoryV1 {
        let context = request_context();
        let network = fixture.network();
        fixture.current(&ParticipantEnrollmentRequestV1 {
            network_id: &network,
            authentication_namespace: &context.authentication_namespace,
            actor_id: &context.actor_id,
            session_sha256: context.session_sha256,
            signatory: fixture.signatory(),
            wallet: fixture.wallet(),
            request_id: &context.request_id,
            idempotency_key: &context.idempotency_key,
            operation: context.operation,
            target: &context.target,
            body: &context.body,
            timestamp_ms: 1_000_000,
            nonce: "fixture-fresh-native-custody-nonce-0001",
        })
    }
    #[test]
    fn actual_native_key_signs_only_same_current_s_w_and_native_clock_prepared_request() {
        let fixture = NativeCustodyFixture::new();
        let custody = KagemushaNativeAccountCustodyV1::from_current_wallet(
            context(&fixture),
            initial_current(&fixture),
        )
        .unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let clock = Arc::new(Mutex::new(fixture.clock(temporary.path())));
        let prepared = custody
            .prepare_enrollment_request(request_context(), clock)
            .unwrap();
        let request = prepared.request();
        let message = request.signing_message().unwrap();
        let body = request.body.to_vec();
        let current = fixture.current(&request);
        let (signature, verified) = custody
            .sign_current_enrollment_request(prepared, current)
            .unwrap();
        signature
            .verify(fixture.key().public_key(), &message)
            .unwrap();
        assert_eq!(verified.wallet(), fixture.wallet());
        assert_eq!(verified.signatory(), fixture.signatory());
        verified.verify_original_body(&body).unwrap();
        assert!(verified.verify_original_body(b"changed-body").is_err());
    }
    #[test]
    fn certified_current_wallet_cannot_create_custody_for_another_native_key_or_context() {
        let fixture = NativeCustodyFixture::new();
        let mut wrong = context(&fixture).context.as_ref().clone();
        wrong.key_pair =
            iroha_crypto::KeyPair::from_seed(vec![99; 32], iroha_crypto::Algorithm::Ed25519);
        wrong.account = AccountId::new_multisig(
            iroha_data_model::account::MultisigPolicy::new(
                1,
                vec![
                    iroha_data_model::account::MultisigMember::new(
                        wrong.key_pair.public_key().clone(),
                        1,
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        assert!(
            KagemushaNativeAccountCustodyV1::from_current_wallet(
                wrong.account_client().unwrap(),
                initial_current(&fixture)
            )
            .is_err()
        );
        let custody = KagemushaNativeAccountCustodyV1::from_current_wallet(
            context(&fixture),
            initial_current(&fixture),
        )
        .unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let prepared = custody
            .prepare_enrollment_request(
                request_context(),
                Arc::new(Mutex::new(fixture.clock(temporary.path()))),
            )
            .unwrap();
        // A separately constructed same-key client is another Native holder; identical DTOs
        // cannot replace the Arc identity retained before preparation.
        assert!(prepared.recheck(&context(&fixture)).is_err());
    }
    #[test]
    fn native_prepared_request_expires_even_if_its_signed_clock_projection_remains_live() {
        let fixture = NativeCustodyFixture::new();
        let custody = KagemushaNativeAccountCustodyV1::from_current_wallet(
            context(&fixture),
            initial_current(&fixture),
        )
        .unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let clock = Arc::new(Mutex::new(fixture.clock(temporary.path())));
        let prepared = custody
            .prepare_enrollment_request(request_context(), clock.clone())
            .unwrap();
        // A live signed clock cannot renew this finite Native-generated request holder.
        // The elapsed budget includes actual platform suspension without copying Unix time.
        std::thread::sleep(std::time::Duration::from_secs(10));
        assert!(prepared.recheck(&custody.account).is_err());
    }
}
