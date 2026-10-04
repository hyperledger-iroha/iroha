//! Exact customer-owned enrollment request signatures and certified S/W membership.
//! This is identity-request evidence, not KYC, issuer, installation or monetary authority.
//! Independently admitted node/schema inputs and FI-native current rows remain mandatory.
use crate::client::{
    canonical_network_request_signature_message, canonical_request_account_header_value,
};
use eyre::{Result, ensure};
use iroha_crypto::{Algorithm, Hash, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    account::{AccountId, AccountValue},
    sumeragi_finality::{
        SumeragiFinalityAttestation, SumeragiFinalityProof, SumeragiFinalityVerifier,
        VerifiedSumeragiBlock, VerifiedWorldStateSnapshotV1,
    },
};
use iroha_model_base::peer::PeerId;
use iroha_primitives::time::NativeContinuousReading;
use sha2::{Digest as _, Sha256};
use std::time::Duration;
use url::Url;

const DOMAIN: &[u8] = b"iroha.participant.ordinary-enrollment-request.v1\0";
const READ_BUDGET: Duration = Duration::from_secs(10);
/// Maximum exact public enrollment JSON request size.
pub const MAX_PARTICIPANT_ENROLLMENT_BODY_BYTES: usize = 256 * 1024;
/// Maximum timestamp skew accepted by the FI's actual native leader-time CAS.
pub const PARTICIPANT_ENROLLMENT_REQUEST_SKEW_MS: u64 = 30_000;

/// One fixed SDK carrier route; another operation cannot share the signing purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantEnrollmentOperationV1 {
    /// Original native preparation reservation.
    Prepare,
    /// Full original platform attestation, before possession/final credential.
    RawAttestation,
    /// Original consumed possession and final identity credential.
    Certificate,
}
impl ParticipantEnrollmentOperationV1 {
    /// Fixed suffix under the independently configured FI public mount.
    #[must_use]
    pub const fn path_suffix(self) -> &'static str {
        match self {
            Self::Prepare => "/v1/kagemusha/enrollment/ordinary/prepare",
            Self::RawAttestation => "/v1/kagemusha/enrollment/ordinary/raw-attestation",
            Self::Certificate => "/v1/kagemusha/enrollment/ordinary/certificate",
        }
    }
    fn tag(self) -> u8 {
        match self {
            Self::Prepare => 1,
            Self::RawAttestation => 2,
            Self::Certificate => 3,
        }
    }
}

/// Exact actual HTTP context. These fields select bytes, never manufacture an owner.
#[derive(Debug, Clone, Copy)]
pub struct ParticipantEnrollmentRequestV1<'a> {
    /// Independently configured exact genesis-derived network.
    pub network_id: &'a NetworkId,
    /// Explicit participant authentication namespace; no default or alias rewrite.
    pub authentication_namespace: &'a str,
    /// Current FI-local subject derived from authenticated server session/rows.
    pub actor_id: &'a str,
    /// SHA-256 of the exact Authorization header bytes, including its scheme.
    pub session_sha256: [u8; 32],
    /// Current original client Ledger Ed signatory S.
    pub signatory: &'a AccountId,
    /// Exact current one-member multisig W selected by the native FI profile.
    pub wallet: &'a AccountId,
    /// Original SDK-selected HTTP request ID.
    pub request_id: &'a str,
    /// Exact actual idempotency header; retained business results use this identity.
    pub idempotency_key: &'a str,
    /// Fixed purpose/route selected by the server handler and native carrier.
    pub operation: ParticipantEnrollmentOperationV1,
    /// Exact final externally configured HTTPS target, before any dispatch/rewrite.
    pub target: &'a Url,
    /// Exact bytes received/sent; JSON is never reserialized for verification.
    pub body: &'a [u8],
    /// Canonical timestamp header, checked against native leader time by the consumer.
    pub timestamp_ms: u64,
    /// Fresh bounded nonce whose permanent consumption belongs to the FI native store.
    pub nonce: &'a str,
}
fn text(value: &str, maximum: usize) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= maximum
            && value.bytes().all(|b| (0x21..=0x7e).contains(&b)),
        "invalid exact enrollment request context"
    );
    Ok(())
}
fn field(out: &mut Vec<u8>, raw: &[u8]) -> Result<()> {
    out.extend_from_slice(&u32::try_from(raw.len())?.to_le_bytes());
    out.extend_from_slice(raw);
    Ok(())
}
impl ParticipantEnrollmentRequestV1<'_> {
    /// Bounds applied by the actual native FI snapshot and commit, not a handset clock.
    /// # Errors
    /// Invalid header, timestamp underflow or overflow.
    pub fn native_time_window(&self) -> Result<(u64, u64)> {
        let low = self
            .timestamp_ms
            .checked_sub(PARTICIPANT_ENROLLMENT_REQUEST_SKEW_MS)
            .ok_or_else(|| eyre::eyre!("invalid request time"))?;
        let high = self
            .timestamp_ms
            .checked_add(PARTICIPANT_ENROLLMENT_REQUEST_SKEW_MS)
            .ok_or_else(|| eyre::eyre!("request time overflow"))?;
        ensure!(low > 0, "request time is not positive");
        Ok((low, high))
    }
    /// First-release signing subject, explicitly purpose-separated from Torii.
    /// It reuses Iroha's exact-network canonical request grammar and additionally binds FI,
    /// actor, S/W, exact HTTPS origin/session/request/idempotency/purpose.
    /// # Errors
    /// Noncanonical/unbounded context, foreign route or incomplete body.
    pub fn signing_message(&self) -> Result<Vec<u8>> {
        for (value, maximum) in [
            (self.authentication_namespace, 256),
            (self.actor_id, 256),
            (self.request_id, 128),
            (self.idempotency_key, 256),
            (self.nonce, 128),
        ] {
            text(value, maximum)?;
        }
        ensure!(
            self.nonce.len() >= 16
                && self.session_sha256 != [0; 32]
                && !self.body.is_empty()
                && self.body.len() <= MAX_PARTICIPANT_ENROLLMENT_BODY_BYTES,
            "incomplete enrollment request"
        );
        ensure!(
            self.target.scheme() == "https"
                && self.target.host_str().is_some()
                && self.target.username().is_empty()
                && self.target.password().is_none()
                && self.target.fragment().is_none()
                && self.target.query().is_none()
                && self.target.path().ends_with(self.operation.path_suffix()),
            "enrollment target differs from the actual pinned FI mount"
        );
        self.native_time_window()?;
        require_single_member_wallet(self.signatory, self.wallet)?;
        let signatory = canonical_request_account_header_value(self.signatory)?;
        let wallet = canonical_request_account_header_value(self.wallet)?;
        let origin = self.target.origin().ascii_serialization();
        let canonical = canonical_network_request_signature_message(
            self.network_id,
            &reqwest::Method::POST,
            self.target,
            self.body,
            self.timestamp_ms,
            self.nonce,
        )?;
        let mut out = Vec::with_capacity(canonical.len() + 2048);
        out.extend_from_slice(DOMAIN);
        out.push(self.operation.tag());
        for raw in [
            self.authentication_namespace.as_bytes(),
            self.actor_id.as_bytes(),
            signatory.as_bytes(),
            wallet.as_bytes(),
            origin.as_bytes(),
            self.session_sha256.as_slice(),
            self.request_id.as_bytes(),
            self.idempotency_key.as_bytes(),
            canonical.as_slice(),
        ] {
            field(&mut out, raw)?;
        }
        Ok(out)
    }
}

/// Independently selected node identities; offered statements cannot select these pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedEnrollmentReadNodeV1 {
    /// Installed BLS reporting identity.
    pub peer_id: PeerId,
    /// Independently pinned native executable fingerprint.
    pub build_fingerprint: Hash,
    /// Independently pinned current effective configuration fingerprint.
    pub config_fingerprint: Hash,
}
/// Fresh non-cloneable challenge retained before the current native account read.
/// It binds that read to one exact request message and a finite monotonic budget.
// Keep this request capability move-only even though its individual fields can
// be copied; deriving Copy would undo the deliberate one-read ownership seam.
#[allow(missing_copy_implementations)]
pub struct EnrollmentWalletReadChallengeV1 {
    challenge: [u8; 32],
    request_sha256: [u8; 32],
    started: NativeContinuousReading,
}
impl EnrollmentWalletReadChallengeV1 {
    /// Reserve fresh public challenge entropy for the exact bounded signing subject.
    /// # Errors
    /// The request is not the sole canonical first-release subject.
    pub fn for_request(request: &ParticipantEnrollmentRequestV1<'_>) -> Result<Self> {
        let message = request.signing_message()?;
        let request_sha256 = Sha256::digest(&message).into();
        let entropy = rand::random::<[u8; 32]>();
        let mut digest = Sha256::new();
        digest.update(b"iroha.participant.enrollment-wallet-read.v1\0");
        digest.update(entropy);
        digest.update(request_sha256);
        Ok(Self {
            challenge: digest.finalize().into(),
            request_sha256,
            started: NativeContinuousReading::now()?,
        })
    }
    /// Reserve a distinct Native startup read for the actual immutable S/W/network selection.
    /// This establishes no FI or monetary authority and cannot verify an enrollment HTTP request.
    /// # Errors
    /// Refuses another controller shape or unavailable actual Native elapsed-clock custody.
    pub fn for_native_wallet_selection(
        network: NetworkId,
        signatory: &AccountId,
        wallet: &AccountId,
    ) -> Result<Self> {
        require_single_member_wallet(signatory, wallet)?;
        let entropy = rand::random::<[u8; 32]>();
        ensure!(entropy != [0; 32], "Native startup entropy unavailable");
        let mut subject = Sha256::new();
        subject.update(b"iroha:kagemusha:v1:ordinary-native-startup-wallet-read\0");
        subject.update(network.as_bytes());
        subject.update(norito::encode_canonical(signatory)?);
        subject.update(norito::encode_canonical(wallet)?);
        let request_sha256: [u8; 32] = subject.finalize().into();
        let mut nonce = Sha256::new();
        nonce.update(b"iroha:kagemusha:v1:ordinary-native-startup-wallet-nonce\0");
        nonce.update(request_sha256);
        nonce.update(entropy);
        Ok(Self {
            challenge: nonce.finalize().into(),
            request_sha256,
            started: NativeContinuousReading::now()?,
        })
    }

    /// Public request challenge; possessing it cannot create verified native evidence.
    #[must_use]
    pub fn bytes(&self) -> [u8; 32] {
        self.challenge
    }
    /// Remaining suspend-inclusive budget of the same original Native read.
    /// # Errors
    /// Refuses an expired read; callers cannot renew it with another local deadline.
    pub(crate) fn remaining_native_budget(&self) -> Result<std::time::Duration> {
        READ_BUDGET
            .checked_sub(self.started.elapsed()?)
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| eyre::eyre!("Native wallet read expired"))
    }
    fn recheck(&self) -> Result<()> {
        ensure!(
            self.started.elapsed()? < READ_BUDGET,
            "current enrollment wallet observation expired"
        );
        Ok(())
    }
}
/// Current certified S/W membership retained from actual native proofs. No decoder/constructor.
/// This does not authenticate FI customer status, aliases, directory ownership or monetary state.
pub struct VerifiedEnrollmentWalletSignatoryV1 {
    signatory: AccountId,
    wallet: AccountId,
    public_key: PublicKey,
    network: NetworkId,
    challenge: EnrollmentWalletReadChallengeV1,
    block: VerifiedSumeragiBlock,
    snapshot: VerifiedWorldStateSnapshotV1,
    nodes: [SelectedEnrollmentReadNodeV1; 4],
    proof: SumeragiFinalityProof,
}
impl VerifiedEnrollmentWalletSignatoryV1 {
    /// Authenticate exact stored S/W account values under the same complete compiled-schema World
    /// and independently anchored decision, then all four freshly challenged installed nodes.
    /// # Errors
    /// Any certificate, complete-cut, schema, selected-node, S/W membership or freshness mismatch.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate(
        challenge: EnrollmentWalletReadChallengeV1,
        network: NetworkId,
        nodes: &[SelectedEnrollmentReadNodeV1; 4],
        verifier: &SumeragiFinalityVerifier,
        retained: &SumeragiFinalityProof,
        snapshot: VerifiedWorldStateSnapshotV1,
        expected_schema: Hash,
        statements: &[SumeragiFinalityAttestation; 4],
        signatory: AccountId,
        signatory_value: &AccountValue,
        wallet: AccountId,
        wallet_value: &AccountValue,
    ) -> Result<Self> {
        challenge.recheck()?;
        let block = verifier.verify_retained_decision(retained)?;
        ensure!(
            snapshot.schema_hash() == expected_schema
                && snapshot.height() == block.height()
                && snapshot.context_id() == block.context_id()
                && snapshot.world_root() == block.execution().world_state_root,
            "wallet read changed compiled schema or certified complete cut"
        );
        let public_key = require_single_member_wallet(&signatory, &wallet)?;
        snapshot.verify_table_value("world.accounts", &signatory, signatory_value)?;
        snapshot.verify_table_value("world.accounts", &wallet, wallet_value)?;
        for i in 0..4 {
            ensure!(
                nodes[..i].iter().all(|n| n.peer_id != nodes[i].peer_id),
                "wallet read repeats selected node"
            );
            let a = &statements[i];
            a.verify()?;
            let b = &a.body;
            ensure!(
                b.challenge == challenge.challenge
                    && b.node_id == nodes[i].peer_id
                    && b.network_id == network
                    && NetworkId::from_genesis_hash(b.genesis_block_hash) == network
                    && b.build_fingerprint == nodes[i].build_fingerprint
                    && b.config_fingerprint == nodes[i].config_fingerprint
                    && b.status.config_fingerprint == nodes[i].config_fingerprint
                    && b.status.instance == verifier.instance().0
                    && b.status.signer.as_ref() == Some(nodes[i].peer_id.public_key())
                    && !b.status.unanchored
                    && !b.status.abstaining
                    && b.status.halted.is_none()
                    && b.status.committed_height == block.height()
                    && b.status.applied_height == block.height(),
                "wallet read changed fresh selected native authority"
            );
            let alternate = verifier.verify_same_decision(retained, &b.finality_proof)?;
            ensure!(
                alternate.context_id() == block.context_id(),
                "wallet read changed certified decision"
            );
        }
        challenge.recheck()?;
        Ok(Self {
            signatory,
            wallet,
            public_key,
            network,
            challenge,
            block,
            snapshot,
            nodes: nodes.clone(),
            proof: retained.clone(),
        })
    }
    /// Recheck this retained read's finite budget before FI admission or dispatch.
    /// # Errors
    /// The original challenged observation expired.
    pub fn recheck(&self) -> Result<()> {
        self.challenge.recheck()
    }
    /// Original independently verified network; this grants no FI or installation authority.
    #[must_use]
    pub fn network_id(&self) -> &NetworkId {
        &self.network
    }
    /// Recheck this same challenged current read under the independently installed current
    /// finality prefix and exact node/schema/network selections before Native startup.
    /// # Errors
    /// Refuses another root, committee, node inventory, schema, network or expired read.
    pub fn recheck_under_installed_finality(
        &self,
        verifier: &SumeragiFinalityVerifier,
        nodes: &[SelectedEnrollmentReadNodeV1; 4],
        expected_schema: Hash,
        network: NetworkId,
    ) -> Result<()> {
        self.recheck()?;
        let block = verifier.verify_retained_decision(&self.proof)?;
        ensure!(
            self.nodes == *nodes
                && self.network == network
                && self.snapshot.schema_hash() == expected_schema
                && block.context_id() == self.block.context_id()
                && block.execution().world_state_root == self.snapshot.world_root(),
            "current Native wallet read changed installed root/schema/node selection"
        );
        self.recheck()
    }

    /// Exact current S; no label-based or broker-selected account substitution.
    #[must_use]
    pub fn signatory(&self) -> &AccountId {
        &self.signatory
    }
    /// Exact current original W.
    #[must_use]
    pub fn wallet(&self) -> &AccountId {
        &self.wallet
    }
    /// Retained authenticated decision identity, not a replacement trust root.
    #[must_use]
    pub fn context_id(&self) -> Hash {
        self.block.context_id()
    }
    /// Retained complete World root.
    #[must_use]
    pub fn world_root(&self) -> Hash {
        self.snapshot.world_root()
    }
    /// Verify the actual S signature over the same original request selected before observation.
    /// The result cannot be used for another body/session/FI or reconstructed from DTOs.
    /// # Errors
    /// Any request binding, signature or finite read-budget mismatch.
    pub fn verify_request(
        self,
        request: &ParticipantEnrollmentRequestV1<'_>,
        signature: &Signature,
    ) -> Result<VerifiedParticipantEnrollmentRequestV1> {
        self.recheck()?;
        ensure!(
            request.network_id == &self.network
                && request.signatory == &self.signatory
                && request.wallet == &self.wallet,
            "request changed original native S/W/network"
        );
        let message = request.signing_message()?;
        let digest: [u8; 32] = Sha256::digest(&message).into();
        ensure!(
            digest == self.challenge.request_sha256,
            "request changed the original challenged signing subject"
        );
        signature.verify(&self.public_key, &message)?;
        let (not_before_ms, not_after_ms) = request.native_time_window()?;
        Ok(VerifiedParticipantEnrollmentRequestV1 {
            wallet: self,
            namespace: request.authentication_namespace.into(),
            actor: request.actor_id.into(),
            operation: request.operation,
            request_id: request.request_id.into(),
            idempotency_key: request.idempotency_key.into(),
            target: request.target.as_str().into(),
            request_sha256: digest,
            body_sha256: Sha256::digest(request.body).into(),
            nonce_sha256: Sha256::digest(request.nonce.as_bytes()).into(),
            not_before_ms,
            not_after_ms,
        })
    }
}
fn require_single_member_wallet(signatory: &AccountId, wallet: &AccountId) -> Result<PublicKey> {
    let key = signatory
        .try_signatory()
        .ok_or_else(|| eyre::eyre!("S is not a native signatory"))?;
    let policy = wallet
        .multisig_policy()
        .ok_or_else(|| eyre::eyre!("W is not native multisig"))?;
    ensure!(
        key.algorithm() == Algorithm::Ed25519
            && policy.threshold() == 1
            && policy.members().len() == 1
            && policy.members()[0].weight() == 1
            && policy.members()[0].public_key() == key
            && signatory != wallet,
        "current W does not grant exact first-release S membership"
    );
    Ok(key.clone())
}
/// Signature plus current ledger membership only. FI-native owner/nonce CAS remains required.
pub struct VerifiedParticipantEnrollmentRequestV1 {
    wallet: VerifiedEnrollmentWalletSignatoryV1,
    namespace: String,
    actor: String,
    operation: ParticipantEnrollmentOperationV1,
    request_id: String,
    idempotency_key: String,
    target: String,
    request_sha256: [u8; 32],
    body_sha256: [u8; 32],
    nonce_sha256: [u8; 32],
    not_before_ms: u64,
    not_after_ms: u64,
}
impl VerifiedParticipantEnrollmentRequestV1 {
    /// Recheck the same original current ledger read before native mutation/dispatch.
    /// # Errors
    /// The finite read expired.
    pub fn recheck(&self) -> Result<()> {
        self.wallet.recheck()
    }
    /// Independently compare primitive evidence with an actual FI release owner.
    /// A caller-selected verifier used at initial crypto verification supplies no FI authority.
    /// The FI must supply its own installed network, four pins, compiled schema and
    /// latest retained verifier/certified height; this method imports no offered root.
    /// # Errors
    /// Foreign network, executable/config selection, schema, root, decision or stale tip.
    pub fn verify_fi_owned_selection(
        &self,
        network: &NetworkId,
        nodes: &[SelectedEnrollmentReadNodeV1; 4],
        compiled_schema: Hash,
        current_verifier: &SumeragiFinalityVerifier,
        current_height: u64,
    ) -> Result<()> {
        self.recheck()?;
        ensure!(
            &self.wallet.network == network
                && &self.wallet.nodes == nodes
                && self.wallet.snapshot.schema_hash() == compiled_schema
                && self.wallet.block.height() == current_height,
            "request differs from actual FI current selection"
        );
        let current = current_verifier.verify_retained_decision(&self.wallet.proof)?;
        ensure!(
            current.context_id() == self.wallet.block.context_id()
                && current.execution().world_state_root == self.wallet.snapshot.world_root(),
            "request differs from actual FI retained certified root"
        );
        self.recheck()
    }
    /// Network selected during primitive verification; not an FI deployment selection.
    #[must_use]
    pub fn network_id(&self) -> &NetworkId {
        &self.wallet.network
    }
    /// Exact FI scope bound by the signature.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    /// Exact FI subject bound by the signature.
    #[must_use]
    pub fn actor_id(&self) -> &str {
        &self.actor
    }
    /// Retained native S.
    #[must_use]
    pub fn signatory(&self) -> &AccountId {
        self.wallet.signatory()
    }
    /// Retained native W.
    #[must_use]
    pub fn wallet(&self) -> &AccountId {
        self.wallet.wallet()
    }
    /// Domain-separated complete signing subject digest.
    #[must_use]
    pub fn request_sha256(&self) -> [u8; 32] {
        self.request_sha256
    }
    /// Exact received body digest.
    #[must_use]
    pub fn body_sha256(&self) -> [u8; 32] {
        self.body_sha256
    }
    /// Fixed purpose authenticated with the exact route and original body.
    #[must_use]
    pub fn operation(&self) -> ParticipantEnrollmentOperationV1 {
        self.operation
    }
    /// Original SDK request identity, retained across uncertain business responses.
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    /// Original business idempotency identity; nonce consumption never replaces it.
    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }
    /// Exact authenticated final HTTPS target, including the independently selected mount.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }
    /// Check the original bytes again at the native dispatch boundary.
    /// # Errors
    /// The retained observation expired or a different body is offered.
    pub fn verify_original_body(&self, body: &[u8]) -> Result<()> {
        self.recheck()?;
        ensure!(
            <[u8; 32]>::from(Sha256::digest(body)) == self.body_sha256,
            "dispatch changed original enrollment body"
        );
        Ok(())
    }
    /// Nonce digest for the FI's permanent claim; the store scopes it by FI and native S.
    #[must_use]
    pub fn nonce_sha256(&self) -> [u8; 32] {
        self.nonce_sha256
    }
    /// Exact bounded native leader-time admission window.
    #[must_use]
    pub fn time_window(&self) -> (u64, u64) {
        (self.not_before_ms, self.not_after_ms)
    }
}

/// Join only actual FI-fetched current originals under its independently held certified prefix.
/// Raw rows/snapshot are data; this constructor establishes primitive request evidence only.
/// The FI's actual customer/session/current selection and nonce CAS remain mandatory.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_fi_current_request_cut(
    challenge: EnrollmentWalletReadChallengeV1,
    request: &ParticipantEnrollmentRequestV1<'_>,
    signature: &Signature,
    verifier: &SumeragiFinalityVerifier,
    proof: &SumeragiFinalityProof,
    compiled_schema: Hash,
    snapshot: &iroha_data_model::sumeragi_finality::WorldStateSnapshotV1,
    signatory_row: (AccountId, iroha_data_model::account::AccountValue),
    wallet_row: (AccountId, iroha_data_model::account::AccountValue),
    nodes: &[SelectedEnrollmentReadNodeV1; 4],
    statements: &[SumeragiFinalityAttestation; 4],
) -> Result<VerifiedParticipantEnrollmentRequestV1> {
    ensure!(
        &signatory_row.0 == request.signatory && &wallet_row.0 == request.wallet,
        "FI account queries changed the exact requested S/W"
    );
    let block = verifier.verify_retained_decision(proof)?;
    let snapshot = snapshot.authenticate(&block)?;
    let wallet = VerifiedEnrollmentWalletSignatoryV1::authenticate(
        challenge,
        *request.network_id,
        nodes,
        verifier,
        proof,
        snapshot,
        compiled_schema,
        statements,
        signatory_row.0,
        &signatory_row.1,
        wallet_row.0,
        &wallet_row.1,
    )?;
    let authenticated = wallet.verify_request(request, signature)?;
    authenticated.verify_fi_owned_selection(
        request.network_id,
        nodes,
        compiled_schema,
        verifier,
        block.height(),
    )?;
    authenticated.recheck()?;
    Ok(authenticated)
}

/// Purpose-specific exact HTTP metadata codec; decoding establishes no FI or Native owner.
#[path = "participant_enrollment_http.rs"]
pub mod http;

#[cfg(test)]
mod tests;

#[cfg(all(test, unix))]
pub(crate) use tests::NativeCustodyFixture;
