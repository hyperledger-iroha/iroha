//! Original-owner direct X509 preparation and signing through the canonical Rust model.

use iroha_core_privacy::{
    privacy_engines::validate_zk_x509_credential_proof_container_v1,
    privacy_profiles::compiled_privacy_profile_v1,
};
use iroha_crypto::{Algorithm, PrivateKey};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    isi::privacy::SubmitPrivacyZkX509CertificateV1,
    privacy::{
        IrohaZkX509StarkP256StatementV1, PrivacyCompiledProfileResultV1,
        PrivacyCompiledProfileSnapshotV1, PrivacyConsensusLimitsV1,
        PrivacyExact12CapabilityManifestV1, PrivacyProtocolIdV1, PrivacyProtocolLimitsV1,
        PrivacyStatementV1,
    },
    transaction::{TransactionBuilder, TransactionPayload, signed_attachments::*},
};
use napi::bindgen_prelude::{Buffer, Uint8Array};
use napi_derive::napi;

const MAX_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;
const MAX_CONTEXT_JSON_BYTES: usize = 256 * 1024;
const PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaZkX509StarkP256V1;

/// Exact inputs for one original direct X509 action; transport authentication belongs to the SDK.
#[napi(object)]
pub struct JsPrivacyX509DirectInputV1 {
    /// Canonical manifest obtained through the SDK's authenticated Torii transport.
    pub manifest: Uint8Array,
    /// Expected immutable genesis-derived network identity.
    pub network_id: Uint8Array,
    /// Canonical proof-free request with ZERO derived intent and action/slot zero.
    pub request_norito: Uint8Array,
    /// Original transaction authority; it must equal the request's wallet account.
    pub authority: String,
    /// Exact ordinary fee-payment intent in Norito JSON.
    pub fee_payment_json: String,
    /// Optional metadata in Norito JSON, frozen before proof generation.
    pub metadata_json: Option<String>,
    /// Explicit original creation time, as an exact JavaScript-safe millisecond integer.
    pub creation_time_ms: i64,
    /// Explicit positive original time to live in milliseconds.
    pub ttl_ms: i64,
    /// Optional nonzero original nonce.
    pub nonce: Option<u32>,
}

struct Admission {
    profile: PrivacyCompiledProfileSnapshotV1,
    protocol_limits: PrivacyProtocolLimitsV1,
    limits: PrivacyConsensusLimitsV1,
}

struct Projection {
    statement: IrohaZkX509StarkP256StatementV1,
    statement_norito: Vec<u8>,
    ordered_intent: [u8; 32],
    action_statement: Vec<u8>,
    action_digest: [u8; 32],
}

/// Native owner of one original request and transaction context. There is no public constructor.
///
/// The owner retains no private key or proof. Copies of its public projections cannot replace
/// its original inputs. Runtime construction availability is checked again before signing.
#[napi]
pub struct JsPrivacyX509PreparedDirectV1 {
    manifest: PrivacyExact12CapabilityManifestV1,
    request: SubmitPrivacyZkX509CertificateV1,
    original: TransactionPayload,
    projection: Projection,
}

fn invalid(message: impl Into<String>) -> napi::Error {
    napi::Error::new(napi::Status::InvalidArg, message.into())
}

fn decode_request(bytes: &[u8]) -> napi::Result<SubmitPrivacyZkX509CertificateV1> {
    if bytes.is_empty()
        || bytes.len() > SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1()
    {
        return Err(invalid("X509 request exceeds its canonical byte bound"));
    }
    let request: SubmitPrivacyZkX509CertificateV1 =
        norito::core::with_decode_limits(norito::canonical_decode_limits(bytes.len()), || {
            norito::decode_from_bytes(bytes)
        })
        .map_err(super::norito_to_napi)?;
    if norito::encode_canonical(&request).map_err(super::norito_to_napi)? != bytes {
        return Err(invalid("X509 request must be the exact canonical archive"));
    }
    request
        .validate_proof_free_v1(&PrivacyConsensusLimitsV1::taira_default())
        .map_err(super::norito_to_napi)?;
    if request.proof_ref.action_index != 0 || request.statement.context.action_index != 0 {
        return Err(invalid(
            "X509 direct producer requires action and privacy ordinal zero",
        ));
    }
    Ok(request)
}

fn require_admission(
    manifest: &PrivacyExact12CapabilityManifestV1,
    request: &SubmitPrivacyZkX509CertificateV1,
    network: &NetworkId,
    authority: &AccountId,
) -> napi::Result<Admission> {
    super::require_privacy_exact12_network_v1(&manifest.network_id, network)?;
    if request.statement.context.network_id != *network
        || request.statement.wallet_account != *authority
    {
        return Err(invalid(
            "X509 request differs from the original network or authority",
        ));
    }
    // Preserve the actual construction error, including JointSecurityReductionIncomplete.
    // Neither a remote Available row nor a documentary receipt can manufacture this profile.
    let profile = compiled_privacy_profile_v1(PROTOCOL).map_err(super::norito_to_napi)?;
    let row = manifest
        .protocols
        .iter()
        .find(|row| row.protocol_id == PROTOCOL)
        .ok_or_else(|| invalid("X509 capability row is absent"))?;
    if !row.is_network_available()
        || row.compiled_profile
            != PrivacyCompiledProfileResultV1::Available(PrivacyCompiledProfileSnapshotV1::from(
                profile,
            ))
    {
        return Err(invalid(
            "X509 selected capability is unavailable or differs from the local compiled tuple",
        ));
    }
    let context = &request.statement.context;
    if context.parameter_id != profile.parameter_id
        || context.parameter_digest != profile.parameter_digest
        || context.verifier_digest != profile.verifier_digest
        || context.statement_schema_digest != profile.statement_schema_digest
        || context.engine_manifest_digest != profile.engine_manifest_digest
    {
        return Err(invalid(
            "X509 request differs from the admitted five-field compiled tuple",
        ));
    }
    let protocol_limits = row
        .effective_protocol_limits_v1()
        .ok_or_else(|| invalid("X509 selected native profile is unavailable"))?;
    protocol_limits
        .validate_with_ceiling(&profile.protocol_limits)
        .map_err(super::norito_to_napi)?;
    let limits = manifest.consensus_policy.current_limits;
    request
        .validate_proof_free_v1(&limits)
        .map_err(super::norito_to_napi)?;
    if limits.max_actions_per_transaction < 1 {
        return Err(invalid("X509 committed policy permits no action"));
    }
    Ok(Admission {
        profile: profile.snapshot(),
        protocol_limits,
        limits,
    })
}

// Container and header binding only; native proof verification and State remain consensus work.
fn require_proof_container(
    statement: &IrohaZkX509StarkP256StatementV1,
    network: &NetworkId,
    proof: &[u8],
) -> napi::Result<()> {
    validate_zk_x509_credential_proof_container_v1(statement, *network.as_bytes(), proof)
        .map_err(super::norito_to_napi)
}

fn declared_payload(
    original: &TransactionPayload,
    request: &SubmitPrivacyZkX509CertificateV1,
    proof: Vec<u8>,
) -> napi::Result<TransactionPayload> {
    let mut plan = SignedAttachmentPlanV1::new();
    plan.push_action(PlannedActionV1 {
        operation: ActionOperationV1::NativeInstruction(
            SubmitPrivacyZkX509CertificateV1::WIRE_ID.into(),
        ),
        role: ActionRoleV1::ProofEffect,
        authority: ActionAuthorityBindingV1::TransactionAuthority,
        invocation: ActionInvocationBindingV1::Direct {
            instruction_index: 0,
        },
        public_inputs_digest: request
            .public_inputs_digest_v1()
            .map_err(super::norito_to_napi)?,
        attachments: vec![PlannedAttachmentV1 {
            kind: AttachmentKindV1::Proof,
            role: AttachmentRoleV1::ActionProof,
            bytes: proof,
        }],
    });
    let mut payload = original.clone();
    payload.attachments = Some(plan.declare());
    payload
        .bind_attachment_statements_v1()
        .map_err(super::norito_to_napi)?;
    Ok(payload)
}

fn project(
    original: &TransactionPayload,
    request: &SubmitPrivacyZkX509CertificateV1,
    limits: &PrivacyConsensusLimitsV1,
) -> napi::Result<Projection> {
    let payload = declared_payload(original, request, Vec::new())?;
    let ordered = payload
        .ordered_intent_digest_v1()
        .map_err(super::norito_to_napi)?;
    let action = payload
        .attachments
        .as_ref()
        .and_then(|a| a.action(0))
        .ok_or_else(|| invalid("X509 original action is absent"))?;
    let statement = request
        .materialize_statement_v1(action, ordered, limits)
        .map_err(super::norito_to_napi)?;
    let action_digest = action
        .statement_digest_for_v1(ordered)
        .map_err(super::norito_to_napi)?;
    Ok(Projection {
        statement_norito: norito::encode_canonical(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
            statement.clone(),
        ))
        .map_err(super::norito_to_napi)?,
        statement,
        ordered_intent: *ordered.as_bytes(),
        action_statement: norito::encode_canonical(&action.statement_for_v1(ordered))
            .map_err(super::norito_to_napi)?,
        action_digest: *action_digest.as_bytes(),
    })
}

/// Freeze exact independent transaction inputs and derive the sole original proof statement.
#[napi]
pub fn privacy_prepare_x509_direct_transaction_v1(
    input: JsPrivacyX509DirectInputV1,
) -> napi::Result<JsPrivacyX509PreparedDirectV1> {
    if input.fee_payment_json.len() > MAX_CONTEXT_JSON_BYTES
        || input
            .metadata_json
            .as_ref()
            .is_some_and(|value| value.len() > MAX_CONTEXT_JSON_BYTES)
    {
        return Err(invalid(
            "X509 transaction context JSON exceeds its byte bound",
        ));
    }
    let network = super::parse_transaction_network_id_bytes(input.network_id.as_ref())?;
    let _guard = super::scoped_chain_discriminant_for_literal(&input.authority)?;
    let authority = super::parse_account_id(&input.authority, "X509 authority")?;
    let request = decode_request(input.request_norito.as_ref())?;
    let manifest = super::decode_privacy_exact12_capability_manifest_v1(input.manifest.as_ref())?;
    let admitted = require_admission(&manifest, &request, &network, &authority)?;
    let fee_payment = super::parse_fee_payment_intent(&input.fee_payment_json)?;
    let metadata = super::parse_metadata_payload("X509 transaction", input.metadata_json)?;
    let builder = super::configure_transaction_builder(
        TransactionBuilder::new(network, authority, fee_payment)
            .with_instructions([request.clone()]),
        metadata,
        None,
        Some(input.creation_time_ms),
        Some(input.ttl_ms),
        input.nonce,
    )?;
    let original = builder.into_payload().map_err(super::norito_to_napi)?;
    let projection = project(&original, &request, &admitted.limits)?;
    admitted
        .protocol_limits
        .validate_statement(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
            projection.statement.clone(),
        ))
        .map_err(super::norito_to_napi)?;
    let request_len = input.request_norito.len();
    if request_len > admitted.limits.max_action_bytes as usize
        || request_len > admitted.limits.max_privacy_bytes_per_transaction as usize
    {
        return Err(invalid(
            "X509 request exceeds committed action or transaction privacy bytes",
        ));
    }
    Ok(JsPrivacyX509PreparedDirectV1 {
        manifest,
        request,
        original,
        projection,
    })
}

#[napi]
impl JsPrivacyX509PreparedDirectV1 {
    /// Canonical public statement passed to the holder prover, returned as a copy.
    #[napi(getter)]
    pub fn materialized_statement_norito(&self) -> Buffer {
        Buffer::from(self.projection.statement_norito.clone())
    }
    /// Original ordered-intent digest, returned as a copy.
    #[napi(getter)]
    pub fn ordered_intent_digest(&self) -> Buffer {
        Buffer::from(self.projection.ordered_intent.to_vec())
    }
    /// Exact original ActionStatement frame, returned as a copy.
    #[napi(getter)]
    pub fn action_statement_norito(&self) -> Buffer {
        Buffer::from(self.projection.action_statement.clone())
    }
    /// Exact ActionStatement digest, returned as a copy.
    #[napi(getter)]
    pub fn action_statement_digest(&self) -> Buffer {
        Buffer::from(self.projection.action_digest.to_vec())
    }
    /// Original pre-attachment payload for independent SDK byte-parity checks; not signable.
    #[napi(getter)]
    pub fn original_payload_bytes(&self) -> napi::Result<Buffer> {
        let builder = TransactionBuilder::from_payload(self.original.clone())
            .map_err(super::norito_to_napi)?;
        let bytes = builder.encode_payload();
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(invalid("X509 original payload exceeds local byte ceiling"));
        }
        Ok(Buffer::from(bytes))
    }
    /// Attach the exact proof occurrence and sign after repeating native policy and identity checks.
    /// The canonical X5S1 container must bind the exact original statement and network.
    /// Proof verification, current State and replay protection remain consensus responsibilities.
    #[napi]
    pub fn sign(
        &self,
        raw_proof: Uint8Array,
        secret: Uint8Array,
    ) -> napi::Result<super::JsSignedTransaction> {
        let network = self
            .original
            .network_id()
            .ok_or_else(|| invalid("X509 cannot use the genesis domain"))?;
        let admitted = require_admission(
            &self.manifest,
            &self.request,
            network,
            &self.original.authority,
        )?;
        if raw_proof.len() > admitted.limits.max_proof_bytes_per_action as usize {
            return Err(invalid("X509 proof exceeds the committed byte bound"));
        }
        // Freeze the caller's proof exactly once after its allocation bound. Validation
        // and insertion use the same bytes even if the caller later changes its buffer.
        let proof = raw_proof.as_ref().to_vec();
        self.request
            .validate_materialized_against_profile_v1(
                &self.projection.statement,
                &proof,
                &admitted.profile,
                &admitted.protocol_limits,
                &admitted.limits,
            )
            .map_err(super::norito_to_napi)?;
        require_proof_container(&self.projection.statement, network, &proof)?;
        let payload = declared_payload(&self.original, &self.request, proof)?;
        payload
            .validate_signed_attachments_v1()
            .map_err(super::norito_to_napi)?;
        let ordered = payload
            .ordered_intent_digest_v1()
            .map_err(super::norito_to_napi)?;
        let action = payload
            .attachments
            .as_ref()
            .and_then(|a| a.action(0))
            .ok_or_else(|| invalid("X509 completed original action is absent"))?;
        if ordered.as_bytes() != &self.projection.ordered_intent
            || action
                .statement_digest_for_v1(ordered)
                .map_err(super::norito_to_napi)?
                .as_bytes()
                != &self.projection.action_digest
        {
            return Err(invalid(
                "X509 proof insertion changed the original statement",
            ));
        }
        let builder = TransactionBuilder::from_payload(payload).map_err(super::norito_to_napi)?;
        if builder.encode_payload().len() > MAX_PAYLOAD_BYTES {
            return Err(invalid("X509 completed payload exceeds local byte ceiling"));
        }
        if secret.len() != 32 {
            return Err(invalid(
                "X509 signing requires an exact 32-byte Ed25519 private key",
            ));
        }
        let key = PrivateKey::from_bytes(Algorithm::Ed25519, secret.as_ref())
            .map_err(super::norito_to_napi)?;
        let signed = super::sign_js_transaction(builder, &key, "original X509")?;
        signed.verify_signature().map_err(super::norito_to_napi)?;
        Ok(super::JsSignedTransaction {
            signed_transaction: Buffer::from(super::encode_canonical_signed_transaction_v1(
                &signed,
            )?),
            hash: Buffer::from(signed.hash().as_ref().to_vec()),
        })
    }
}

#[cfg(test)]
mod tests;
