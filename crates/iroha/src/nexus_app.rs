//! App-developer facade for the SORA Nexus Connect transfer flow.
//!
//! The facade keeps the existing low-level Connect, transaction, Torii, and
//! pipeline APIs intact. SDK applications can use this module to build a
//! canonical transfer payload, request a wallet signature, finalize the signed
//! transaction, submit it, and optionally wait for a terminal pipeline status.

use std::{num::NonZeroU32, time::Duration};

use iroha_crypto::{Algorithm, PublicKey, Signature};
use iroha_data_model::{
    prelude::{AccountId, AssetId, ChainId, Metadata, Numeric, Transfer},
    transaction::{SignedTransaction, TransactionBuilder},
};
use thiserror::Error;
use url::Url;

use crate::client::{Client, TransactionWaitOptions, TransactionWaitOutcome};

/// V1 supported signature algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NexusSignatureAlgorithm {
    /// Ed25519 transaction signature.
    Ed25519,
}

impl NexusSignatureAlgorithm {
    fn from_public_key(public_key: &PublicKey) -> Result<Self, NexusAppError> {
        let algorithm = public_key
            .try_algorithm()
            .map_err(|source| NexusAppError::MalformedSigningPublicKey { source })?;
        match algorithm {
            Algorithm::Ed25519 => Ok(Self::Ed25519),
            other => Err(NexusAppError::UnsupportedSignatureAlgorithm {
                algorithm: other.as_static_str().to_owned(),
            }),
        }
    }
}

/// Errors surfaced by [`NexusAppClient`].
#[derive(Debug, Error)]
pub enum NexusAppError {
    /// Connect transport is not configured for this client instance.
    #[error("nexus app connect transport is not configured")]
    ConnectTransportUnavailable,
    /// The requested signature algorithm is not supported by the V1 facade.
    #[error("unsupported signature algorithm `{algorithm}`")]
    UnsupportedSignatureAlgorithm {
        /// Algorithm label returned by a wallet or account key.
        algorithm: String,
    },
    /// The approved account did not expose a single Ed25519 signing key.
    #[error("approved account cannot provide a single Ed25519 signing public key")]
    MissingSigningPublicKey,
    /// The selected signing public key is malformed.
    #[error("signing public key is malformed")]
    MalformedSigningPublicKey {
        /// Underlying compact-key parse error.
        #[source]
        source: iroha_crypto::error::ParseError,
    },
    /// No authority was supplied in the input, config, or approved Connect session.
    #[error("transfer authority is required before building a transfer draft")]
    MissingAuthority,
    /// The explicit signing public key does not match the transaction authority.
    #[error("signing public key does not match the transaction authority")]
    SigningPublicKeyMismatch,
    /// Wallet signature was malformed.
    #[error("wallet signature is malformed: {0}")]
    InvalidSignature(String),
    /// Signed transaction verification failed before submission.
    #[error("signed transaction verification failed: {0}")]
    SignatureVerification(String),
    /// Torii returned a transaction hash that does not match the local signed transaction.
    #[error("submitted transaction hash mismatch: local `{local}`, submitted `{submitted}`")]
    TransactionHashMismatch {
        /// Hash computed locally from the signed transaction.
        local: String,
        /// Hash returned by Torii or a submitter implementation.
        submitted: String,
    },
    /// Torii submit failed.
    #[error("transaction submit failed: {0}")]
    Submit(String),
    /// Pipeline status wait failed.
    #[error("transaction status wait failed: {0}")]
    StatusWait(String),
}

impl NexusAppError {
    /// Stable machine-readable error code shared across SDK facades.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::ConnectTransportUnavailable => "connect_transport_unavailable",
            Self::UnsupportedSignatureAlgorithm { .. } => "unsupported_signature_algorithm",
            Self::MissingSigningPublicKey => "missing_signing_public_key",
            Self::MalformedSigningPublicKey { .. } => "malformed_signing_public_key",
            Self::MissingAuthority => "missing_authority",
            Self::SigningPublicKeyMismatch => "approval_account_mismatch",
            Self::InvalidSignature(_) | Self::SignatureVerification(_) => "invalid_signature",
            Self::TransactionHashMismatch { .. } => "transaction_hash_mismatch",
            Self::Submit(_) => "submit_failed",
            Self::StatusWait(_) => "status_wait_failed",
        }
    }
}

/// Static configuration for a Nexus app facade instance.
#[derive(Debug, Clone)]
pub struct NexusAppConfig {
    /// Chain id used by transfer drafts and Connect previews.
    pub chain_id: ChainId,
    /// Optional default transaction authority.
    pub authority: Option<AccountId>,
    /// Optional Torii/Connect base URL used by transport implementations.
    pub torii_url: Option<Url>,
    /// Optional node hint embedded in wallet launch URIs.
    pub node: Option<String>,
    /// Optional app display name sent in Connect open frames.
    pub app_name: Option<String>,
    /// Explicit signing public key override for accounts that cannot derive it.
    pub signing_public_key: Option<PublicKey>,
}

impl NexusAppConfig {
    /// Construct config with the minimum chain id.
    #[must_use]
    pub fn new(chain_id: ChainId) -> Self {
        Self {
            chain_id,
            authority: None,
            torii_url: None,
            node: None,
            app_name: None,
            signing_public_key: None,
        }
    }
}

/// Options for creating a Connect session.
#[derive(Debug, Clone, Default)]
pub struct NexusConnectOptions {
    /// Optional deterministic session id. Transport implementations may
    /// generate one when omitted.
    pub sid: Option<String>,
    /// Optional node hint overriding [`NexusAppConfig::node`].
    pub node: Option<String>,
}

/// Registered Connect session plus wallet launch metadata.
#[derive(Debug, Clone)]
pub struct NexusConnectSession {
    /// Session id.
    pub sid: String,
    /// Wallet deeplink URI.
    pub wallet_launch_uri: String,
    /// App deeplink URI, when returned by Torii.
    pub app_launch_uri: Option<String>,
    /// App role token.
    pub token_app: Option<String>,
    /// Wallet role token.
    pub token_wallet: Option<String>,
    /// Management token for session deletion/status.
    pub token_management: Option<String>,
    /// Relay token used by Connect approval verification.
    pub token_relay: Option<String>,
    /// Account approved by the wallet after [`NexusAppClient::await_approval`].
    pub approved_account: Option<AccountId>,
    /// Ed25519 signing public key resolved from the approval or explicit override.
    pub signing_public_key: Option<PublicKey>,
}

/// Account approved by a wallet Connect session.
#[derive(Debug, Clone)]
pub struct NexusApprovedAccount {
    /// Approved account id.
    pub account_id: AccountId,
    /// Resolved Ed25519 signing public key.
    pub signing_public_key: PublicKey,
}

/// Numeric asset transfer input covered by V1.
#[derive(Debug, Clone)]
pub struct NexusTransferInput {
    /// Source asset holding id.
    pub source_asset_id: AssetId,
    /// Quantity to transfer.
    pub quantity: Numeric,
    /// Destination account id.
    pub destination_account_id: AccountId,
    /// Transaction authority. When omitted, the facade uses config/session authority.
    pub authority: Option<AccountId>,
    /// Transaction metadata.
    pub metadata: Metadata,
    /// Optional deterministic creation timestamp.
    pub creation_time_ms: Option<u64>,
    /// Optional transaction TTL.
    pub ttl: Option<Duration>,
    /// Optional transaction nonce.
    pub nonce: Option<NonZeroU32>,
}

/// Canonical draft returned by [`NexusAppClient::build_transfer_draft`].
#[derive(Debug, Clone)]
pub struct NexusTransferDraft {
    /// Source transfer input after default authority resolution.
    pub input: NexusTransferInput,
    /// Canonical signable transaction.
    pub signable: NexusSignableTransaction,
}

/// Canonical transaction payload that can be signed by a wallet.
#[derive(Debug, Clone)]
pub struct NexusSignableTransaction {
    builder: TransactionBuilder,
    /// Canonical `TransactionPayload` bytes.
    pub payload_bytes: Vec<u8>,
    /// Hex-encoded Iroha payload prehash signed by Ed25519.
    pub payload_hash_hex: String,
    /// Transaction authority.
    pub authority: AccountId,
    /// Signing algorithm requested from the wallet.
    pub signature_algorithm: NexusSignatureAlgorithm,
    /// Expected signing public key, when known.
    pub signing_public_key: Option<PublicKey>,
}

/// Wallet signature response.
#[derive(Debug, Clone)]
pub struct NexusWalletSignature {
    /// Signature algorithm used by the wallet.
    pub algorithm: NexusSignatureAlgorithm,
    /// Raw signature bytes.
    pub signature: Vec<u8>,
}

/// Options for finalization and Torii submission.
#[derive(Debug, Clone, Default)]
pub struct NexusFinalizeOptions {
    /// Wait options. When omitted, the transaction is submitted without polling.
    pub wait: Option<TransactionWaitOptions>,
}

/// Receipt returned after finalization and Torii submission.
#[derive(Debug, Clone)]
pub struct NexusTransferReceipt {
    /// Final signed transaction submitted to Torii.
    pub signed_transaction: SignedTransaction,
    /// Hex-encoded signed transaction hash.
    pub signed_transaction_hash_hex: String,
    /// Optional terminal pipeline status.
    pub status: Option<TransactionWaitOutcome>,
}

/// Connect transport required by the facade.
pub trait NexusConnectTransport {
    /// Register a Connect session and return launch metadata.
    ///
    /// # Errors
    /// Returns an error if the transport is unavailable or cannot register the
    /// Connect session.
    fn start_connect(
        &self,
        config: &NexusAppConfig,
        options: NexusConnectOptions,
    ) -> Result<NexusConnectSession, NexusAppError>;

    /// Wait for wallet approval and return the approved account.
    ///
    /// # Errors
    /// Returns an error if approval fails or the wallet response cannot be
    /// converted into a Nexus-approved account.
    fn await_approval(
        &self,
        session: &mut NexusConnectSession,
    ) -> Result<NexusApprovedAccount, NexusAppError>;

    /// Request a wallet signature for the canonical payload bytes.
    ///
    /// # Errors
    /// Returns an error if the transport cannot request or receive a wallet
    /// signature.
    fn request_signature(
        &self,
        session: &NexusConnectSession,
        signable: &NexusSignableTransaction,
    ) -> Result<NexusWalletSignature, NexusAppError>;
}

/// Torii submission dependency used by the facade.
pub trait NexusToriiSubmitter {
    /// Submit the signed transaction and optionally wait for final status.
    ///
    /// # Errors
    /// Returns an error if Torii submission fails or waiting for terminal status
    /// fails.
    fn submit_and_wait(
        &self,
        transaction: &SignedTransaction,
        options: NexusFinalizeOptions,
    ) -> Result<NexusTransferReceipt, NexusAppError>;
}

/// Placeholder Connect transport used when callers construct from a bare Torii client.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedConnectTransport;

impl NexusConnectTransport for UnsupportedConnectTransport {
    fn start_connect(
        &self,
        _config: &NexusAppConfig,
        _options: NexusConnectOptions,
    ) -> Result<NexusConnectSession, NexusAppError> {
        Err(NexusAppError::ConnectTransportUnavailable)
    }

    fn await_approval(
        &self,
        _session: &mut NexusConnectSession,
    ) -> Result<NexusApprovedAccount, NexusAppError> {
        Err(NexusAppError::ConnectTransportUnavailable)
    }

    fn request_signature(
        &self,
        _session: &NexusConnectSession,
        _signable: &NexusSignableTransaction,
    ) -> Result<NexusWalletSignature, NexusAppError> {
        Err(NexusAppError::ConnectTransportUnavailable)
    }
}

impl NexusToriiSubmitter for Client {
    fn submit_and_wait(
        &self,
        transaction: &SignedTransaction,
        options: NexusFinalizeOptions,
    ) -> Result<NexusTransferReceipt, NexusAppError> {
        let hash = self
            .submit_transaction(transaction)
            .map_err(|err| NexusAppError::Submit(err.to_string()))?;
        let status = options
            .wait
            .map(|wait| {
                self.wait_for_transaction_terminal_status(hash, wait)
                    .map_err(|err| NexusAppError::StatusWait(err.to_string()))
            })
            .transpose()?;

        Ok(NexusTransferReceipt {
            signed_transaction: transaction.clone(),
            signed_transaction_hash_hex: hex::encode(hash.as_ref()),
            status,
        })
    }
}

/// High-level Nexus app facade.
#[derive(Debug, Clone)]
pub struct NexusAppClient<C = UnsupportedConnectTransport, S = Client> {
    config: NexusAppConfig,
    connect: C,
    submitter: S,
}

impl NexusAppClient<UnsupportedConnectTransport, Client> {
    /// Construct a facade over an existing Torii client.
    ///
    /// Connect operations require constructing the generic client with a
    /// concrete [`NexusConnectTransport`].
    #[must_use]
    pub fn from_client(config: NexusAppConfig, client: Client) -> Self {
        Self {
            config,
            connect: UnsupportedConnectTransport,
            submitter: client,
        }
    }
}

impl<C, S> NexusAppClient<C, S>
where
    C: NexusConnectTransport,
    S: NexusToriiSubmitter,
{
    /// Construct a facade from explicit Connect and Torii dependencies.
    #[must_use]
    pub fn new(config: NexusAppConfig, connect: C, submitter: S) -> Self {
        Self {
            config,
            connect,
            submitter,
        }
    }

    /// Register an app-role Connect session and return wallet launch metadata.
    ///
    /// # Errors
    /// Returns an error if the configured Connect transport cannot register a
    /// session.
    pub fn start_connect(
        &self,
        options: NexusConnectOptions,
    ) -> Result<NexusConnectSession, NexusAppError> {
        self.connect.start_connect(&self.config, options)
    }

    /// Wait for wallet approval and cache the resolved account on the session.
    ///
    /// # Errors
    /// Returns an error if wallet approval fails, the approved signing key is
    /// unsupported, or it does not match the approved account.
    pub fn await_approval(
        &self,
        session: &mut NexusConnectSession,
    ) -> Result<NexusApprovedAccount, NexusAppError> {
        let mut approved = self.connect.await_approval(session)?;
        approved.signing_public_key = resolve_signing_public_key(
            &approved.account_id,
            self.config
                .signing_public_key
                .as_ref()
                .or(Some(&approved.signing_public_key)),
        )?;
        session.approved_account = Some(approved.account_id.clone());
        session.signing_public_key = Some(approved.signing_public_key.clone());
        Ok(approved)
    }

    /// Build a canonical signable numeric asset transfer.
    ///
    /// # Errors
    /// Returns an error if the authority or signing key cannot be resolved, the
    /// selected key is malformed, or the selected signing algorithm is unsupported.
    pub fn build_transfer_draft(
        &self,
        input: NexusTransferInput,
    ) -> Result<NexusTransferDraft, NexusAppError> {
        let signable = self.build_signable_transfer(&input, None)?;
        Ok(NexusTransferDraft { input, signable })
    }

    /// Send a wallet `SIGN_REQUEST_TX` for the canonical transaction payload.
    ///
    /// # Errors
    /// Returns an error if the configured Connect transport cannot request or
    /// receive a wallet signature.
    pub fn request_signature(
        &self,
        session: &NexusConnectSession,
        signable: &NexusSignableTransaction,
    ) -> Result<NexusWalletSignature, NexusAppError> {
        self.connect.request_signature(session, signable)
    }

    /// Build a signed transaction from a wallet signature, submit it to Torii,
    /// and optionally wait for a terminal pipeline status.
    ///
    /// # Errors
    /// Returns an error if the wallet signature is unsupported or malformed, if
    /// signed transaction verification fails, or if Torii submission/status
    /// waiting fails.
    pub fn finalize_and_submit(
        &self,
        signable: NexusSignableTransaction,
        signature: NexusWalletSignature,
        options: NexusFinalizeOptions,
    ) -> Result<NexusTransferReceipt, NexusAppError> {
        let NexusWalletSignature {
            algorithm,
            signature: signature_bytes,
        } = signature;
        if algorithm != NexusSignatureAlgorithm::Ed25519 {
            return Err(NexusAppError::UnsupportedSignatureAlgorithm {
                algorithm: format!("{algorithm:?}"),
            });
        }
        if signature_bytes.len() != 64 {
            return Err(NexusAppError::InvalidSignature(format!(
                "Ed25519 signature must be 64 bytes, got {}",
                signature_bytes.len()
            )));
        }

        if let Some(signing_public_key) = signable.signing_public_key.as_ref() {
            ensure_authority_matches_public_key(&signable.authority, signing_public_key)?;
        } else {
            let _ = resolve_signing_public_key(
                &signable.authority,
                self.config.signing_public_key.as_ref(),
            )?;
        }

        let signed = signable
            .builder
            .build_with_signature(Signature::from_bytes(&signature_bytes));
        signed
            .verify_signature()
            .map_err(|err| NexusAppError::SignatureVerification(err.to_string()))?;

        let receipt = self.submitter.submit_and_wait(&signed, options)?;
        let local_hash_hex = hex::encode(signed.hash().as_ref());
        if receipt.signed_transaction_hash_hex != local_hash_hex {
            return Err(NexusAppError::TransactionHashMismatch {
                local: local_hash_hex,
                submitted: receipt.signed_transaction_hash_hex,
            });
        }
        Ok(receipt)
    }

    /// Convenience wrapper over draft, wallet signature, finalize, submit, and wait.
    ///
    /// # Errors
    /// Returns an error if authority resolution, wallet signing, transaction
    /// verification, submission, or status waiting fails.
    pub fn transfer_with_wallet(
        &self,
        session: &NexusConnectSession,
        input: NexusTransferInput,
        options: NexusFinalizeOptions,
    ) -> Result<NexusTransferReceipt, NexusAppError> {
        let authority = input
            .authority
            .clone()
            .or_else(|| session.approved_account.clone())
            .or_else(|| self.config.authority.clone())
            .ok_or(NexusAppError::MissingAuthority)?;
        if session
            .approved_account
            .as_ref()
            .zip(input.authority.as_ref())
            .is_some_and(|(approved, requested)| approved != requested)
        {
            return Err(NexusAppError::SigningPublicKeyMismatch);
        }
        let signing_public_key = resolve_signing_public_key(
            &authority,
            session
                .signing_public_key
                .as_ref()
                .or(self.config.signing_public_key.as_ref()),
        )?;
        let mut resolved_input = input;
        resolved_input.authority = Some(authority);
        let signable = self.build_signable_transfer(&resolved_input, Some(signing_public_key))?;
        let signature = self.request_signature(session, &signable)?;
        self.finalize_and_submit(signable, signature, options)
    }

    fn build_signable_transfer(
        &self,
        input: &NexusTransferInput,
        signing_public_key: Option<PublicKey>,
    ) -> Result<NexusSignableTransaction, NexusAppError> {
        let authority = input
            .authority
            .clone()
            .or_else(|| self.config.authority.clone())
            .ok_or(NexusAppError::MissingAuthority)?;
        let signing_public_key = signing_public_key
            .or_else(|| self.config.signing_public_key.clone())
            .or_else(|| authority.try_signatory().cloned());
        let signature_algorithm = match signing_public_key.as_ref() {
            Some(public_key) => NexusSignatureAlgorithm::from_public_key(public_key)?,
            None => return Err(NexusAppError::MissingSigningPublicKey),
        };

        let mut builder = TransactionBuilder::new(self.config.chain_id.clone(), authority.clone())
            .with_instructions([Transfer::asset_numeric(
                input.source_asset_id.clone(),
                input.quantity.clone(),
                input.destination_account_id.clone(),
            )])
            .with_metadata(input.metadata.clone());
        if let Some(creation_time_ms) = input.creation_time_ms {
            builder.set_creation_time(Duration::from_millis(creation_time_ms));
        }
        if let Some(ttl) = input.ttl {
            builder.set_ttl(ttl);
        }
        if let Some(nonce) = input.nonce {
            builder.set_nonce(nonce);
        }
        let payload_bytes = builder.encode_payload();
        let payload_hash_hex = hex::encode(builder.payload_hash_bytes());

        Ok(NexusSignableTransaction {
            builder,
            payload_bytes,
            payload_hash_hex,
            authority,
            signature_algorithm,
            signing_public_key,
        })
    }
}

fn resolve_signing_public_key(
    account_id: &AccountId,
    explicit: Option<&PublicKey>,
) -> Result<PublicKey, NexusAppError> {
    let public_key = explicit
        .cloned()
        .or_else(|| account_id.try_signatory().cloned())
        .ok_or(NexusAppError::MissingSigningPublicKey)?;
    NexusSignatureAlgorithm::from_public_key(&public_key)?;
    ensure_authority_matches_public_key(account_id, &public_key)?;
    Ok(public_key)
}

fn ensure_authority_matches_public_key(
    account_id: &AccountId,
    public_key: &PublicKey,
) -> Result<(), NexusAppError> {
    match account_id.try_signatory() {
        Some(signatory) if signatory == public_key => Ok(()),
        Some(_) => Err(NexusAppError::SigningPublicKeyMismatch),
        None => Err(NexusAppError::MissingSigningPublicKey),
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use iroha_crypto::{KeyPair, Signature};
    use iroha_data_model::{asset::AssetDefinitionId, prelude::Name};
    use iroha_primitives::json::Json;

    use super::*;

    const FIXTURE: &str = include_str!("../../../fixtures/sdk/nexus_connect_transfer_v1.json");

    #[derive(Debug, Clone)]
    struct FakeConnect {
        account: AccountId,
        signature: Vec<u8>,
        requested_payloads: Rc<RefCell<Vec<Vec<u8>>>>,
    }

    impl NexusConnectTransport for FakeConnect {
        fn start_connect(
            &self,
            _config: &NexusAppConfig,
            options: NexusConnectOptions,
        ) -> Result<NexusConnectSession, NexusAppError> {
            let sid = options.sid.unwrap_or_else(|| "test-sid".to_owned());
            Ok(NexusConnectSession {
                sid: sid.clone(),
                wallet_launch_uri: format!("iroha://connect?sid={sid}&role=wallet"),
                app_launch_uri: None,
                token_app: Some("app-token".to_owned()),
                token_wallet: Some("wallet-token".to_owned()),
                token_management: Some("management-token".to_owned()),
                token_relay: Some("relay-token".to_owned()),
                approved_account: None,
                signing_public_key: None,
            })
        }

        fn await_approval(
            &self,
            _session: &mut NexusConnectSession,
        ) -> Result<NexusApprovedAccount, NexusAppError> {
            Ok(NexusApprovedAccount {
                account_id: self.account.clone(),
                signing_public_key: self.account.signatory().clone(),
            })
        }

        fn request_signature(
            &self,
            _session: &NexusConnectSession,
            signable: &NexusSignableTransaction,
        ) -> Result<NexusWalletSignature, NexusAppError> {
            self.requested_payloads
                .borrow_mut()
                .push(signable.payload_bytes.clone());
            Ok(NexusWalletSignature {
                algorithm: NexusSignatureAlgorithm::Ed25519,
                signature: self.signature.clone(),
            })
        }
    }

    #[derive(Debug, Clone, Default)]
    struct FakeSubmitter {
        submitted_hashes: Rc<RefCell<Vec<String>>>,
    }

    impl NexusToriiSubmitter for FakeSubmitter {
        fn submit_and_wait(
            &self,
            transaction: &SignedTransaction,
            _options: NexusFinalizeOptions,
        ) -> Result<NexusTransferReceipt, NexusAppError> {
            let hash_hex = hex::encode(transaction.hash().as_ref());
            self.submitted_hashes.borrow_mut().push(hash_hex.clone());
            Ok(NexusTransferReceipt {
                signed_transaction: transaction.clone(),
                signed_transaction_hash_hex: hash_hex,
                status: None,
            })
        }
    }

    #[derive(Debug, Clone)]
    struct MismatchedHashSubmitter;

    impl NexusToriiSubmitter for MismatchedHashSubmitter {
        fn submit_and_wait(
            &self,
            transaction: &SignedTransaction,
            _options: NexusFinalizeOptions,
        ) -> Result<NexusTransferReceipt, NexusAppError> {
            Ok(NexusTransferReceipt {
                signed_transaction: transaction.clone(),
                signed_transaction_hash_hex: "00".repeat(32),
                status: None,
            })
        }
    }

    #[derive(Debug)]
    struct FailingSubmitter {
        error: NexusAppError,
    }

    impl NexusToriiSubmitter for FailingSubmitter {
        fn submit_and_wait(
            &self,
            _transaction: &SignedTransaction,
            _options: NexusFinalizeOptions,
        ) -> Result<NexusTransferReceipt, NexusAppError> {
            Err(match &self.error {
                NexusAppError::Submit(message) => NexusAppError::Submit(message.clone()),
                NexusAppError::StatusWait(message) => NexusAppError::StatusWait(message.clone()),
                other => panic!("unexpected fake submitter error: {other:?}"),
            })
        }
    }

    fn sample_input(authority: AccountId) -> NexusTransferInput {
        let definition = AssetDefinitionId::from_uuid_bytes_unchecked([
            0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x42, 0x22, 0x82, 0x22, 0x22, 0x22, 0x22, 0x22,
            0x22, 0x22,
        ]);
        NexusTransferInput {
            source_asset_id: AssetId::new(definition, authority.clone()),
            quantity: Numeric::new(125_u32, 2),
            destination_account_id: authority.clone(),
            authority: Some(authority),
            metadata: Metadata::default(),
            creation_time_ms: Some(1_700_000_000_000),
            ttl: Some(Duration::from_secs(60)),
            nonce: Some(NonZeroU32::new(7).expect("nonzero")),
        }
    }

    fn fixture_string(key: &str) -> String {
        let needle = format!("\"{key}\": \"");
        let start = FIXTURE.find(&needle).expect("fixture key") + needle.len();
        let rest = &FIXTURE[start..];
        rest[..rest.find('"').expect("fixture string terminator")].to_owned()
    }

    fn fixture_u64(key: &str) -> u64 {
        let needle = format!("\"{key}\": ");
        let start = FIXTURE.find(&needle).expect("fixture numeric key") + needle.len();
        let rest = &FIXTURE[start..];
        let end = rest
            .find(|ch: char| !ch.is_ascii_digit())
            .expect("fixture numeric terminator");
        rest[..end].parse().expect("fixture integer")
    }

    fn fixture_account(key: &str) -> AccountId {
        AccountId::parse_encoded(&fixture_string(key))
            .map(|parsed| parsed.into_account_id())
            .expect("fixture account")
    }

    fn fixture_transfer_input() -> NexusTransferInput {
        let authority = fixture_account("authority");
        let source_asset = fixture_string("source_asset_id");
        let (definition, source_account) = source_asset.split_once('#').expect("asset separator");
        assert_eq!(source_account, fixture_string("authority"));
        let mut metadata = Metadata::default();
        metadata.insert(
            "purpose".parse::<Name>().expect("metadata key"),
            Json::from("nexus-app-fixture"),
        );

        NexusTransferInput {
            source_asset_id: AssetId::new(
                definition.parse().expect("asset definition"),
                authority.clone(),
            ),
            quantity: fixture_string("quantity").parse().expect("quantity"),
            destination_account_id: fixture_account("destination_account_id"),
            authority: Some(authority),
            metadata,
            creation_time_ms: Some(fixture_u64("creation_time_ms")),
            ttl: Some(Duration::from_millis(fixture_u64("ttl_ms"))),
            nonce: Some(NonZeroU32::new(fixture_u64("nonce") as u32).expect("nonce")),
        }
    }

    #[test]
    fn nexus_app_builds_transfer_draft_and_finalizes_wallet_signature() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let config = NexusAppConfig {
            authority: Some(account.clone()),
            ..NexusAppConfig::new("test-chain".into())
        };
        let bootstrap_client = NexusAppClient::new(
            config.clone(),
            UnsupportedConnectTransport,
            FakeSubmitter::default(),
        );
        let draft = bootstrap_client
            .build_transfer_draft(sample_input(account.clone()))
            .expect("draft");
        assert!(!draft.signable.payload_bytes.is_empty());
        assert_eq!(
            draft.signable.payload_hash_hex,
            hex::encode(draft.signable.builder.payload_hash_bytes())
        );

        let signature = Signature::new(
            key_pair.private_key(),
            &draft.signable.builder.payload_hash_bytes(),
        );
        let submitter = FakeSubmitter::default();
        let client = NexusAppClient::new(config, UnsupportedConnectTransport, submitter.clone());
        let receipt = client
            .finalize_and_submit(
                draft.signable,
                NexusWalletSignature {
                    algorithm: NexusSignatureAlgorithm::Ed25519,
                    signature: signature.payload().to_vec(),
                },
                NexusFinalizeOptions::default(),
            )
            .expect("receipt");
        assert!(receipt.signed_transaction.verify_signature().is_ok());
        assert_eq!(submitter.submitted_hashes.borrow().len(), 1);
    }

    #[test]
    fn nexus_app_transfer_with_wallet_runs_connect_sign_submit_flow() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let config = NexusAppConfig {
            authority: Some(account.clone()),
            ..NexusAppConfig::new("test-chain".into())
        };
        let draft = NexusAppClient::new(
            config.clone(),
            UnsupportedConnectTransport,
            FakeSubmitter::default(),
        )
        .build_transfer_draft(sample_input(account.clone()))
        .expect("draft");
        let signature = Signature::new(
            key_pair.private_key(),
            &draft.signable.builder.payload_hash_bytes(),
        );
        let connect = FakeConnect {
            account: account.clone(),
            signature: signature.payload().to_vec(),
            requested_payloads: Rc::new(RefCell::new(Vec::new())),
        };
        let submitter = FakeSubmitter::default();
        let client = NexusAppClient::new(config, connect.clone(), submitter.clone());
        let mut session = client
            .start_connect(NexusConnectOptions {
                sid: Some("sid-1".to_owned()),
                node: None,
            })
            .expect("session");
        client.await_approval(&mut session).expect("approval");
        let receipt = client
            .transfer_with_wallet(
                &session,
                sample_input(account),
                NexusFinalizeOptions::default(),
            )
            .expect("receipt");

        assert!(receipt.signed_transaction.verify_signature().is_ok());
        assert_eq!(connect.requested_payloads.borrow().len(), 1);
        assert_eq!(submitter.submitted_hashes.borrow().len(), 1);
    }

    #[test]
    fn nexus_app_transfer_payload_matches_shared_fixture() {
        let public_key = PublicKey::from_bytes(
            Algorithm::Ed25519,
            &hex::decode(fixture_string("signing_public_key_hex")).expect("public key hex"),
        )
        .expect("fixture public key");
        let config = NexusAppConfig {
            signing_public_key: Some(public_key),
            ..NexusAppConfig::new(fixture_string("chain_id").into())
        };
        let client = NexusAppClient::new(
            config,
            UnsupportedConnectTransport,
            FakeSubmitter::default(),
        );

        let draft = client
            .build_transfer_draft(fixture_transfer_input())
            .expect("fixture draft");

        assert_eq!(
            hex::encode(&draft.signable.payload_bytes),
            fixture_string("payload_bytes_hex")
        );
        assert_eq!(
            draft.signable.payload_hash_hex,
            fixture_string("payload_hash_hex")
        );
    }

    #[test]
    fn nexus_app_signed_transaction_hash_matches_shared_fixture() {
        let public_key = PublicKey::from_bytes(
            Algorithm::Ed25519,
            &hex::decode(fixture_string("signing_public_key_hex")).expect("public key hex"),
        )
        .expect("fixture public key");
        let config = NexusAppConfig {
            signing_public_key: Some(public_key),
            ..NexusAppConfig::new(fixture_string("chain_id").into())
        };
        let submitter = FakeSubmitter::default();
        let client = NexusAppClient::new(config, UnsupportedConnectTransport, submitter);
        let draft = client
            .build_transfer_draft(fixture_transfer_input())
            .expect("fixture draft");
        let receipt = client
            .finalize_and_submit(
                draft.signable,
                NexusWalletSignature {
                    algorithm: NexusSignatureAlgorithm::Ed25519,
                    signature: hex::decode(fixture_string("wallet_signature_hex"))
                        .expect("wallet signature hex"),
                },
                NexusFinalizeOptions::default(),
            )
            .expect("fixture receipt");

        assert_eq!(
            receipt.signed_transaction_hash_hex,
            fixture_string("signed_transaction_hash_hex")
        );
    }

    #[test]
    fn nexus_app_error_codes_are_stable() {
        assert_eq!(
            NexusAppError::UnsupportedSignatureAlgorithm {
                algorithm: "secp256k1".to_owned(),
            }
            .code(),
            "unsupported_signature_algorithm"
        );
        assert_eq!(
            NexusAppError::MissingSigningPublicKey.code(),
            "missing_signing_public_key"
        );
        let secp_key_pair = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
        let malformed_or_unsupported =
            NexusSignatureAlgorithm::from_public_key(secp_key_pair.public_key())
                .expect_err("secp256k1 is unsupported by the V1 facade");
        assert_eq!(
            malformed_or_unsupported.code(),
            "unsupported_signature_algorithm"
        );
        assert_eq!(
            NexusAppError::SigningPublicKeyMismatch.code(),
            "approval_account_mismatch"
        );
        assert_eq!(
            NexusAppError::TransactionHashMismatch {
                local: "local".to_owned(),
                submitted: "submitted".to_owned(),
            }
            .code(),
            "transaction_hash_mismatch"
        );
        assert_eq!(
            NexusAppError::ConnectTransportUnavailable.code(),
            "connect_transport_unavailable"
        );
        assert_eq!(NexusAppError::MissingAuthority.code(), "missing_authority");
        assert_eq!(
            NexusAppError::InvalidSignature("bad".to_owned()).code(),
            "invalid_signature"
        );
        assert_eq!(
            NexusAppError::Submit("down".to_owned()).code(),
            "submit_failed"
        );
        assert_eq!(
            NexusAppError::StatusWait("timeout".to_owned()).code(),
            "status_wait_failed"
        );
    }

    #[test]
    fn nexus_app_rejects_non_ed25519_signing_key_before_building_draft() {
        let secp_key_pair = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
        let account = AccountId::new(secp_key_pair.public_key().clone());
        let client = NexusAppClient::new(
            NexusAppConfig {
                authority: Some(account.clone()),
                signing_public_key: Some(secp_key_pair.public_key().clone()),
                ..NexusAppConfig::new("test-chain".into())
            },
            UnsupportedConnectTransport,
            FakeSubmitter::default(),
        );

        let error = client
            .build_transfer_draft(sample_input(account))
            .expect_err("non-Ed25519 signing key must be rejected");

        assert!(matches!(
            error,
            NexusAppError::UnsupportedSignatureAlgorithm { algorithm }
                if algorithm == Algorithm::Secp256k1.as_static_str()
        ));
    }

    #[test]
    fn nexus_app_rejects_authority_mismatch_before_requesting_signature() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let other = AccountId::new(
            KeyPair::random_with_algorithm(Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let connect = FakeConnect {
            account: account.clone(),
            signature: vec![7; 64],
            requested_payloads: Rc::new(RefCell::new(Vec::new())),
        };
        let client = NexusAppClient::new(
            NexusAppConfig {
                authority: Some(account.clone()),
                ..NexusAppConfig::new("test-chain".into())
            },
            connect.clone(),
            FakeSubmitter::default(),
        );
        let mut session = client
            .start_connect(NexusConnectOptions::default())
            .expect("session");
        client.await_approval(&mut session).expect("approval");
        let error = client
            .transfer_with_wallet(
                &session,
                sample_input(other),
                NexusFinalizeOptions::default(),
            )
            .expect_err("authority mismatch");

        assert_eq!(error.code(), "approval_account_mismatch");
        assert!(connect.requested_payloads.borrow().is_empty());
    }

    #[test]
    fn nexus_app_rejects_invalid_signature_length_before_submission() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let client = NexusAppClient::new(
            NexusAppConfig {
                authority: Some(account.clone()),
                ..NexusAppConfig::new("test-chain".into())
            },
            UnsupportedConnectTransport,
            FakeSubmitter::default(),
        );
        let draft = client
            .build_transfer_draft(sample_input(account))
            .expect("draft");
        let error = client
            .finalize_and_submit(
                draft.signable,
                NexusWalletSignature {
                    algorithm: NexusSignatureAlgorithm::Ed25519,
                    signature: vec![7; 63],
                },
                NexusFinalizeOptions::default(),
            )
            .expect_err("invalid signature");

        assert_eq!(error.code(), "invalid_signature");
    }

    #[test]
    fn nexus_app_rejects_transaction_hash_mismatch() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let client = NexusAppClient::new(
            NexusAppConfig {
                authority: Some(account.clone()),
                ..NexusAppConfig::new("test-chain".into())
            },
            UnsupportedConnectTransport,
            MismatchedHashSubmitter,
        );
        let draft = client
            .build_transfer_draft(sample_input(account))
            .expect("draft");
        let signature = Signature::new(
            key_pair.private_key(),
            &draft.signable.builder.payload_hash_bytes(),
        );
        let error = client
            .finalize_and_submit(
                draft.signable,
                NexusWalletSignature {
                    algorithm: NexusSignatureAlgorithm::Ed25519,
                    signature: signature.payload().to_vec(),
                },
                NexusFinalizeOptions::default(),
            )
            .expect_err("hash mismatch");

        assert_eq!(error.code(), "transaction_hash_mismatch");
    }

    #[test]
    fn nexus_app_preserves_submit_and_status_wait_error_codes() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let config = NexusAppConfig {
            authority: Some(account.clone()),
            ..NexusAppConfig::new("test-chain".into())
        };
        for (error, expected_code) in [
            (NexusAppError::Submit("down".to_owned()), "submit_failed"),
            (
                NexusAppError::StatusWait("timeout".to_owned()),
                "status_wait_failed",
            ),
        ] {
            let client = NexusAppClient::new(
                config.clone(),
                UnsupportedConnectTransport,
                FailingSubmitter { error },
            );
            let draft = client
                .build_transfer_draft(sample_input(account.clone()))
                .expect("draft");
            let signature = Signature::new(
                key_pair.private_key(),
                &draft.signable.builder.payload_hash_bytes(),
            );
            let error = client
                .finalize_and_submit(
                    draft.signable,
                    NexusWalletSignature {
                        algorithm: NexusSignatureAlgorithm::Ed25519,
                        signature: signature.payload().to_vec(),
                    },
                    NexusFinalizeOptions::default(),
                )
                .expect_err("submitter failure");

            assert_eq!(error.code(), expected_code);
        }
    }
}
