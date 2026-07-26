//! Exhaustive native privacy proof verification boundary.
//!
//! An admitted envelope must pass the locally compiled governance manifest,
//! intrinsic typed validation, execution-context binding, strict native wire
//! decoding, and the protocol's cryptographic verifier in that order. Only
//! this module can construct [`VerifiedPrivacyEffectsV1`], so state handlers
//! cannot derive ledger effects from unverified caller-controlled bytes.

use iroha_data_model::{
    ChainId,
    privacy::{
        AnonymousPgcKOutOfNStatementV1, PrivacyConsensusLimitsV1, PrivacyNamespaceV1,
        PrivacyP256CiphertextV1, PrivacyP256PointV1, PrivacyPgcAccountBootstrapDigestV1,
        PrivacyPgcAccountV1, PrivacyPgcBootstrapProofDigestV1, PrivacyProofBytesV1,
        PrivacyProofEnvelopeV1, PrivacyProofEnvelopeValidationError, PrivacyProofV1,
        PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1, PrivacyRootV1,
        PrivacyStatementDigestV1, PrivacyStatementV1, PrivacyVeRangeBitLengthV1,
    },
};
use thiserror::Error;

use crate::{
    privacy_engines::{
        anonymous_pgc::{
            AnonymousPgcError, AnonymousPgcParametersV1, AnonymousPgcPoolInvariantV1,
            TwistedElGamalCiphertextV1, TwistedElGamalPublicKeyV1,
            payment::{AnonymousPgcPaymentStatementV1, verify_payment_encoded},
        },
        p256::{CompressedPointV1, TranscriptBindingV1},
        verange::{
            VeRangeBitLengthV1, VeRangeError, VeRangeParametersV1, VeRangeType1BatchStatementV1,
            verify_batch_encoded,
        },
    },
    privacy_profiles::{
        CompiledPrivacyProfileValidationErrorV1, validate_compiled_privacy_activation_v1,
    },
    privacy_state::compute_privacy_pgc_account_state_root_v1,
};

/// Complete trusted PGC pool state selected before native verification.
///
/// The submit handler constructs this only after its bounded state loader has
/// validated the persisted invariant, head, retained history, account epochs,
/// provenance, and complete strict account order.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PrivacyPgcVerificationStateV1<'a> {
    /// Exact protocol/pool namespace selected by the statement.
    pub(crate) namespace: PrivacyNamespaceV1,
    /// Immutable public supply established by bootstrap.
    pub(crate) total_supply: u32,
    /// Digest of the canonical bootstrap public input.
    pub(crate) bootstrap_digest: PrivacyPgcAccountBootstrapDigestV1,
    /// Digest of the exact canonical bootstrap proof admitted by core.
    pub(crate) bootstrap_proof_digest: PrivacyPgcBootstrapProofDigestV1,
    /// Current account-state root from the persisted head.
    pub(crate) current_root: PrivacyRootV1,
    /// Current account-state epoch from the persisted head.
    pub(crate) current_epoch: u64,
    /// Exact retained-history membership record for the current head.
    pub(crate) retained_current_root: Option<(u64, PrivacyRootV1)>,
    /// Complete account table in strict public-key order.
    pub(crate) accounts: &'a [PrivacyPgcAccountV1],
}

/// Consensus context not supplied by the proof submitter.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PrivacyVerificationContextV1<'a> {
    /// Exact locally stored active record selected by protocol id.
    pub(crate) activation: &'a PrivacyProtocolActivationRecordV1,
    /// Singleton chain-wide limits effective for this incoming block.
    pub(crate) consensus_limits: &'a PrivacyConsensusLimitsV1,
    /// Exact node-configured chain identity.
    pub(crate) chain_id: &'a ChainId,
    /// Hash of the committed genesis block.
    pub(crate) genesis_hash: [u8; 32],
    /// Height of the block executing this proof.
    pub(crate) current_height: u64,
    /// Next zero-based privacy action index in this transaction.
    pub(crate) expected_action_index: u32,
    /// Canonical current block timestamp in Unix milliseconds.
    ///
    /// VeRange does not use time, but credential profiles consume this same
    /// trusted field rather than accepting a prover-selected clock.
    pub(crate) block_timestamp_ms: u64,
    /// Complete trusted PGC state, required only by Anonymous-PGC payments.
    pub(crate) pgc_state: Option<PrivacyPgcVerificationStateV1<'a>>,
}

/// Complete successor account-table transition derived by the native verifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VerifiedAnonymousPgcLedgerEffectV1 {
    namespace: PrivacyNamespaceV1,
    total_supply: u32,
    current_root: PrivacyRootV1,
    current_epoch: u64,
    next_root: PrivacyRootV1,
    next_epoch: u64,
    accounts: Vec<PrivacyPgcAccountV1>,
}

impl VerifiedAnonymousPgcLedgerEffectV1 {
    /// Exact pool namespace whose complete table must change.
    #[must_use]
    pub(crate) const fn namespace(&self) -> PrivacyNamespaceV1 {
        self.namespace
    }

    /// Immutable verified total supply.
    #[must_use]
    pub(crate) const fn total_supply(&self) -> u32 {
        self.total_supply
    }

    /// Persisted head root consumed by this transition.
    #[must_use]
    pub(crate) const fn current_root(&self) -> PrivacyRootV1 {
        self.current_root
    }

    /// Persisted head epoch consumed by this transition.
    #[must_use]
    pub(crate) const fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    /// Canonical successor root derived from every successor ciphertext.
    #[must_use]
    pub(crate) const fn next_root(&self) -> PrivacyRootV1 {
        self.next_root
    }

    /// Canonical successor epoch.
    #[must_use]
    pub(crate) const fn next_epoch(&self) -> u64 {
        self.next_epoch
    }

    /// Complete successor account table in unchanged strict key order.
    #[must_use]
    pub(crate) fn accounts(&self) -> &[PrivacyPgcAccountV1] {
        &self.accounts
    }
}

/// Ledger mutation class produced only after successful native verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VerifiedPrivacyLedgerEffectsV1 {
    /// Reusable proof component with no replay marker, output, or root update.
    None,
    /// Complete Anonymous-PGC encrypted account-table transition.
    AnonymousPgcPayment(VerifiedAnonymousPgcLedgerEffectV1),
}

/// Fully verified, statement-derived effects ready for atomic admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VerifiedPrivacyEffectsV1 {
    protocol_id: PrivacyProtocolIdV1,
    statement_digest: PrivacyStatementDigestV1,
    action_index: u32,
    encoded_action_bytes: u64,
    ledger: VerifiedPrivacyLedgerEffectsV1,
}

impl VerifiedPrivacyEffectsV1 {
    /// Return the cryptographically verified protocol.
    #[must_use]
    pub(crate) const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        self.protocol_id
    }

    /// Return the digest of the exact verified typed statement.
    #[must_use]
    pub(crate) const fn statement_digest(&self) -> PrivacyStatementDigestV1 {
        self.statement_digest
    }

    /// Return the verified transaction-local privacy action index.
    #[must_use]
    pub(crate) const fn action_index(&self) -> u32 {
        self.action_index
    }

    /// Return the canonical envelope byte charge.
    #[must_use]
    pub(crate) const fn encoded_action_bytes(&self) -> u64 {
        self.encoded_action_bytes
    }

    /// Return the exact verified ledger mutation class.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn ledger(&self) -> &VerifiedPrivacyLedgerEffectsV1 {
        &self.ledger
    }

    /// Consume this verified action and return its opaque ledger transition.
    #[must_use]
    pub(crate) fn into_ledger(self) -> VerifiedPrivacyLedgerEffectsV1 {
        self.ledger
    }
}

/// Verify one envelope and derive its exact atomic effects.
///
/// # Errors
///
/// Rejects a missing/altered compiled profile, malformed or inactive
/// activation, envelope inconsistency, wrong execution context, unsupported
/// engine, non-canonical native wire value, or failed proof equation.
pub(crate) fn verify_privacy_envelope_v1(
    envelope: &PrivacyProofEnvelopeV1,
    context: PrivacyVerificationContextV1<'_>,
) -> Result<VerifiedPrivacyEffectsV1, PrivacyVerificationErrorV1> {
    if context.genesis_hash == [0; 32] {
        return Err(PrivacyVerificationErrorV1::Context(Box::new(
            PrivacyVerificationContextFailureV1::new(
                PrivacyVerificationContextFailureCodeV1::ZeroGenesisHash,
                "non-zero committed genesis hash",
                "all-zero digest",
            ),
        )));
    }

    validate_compiled_privacy_activation_v1(context.activation).map_err(|source| {
        PrivacyVerificationErrorV1::CompiledActivation(Box::new(
            PrivacyCompiledActivationFailureV1 { source },
        ))
    })?;

    let statement_context = envelope.statement.context();
    if statement_context.chain_id != *context.chain_id {
        return Err(PrivacyVerificationErrorV1::Context(Box::new(
            PrivacyVerificationContextFailureV1::new(
                PrivacyVerificationContextFailureCodeV1::ChainIdMismatch,
                context.chain_id.as_str(),
                statement_context.chain_id.as_str(),
            ),
        )));
    }
    if statement_context.action_index != context.expected_action_index {
        return Err(PrivacyVerificationErrorV1::Context(Box::new(
            PrivacyVerificationContextFailureV1::new(
                PrivacyVerificationContextFailureCodeV1::ActionIndexMismatch,
                context.expected_action_index.to_string(),
                statement_context.action_index.to_string(),
            ),
        )));
    }

    envelope
        .validate_against_activation(
            context.activation,
            context.consensus_limits,
            context.current_height,
        )
        .map_err(|source| {
            PrivacyVerificationErrorV1::Envelope(Box::new(PrivacyEnvelopeFailureV1 { source }))
        })?;

    // Canonical encoding is repeated here deliberately: the validated exact
    // bytes become the rollback-safe budget charge returned with the effects.
    let encoded_action_bytes = norito::to_bytes(envelope)
        .ok()
        .and_then(|bytes| u64::try_from(bytes.len()).ok())
        .ok_or_else(|| {
            PrivacyVerificationErrorV1::CanonicalEncoding(Box::new(
                PrivacyCanonicalEncodingFailureV1,
            ))
        })?;

    let ledger = match (&envelope.statement, &envelope.proof) {
        (
            PrivacyStatementV1::AnonymousPgcKOutOfNV1(statement),
            PrivacyProofV1::AnonymousPgcKOutOfNV1(proof),
        ) => verify_anonymous_pgc_payment_v1(statement, proof, envelope, &context)?,
        (
            PrivacyStatementV1::VeRangeTransparentRangeV1(statement),
            PrivacyProofV1::VeRangeTransparentRangeV1(proof),
        ) => {
            let profile = match statement.bit_length {
                PrivacyVeRangeBitLengthV1::Bits32 => VeRangeBitLengthV1::Bits32,
                PrivacyVeRangeBitLengthV1::Bits64 => VeRangeBitLengthV1::Bits64,
            };
            let parameters = VeRangeParametersV1::for_profile(profile).map_err(|source| {
                PrivacyVerificationErrorV1::NativeVeRange(Box::new(
                    PrivacyVeRangeVerificationFailureV1 { source },
                ))
            })?;
            let commitments = statement
                .value_commitments
                .iter()
                .map(|point| CompressedPointV1::from_slice(point.as_bytes()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| {
                    PrivacyVerificationErrorV1::NativeVeRange(Box::new(
                        PrivacyVeRangeVerificationFailureV1 {
                            source: source.into(),
                        },
                    ))
                })?;
            let transcript = TranscriptBindingV1 {
                chain_id: context.chain_id.as_str().as_bytes(),
                genesis_hash: context.genesis_hash,
                action_index: context.expected_action_index,
                statement_digest: *envelope.statement_digest.as_bytes(),
                parameter_id: *envelope.parameter_id.as_bytes(),
                parameter_digest: *envelope.parameter_digest.as_bytes(),
                verifier_digest: *envelope.verifier_digest.as_bytes(),
                statement_schema_digest: *envelope.statement_schema_digest.as_bytes(),
                engine_manifest_digest: *envelope.engine_manifest_digest.as_bytes(),
                generator_digest: parameters.generator_digest(),
            };
            let native_statement =
                VeRangeType1BatchStatementV1::new(profile, commitments, transcript).map_err(
                    |source| {
                        PrivacyVerificationErrorV1::NativeVeRange(Box::new(
                            PrivacyVeRangeVerificationFailureV1 { source },
                        ))
                    },
                )?;
            verify_batch_encoded(&native_statement, proof.as_bytes()).map_err(|source| {
                PrivacyVerificationErrorV1::NativeVeRange(Box::new(
                    PrivacyVeRangeVerificationFailureV1 { source },
                ))
            })?;
            VerifiedPrivacyLedgerEffectsV1::None
        }
        _ => {
            return Err(PrivacyVerificationErrorV1::EngineUnavailable(Box::new(
                PrivacyEngineUnavailableFailureV1 {
                    protocol_id: envelope.protocol_id,
                },
            )));
        }
    };

    let _trusted_block_timestamp_ms = context.block_timestamp_ms;
    Ok(VerifiedPrivacyEffectsV1 {
        protocol_id: envelope.protocol_id,
        statement_digest: envelope.statement_digest,
        action_index: context.expected_action_index,
        encoded_action_bytes,
        ledger,
    })
}

fn verify_anonymous_pgc_payment_v1(
    statement: &AnonymousPgcKOutOfNStatementV1,
    proof: &PrivacyProofBytesV1,
    envelope: &PrivacyProofEnvelopeV1,
    context: &PrivacyVerificationContextV1<'_>,
) -> Result<VerifiedPrivacyLedgerEffectsV1, PrivacyVerificationErrorV1> {
    let state = context.pgc_state.ok_or_else(|| {
        pgc_state_error(PrivacyAnonymousPgcStateFailureCodeV1::MissingTrustedState)
    })?;
    let expected_namespace = PrivacyNamespaceV1::from_statement(&envelope.statement);
    if state.namespace != expected_namespace {
        return Err(pgc_state_error(
            PrivacyAnonymousPgcStateFailureCodeV1::NamespaceMismatch,
        ));
    }
    if statement.account_state_root != state.current_root
        || statement.account_state_root_epoch != state.current_epoch
    {
        return Err(pgc_state_error(
            PrivacyAnonymousPgcStateFailureCodeV1::StaleHead,
        ));
    }
    if state.retained_current_root != Some((state.current_epoch, state.current_root)) {
        return Err(pgc_state_error(
            PrivacyAnonymousPgcStateFailureCodeV1::CurrentRootNotRetained,
        ));
    }
    if state.accounts.len() != statement.anonymity_set_public_keys.len()
        || state
            .accounts
            .iter()
            .zip(&statement.anonymity_set_public_keys)
            .any(|(account, statement_key)| account.public_key != *statement_key)
    {
        return Err(pgc_state_error(
            PrivacyAnonymousPgcStateFailureCodeV1::AccountTableMismatch,
        ));
    }
    let computed_current_root = compute_privacy_pgc_account_state_root_v1(
        state.namespace,
        state.current_epoch,
        state.total_supply,
        state.accounts,
    )
    .map_err(|_| {
        pgc_state_error(PrivacyAnonymousPgcStateFailureCodeV1::CurrentRootRecomputationFailed)
    })?;
    if computed_current_root != state.current_root {
        return Err(pgc_state_error(
            PrivacyAnonymousPgcStateFailureCodeV1::CurrentRootMismatch,
        ));
    }

    let parameters = AnonymousPgcParametersV1::get().map_err(|source| {
        PrivacyVerificationErrorV1::NativeAnonymousPgc(Box::new(
            PrivacyAnonymousPgcVerificationFailureV1 { source },
        ))
    })?;
    let pool_invariant = AnonymousPgcPoolInvariantV1::new(
        state.total_supply,
        *state.bootstrap_digest.as_bytes(),
        *state.bootstrap_proof_digest.as_bytes(),
    )
    .map_err(|_| pgc_state_error(PrivacyAnonymousPgcStateFailureCodeV1::InvalidPoolInvariant))?;
    let public_keys = statement
        .anonymity_set_public_keys
        .iter()
        .map(|point| TwistedElGamalPublicKeyV1::from_sec1_bytes(point.as_bytes()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(native_pgc_error)?;
    let transfer_ciphertexts = statement
        .transfer_ciphertexts
        .iter()
        .map(|ciphertext| {
            TwistedElGamalCiphertextV1::from_sec1_bytes(
                ciphertext.left.as_bytes(),
                ciphertext.right.as_bytes(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(native_pgc_error)?;
    let current_balance_ciphertexts = state
        .accounts
        .iter()
        .map(|account| {
            TwistedElGamalCiphertextV1::from_sec1_bytes(
                account.encrypted_balance.left.as_bytes(),
                account.encrypted_balance.right.as_bytes(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(native_pgc_error)?;
    let transcript = TranscriptBindingV1 {
        chain_id: context.chain_id.as_str().as_bytes(),
        genesis_hash: context.genesis_hash,
        action_index: context.expected_action_index,
        statement_digest: *envelope.statement_digest.as_bytes(),
        parameter_id: *envelope.parameter_id.as_bytes(),
        parameter_digest: *envelope.parameter_digest.as_bytes(),
        verifier_digest: *envelope.verifier_digest.as_bytes(),
        statement_schema_digest: *envelope.statement_schema_digest.as_bytes(),
        engine_manifest_digest: *envelope.engine_manifest_digest.as_bytes(),
        generator_digest: parameters.generator_digest(),
    };
    let recipient_count = usize::try_from(statement.recipient_count).map_err(|_| {
        pgc_state_error(PrivacyAnonymousPgcStateFailureCodeV1::AccountTableMismatch)
    })?;
    let native_statement = AnonymousPgcPaymentStatementV1::new(
        &public_keys,
        &transfer_ciphertexts,
        &current_balance_ciphertexts,
        recipient_count,
        pool_invariant,
        transcript,
    )
    .map_err(native_pgc_error)?;
    let verified =
        verify_payment_encoded(&native_statement, proof.as_bytes()).map_err(native_pgc_error)?;
    if verified.next_balance_ciphertexts().len() != state.accounts.len() {
        return Err(pgc_state_error(
            PrivacyAnonymousPgcStateFailureCodeV1::SuccessorTableMismatch,
        ));
    }
    let accounts = state
        .accounts
        .iter()
        .zip(verified.next_balance_ciphertexts())
        .map(|(current, ciphertext)| PrivacyPgcAccountV1 {
            public_key: current.public_key,
            encrypted_balance: PrivacyP256CiphertextV1 {
                left: PrivacyP256PointV1::new(*ciphertext.left().as_bytes()),
                right: PrivacyP256PointV1::new(*ciphertext.right().as_bytes()),
            },
        })
        .collect::<Vec<_>>();
    let computed_next_root = compute_privacy_pgc_account_state_root_v1(
        state.namespace,
        statement.next_account_state_root_epoch,
        state.total_supply,
        &accounts,
    )
    .map_err(|_| {
        pgc_state_error(PrivacyAnonymousPgcStateFailureCodeV1::NextRootRecomputationFailed)
    })?;
    if computed_next_root != statement.next_account_state_root {
        return Err(pgc_state_error(
            PrivacyAnonymousPgcStateFailureCodeV1::NextRootMismatch,
        ));
    }
    Ok(VerifiedPrivacyLedgerEffectsV1::AnonymousPgcPayment(
        VerifiedAnonymousPgcLedgerEffectV1 {
            namespace: state.namespace,
            total_supply: state.total_supply,
            current_root: state.current_root,
            current_epoch: state.current_epoch,
            next_root: computed_next_root,
            next_epoch: statement.next_account_state_root_epoch,
            accounts,
        },
    ))
}

fn native_pgc_error(source: AnonymousPgcError) -> PrivacyVerificationErrorV1 {
    PrivacyVerificationErrorV1::NativeAnonymousPgc(Box::new(
        PrivacyAnonymousPgcVerificationFailureV1 { source },
    ))
}

fn pgc_state_error(code: PrivacyAnonymousPgcStateFailureCodeV1) -> PrivacyVerificationErrorV1 {
    PrivacyVerificationErrorV1::AnonymousPgcState(Box::new(PrivacyAnonymousPgcStateFailureV1 {
        code,
    }))
}

/// Exhaustive privacy-verification failure.
///
/// Every variant boxes a uniformly sized detail so adding a diagnostic cannot
/// silently inflate consensus execution stack frames.
#[derive(Debug, Error)]
pub(crate) enum PrivacyVerificationErrorV1 {
    /// The execution context differs from the statement or is incomplete.
    #[error(transparent)]
    Context(Box<PrivacyVerificationContextFailureV1>),
    /// The active governance record differs from executable consensus code.
    #[error(transparent)]
    CompiledActivation(Box<PrivacyCompiledActivationFailureV1>),
    /// Typed envelope or governed lifecycle validation failed.
    #[error(transparent)]
    Envelope(Box<PrivacyEnvelopeFailureV1>),
    /// The selected protocol has no complete native verifier.
    #[error(transparent)]
    EngineUnavailable(Box<PrivacyEngineUnavailableFailureV1>),
    /// Native VeRange decoding or verification failed.
    #[error(transparent)]
    NativeVeRange(Box<PrivacyVeRangeVerificationFailureV1>),
    /// Trusted persisted Anonymous-PGC state was absent or inconsistent.
    #[error(transparent)]
    AnonymousPgcState(Box<PrivacyAnonymousPgcStateFailureV1>),
    /// Native Anonymous-PGC decoding or verification failed.
    #[error(transparent)]
    NativeAnonymousPgc(Box<PrivacyAnonymousPgcVerificationFailureV1>),
    /// Canonical envelope encoding or length conversion failed.
    #[error(transparent)]
    CanonicalEncoding(Box<PrivacyCanonicalEncodingFailureV1>),
}

/// Stable execution-context mismatch category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PrivacyVerificationContextFailureCodeV1 {
    /// No committed genesis digest was available.
    ZeroGenesisHash,
    /// Prover-selected and node-configured chains differ.
    ChainIdMismatch,
    /// Prover-selected and transaction-local action indexes differ.
    ActionIndexMismatch,
}

#[derive(Debug, Error)]
#[error("privacy verification context {code:?}: expected {expected}, observed {actual}")]
pub(crate) struct PrivacyVerificationContextFailureV1 {
    pub(crate) code: PrivacyVerificationContextFailureCodeV1,
    expected: Box<str>,
    actual: Box<str>,
}

impl PrivacyVerificationContextFailureV1 {
    fn new(
        code: PrivacyVerificationContextFailureCodeV1,
        expected: impl Into<Box<str>>,
        actual: impl Into<Box<str>>,
    ) -> Self {
        Self {
            code,
            expected: expected.into(),
            actual: actual.into(),
        }
    }
}

#[derive(Debug, Error)]
#[error("privacy activation does not match the compiled native profile: {source}")]
pub(crate) struct PrivacyCompiledActivationFailureV1 {
    source: CompiledPrivacyProfileValidationErrorV1,
}

#[derive(Debug, Error)]
#[error("privacy envelope admission failed: {source}")]
pub(crate) struct PrivacyEnvelopeFailureV1 {
    source: PrivacyProofEnvelopeValidationError,
}

#[derive(Debug, Error)]
#[error("native privacy engine for {protocol_id:?} is not available")]
pub(crate) struct PrivacyEngineUnavailableFailureV1 {
    protocol_id: PrivacyProtocolIdV1,
}

#[derive(Debug, Error)]
#[error("native VeRange verification failed: {source}")]
pub(crate) struct PrivacyVeRangeVerificationFailureV1 {
    source: VeRangeError,
}

/// Stable trusted-state failure detected before or after native PGC proof
/// verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PrivacyAnonymousPgcStateFailureCodeV1 {
    /// The submit path did not supply a trusted pool snapshot.
    MissingTrustedState,
    /// Trusted state belongs to another namespace.
    NamespaceMismatch,
    /// The statement does not reference the current persisted head.
    StaleHead,
    /// The current head has no exact retained-history record.
    CurrentRootNotRetained,
    /// The statement keys differ from the complete persisted account table.
    AccountTableMismatch,
    /// Persisted supply or bootstrap audit digests are invalid.
    InvalidPoolInvariant,
    /// The current account table could not be hashed canonically.
    CurrentRootRecomputationFailed,
    /// The recomputed current account root differs from the persisted head.
    CurrentRootMismatch,
    /// Native verification returned a non-complete successor table.
    SuccessorTableMismatch,
    /// The successor account table could not be hashed canonically.
    NextRootRecomputationFailed,
    /// The recomputed successor root differs from the statement.
    NextRootMismatch,
}

#[derive(Debug, Error)]
#[error("trusted Anonymous-PGC state failed validation: {code:?}")]
pub(crate) struct PrivacyAnonymousPgcStateFailureV1 {
    /// Exact stable failure category.
    pub(crate) code: PrivacyAnonymousPgcStateFailureCodeV1,
}

#[derive(Debug, Error)]
#[error("native Anonymous-PGC verification failed: {source}")]
pub(crate) struct PrivacyAnonymousPgcVerificationFailureV1 {
    source: AnonymousPgcError,
}

#[derive(Debug, Error)]
#[error("canonical privacy envelope encoding failed")]
pub(crate) struct PrivacyCanonicalEncodingFailureV1;

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use iroha_data_model::{
        asset::AssetDefinitionId,
        domain::DomainId,
        name::Name,
        privacy::{
            PrivacyActiveLifecycleV1, PrivacyEngineIdV1, PrivacyNamespaceScopeV1,
            PrivacyP256PointV1, PrivacyPgcAccountBootstrapDigestV1,
            PrivacyPgcBootstrapProofDigestV1, PrivacyPolicyIdV1, PrivacyPoolIdV1,
            PrivacyPoolNamespaceV1, PrivacyProofBytesV1, PrivacyProofSystemIdV1,
            PrivacyProposedLifecycleV1, PrivacyProtocolLifecycleV1, PrivacyStatementContextV1,
            VeRangeTransparentRangeStatementV1,
        },
    };
    use rand_core_06::{CryptoRng, Error as RngError, RngCore};
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::{
        privacy_engines::{
            anonymous_pgc::{
                TwistedElGamalKeyPairV1, add_ciphertexts, encrypt_with_randomness,
                payment::{
                    AnonymousPgcPaymentWitnessV1, encrypt_signed_with_randomness, prove_payment,
                },
            },
            p256::SecretScalarV1,
            verange::{commit, prove_batch},
        },
        privacy_profiles::{CompiledPrivacyProfileV1, compiled_privacy_profile_v1},
    };

    const TEST_CONSENSUS_LIMITS: PrivacyConsensusLimitsV1 =
        PrivacyConsensusLimitsV1::taira_default();

    struct KatRng {
        seed: [u8; 32],
        counter: u64,
    }

    impl KatRng {
        fn new(seed: [u8; 32]) -> Self {
            Self { seed, counter: 0 }
        }
    }

    impl RngCore for KatRng {
        fn next_u32(&mut self) -> u32 {
            let mut bytes = [0; 4];
            self.fill_bytes(&mut bytes);
            u32::from_be_bytes(bytes)
        }

        fn next_u64(&mut self) -> u64 {
            let mut bytes = [0; 8];
            self.fill_bytes(&mut bytes);
            u64::from_be_bytes(bytes)
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            for chunk in destination.chunks_mut(32) {
                let mut hash = Sha256::new();
                hash.update(b"iroha.privacy.verifier.kat-rng.v1");
                hash.update(self.seed);
                hash.update(self.counter.to_be_bytes());
                self.counter = self.counter.wrapping_add(1);
                let block: [u8; 32] = hash.finalize().into();
                chunk.copy_from_slice(&block[..chunk.len()]);
            }
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
            self.fill_bytes(destination);
            Ok(())
        }
    }

    impl CryptoRng for KatRng {}

    fn secret(value: u8) -> SecretScalarV1 {
        let mut bytes = [0; 32];
        bytes[31] = value;
        SecretScalarV1::from_bytes(bytes).expect("canonical non-zero scalar")
    }

    fn active_profile() -> (CompiledPrivacyProfileV1, PrivacyProtocolActivationRecordV1) {
        let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("compiled VeRange");
        let activation = profile.activation_record(PrivacyProtocolLifecycleV1::Active(
            PrivacyActiveLifecycleV1 {
                proposed_at_height: 1,
                activated_at_height: 2,
                state_since_height: 2,
            },
        ));
        (profile, activation)
    }

    fn valid_envelope() -> (
        PrivacyProofEnvelopeV1,
        PrivacyProtocolActivationRecordV1,
        ChainId,
    ) {
        let (compiled, activation) = active_profile();
        let chain_id = ChainId::from("taira-privacy-test");
        let native_profile = VeRangeBitLengthV1::Bits32;
        let values = [7_u64, 19_u64];
        let blindings = [secret(3), secret(5)];
        let native_commitments = values
            .iter()
            .zip(&blindings)
            .map(|(value, blinding)| {
                commit(native_profile, *value, blinding).expect("valid commitment")
            })
            .collect::<Vec<_>>();
        let value_commitments = native_commitments
            .iter()
            .map(|point| PrivacyP256PointV1::new(*point.as_bytes()))
            .collect();
        let context = PrivacyStatementContextV1 {
            chain_id: chain_id.clone(),
            action_index: 0,
            parameter_id: compiled.parameter_id,
            parameter_digest: compiled.parameter_digest,
            verifier_digest: compiled.verifier_digest,
            statement_schema_digest: compiled.statement_schema_digest,
            engine_manifest_digest: compiled.engine_manifest_digest,
        };
        let statement =
            PrivacyStatementV1::VeRangeTransparentRangeV1(VeRangeTransparentRangeStatementV1 {
                context,
                asset_definition_id: AssetDefinitionId::new(
                    DomainId::try_new("privacy", "universal").expect("domain"),
                    Name::from_str("asset").expect("name"),
                ),
                policy_id: PrivacyPolicyIdV1::new([0x91; 32]),
                value_commitments,
                bit_length: PrivacyVeRangeBitLengthV1::Bits32,
                aggregation_count: 2,
            });
        let statement_digest = statement.digest().expect("statement digest");
        let parameters =
            VeRangeParametersV1::for_profile(native_profile).expect("VeRange parameters");
        let transcript = TranscriptBindingV1 {
            chain_id: chain_id.as_str().as_bytes(),
            genesis_hash: [0xA7; 32],
            action_index: 0,
            statement_digest: *statement_digest.as_bytes(),
            parameter_id: *compiled.parameter_id.as_bytes(),
            parameter_digest: *compiled.parameter_digest.as_bytes(),
            verifier_digest: *compiled.verifier_digest.as_bytes(),
            statement_schema_digest: *compiled.statement_schema_digest.as_bytes(),
            engine_manifest_digest: *compiled.engine_manifest_digest.as_bytes(),
            generator_digest: parameters.generator_digest(),
        };
        let native_statement =
            VeRangeType1BatchStatementV1::new(native_profile, native_commitments, transcript)
                .expect("native statement");
        let proof = prove_batch(
            &native_statement,
            &values,
            &blindings,
            &mut KatRng::new([0x62; 32]),
        )
        .expect("proof")
        .encode();
        (
            PrivacyProofEnvelopeV1 {
                protocol_id: compiled.protocol_id,
                proof_system_id: compiled.proof_system_id,
                engine_id: compiled.engine_id,
                parameter_id: compiled.parameter_id,
                parameter_digest: compiled.parameter_digest,
                verifier_digest: compiled.verifier_digest,
                statement_schema_digest: compiled.statement_schema_digest,
                engine_manifest_digest: compiled.engine_manifest_digest,
                statement_digest,
                statement,
                proof: PrivacyProofV1::VeRangeTransparentRangeV1(PrivacyProofBytesV1::new(proof)),
            },
            activation,
            chain_id,
        )
    }

    struct PgcFixture {
        envelope: PrivacyProofEnvelopeV1,
        activation: PrivacyProtocolActivationRecordV1,
        chain_id: ChainId,
        namespace: PrivacyNamespaceV1,
        total_supply: u32,
        bootstrap_digest: PrivacyPgcAccountBootstrapDigestV1,
        bootstrap_proof_digest: PrivacyPgcBootstrapProofDigestV1,
        current_root: PrivacyRootV1,
        current_epoch: u64,
        accounts: Vec<PrivacyPgcAccountV1>,
    }

    impl PgcFixture {
        fn new() -> Self {
            Self::with_declared_transition(None, None)
        }

        fn with_declared_transition(
            next_root_override: Option<PrivacyRootV1>,
            next_epoch_override: Option<u64>,
        ) -> Self {
            let compiled = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
                .expect("compiled Anonymous PGC");
            let activation = compiled.activation_record(PrivacyProtocolLifecycleV1::Active(
                PrivacyActiveLifecycleV1 {
                    proposed_at_height: 1,
                    activated_at_height: 2,
                    state_since_height: 2,
                },
            ));
            let chain_id = ChainId::from("taira-privacy-test");
            let pool_id = PrivacyPoolIdV1::new([0xb1; 32]);
            let namespace = PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 { pool_id }),
            );
            let total_supply = 1_600;
            let bootstrap_digest = PrivacyPgcAccountBootstrapDigestV1::new([0xb2; 32]);
            let bootstrap_proof_digest = PrivacyPgcBootstrapProofDigestV1::new([0xb3; 32]);
            let current_epoch = 1;

            let mut key_pairs = (2_u8..18)
                .map(|value| {
                    TwistedElGamalKeyPairV1::from_secret(secret(value)).expect("PGC key pair")
                })
                .collect::<Vec<_>>();
            key_pairs.sort_by_key(TwistedElGamalKeyPairV1::public_key);
            let public_keys = key_pairs
                .iter()
                .map(TwistedElGamalKeyPairV1::public_key)
                .collect::<Vec<_>>();
            let public_key_wires = public_keys
                .iter()
                .map(|key| PrivacyP256PointV1::new(*key.as_point().as_bytes()))
                .collect::<Vec<_>>();
            let current_randomness = (0_u8..16)
                .map(|index| secret(100 + index))
                .collect::<Vec<_>>();
            let current_ciphertexts = public_keys
                .iter()
                .copied()
                .zip(&current_randomness)
                .map(|(key, randomness)| {
                    encrypt_with_randomness(key, 100, randomness).expect("current balance")
                })
                .collect::<Vec<_>>();
            let accounts = public_key_wires
                .iter()
                .copied()
                .zip(current_ciphertexts.iter().copied())
                .map(|(public_key, encrypted_balance)| PrivacyPgcAccountV1 {
                    public_key,
                    encrypted_balance: pgc_ciphertext_wire(encrypted_balance),
                })
                .collect::<Vec<_>>();
            let current_root = compute_privacy_pgc_account_state_root_v1(
                namespace,
                current_epoch,
                total_supply,
                &accounts,
            )
            .expect("current account root");

            let sender_index = 7;
            let recipient_count = 2;
            let mut transfer_values = vec![0_i64; 16];
            transfer_values[2] = 20;
            transfer_values[12] = 30;
            transfer_values[sender_index] = -50;
            let transfer_randomness = (0_u8..16)
                .map(|index| secret(40 + index))
                .collect::<Vec<_>>();
            let transfer_ciphertexts = public_keys
                .iter()
                .copied()
                .zip(&transfer_values)
                .zip(&transfer_randomness)
                .map(|((key, value), randomness)| {
                    encrypt_signed_with_randomness(key, *value, randomness)
                        .expect("transfer ciphertext")
                })
                .collect::<Vec<_>>();
            let next_accounts = public_key_wires
                .iter()
                .copied()
                .zip(
                    current_ciphertexts
                        .iter()
                        .copied()
                        .zip(transfer_ciphertexts.iter().copied())
                        .map(|(current, transfer)| {
                            add_ciphertexts(current, transfer).expect("successor ciphertext")
                        }),
                )
                .map(|(public_key, encrypted_balance)| PrivacyPgcAccountV1 {
                    public_key,
                    encrypted_balance: pgc_ciphertext_wire(encrypted_balance),
                })
                .collect::<Vec<_>>();
            let next_epoch = current_epoch + 1;
            let next_root = compute_privacy_pgc_account_state_root_v1(
                namespace,
                next_epoch,
                total_supply,
                &next_accounts,
            )
            .expect("next account root");
            let declared_next_root = next_root_override.unwrap_or(next_root);
            let declared_next_epoch = next_epoch_override.unwrap_or(next_epoch);
            let statement_context = PrivacyStatementContextV1 {
                chain_id: chain_id.clone(),
                action_index: 0,
                parameter_id: compiled.parameter_id,
                parameter_digest: compiled.parameter_digest,
                verifier_digest: compiled.verifier_digest,
                statement_schema_digest: compiled.statement_schema_digest,
                engine_manifest_digest: compiled.engine_manifest_digest,
            };
            let statement =
                PrivacyStatementV1::AnonymousPgcKOutOfNV1(AnonymousPgcKOutOfNStatementV1 {
                    context: statement_context,
                    asset_definition_id: AssetDefinitionId::new(
                        DomainId::try_new("privacy", "universal").expect("domain"),
                        Name::from_str("asset").expect("name"),
                    ),
                    pool_id,
                    account_state_root: current_root,
                    account_state_root_epoch: current_epoch,
                    next_account_state_root: declared_next_root,
                    next_account_state_root_epoch: declared_next_epoch,
                    anonymity_set_public_keys: public_key_wires,
                    transfer_ciphertexts: transfer_ciphertexts
                        .iter()
                        .copied()
                        .map(pgc_ciphertext_wire)
                        .collect(),
                    recipient_count,
                });
            let statement_digest = statement.digest().expect("PGC statement digest");
            let parameters = AnonymousPgcParametersV1::get().expect("PGC parameters");
            let transcript = TranscriptBindingV1 {
                chain_id: chain_id.as_str().as_bytes(),
                genesis_hash: [0xa7; 32],
                action_index: 0,
                statement_digest: *statement_digest.as_bytes(),
                parameter_id: *compiled.parameter_id.as_bytes(),
                parameter_digest: *compiled.parameter_digest.as_bytes(),
                verifier_digest: *compiled.verifier_digest.as_bytes(),
                statement_schema_digest: *compiled.statement_schema_digest.as_bytes(),
                engine_manifest_digest: *compiled.engine_manifest_digest.as_bytes(),
                generator_digest: parameters.generator_digest(),
            };
            let native_invariant = AnonymousPgcPoolInvariantV1::new(
                total_supply,
                *bootstrap_digest.as_bytes(),
                *bootstrap_proof_digest.as_bytes(),
            )
            .expect("native invariant");
            let native_statement = AnonymousPgcPaymentStatementV1::new(
                &public_keys,
                &transfer_ciphertexts,
                &current_ciphertexts,
                usize::try_from(recipient_count).expect("recipient count"),
                native_invariant,
                transcript,
            )
            .expect("native PGC statement");
            let witness = AnonymousPgcPaymentWitnessV1 {
                transfer_values: &transfer_values,
                transfer_randomness: &transfer_randomness,
                sender_index,
                sender_secret: key_pairs[sender_index].secret_scalar(),
            };
            let proof = prove_payment(&native_statement, &witness, &mut KatRng::new([0xb4; 32]))
                .expect("PGC payment proof")
                .encode();
            let envelope = PrivacyProofEnvelopeV1 {
                protocol_id: compiled.protocol_id,
                proof_system_id: compiled.proof_system_id,
                engine_id: compiled.engine_id,
                parameter_id: compiled.parameter_id,
                parameter_digest: compiled.parameter_digest,
                verifier_digest: compiled.verifier_digest,
                statement_schema_digest: compiled.statement_schema_digest,
                engine_manifest_digest: compiled.engine_manifest_digest,
                statement_digest,
                statement,
                proof: PrivacyProofV1::AnonymousPgcKOutOfNV1(PrivacyProofBytesV1::new(proof)),
            };
            Self {
                envelope,
                activation,
                chain_id,
                namespace,
                total_supply,
                bootstrap_digest,
                bootstrap_proof_digest,
                current_root,
                current_epoch,
                accounts,
            }
        }

        fn verification_context(&self) -> PrivacyVerificationContextV1<'_> {
            PrivacyVerificationContextV1 {
                activation: &self.activation,
                consensus_limits: &TEST_CONSENSUS_LIMITS,
                chain_id: &self.chain_id,
                genesis_hash: [0xa7; 32],
                current_height: 10,
                expected_action_index: 0,
                block_timestamp_ms: 1_800_000_000_000,
                pgc_state: Some(self.pgc_state(&self.accounts)),
            }
        }

        fn pgc_state<'a>(
            &self,
            accounts: &'a [PrivacyPgcAccountV1],
        ) -> PrivacyPgcVerificationStateV1<'a> {
            PrivacyPgcVerificationStateV1 {
                namespace: self.namespace,
                total_supply: self.total_supply,
                bootstrap_digest: self.bootstrap_digest,
                bootstrap_proof_digest: self.bootstrap_proof_digest,
                current_root: self.current_root,
                current_epoch: self.current_epoch,
                retained_current_root: Some((self.current_epoch, self.current_root)),
                accounts,
            }
        }
    }

    fn pgc_ciphertext_wire(ciphertext: TwistedElGamalCiphertextV1) -> PrivacyP256CiphertextV1 {
        PrivacyP256CiphertextV1 {
            left: PrivacyP256PointV1::new(*ciphertext.left().as_bytes()),
            right: PrivacyP256PointV1::new(*ciphertext.right().as_bytes()),
        }
    }

    fn verification_context<'a>(
        activation: &'a PrivacyProtocolActivationRecordV1,
        chain_id: &'a ChainId,
    ) -> PrivacyVerificationContextV1<'a> {
        PrivacyVerificationContextV1 {
            activation,
            consensus_limits: &TEST_CONSENSUS_LIMITS,
            chain_id,
            genesis_hash: [0xA7; 32],
            current_height: 10,
            expected_action_index: 0,
            block_timestamp_ms: 1_800_000_000_000,
            pgc_state: None,
        }
    }

    fn verange_statement_mut(
        envelope: &mut PrivacyProofEnvelopeV1,
    ) -> &mut VeRangeTransparentRangeStatementV1 {
        let PrivacyStatementV1::VeRangeTransparentRangeV1(statement) = &mut envelope.statement
        else {
            unreachable!("VeRange fixture")
        };
        statement
    }

    fn refresh_statement_digest(envelope: &mut PrivacyProofEnvelopeV1) {
        envelope.statement_digest = envelope
            .statement
            .digest()
            .expect("mutated statement remains canonically encodable");
    }

    fn assert_rejected(
        envelope: &PrivacyProofEnvelopeV1,
        activation: &PrivacyProtocolActivationRecordV1,
        chain_id: &ChainId,
        label: &str,
    ) {
        assert!(
            verify_privacy_envelope_v1(envelope, verification_context(activation, chain_id))
                .is_err(),
            "adversarial envelope `{label}` was accepted"
        );
    }

    #[test]
    fn verified_verange_effects_are_exact_and_non_mutating() {
        let (envelope, activation, chain_id) = valid_envelope();
        let effects =
            verify_privacy_envelope_v1(&envelope, verification_context(&activation, &chain_id))
                .expect("valid proof");
        assert_eq!(
            effects.protocol_id(),
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1
        );
        assert_eq!(effects.statement_digest(), envelope.statement_digest);
        assert_eq!(effects.action_index(), 0);
        assert_eq!(effects.ledger(), &VerifiedPrivacyLedgerEffectsV1::None);
        assert_eq!(
            effects.encoded_action_bytes(),
            u64::try_from(norito::to_bytes(&envelope).expect("encode").len()).expect("length")
        );
    }

    #[test]
    fn verified_pgc_effect_is_complete_exact_and_replay_becomes_stale() {
        let fixture = PgcFixture::new();
        let effects = verify_privacy_envelope_v1(&fixture.envelope, fixture.verification_context())
            .expect("valid PGC payment");
        assert_eq!(
            effects.protocol_id(),
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1
        );
        assert_eq!(
            effects.statement_digest(),
            fixture.envelope.statement_digest
        );
        let effect = match effects.ledger() {
            VerifiedPrivacyLedgerEffectsV1::AnonymousPgcPayment(effect) => effect,
            VerifiedPrivacyLedgerEffectsV1::None => panic!("missing PGC ledger effect"),
        };
        assert_eq!(effect.namespace(), fixture.namespace);
        assert_eq!(effect.total_supply(), fixture.total_supply);
        assert_eq!(effect.current_root(), fixture.current_root);
        assert_eq!(effect.current_epoch(), fixture.current_epoch);
        assert_eq!(effect.next_epoch(), fixture.current_epoch + 1);
        assert_eq!(effect.accounts().len(), fixture.accounts.len());
        assert!(
            effect
                .accounts()
                .iter()
                .zip(&fixture.accounts)
                .all(|(next, current)| next.public_key == current.public_key)
        );
        assert_eq!(
            compute_privacy_pgc_account_state_root_v1(
                effect.namespace(),
                effect.next_epoch(),
                effect.total_supply(),
                effect.accounts(),
            )
            .expect("recompute verified next root"),
            effect.next_root()
        );
        let next_accounts = effect.accounts().to_vec();
        let next_root = effect.next_root();
        let next_epoch = effect.next_epoch();
        assert!(matches!(
            effects.into_ledger(),
            VerifiedPrivacyLedgerEffectsV1::AnonymousPgcPayment(_)
        ));

        let mut replay_state = fixture.pgc_state(&next_accounts);
        replay_state.current_root = next_root;
        replay_state.current_epoch = next_epoch;
        replay_state.retained_current_root = Some((next_epoch, next_root));
        let mut replay_context = fixture.verification_context();
        replay_context.pgc_state = Some(replay_state);
        assert!(matches!(
            verify_privacy_envelope_v1(&fixture.envelope, replay_context),
            Err(PrivacyVerificationErrorV1::AnonymousPgcState(detail))
                if detail.code == PrivacyAnonymousPgcStateFailureCodeV1::StaleHead
        ));
    }

    #[test]
    fn pgc_trusted_state_memo_and_proof_tampering_fail_closed() {
        let fixture = PgcFixture::new();

        let mut missing = fixture.verification_context();
        missing.pgc_state = None;
        assert!(matches!(
            verify_privacy_envelope_v1(&fixture.envelope, missing),
            Err(PrivacyVerificationErrorV1::AnonymousPgcState(detail))
                if detail.code == PrivacyAnonymousPgcStateFailureCodeV1::MissingTrustedState
        ));

        let mut state = fixture.pgc_state(&fixture.accounts);
        state.retained_current_root = None;
        let mut context = fixture.verification_context();
        context.pgc_state = Some(state);
        assert!(matches!(
            verify_privacy_envelope_v1(&fixture.envelope, context),
            Err(PrivacyVerificationErrorV1::AnonymousPgcState(detail))
                if detail.code
                    == PrivacyAnonymousPgcStateFailureCodeV1::CurrentRootNotRetained
        ));

        let mut state = fixture.pgc_state(&fixture.accounts);
        state.current_epoch += 1;
        let mut context = fixture.verification_context();
        context.pgc_state = Some(state);
        assert!(matches!(
            verify_privacy_envelope_v1(&fixture.envelope, context),
            Err(PrivacyVerificationErrorV1::AnonymousPgcState(detail))
                if detail.code == PrivacyAnonymousPgcStateFailureCodeV1::StaleHead
        ));

        let mut reordered = fixture.accounts.clone();
        reordered.swap(0, 1);
        let mut context = fixture.verification_context();
        context.pgc_state = Some(fixture.pgc_state(&reordered));
        assert!(matches!(
            verify_privacy_envelope_v1(&fixture.envelope, context),
            Err(PrivacyVerificationErrorV1::AnonymousPgcState(detail))
                if detail.code == PrivacyAnonymousPgcStateFailureCodeV1::AccountTableMismatch
        ));

        let mut context = fixture.verification_context();
        context.pgc_state = Some(fixture.pgc_state(&fixture.accounts[..15]));
        assert!(matches!(
            verify_privacy_envelope_v1(&fixture.envelope, context),
            Err(PrivacyVerificationErrorV1::AnonymousPgcState(detail))
                if detail.code == PrivacyAnonymousPgcStateFailureCodeV1::AccountTableMismatch
        ));

        let mut duplicate = fixture.accounts.clone();
        duplicate[1].public_key = duplicate[0].public_key;
        let mut context = fixture.verification_context();
        context.pgc_state = Some(fixture.pgc_state(&duplicate));
        assert!(matches!(
            verify_privacy_envelope_v1(&fixture.envelope, context),
            Err(PrivacyVerificationErrorV1::AnonymousPgcState(detail))
                if detail.code == PrivacyAnonymousPgcStateFailureCodeV1::AccountTableMismatch
        ));

        let mut changed_ciphertext = fixture.accounts.clone();
        changed_ciphertext[0].encrypted_balance = changed_ciphertext[1].encrypted_balance;
        let mut context = fixture.verification_context();
        context.pgc_state = Some(fixture.pgc_state(&changed_ciphertext));
        assert!(matches!(
            verify_privacy_envelope_v1(&fixture.envelope, context),
            Err(PrivacyVerificationErrorV1::AnonymousPgcState(detail))
                if detail.code == PrivacyAnonymousPgcStateFailureCodeV1::CurrentRootMismatch
        ));

        let mut state = fixture.pgc_state(&fixture.accounts);
        state.total_supply += 1;
        let mut context = fixture.verification_context();
        context.pgc_state = Some(state);
        assert!(matches!(
            verify_privacy_envelope_v1(&fixture.envelope, context),
            Err(PrivacyVerificationErrorV1::AnonymousPgcState(detail))
                if detail.code == PrivacyAnonymousPgcStateFailureCodeV1::CurrentRootMismatch
        ));

        let mut changed_provenance = fixture.pgc_state(&fixture.accounts);
        changed_provenance.bootstrap_proof_digest =
            PrivacyPgcBootstrapProofDigestV1::new([0xb5; 32]);
        let mut context = fixture.verification_context();
        context.pgc_state = Some(changed_provenance);
        assert!(matches!(
            verify_privacy_envelope_v1(&fixture.envelope, context),
            Err(PrivacyVerificationErrorV1::NativeAnonymousPgc(_))
        ));

        let mut altered_proof = fixture.envelope.clone();
        let PrivacyProofV1::AnonymousPgcKOutOfNV1(proof) = &altered_proof.proof else {
            unreachable!("PGC proof")
        };
        let mut proof_bytes = proof.as_bytes().to_vec();
        let proof_middle = proof_bytes.len() / 2;
        proof_bytes[proof_middle] ^= 1;
        altered_proof.proof =
            PrivacyProofV1::AnonymousPgcKOutOfNV1(PrivacyProofBytesV1::new(proof_bytes));
        assert!(matches!(
            verify_privacy_envelope_v1(&altered_proof, fixture.verification_context()),
            Err(PrivacyVerificationErrorV1::NativeAnonymousPgc(_))
        ));

        let PrivacyProofV1::AnonymousPgcKOutOfNV1(valid_proof) = &fixture.envelope.proof else {
            unreachable!("PGC proof")
        };
        for invalid_bytes in [
            valid_proof.as_bytes()[..valid_proof.as_bytes().len() - 1].to_vec(),
            {
                let mut trailing = valid_proof.as_bytes().to_vec();
                trailing.push(0);
                trailing
            },
        ] {
            let mut invalid = fixture.envelope.clone();
            invalid.proof =
                PrivacyProofV1::AnonymousPgcKOutOfNV1(PrivacyProofBytesV1::new(invalid_bytes));
            assert!(matches!(
                verify_privacy_envelope_v1(&invalid, fixture.verification_context()),
                Err(PrivacyVerificationErrorV1::NativeAnonymousPgc(_))
            ));
        }
        let mut empty = fixture.envelope.clone();
        empty.proof = PrivacyProofV1::AnonymousPgcKOutOfNV1(PrivacyProofBytesV1::new(Vec::new()));
        assert!(matches!(
            verify_privacy_envelope_v1(&empty, fixture.verification_context()),
            Err(PrivacyVerificationErrorV1::Envelope(_))
        ));
        let mut all_zero = fixture.envelope.clone();
        all_zero.proof = PrivacyProofV1::AnonymousPgcKOutOfNV1(PrivacyProofBytesV1::new(vec![
            0;
            valid_proof.as_bytes().len()
        ]));
        assert!(matches!(
            verify_privacy_envelope_v1(&all_zero, fixture.verification_context()),
            Err(PrivacyVerificationErrorV1::Envelope(_))
        ));

        let mut duplicate_statement = fixture.envelope.clone();
        let PrivacyStatementV1::AnonymousPgcKOutOfNV1(statement) =
            &mut duplicate_statement.statement
        else {
            unreachable!("PGC statement")
        };
        statement.anonymity_set_public_keys[1] = statement.anonymity_set_public_keys[0];
        refresh_statement_digest(&mut duplicate_statement);
        assert!(matches!(
            verify_privacy_envelope_v1(&duplicate_statement, fixture.verification_context()),
            Err(PrivacyVerificationErrorV1::Envelope(_))
        ));

        let forged_next_root =
            PgcFixture::with_declared_transition(Some(PrivacyRootV1::new([0xc1; 32])), None);
        assert!(matches!(
            verify_privacy_envelope_v1(
                &forged_next_root.envelope,
                forged_next_root.verification_context(),
            ),
            Err(PrivacyVerificationErrorV1::AnonymousPgcState(detail))
                if detail.code == PrivacyAnonymousPgcStateFailureCodeV1::NextRootMismatch
        ));

        let forged_next_epoch =
            PgcFixture::with_declared_transition(None, Some(fixture.current_epoch + 2));
        assert!(matches!(
            verify_privacy_envelope_v1(
                &forged_next_epoch.envelope,
                forged_next_epoch.verification_context(),
            ),
            Err(PrivacyVerificationErrorV1::Envelope(_))
        ));
    }

    #[test]
    fn pgc_rejects_cross_suite_proof_replay() {
        let fixture = PgcFixture::new();
        let (verange_envelope, _, _) = valid_envelope();
        let mut replayed = fixture.envelope.clone();
        replayed.proof = verange_envelope.proof;

        assert!(matches!(
            verify_privacy_envelope_v1(&replayed, fixture.verification_context()),
            Err(PrivacyVerificationErrorV1::Envelope(_))
        ));
    }

    #[test]
    fn context_chain_action_and_genesis_are_fail_closed() {
        let (envelope, activation, chain_id) = valid_envelope();

        let other_chain = ChainId::from("wrong-chain");
        let error =
            verify_privacy_envelope_v1(&envelope, verification_context(&activation, &other_chain))
                .expect_err("wrong chain");
        assert!(matches!(
            error,
            PrivacyVerificationErrorV1::Context(detail)
                if detail.code == PrivacyVerificationContextFailureCodeV1::ChainIdMismatch
        ));

        let mut context = verification_context(&activation, &chain_id);
        context.expected_action_index = 1;
        let error = verify_privacy_envelope_v1(&envelope, context).expect_err("wrong action index");
        assert!(matches!(
            error,
            PrivacyVerificationErrorV1::Context(detail)
                if detail.code == PrivacyVerificationContextFailureCodeV1::ActionIndexMismatch
        ));

        let mut context = verification_context(&activation, &chain_id);
        context.genesis_hash = [0; 32];
        let error = verify_privacy_envelope_v1(&envelope, context).expect_err("zero genesis");
        assert!(matches!(
            error,
            PrivacyVerificationErrorV1::Context(detail)
                if detail.code == PrivacyVerificationContextFailureCodeV1::ZeroGenesisHash
        ));
    }

    #[test]
    fn altered_activation_statement_and_proof_are_rejected() {
        let (envelope, activation, chain_id) = valid_envelope();

        let mut altered_activation = activation;
        altered_activation.verifier_digest.0[0] ^= 1;
        assert!(matches!(
            verify_privacy_envelope_v1(
                &envelope,
                verification_context(&altered_activation, &chain_id)
            ),
            Err(PrivacyVerificationErrorV1::CompiledActivation(_))
        ));

        let mut altered_statement = envelope.clone();
        let PrivacyStatementV1::VeRangeTransparentRangeV1(statement) =
            &mut altered_statement.statement
        else {
            unreachable!("fixture")
        };
        statement.value_commitments[0] = PrivacyP256PointV1::new([0xFF; 33]);
        altered_statement.statement_digest = altered_statement
            .statement
            .digest()
            .expect("changed digest");
        assert!(matches!(
            verify_privacy_envelope_v1(
                &altered_statement,
                verification_context(&activation, &chain_id)
            ),
            Err(PrivacyVerificationErrorV1::NativeVeRange(_))
        ));

        let mut altered_proof = envelope.clone();
        let PrivacyProofV1::VeRangeTransparentRangeV1(proof) = &mut altered_proof.proof else {
            unreachable!("fixture")
        };
        let last = proof.bytes.last_mut().expect("proof is non-empty");
        *last ^= 1;
        assert!(matches!(
            verify_privacy_envelope_v1(
                &altered_proof,
                verification_context(&activation, &chain_id)
            ),
            Err(PrivacyVerificationErrorV1::NativeVeRange(_))
        ));

        let mut context = verification_context(&activation, &chain_id);
        context.genesis_hash[0] ^= 1;
        assert!(matches!(
            verify_privacy_envelope_v1(&envelope, context),
            Err(PrivacyVerificationErrorV1::NativeVeRange(_))
        ));
    }

    #[test]
    fn proof_wire_rejects_truncation_extensions_and_single_byte_malleation() {
        let (envelope, activation, chain_id) = valid_envelope();
        let PrivacyProofV1::VeRangeTransparentRangeV1(proof) = &envelope.proof else {
            unreachable!("VeRange fixture")
        };
        let proof_len = proof.bytes.len();
        assert!(proof_len > 8, "fixture must exercise a structured proof");

        for length in [0, 1, proof_len / 3, proof_len / 2, proof_len - 1] {
            let mut candidate = envelope.clone();
            let PrivacyProofV1::VeRangeTransparentRangeV1(bytes) = &mut candidate.proof else {
                unreachable!("VeRange fixture")
            };
            bytes.bytes.truncate(length);
            assert_rejected(
                &candidate,
                &activation,
                &chain_id,
                &format!("truncated-{length}"),
            );
        }

        for suffix in [&[0_u8][..], &[0xFF][..], &[0x00, 0xFF][..]] {
            let mut candidate = envelope.clone();
            let PrivacyProofV1::VeRangeTransparentRangeV1(bytes) = &mut candidate.proof else {
                unreachable!("VeRange fixture")
            };
            bytes.bytes.extend_from_slice(suffix);
            assert_rejected(
                &candidate,
                &activation,
                &chain_id,
                &format!("trailing-{}", suffix.len()),
            );
        }

        for offset in [0, 1, proof_len / 3, proof_len / 2, proof_len - 1] {
            let mut candidate = envelope.clone();
            let PrivacyProofV1::VeRangeTransparentRangeV1(bytes) = &mut candidate.proof else {
                unreachable!("VeRange fixture")
            };
            bytes.bytes[offset] ^= 0x80;
            assert_rejected(
                &candidate,
                &activation,
                &chain_id,
                &format!("bit-flip-{offset}"),
            );
        }

        let mut all_zero = envelope.clone();
        let PrivacyProofV1::VeRangeTransparentRangeV1(bytes) = &mut all_zero.proof else {
            unreachable!("VeRange fixture")
        };
        bytes.bytes.fill(0);
        assert_rejected(&all_zero, &activation, &chain_id, "all-zero-proof");
    }

    #[test]
    fn statement_shape_order_and_public_inputs_are_cryptographically_bound() {
        let (envelope, activation, chain_id) = valid_envelope();

        let mut empty = envelope.clone();
        let statement = verange_statement_mut(&mut empty);
        statement.value_commitments.clear();
        statement.aggregation_count = 0;
        refresh_statement_digest(&mut empty);
        assert_rejected(&empty, &activation, &chain_id, "empty-batch");

        let mut count_mismatch = envelope.clone();
        verange_statement_mut(&mut count_mismatch).aggregation_count = 1;
        refresh_statement_digest(&mut count_mismatch);
        assert_rejected(
            &count_mismatch,
            &activation,
            &chain_id,
            "aggregation-count-mismatch",
        );

        let mut duplicate = envelope.clone();
        let statement = verange_statement_mut(&mut duplicate);
        statement.value_commitments[1] = statement.value_commitments[0];
        refresh_statement_digest(&mut duplicate);
        assert_rejected(&duplicate, &activation, &chain_id, "duplicate-commitment");

        let mut reordered = envelope.clone();
        verange_statement_mut(&mut reordered)
            .value_commitments
            .swap(0, 1);
        refresh_statement_digest(&mut reordered);
        assert_rejected(&reordered, &activation, &chain_id, "reordered-commitments");

        let mut changed_policy = envelope.clone();
        verange_statement_mut(&mut changed_policy).policy_id = PrivacyPolicyIdV1::new([0xA3; 32]);
        refresh_statement_digest(&mut changed_policy);
        assert_rejected(&changed_policy, &activation, &chain_id, "changed-policy");

        let mut changed_asset = envelope.clone();
        verange_statement_mut(&mut changed_asset).asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("other_asset").expect("name"),
        );
        refresh_statement_digest(&mut changed_asset);
        assert_rejected(&changed_asset, &activation, &chain_id, "changed-asset");

        let mut changed_profile = envelope.clone();
        verange_statement_mut(&mut changed_profile).bit_length = PrivacyVeRangeBitLengthV1::Bits64;
        refresh_statement_digest(&mut changed_profile);
        assert_rejected(
            &changed_profile,
            &activation,
            &chain_id,
            "changed-bit-length",
        );
    }

    #[test]
    fn governance_lifecycle_and_transcript_context_cannot_be_replayed() {
        let (envelope, activation, chain_id) = valid_envelope();

        let mut proposed = activation;
        proposed.lifecycle = PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
            proposed_at_height: 1,
            activate_at_height: 20,
        });
        assert!(matches!(
            verify_privacy_envelope_v1(
                &envelope,
                verification_context(&proposed, &chain_id)
            ),
            Err(PrivacyVerificationErrorV1::Envelope(detail))
                if matches!(
                    detail.source,
                    PrivacyProofEnvelopeValidationError::ActivationNotActive
                )
        ));

        let mut future = activation;
        let PrivacyProtocolLifecycleV1::Active(ref mut active) = future.lifecycle else {
            unreachable!("active fixture")
        };
        active.state_since_height = 11;
        assert!(matches!(
            verify_privacy_envelope_v1(
                &envelope,
                verification_context(&future, &chain_id)
            ),
            Err(PrivacyVerificationErrorV1::Envelope(detail))
                if matches!(
                    detail.source,
                    PrivacyProofEnvelopeValidationError::ActivationNotEffective {
                        current_height: 10,
                        effective_height: 11,
                    }
                )
        ));

        let replay_chain = ChainId::from("taira-privacy-replay");
        let mut replayed = envelope.clone();
        verange_statement_mut(&mut replayed).context.chain_id = replay_chain.clone();
        refresh_statement_digest(&mut replayed);
        assert!(matches!(
            verify_privacy_envelope_v1(&replayed, verification_context(&activation, &replay_chain)),
            Err(PrivacyVerificationErrorV1::NativeVeRange(_))
        ));

        let mut replayed = envelope.clone();
        verange_statement_mut(&mut replayed).context.action_index = 1;
        refresh_statement_digest(&mut replayed);
        let mut context = verification_context(&activation, &chain_id);
        context.expected_action_index = 1;
        assert!(matches!(
            verify_privacy_envelope_v1(&replayed, context),
            Err(PrivacyVerificationErrorV1::Envelope(_))
                | Err(PrivacyVerificationErrorV1::NativeVeRange(_))
        ));
    }

    #[test]
    fn every_governed_envelope_binding_fails_closed_when_tampered() {
        let (envelope, activation, chain_id) = valid_envelope();
        let mutations: [(&str, fn(&mut PrivacyProofEnvelopeV1)); 9] = [
            ("protocol", |value| {
                value.protocol_id = PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1
            }),
            ("parameter-id", |value| value.parameter_id.0[0] ^= 1),
            ("parameter-digest", |value| value.parameter_digest.0[0] ^= 1),
            ("verifier-digest", |value| value.verifier_digest.0[0] ^= 1),
            ("schema-digest", |value| {
                value.statement_schema_digest.0[0] ^= 1
            }),
            ("engine-manifest", |value| {
                value.engine_manifest_digest.0[0] ^= 1
            }),
            ("statement-digest", |value| value.statement_digest.0[0] ^= 1),
            ("proof-system", |value| {
                value.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
            }),
            ("engine", |value| {
                value.engine_id = PrivacyEngineIdV1::NativeGoldilocksStarkFri
            }),
        ];
        for (label, mutate) in mutations {
            let mut candidate = envelope.clone();
            mutate(&mut candidate);
            assert_rejected(&candidate, &activation, &chain_id, label);
        }
    }

    #[test]
    fn every_statement_context_artifact_binding_fails_closed_when_tampered() {
        let (envelope, activation, chain_id) = valid_envelope();
        let mutations: [(&str, fn(&mut PrivacyStatementContextV1)); 5] = [
            ("statement-parameter-id", |context| {
                context.parameter_id.0[0] ^= 1
            }),
            ("statement-parameter-digest", |context| {
                context.parameter_digest.0[0] ^= 1
            }),
            ("statement-verifier-digest", |context| {
                context.verifier_digest.0[0] ^= 1
            }),
            ("statement-schema-digest", |context| {
                context.statement_schema_digest.0[0] ^= 1
            }),
            ("statement-engine-manifest", |context| {
                context.engine_manifest_digest.0[0] ^= 1
            }),
        ];
        for (label, mutate) in mutations {
            let mut candidate = envelope.clone();
            mutate(&mut verange_statement_mut(&mut candidate).context);
            refresh_statement_digest(&mut candidate);
            assert_rejected(&candidate, &activation, &chain_id, label);
        }
    }
}
