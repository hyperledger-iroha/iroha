//! Shared Native signer custody and real four-node current-clock transport.
//! These owners do not admit a runtime inventory, FI customer or monetary state by themselves.

use super::*;
use crate::participant_enrollment_request::{
    ParticipantEnrollmentOperationV1, ParticipantEnrollmentRequestV1,
    VerifiedEnrollmentWalletSignatoryV1, VerifiedParticipantEnrollmentRequestV1,
};
use eyre::ensure;
use iroha_core_zk::kagemusha_v1_state::{
    KagemushaOrdinaryAppPossessionAttemptV1, KagemushaOrdinaryNativeClockOwnerV1,
    KagemushaOrdinaryRetailEnrollmentAttemptV1, KagemushaPendingAppIdentityV1,
};
use iroha_data_model::kagemusha::KagemushaOrdinaryRetailEnrollmentChallengeV1;
use iroha_primitives::time::NativeContinuousReading;
use std::sync::Mutex;

#[path = "ordinary_native/public_clock.rs"]
mod public_clock;
use public_clock::ClockNodes;

mod current_wallet;
mod endpoint;
mod inventory;
pub use inventory::{
    KagemushaAdmittedOrdinaryNativeInventoryV1, KagemushaNativeInstalledRuntimeAuthorityV1,
    KagemushaNativeOrdinaryInstalledContextV1, KagemushaOrdinaryNativeArtifactResolverV1,
    KagemushaOrdinaryNativeInventoryV1, KagemushaOrdinaryNativeNodeTargetV1,
    KagemushaOrdinaryNativeOriginalDescriptorV1,
    assemble_kagemusha_ordinary_native_clock_selection_v1,
    assemble_kagemusha_ordinary_native_inventory_v1,
};

/// Complete untrusted current account cut returned by a Native original-data transport.
/// The full World snapshot and all four original signatures remain mandatory; this grants no root.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::KagemushaOrdinaryNativeCurrentWalletOriginalV1")]
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

/// Fresh actual Native current account read retained before any response; no decoder or clone.
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
    // A request read uses the same move-only preparation and actual account/clock context.
    // The startup read domain intentionally cannot stand in for this signing subject.
    fn reserve_for_enrollment_request(
        account: AccountClient,
        inventory: Arc<KagemushaAdmittedOrdinaryNativeInventoryV1>,
        prepared: &KagemushaNativePreparedEnrollmentRequestV1,
    ) -> Result<Self> {
        inventory.require_account_transport(&account)?;
        let challenge = prepared.reserve_wallet_read_challenge(&account)?;
        let clock = prepared.clock.clone();
        let height = require_installed_request_clock(&inventory, &clock, None)?;
        prepared.recheck(&account)?;
        inventory.recheck()?;
        Ok(Self {
            account,
            inventory,
            clock,
            challenge,
            signatory: prepared.signatory.clone(),
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

// Join only the genuine admitted inventory and the same held Native clock. The optional height
// is copied from the read reservation, never supplied by a public DTO or offered response.
fn require_installed_request_clock(
    inventory: &KagemushaAdmittedOrdinaryNativeInventoryV1,
    clock: &Mutex<KagemushaOrdinaryNativeClockOwnerV1>,
    reserved_height: Option<u64>,
) -> Result<u64> {
    let expected = inventory.clock_originals()?;
    let mut actual = clock
        .lock()
        .map_err(|_| eyre!("Native request clock owner unavailable"))?;
    ensure!(
        actual
            .installed_selection_digest()
            .map_err(|_| eyre!("Native request clock custody rejected"))?
            == expected.selection_digest(),
        "Native request clock differs from installed original selection"
    );
    let height = actual
        .current_certified_height()
        .map_err(|_| eyre!("Native request current prefix unavailable"))?;
    ensure!(
        reserved_height.is_none_or(|original| height == original),
        "Native request current prefix changed after reservation"
    );
    drop(actual);
    inventory.recheck()?;
    Ok(height)
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

    /// Authenticate the immutable retained Native key/S/W/network relationships only.
    /// The original certified read was admitted at construction. This projection grants
    /// no current membership, signature, financial control or State effect; those paths
    /// continue to call `recheck` and require a genuinely refreshed current read.
    /// # Errors
    /// Refuses a changed Native key, account, controller capability or network.
    pub fn recheck_retained_identity(&self) -> Result<()> {
        Self::require_native_key_identity(&self.account, &self.current)
    }

    fn require_native_key(
        account: &AccountClient,
        current: &VerifiedEnrollmentWalletSignatoryV1,
    ) -> Result<()> {
        current.recheck()?;
        Self::require_native_key_identity(account, current)
    }

    fn require_native_key_identity(
        account: &AccountClient,
        current: &VerifiedEnrollmentWalletSignatoryV1,
    ) -> Result<()> {
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

    /// Sign only a closed, durably fenced Main/CAS invocation with the same actual Native account.
    /// No offered request, key callback or managed frame can create the required signing borrow.
    /// The Main CAS journal retains the resulting exact Ed64 before transport exposure.
    /// # Errors
    /// Refuses stale custody, another installed purpose, wrong W/key/network or changed invocation.
    pub fn sign_retained_lineage_request(
        &self,
        inventory: &KagemushaAdmittedOrdinaryNativeInventoryV1,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'_>,
    ) -> Result<[u8; 64]> {
        self.recheck()?;
        inventory.require_account_transport(&self.account)?;
        original
            .recheck()
            .map_err(|_| eyre!("Native lineage invocation custody rejected"))?;
        let request = original
            .request()
            .map_err(|_| eyre!("Native lineage original request rejected"))?
            .clone();
        inventory.require_lineage_request(&request)?;
        ensure!(
            original
                .installed_policy_original()
                .map_err(|_| eyre!("Native lineage purpose custody rejected"))?
                == inventory.lineage_policy_original()?.as_slice()
                && request.operation.lineage().owner.account_id == *self.current.wallet()
                && &request.operation.lineage().owner.runtime.network_id
                    == self.account.network_id(),
            "Native lineage invocation changed exact installed purpose/W/network"
        );
        let message = original
            .account_signing_message()
            .map_err(|_| eyre!("Native lineage message custody rejected"))?;
        ensure!(
            message
                == request
                    .account_signing_message()
                    .map_err(|_| eyre!("Native lineage message rejected"))?,
            "Native lineage invocation message changed"
        );
        self.recheck()?;
        original
            .recheck()
            .map_err(|_| eyre!("Native lineage invocation expired before signing"))?;
        let signature: [u8; 64] = iroha_crypto::Signature::try_new(
            self.account.context.key_pair.private_key(),
            &message,
        )?
        .payload()
        .try_into()
        .map_err(|_| eyre!("Native lineage Ed64 shape rejected"))?;
        request
            .verify_account_signature(&iroha_crypto::Signature::from_bytes(&signature))
            .map_err(|_| eyre!("Native lineage signature differs from exact account controller"))?;
        original
            .recheck()
            .map_err(|_| eyre!("Native lineage custody changed after signing"))?;
        inventory.require_lineage_request(&request)?;
        self.recheck()?;
        Ok(signature)
    }

    /// Sign the actual Main-retained unsigned Mint request after its durable consent fence.
    /// The existing account signatory and exact certified W remain unchanged; this is consent,
    /// not a Core permission, debit decision or finalized funding capability.
    /// # Errors
    /// Refuses foreign W/network/release, stale custody or any changed invocation/original.
    pub fn sign_retained_mint_consent(
        &self,
        inventory: &KagemushaAdmittedOrdinaryNativeInventoryV1,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryMintAccountSigningV1<'_>,
    ) -> Result<[u8; 64]> {
        self.recheck()?;
        inventory.require_account_transport(&self.account)?;
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint consent invocation rejected"))?;
        let request =
            iroha_data_model::kagemusha::KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
                original
                    .request_original()
                    .map_err(|_| eyre!("Native Mint request custody rejected"))?,
            )
            .map_err(|_| eyre!("Native Mint request original rejected"))?;
        let context = &request.authorization.statement.context;
        inventory.require_mint_request(&request)?;
        ensure!(
            context.lineage.owner.account_id == *self.current.wallet()
                && &context.lineage.owner.runtime.network_id == self.account.network_id(),
            "Native Mint consent differs from retained W/network"
        );
        let message = original
            .account_signing_message()
            .map_err(|_| eyre!("Native Mint account subject rejected"))?;
        ensure!(
            message
                == request
                    .account_signing_message()
                    .map_err(|_| eyre!("Native Mint account subject codec rejected"))?,
            "Native Mint account subject changed original"
        );
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint consent expired before signing"))?;
        self.recheck()?;
        let signature = iroha_crypto::Signature::try_new(
            self.account.context.key_pair.private_key(),
            &message,
        )?;
        request
            .verify_account_signature(&signature)
            .map_err(|_| eyre!("Native Mint consent key differs from W"))?;
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint consent changed after signing"))?;
        inventory.require_mint_request(&request)?;
        self.recheck()?;
        signature
            .payload()
            .try_into()
            .map_err(|_| eyre!("Native Mint consent is not Ed64"))
    }

    /// Quote and sign the sole actual Main-selected Node funding instruction through the
    /// existing Ed25519 signatory. A distinct one-member W witness uses the maintained canonical
    /// HTTP witness grammar; generic direct-account and multi-member quote APIs stay unchanged.
    /// Main fsyncs both returned canonical SignedTransaction and exact wire before dispatch.
    /// # Errors
    /// Refuses another installed purpose/original/W, changed payload, fee substitution,
    /// expired Core decision/current FI or any failed transport/signing/current custody check.
    pub fn sign_retained_mint_transaction(
        &self,
        inventory: &KagemushaAdmittedOrdinaryNativeInventoryV1,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryMintTransactionSigningV1<'_>,
    ) -> Result<[Vec<u8>; 2]> {
        self.recheck()?;
        inventory.require_account_transport(&self.account)?;
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint transaction invocation rejected"))?;
        let submission = original
            .submission()
            .map_err(|_| eyre!("Native Mint Node original rejected"))?;
        inventory.require_mint_submission(&submission)?;
        let request =
            iroha_data_model::kagemusha::KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
                &submission.topup_request_original,
            )
            .map_err(|_| eyre!("Native Mint transaction request rejected"))?;
        ensure!(
            request
                .authorization
                .statement
                .context
                .lineage
                .owner
                .account_id
                == *self.current.wallet(),
            "Native Mint transaction changed retained W"
        );
        let AccountController::Multisig(policy) = self.current.wallet().controller() else {
            return Err(eyre!("Native Mint requires original one-member W"));
        };
        ensure!(
            policy.threshold() == 1
                && policy.members().len() == 1
                && policy.members()[0].weight() == 1
                && policy.members()[0].public_key() == self.account.context.key_pair.public_key(),
            "Native Mint HTTP witness changed exact one-member W"
        );
        let instruction: InstructionBox =
            iroha_data_model::isi::kagemusha_v1::TopUpKagemushaOrdinaryV1::new(submission)
                .map_err(|_| eyre!("Native Mint submission instruction rejected"))?
                .into();
        let mut payload = self.account.prepare_transaction(
            AccountTransactionDraft::new(
                [instruction],
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            )
            .with_time_to_live(Duration::from_millis(
                original
                    .maximum_ttl_ms()
                    .map_err(|_| eyre!("Native Mint decision TTL rejected"))?,
            )),
        )?;
        payload.creation_time_ms = original
            .creation_time_ms()
            .map_err(|_| eyre!("Native Mint retained signing time rejected"))?;
        original
            .validate_payload(&payload)
            .map_err(|_| eyre!("Native Mint exact payload rejected before quote"))?;
        self.account.ensure_fee_quote_domain(&payload)?;
        let client = self.account.client();
        let url = join_torii_url(
            &client.torii_url,
            torii_routes::fees::QUOTE_PATH.trim_start_matches('/'),
        );
        let body = norito::json::to_vec(&FeeQuoteWireRequest {
            payload: payload.clone(),
        })?;
        let mut witness = iroha_data_model::soracloud::CanonicalRequestWitnessV1 {
            schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
            subject_account: self.current.wallet().clone(),
            timestamp_ms: payload.creation_time_ms,
            nonce: Client::signed_request_nonce()?,
            canonical_request_hash: canonical_network_request_hash(
                self.account.network_id(),
                &HttpMethod::POST,
                &url,
                &body,
            )?,
            signatures: Vec::new(),
        };
        let witness_message = canonical_request_witness_message(&witness)?;
        witness.signatures.push(
            iroha_data_model::soracloud::CanonicalRequestSignatureWitnessV1 {
                signer: self.account.context.key_pair.public_key().clone(),
                signature: iroha_crypto::Signature::try_new(
                    self.account.context.key_pair.private_key(),
                    &witness_message,
                )?,
            },
        );
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint decision expired before fee read"))?;
        self.recheck()?;
        let response = client.send_builder(
            client
                .request_without_canonical_account_auth(HttpMethod::POST, url)
                .header(
                    HEADER_WITNESS,
                    &canonical_request_witness_header_value(&witness)?,
                )
                .header("Content-Type", APPLICATION_JSON)
                .header("Accept", APPLICATION_JSON)
                .body(body)
                .max_response_bytes(FEE_QUOTE_RESPONSE_MAX_BYTES),
        )?;
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint decision expired after fee read"))?;
        self.recheck()?;
        let quote = Client::decode_fee_quote_response(&payload, &response)?;
        apply_fee_quote_intent(&mut payload, quote.intent)?;
        original
            .validate_payload(&payload)
            .map_err(|_| eyre!("Native Mint exact payload rejected after quote"))?;
        let transaction = iroha_data_model::transaction::TransactionBuilder::from_payload(payload)?
            .try_sign_multisig([self.account.context.key_pair.private_key()])?;
        transaction.verify_signature()?;
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint decision expired after transaction signature"))?;
        self.recheck()?;
        inventory.recheck()?;
        Ok([
            norito::encode_canonical(&transaction)?,
            transaction.encode_wire_v1()?,
        ])
    }

    /// Send only the exact durably dispatched transaction lent by Main. This single HTTP
    /// invocation cannot create another transaction and does not establish finalized funding.
    /// # Errors
    /// Refuses foreign transaction/W/network, changed custody or rejected/failed transport.
    pub fn submit_retained_mint_transaction(
        &self,
        inventory: &KagemushaAdmittedOrdinaryNativeInventoryV1,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryMintFundingTransportV1<'_>,
    ) -> Result<()> {
        self.recheck()?;
        inventory.require_account_transport(&self.account)?;
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint dispatch custody rejected"))?;
        let raw = original
            .transaction_original()
            .map_err(|_| eyre!("Native Mint transaction original rejected"))?;
        let tx: SignedTransaction =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))?;
        ensure!(
            norito::encode_canonical(&tx)? == raw
                && tx.authority() == self.current.wallet()
                && tx.network_id() == Some(self.account.network_id()),
            "Native Mint transaction changed W/network/original"
        );
        tx.verify_signature()?;
        let payload = PreparedTransactionPayload::from_transaction(&tx);
        ensure!(
            payload.as_bytes()
                == original
                    .transaction_wire()
                    .map_err(|_| eyre!("Native Mint wire custody rejected"))?,
            "Native Mint wire differs from original"
        );
        let client = self.account.client();
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint dispatch expired before HTTP"))?;
        let response = client.send_builder(client.prepare_transaction_payload_request(&payload))?;
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint custody changed after HTTP"))?;
        self.recheck()?;
        TransactionResponseHandler::handle_for_confirmation(&response, &tx)?;
        // HTTP admission or unknown disposition never grants Node finality or incoming funds.
        Ok(())
    }
    /// Read immutable Node/Kura finality under current W's exact canonical witness. Pending
    /// returns None; complete raw data still requires Main's independently anchored admission.
    /// # Errors
    /// Refuses foreign selectors/W/network, stale custody, invalid reply or transport failure.
    pub fn read_retained_mint_finality(
        &self,
        inventory: &KagemushaAdmittedOrdinaryNativeInventoryV1,
        original: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryMintFundingTransportV1<'_>,
    ) -> Result<Option<Vec<u8>>> {
        use iroha_torii_shared::ordinary_mint_finalized::{
            ORDINARY_MINT_FINALIZED_ROUTE_V1, OrdinaryMintFinalizedReadV1,
        };
        self.recheck()?;
        inventory.require_account_transport(&self.account)?;
        let f = original
            .finality_read_fields()
            .map_err(|_| eyre!("Native Mint read selectors rejected"))?;
        ensure!(f.len() == 5, "Native Mint read field count rejected");
        let request = OrdinaryMintFinalizedReadV1 {
            version: 1,
            network_id: norito::decode_canonical_with_limits(
                &f[0],
                norito::canonical_decode_limits(f[0].len()),
            )?,
            payer: norito::decode_canonical_with_limits(
                &f[1],
                norito::canonical_decode_limits(f[1].len()),
            )?,
            operation_id: f[2].as_slice().try_into()?,
            request_original_sha256: f[3].as_slice().try_into()?,
            issuer_decision_original_sha256: f[4].as_slice().try_into()?,
        };
        ensure!(
            &request.network_id == self.account.network_id()
                && &request.payer == self.current.wallet(),
            "Native Mint read changed W/network"
        );
        let AccountController::Multisig(policy) = self.current.wallet().controller() else {
            return Err(eyre!("Native Mint read requires original one-member W"));
        };
        ensure!(
            policy.threshold() == 1
                && policy.members().len() == 1
                && policy.members()[0].weight() == 1
                && policy.members()[0].public_key() == self.account.context.key_pair.public_key(),
            "Native Mint read changed one-member W"
        );
        let body = request.canonical_wire()?;
        let client = self.account.client();
        let url = join_torii_url(
            &client.torii_url,
            ORDINARY_MINT_FINALIZED_ROUTE_V1.trim_start_matches('/'),
        );
        let mut witness = iroha_data_model::soracloud::CanonicalRequestWitnessV1 {
            schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
            subject_account: self.current.wallet().clone(),
            timestamp_ms: original
                .http_timestamp_ms()
                .map_err(|_| eyre!("Native Mint HTTP clock rejected"))?,
            nonce: Client::signed_request_nonce()?,
            canonical_request_hash: canonical_network_request_hash(
                self.account.network_id(),
                &HttpMethod::POST,
                &url,
                &body,
            )?,
            signatures: Vec::new(),
        };
        let message = canonical_request_witness_message(&witness)?;
        witness.signatures.push(
            iroha_data_model::soracloud::CanonicalRequestSignatureWitnessV1 {
                signer: self.account.context.key_pair.public_key().clone(),
                signature: iroha_crypto::Signature::try_new(
                    self.account.context.key_pair.private_key(),
                    &message,
                )?,
            },
        );
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint read expired before HTTP"))?;
        let response = client.send_builder(
            client
                .request_without_canonical_account_auth(HttpMethod::POST, url)
                .header(
                    HEADER_WITNESS,
                    &canonical_request_witness_header_value(&witness)?,
                )
                .header("Content-Type", APPLICATION_NORITO)
                .header("Accept", APPLICATION_NORITO)
                .body(body)
                .max_response_bytes(
                    iroha_data_model::kagemusha::KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1,
                ),
        )?;
        original
            .recheck()
            .map_err(|_| eyre!("Native Mint read expired after HTTP"))?;
        self.recheck()?;
        if response.status() == StatusCode::ACCEPTED {
            ensure!(
                response.body().is_empty(),
                "Native Mint pending read carried an unexpected original"
            );
            return Ok(None);
        }
        ensure!(
            response.status() == StatusCode::OK
                && response
                    .headers()
                    .get(http::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    == Some(APPLICATION_NORITO),
            "Native Mint finality reply rejected"
        );
        Ok(Some(response.into_body()))
    }

    /// Sign only the same financial owner's Native-reserved current FI request after its
    /// durable invocation fence. Ed64 is retained by that owner before any transport projection.
    /// This consent supplies neither FI status nor a money/platform approval.
    /// # Errors
    /// Refuses foreign installed issuer/runtime, original W/S/key/session, stale current custody,
    /// unknown prior invocation or failed same-original signature durability.
    pub fn sign_retained_current_fi_control(
        &self,
        inventory: &KagemushaAdmittedOrdinaryNativeInventoryV1,
        control: &mut iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryCurrentFinancialControlOwnerV1,
        financial: &iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Vec<Vec<u8>>> {
        self.recheck()?;
        inventory.require_account_transport(&self.account)?;
        let request = control
            .pending_account_request(financial)
            .map_err(|_| eyre!("Native current FI request custody rejected"))?
            .clone();
        inventory.require_current_control_request(&request)?;
        ensure!(
            request.owner.account_id == *self.current.wallet()
                && &request.owner.runtime.network_id == self.account.network_id(),
            "Native current FI request changed retained W/network"
        );
        let message = request
            .account_signing_message()
            .map_err(|_| eyre!("Native current FI account subject rejected"))?;
        let retained = control
            .fence_account_request(financial)
            .map_err(|_| eyre!("Native current FI account invocation unavailable or unknown"))?;
        self.recheck()?;
        inventory.recheck()?;
        let signature = match retained {
            Some(original) => original,
            None => iroha_crypto::Signature::try_new(
                self.account.context.key_pair.private_key(),
                &message,
            )?
            .payload()
            .try_into()
            .map_err(|_| eyre!("Native current FI Ed64 shape rejected"))?,
        };
        control
            .retain_account_request_original(financial, signature)
            .map_err(|_| eyre!("Native current FI account original durability rejected"))?;
        self.recheck()?;
        inventory.require_current_control_request(&request)?;
        let fields = control
            .retained_current_read_fields(financial)
            .map_err(|_| eyre!("Native current FI transport original rejected"))?;
        ensure!(
            fields.as_slice()
                == [
                    request
                        .canonical_bytes()
                        .map_err(|_| eyre!("Native current FI request original rejected"))?,
                    signature.to_vec()
                ]
                .as_slice(),
            "Native current FI transport original drifted"
        );
        self.recheck()?;
        Ok(fields)
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

    /// Fetch all four installed-node originals for this exact Native-prepared FI request, then
    /// authenticate the current S/W cut and sign through the existing Native account signer.
    /// The prepared holder is consumed on every outcome. Transport failures require a new Native
    /// preparation/read; this call never retries or renews the original nonce or elapsed budget.
    /// The caller must retain the original FI request/idempotency context and supply actual FI
    /// admission separately. This method neither dispatches to the FI nor grants monetary state.
    /// # Errors
    /// Refuses foreign account/runtime/clock, a changed certified prefix, substituted node or
    /// request originals, expired custody/read, transport failure or failed actual Native signing.
    pub fn fetch_and_sign_current_enrollment_request(
        &self,
        prepared: KagemushaNativePreparedEnrollmentRequestV1,
        inventory: Arc<KagemushaAdmittedOrdinaryNativeInventoryV1>,
    ) -> Result<(
        iroha_crypto::Signature,
        VerifiedParticipantEnrollmentRequestV1,
    )> {
        self.recheck()?;
        let read = KagemushaNativeCurrentWalletReadV1::reserve_for_enrollment_request(
            self.account.clone(),
            inventory.clone(),
            &prepared,
        )?;
        let height = read.height;
        let clock = prepared.clock.clone();
        let current = read.fetch_and_authenticate()?;
        prepared.recheck(&self.account)?;
        require_installed_request_clock(&inventory, &clock, Some(height))?;
        let (signature, verified) = self.sign_current_enrollment_request(prepared, current)?;
        require_installed_request_clock(&inventory, &clock, Some(height))?;
        verified.recheck()?;
        self.recheck()?;
        Ok((signature, verified))
    }

    /// Retain the exact HTTP original after the genuine four-node request read and existing
    /// Native S signing. This borrow keeps the same finite account/read/clock owners alive;
    /// no metadata, retail phase10 signature or offered DTO can manufacture the result.
    /// There is no transport retry or renewed nonce, clock interval or startup lease.
    /// # Errors
    /// Refuses stale or foreign custody, substituted originals, changed certified prefix,
    /// actual current-read failure or Native signing failure.
    pub fn fetch_and_sign_current_enrollment_http_original(
        &self,
        prepared: KagemushaNativePreparedEnrollmentRequestV1,
        inventory: Arc<KagemushaAdmittedOrdinaryNativeInventoryV1>,
    ) -> Result<KagemushaNativeSignedEnrollmentHttpOriginalV1<'_>> {
        self.recheck()?;
        let read = KagemushaNativeCurrentWalletReadV1::reserve_for_enrollment_request(
            self.account.clone(),
            inventory.clone(),
            &prepared,
        )?;
        let height = read.height;
        let clock = prepared.clock.clone();
        let current = read.fetch_and_authenticate()?;
        prepared.recheck(&self.account)?;
        require_installed_request_clock(&inventory, &clock, Some(height))?;
        let (signature, verified) =
            self.sign_current_enrollment_request_originals(&prepared, current)?;
        require_installed_request_clock(&inventory, &clock, Some(height))?;
        verified.recheck()?;
        self.recheck()?;
        let original = KagemushaNativeSignedEnrollmentHttpOriginalV1 {
            custody: self,
            prepared,
            inventory,
            height,
            signature,
            verified,
        };
        original.recheck()?;
        Ok(original)
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
        self.sign_current_enrollment_request_originals(&prepared, current)
    }

    fn sign_current_enrollment_request_originals(
        &self,
        prepared: &KagemushaNativePreparedEnrollmentRequestV1,
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
    // The actual request challenge owns its own finite Native reading while the preparation's
    // original reading and clock window remain unchanged. No public request projection rebuilds it.
    fn reserve_wallet_read_challenge(
        &self,
        account: &AccountClient,
    ) -> Result<crate::participant_enrollment_request::EnrollmentWalletReadChallengeV1> {
        self.recheck(account)?;
        let challenge =
            crate::participant_enrollment_request::EnrollmentWalletReadChallengeV1::for_request(
                &self.request(),
            )?;
        self.recheck(account)?;
        Ok(challenge)
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

/// Move-only exact HTTP original retained by the real Native S signer. It borrows the
/// original account custody and retains the original preparation, inventory and current read.
/// This is transport custody, not FI customer/issuer/installation or monetary authority.
/// No decoder, public constructor, clone or detached-signature callback is provided.
pub struct KagemushaNativeSignedEnrollmentHttpOriginalV1<'a> {
    custody: &'a KagemushaNativeAccountCustodyV1,
    prepared: KagemushaNativePreparedEnrollmentRequestV1,
    inventory: Arc<KagemushaAdmittedOrdinaryNativeInventoryV1>,
    height: u64,
    signature: iroha_crypto::Signature,
    verified: VerifiedParticipantEnrollmentRequestV1,
}
impl KagemushaNativeSignedEnrollmentHttpOriginalV1<'_> {
    /// Recheck the same original finite custody and installed current prefix before every
    /// actual request attempt and before exposing its response. This never renews old owners.
    /// # Errors
    /// Expired startup/read/preparation, changed clock/current prefix or original body.
    pub fn recheck(&self) -> Result<()> {
        self.custody.recheck()?;
        self.prepared.recheck(&self.custody.account)?;
        require_installed_request_clock(&self.inventory, &self.prepared.clock, Some(self.height))?;
        self.verified.recheck()?;
        self.verified
            .verify_original_body(&self.prepared.context.body)?;
        self.custody.recheck()
    }
    /// Exact fixed POST target selected before Native signing; no proxy reconstruction.
    /// # Errors
    /// The original custody, preparation or current cut is no longer usable.
    pub fn target(&self) -> Result<&Url> {
        self.recheck()?;
        Ok(&self.prepared.context.target)
    }
    /// Borrow the exact original body, without JSON reserialization.
    /// # Errors
    /// The original custody, preparation or current cut is no longer usable.
    pub fn original_body(&self) -> Result<&[u8]> {
        self.recheck()?;
        Ok(&self.prepared.context.body)
    }
    /// Exact purpose-specific HTTP header originals. Authorization remains runtime-only
    /// and must equal the exact original session digest; never persist or log these values.
    /// # Errors
    /// Changed session, malformed signature or any expired/changed original owner.
    pub fn headers(&self, authorization: &[u8]) -> Result<Vec<(&'static str, Vec<u8>)>> {
        self.recheck()?;
        let headers = crate::participant_enrollment_request::http::encode_participant_enrollment_http_headers_v1(
            &self.prepared.request(), &self.signature, authorization,
        )?;
        self.recheck()?;
        Ok(headers)
    }
}

/// Closed progress result: this Native call fsynced genuine certified successors but could
/// not reach the current tip within its finite work/time budget. Retry through a new Native
/// request; the result carries no checkpoint, signed clock, session or monetary authority.
#[derive(Clone, Copy, Debug)]
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
    nodes: ClockNodes,
}
impl KagemushaNativeClockTransportV1 {
    /// Fetch the exact historical parents needed by Node's public clock carrier.
    /// Every reply is independently checked against the same retained Clock WAL prefix.
    /// This does not advance that prefix, select a root or lend historical elapsed time.
    /// # Errors
    /// Refuses absent retained decisions, malformed/oversized proofs or the finite budget.
    pub fn fetch_authenticated_signed_clock_parent_originals(
        &self,
        clock: &Mutex<KagemushaOrdinaryNativeClockOwnerV1>,
        original: &[u8],
    ) -> Result<Vec<Vec<u8>>> {
        use iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryNativeSignedClockOriginalV1;
        use iroha_data_model::sumeragi_finality::{
            SumeragiFinalityAttestation, SumeragiFinalityProof,
        };
        let began = NativeContinuousReading::now()?;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let (verifier, height) = {
            let mut held = clock
                .lock()
                .map_err(|_| eyre!("Native parent clock lock unavailable"))?;
            let verified = held
                .authenticate_received_historical_signed_original(original)
                .map_err(|_| eyre!("Native parent original is outside the retained prefix"))?;
            let data =
                KagemushaOrdinaryNativeSignedClockOriginalV1::decode_original(verified.original())
                    .map_err(|_| eyre!("Native parent original rejected"))?;
            let raw = &data.signed_observations()[0];
            let attestation: SumeragiFinalityAttestation = norito::decode_canonical_with_limits(
                raw,
                norito::canonical_decode_limits(raw.len()),
            )?;
            let verifier = held
                .current_finality_verifier()
                .map_err(|_| eyre!("Native parent current prefix unavailable"))?;
            verifier.verify_retained_decision(&attestation.body.finality_proof)?;
            (verifier, attestation.body.finality_proof.height())
        };
        ensure!(height > 0, "Native parent tip absent");
        let mut parents = Vec::with_capacity(2);
        for wanted in height.saturating_sub(2).max(1)..height {
            let wanted =
                NonZeroU64::new(wanted).ok_or_else(|| eyre!("Native parent height rejected"))?;
            let mut retained = None;
            for index in 0..4 {
                ensure!(
                    began.elapsed()? < Duration::from_secs(10),
                    "Native parent budget expired"
                );
                if let Ok(proof) = self
                    .nodes
                    .at(index)
                    .with_request_deadline(deadline)
                    .get_sumeragi_finality_proof(wanted)
                {
                    if proof.height() != wanted.get()
                        || verifier.verify_retained_decision(&proof).is_err()
                    {
                        continue;
                    }
                    let raw = norito::encode_canonical(&proof)?;
                    if raw.len() <= 4 * 1024 * 1024 {
                        retained = Some(raw);
                        break;
                    }
                }
            }
            parents
                .push(retained.ok_or_else(|| eyre!("Native retained clock parent unavailable"))?);
        }
        {
            let mut held = clock
                .lock()
                .map_err(|_| eyre!("Native parent clock lock unavailable"))?;
            held.authenticate_received_historical_signed_original(original)
                .map_err(|_| eyre!("Native parent original changed"))?;
            let after = held
                .current_finality_verifier()
                .map_err(|_| eyre!("Native parent current prefix expired"))?;
            for raw in &parents {
                let proof: SumeragiFinalityProof = norito::decode_canonical_with_limits(
                    raw,
                    norito::canonical_decode_limits(raw.len()),
                )?;
                after.verify_retained_decision(&proof)?;
            }
        }
        ensure!(
            began.elapsed()? < Duration::from_secs(10),
            "Native parent budget expired after verification"
        );
        Ok(parents)
    }

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
            endpoint::require_https_directory_base(node.torii_url.as_str())?;
            ensure!(
                node.network_id == network,
                "native clock transport changed installed network"
            );
        }
        Ok(Self {
            nodes: ClockNodes::AccountContext(Box::new(nodes)),
        })
    }

    /// Retain isolated unsigned transports for an independently authenticated first-device
    /// package. The caller must authenticate all origins before this shape-only constructor.
    /// There is no account key, wallet session, managed callback or root supplied by a reply.
    /// # Errors
    /// Refuses noncanonical HTTPS origins, network mismatch or unavailable default TLS transport.
    pub fn from_public_node_base_urls(
        origins: [String; 4],
        network: NetworkId,
        clock: &Mutex<KagemushaOrdinaryNativeClockOwnerV1>,
    ) -> Result<Self> {
        ensure!(
            clock
                .lock()
                .map_err(|_| eyre!("Native clock lock unavailable"))?
                .network_id()
                .map_err(|_| eyre!("Native clock network unavailable"))?
                == network,
            "public clock network differs"
        );
        Ok(Self {
            nodes: ClockNodes::public(origins, network)?,
        })
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
            for index in 0..4 {
                let client = self.nodes.at(index);
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
                "https://fi.example.invalid/leumi.is2/v1/kagemusha/enrollment/ordinary/prepare",
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
    fn native_enrollment_http_headers_preserve_actual_s_w_original_and_existing_signature() {
        use crate::participant_enrollment_request::http::{
            ParticipantEnrollmentHttpOwnerContextV1,
            decode_participant_enrollment_http_original_v1,
            encode_participant_enrollment_http_headers_v1,
        };
        use sha2::{Digest as _, Sha256};
        let fixture = NativeCustodyFixture::new();
        let custody = KagemushaNativeAccountCustodyV1::from_current_wallet(
            context(&fixture),
            initial_current(&fixture),
        )
        .unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let clock = Arc::new(Mutex::new(fixture.clock(temporary.path())));
        let authorization = b"unit-test-only-not-an-authenticated-session";
        let mut context = request_context();
        context.session_sha256 = Sha256::digest(authorization).into();
        let prepared = custody.prepare_enrollment_request(context, clock).unwrap();
        let challenge = prepared
            .reserve_wallet_read_challenge(&custody.account)
            .unwrap();
        let current = fixture.current_for_challenge(challenge);
        // Exercise the same genuine key/read signing helper used by the HTTP holder. This
        // explicit synthetic fixture supplies no admitted inventory or installed HTTP caller.
        let (signature, verified) = custody
            .sign_current_enrollment_request_originals(&prepared, current)
            .unwrap();
        let request = prepared.request();
        let headers =
            encode_participant_enrollment_http_headers_v1(&request, &signature, authorization)
                .unwrap();
        let owner = ParticipantEnrollmentHttpOwnerContextV1 {
            network_id: request.network_id,
            authentication_namespace: request.authentication_namespace,
            actor_id: request.actor_id,
            operation: request.operation,
            http_method: "POST",
            request_target: request.target.path(),
            target: request.target,
        };
        let received = decode_participant_enrollment_http_original_v1(
            owner,
            request.body,
            headers
                .iter()
                .map(|(name, value)| (*name, value.as_slice())),
        )
        .unwrap();
        assert_eq!(
            received.request().signing_message().unwrap(),
            request.signing_message().unwrap()
        );
        assert_eq!(received.signature().payload(), signature.payload());
        verified
            .verify_original_body(received.request().body)
            .unwrap();
        assert_eq!(verified.wallet(), received.request().wallet);
        assert_eq!(verified.signatory(), received.request().signatory);
        assert_ne!(received.request().wallet, received.request().signatory);
        assert_eq!(verified.request_id(), received.request().request_id);
        assert_eq!(
            verified.idempotency_key(),
            received.request().idempotency_key
        );
        assert!(
            encode_participant_enrollment_http_headers_v1(
                &request,
                &signature,
                b"substituted-test-only-session"
            )
            .is_err()
        );
    }

    #[test]
    fn request_bound_native_read_uses_original_four_statements_and_existing_signer() {
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
        let message = prepared.request().signing_message().unwrap();
        let body = prepared.request().body.to_vec();
        let challenge = prepared
            .reserve_wallet_read_challenge(&custody.account)
            .unwrap();
        // This existing synthetic fixture performs actual Ed/BLS/World/finality verification.
        // It is not an installed-inventory or physical-device transport qualification.
        let current = fixture.current_for_challenge(challenge);
        let (signature, verified) = custody
            .sign_current_enrollment_request(prepared, current)
            .unwrap();
        signature
            .verify(fixture.key().public_key(), &message)
            .unwrap();
        assert_eq!(verified.namespace(), "leumi.is2");
        assert_eq!(verified.actor_id(), "fixture-retail-actor");
        assert_eq!(verified.request_id(), "fixture-request");
        assert_eq!(verified.idempotency_key(), "fixture-stable-attempt");
        assert_eq!(verified.wallet(), fixture.wallet());
        assert_eq!(verified.signatory(), fixture.signatory());
        verified.verify_original_body(&body).unwrap();
        assert!(
            verified
                .verify_original_body(b"changed native request body")
                .is_err()
        );
    }

    #[test]
    fn request_bound_native_read_rejects_each_changed_fi_request_original() {
        let fixture = NativeCustodyFixture::new();
        let temporary = tempfile::tempdir().unwrap();
        let clock = Arc::new(Mutex::new(fixture.clock(temporary.path())));
        for field in 0..10 {
            let custody = KagemushaNativeAccountCustodyV1::from_current_wallet(
                context(&fixture),
                initial_current(&fixture),
            )
            .unwrap();
            let mut prepared = custody
                .prepare_enrollment_request(request_context(), clock.clone())
                .unwrap();
            let challenge = prepared
                .reserve_wallet_read_challenge(&custody.account)
                .unwrap();
            let current = fixture.current_for_challenge(challenge);
            // Simulate a substitution after read reservation, without rebuilding the prepared owner.
            match field {
                0 => prepared.context.authentication_namespace = "hapoalim.is2".into(),
                1 => prepared.context.actor_id = "another-retail-actor".into(),
                2 => prepared.context.session_sha256 = [8; 32],
                3 => prepared.context.request_id = "another-request".into(),
                4 => prepared.context.idempotency_key = "another-attempt".into(),
                5 => prepared.context.body = b"changed-original-json".to_vec(),
                6 => prepared.context.target = Url::parse(
                    "https://another-fi.example.invalid/leumi.is2/v1/kagemusha/enrollment/ordinary/prepare",
                )
                .unwrap(),
                7 => {
                    prepared.context.operation = ParticipantEnrollmentOperationV1::Certificate;
                    prepared.context.target = Url::parse(
                        "https://fi.example.invalid/leumi.is2/v1/kagemusha/enrollment/ordinary/certificate",
                    )
                    .unwrap();
                }
                8 => prepared.nonce = "another-native-request-nonce-0001".into(),
                _ => prepared.timestamp_ms += 1,
            }
            // Every changed subject remains well formed and all original time/key custody is
            // live. Refusal must come from the challenged subject, not an expired fixture.
            prepared.request().signing_message().unwrap();
            prepared.recheck(&custody.account).unwrap();
            current.recheck().unwrap();
            custody.recheck().unwrap();
            assert!(
                custody
                    .sign_current_enrollment_request(prepared, current)
                    .is_err(),
                "changed request field {field} must not return a signature/verified request",
            );
        }
    }

    #[test]
    fn native_request_retry_preserves_business_identity_and_refuses_old_read() {
        let fixture = NativeCustodyFixture::new();
        let custody = KagemushaNativeAccountCustodyV1::from_current_wallet(
            context(&fixture),
            initial_current(&fixture),
        )
        .unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let clock = Arc::new(Mutex::new(fixture.clock(temporary.path())));
        let original = custody
            .prepare_enrollment_request(request_context(), clock.clone())
            .unwrap();
        let old_nonce = original.request().nonce.to_owned();
        let old_challenge = original
            .reserve_wallet_read_challenge(&custody.account)
            .unwrap();
        let old_challenge_bytes = old_challenge.bytes();
        let old_current = fixture.current_for_challenge(old_challenge);
        // An abandoned preparation/read cannot authenticate a fresh retry, even with the same
        // exact business request ID, body and idempotency key. No deadline/nonce is renewed.
        drop(original);
        let substituted_retry = custody
            .prepare_enrollment_request(request_context(), clock.clone())
            .unwrap();
        assert_ne!(substituted_retry.request().nonce, old_nonce);
        substituted_retry.request().signing_message().unwrap();
        substituted_retry.recheck(&custody.account).unwrap();
        old_current.recheck().unwrap();
        custody.recheck().unwrap();
        assert!(
            custody
                .sign_current_enrollment_request(substituted_retry, old_current)
                .is_err()
        );
        let retry = custody
            .prepare_enrollment_request(request_context(), clock)
            .unwrap();
        assert_eq!(retry.request().request_id, "fixture-request");
        assert_eq!(retry.request().idempotency_key, "fixture-stable-attempt");
        assert_eq!(retry.request().body, b"exact-native-prepared-json");
        assert_ne!(retry.request().nonce, old_nonce);
        let message = retry.request().signing_message().unwrap();
        let challenge = retry
            .reserve_wallet_read_challenge(&custody.account)
            .unwrap();
        assert_ne!(challenge.bytes(), old_challenge_bytes);
        let current = fixture.current_for_challenge(challenge);
        let (signature, verified) = custody
            .sign_current_enrollment_request(retry, current)
            .unwrap();
        signature
            .verify(fixture.key().public_key(), &message)
            .unwrap();
        verified
            .verify_original_body(b"exact-native-prepared-json")
            .unwrap();
        assert_eq!(verified.idempotency_key(), "fixture-stable-attempt");
    }

    #[test]
    fn native_request_read_refuses_same_key_foreign_holder_and_startup_domain() {
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
        // A same-key/same-W reconstructed AccountClient has another actual Arc identity.
        prepared.recheck(&custody.account).unwrap();
        assert!(
            prepared
                .reserve_wallet_read_challenge(&context(&fixture))
                .is_err()
        );
        let startup =
            crate::participant_enrollment_request::EnrollmentWalletReadChallengeV1::for_native_wallet_selection(
                fixture.network(),
                fixture.signatory(),
                fixture.wallet(),
            )
            .unwrap();
        let startup_current = fixture.current_for_challenge(startup);
        startup_current.recheck().unwrap();
        prepared.recheck(&custody.account).unwrap();
        custody.recheck().unwrap();
        assert!(
            custody
                .sign_current_enrollment_request(prepared, startup_current)
                .is_err()
        );
    }

    #[test]
    fn retained_key_identity_survives_expiry_without_granting_current_signing() {
        let fixture = NativeCustodyFixture::new();
        let mut custody = KagemushaNativeAccountCustodyV1::from_current_wallet(
            context(&fixture),
            initial_current(&fixture),
        )
        .unwrap();
        custody.recheck().unwrap();
        custody.recheck_retained_identity().unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let clock = Arc::new(Mutex::new(fixture.clock(temporary.path())));
        // Let the real Native continuous reading expire; no offered/test clock renewal.
        std::thread::sleep(std::time::Duration::from_secs(10));
        assert!(custody.recheck().is_err());
        custody.recheck_retained_identity().unwrap();
        assert_eq!(custody.wallet(), fixture.wallet());
        assert_eq!(custody.signatory(), fixture.signatory());
        assert!(
            custody
                .prepare_enrollment_request(request_context(), clock.clone())
                .is_err()
        );
        // Only another fully verified same-S/W read restores current signing custody.
        custody
            .refresh_current_wallet(initial_current(&fixture))
            .unwrap();
        custody.recheck().unwrap();
        let prepared = custody
            .prepare_enrollment_request(request_context(), clock)
            .unwrap();
        let message = prepared.request().signing_message().unwrap();
        let current = fixture.current(&prepared.request());
        let (signature, _) = custody
            .sign_current_enrollment_request(prepared, current)
            .unwrap();
        signature
            .verify(fixture.key().public_key(), &message)
            .unwrap();
        // Data-only identity still refuses a substituted account key/context.
        let mut changed = custody.account.context.as_ref().clone();
        changed.key_pair =
            iroha_crypto::KeyPair::from_seed(vec![99; 32], iroha_crypto::Algorithm::Ed25519);
        changed.account = AccountId::new_multisig(
            iroha_data_model::account::MultisigPolicy::new(
                1,
                vec![
                    iroha_data_model::account::MultisigMember::new(
                        changed.key_pair.public_key().clone(),
                        1,
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        custody.account = changed.account_client().unwrap();
        assert!(custody.recheck_retained_identity().is_err());
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
