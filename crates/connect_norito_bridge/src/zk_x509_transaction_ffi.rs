//! Opaque C ownership of one canonical direct X509 transaction.
//!
//! Rust owns the original payload and selected native policy. A supplied archive is
//! NOT authenticated transport: SDKs must retain their sealed HTTPS origin separately.
//! There is no mutable draft, alternative signing payload, or legacy inline route.
use iroha_core_privacy::{
    privacy_engines::{
        ZK_X509_CREDENTIAL_PROOF_MAX_BYTES_V1, validate_zk_x509_credential_proof_container_v1,
    },
    privacy_profiles::compiled_privacy_profile_snapshot_result_v1,
};
use iroha_crypto::{Algorithm, PrivateKey, PublicKey};
use iroha_data_model::{
    NetworkId,
    isi::privacy::SubmitPrivacyZkX509CertificateV1,
    privacy::*,
    transaction::{
        Executable, TransactionBuilder as ModelBuilder, TransactionPayload, signed_attachments::*,
    },
};
use iroha_version::codec::EncodeVersioned;
use libc::{c_int, c_ulong};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};
use zeroize::Zeroizing;

const INVALID: c_int = -1;
const CLOSED: c_int = -2;
const RESOURCE: c_int = -3;
const CANONICAL: c_int = -4;
const NETWORK: c_int = -5;
const AUTHORITY: c_int = -6;
const SOURCE: c_int = -7;
const POLICY: c_int = -8;
const PROOF: c_int = -9;
const TUPLE: c_int = -11;
const JOINT_SECURITY: c_int = -24;
const INTERNAL: c_int = -100;
const MAX_TRANSACTION: usize = 10 * 1024 * 1024;
const MAX_OWNERS: usize = 64;
const PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::IrohaZkX509StarkP256V1;
type Result<T> = std::result::Result<T, c_int>;
fn invalid(_: impl std::fmt::Display) -> c_int {
    SOURCE
}
fn require_key(payload: &TransactionPayload, key: &PrivateKey) -> Result<()> {
    let public = PublicKey::from(key.clone());
    if public.algorithm() != Algorithm::Ed25519
        || payload.authority.try_signatory() != Some(&public)
    {
        return Err(AUTHORITY);
    }
    Ok(())
}
fn require_proof_bound(proof: &[u8], limits: &PrivacyConsensusLimitsV1) -> Result<()> {
    if proof.is_empty()
        || proof.len() > ZK_X509_CREDENTIAL_PROOF_MAX_BYTES_V1
        || proof.len() > limits.max_proof_bytes_per_action as usize
    {
        return Err(PROOF);
    }
    Ok(())
}
/// Private wire plan. No caller may supply an alternative payload at signing.
struct DirectPlan {
    payload: TransactionPayload,
    request: SubmitPrivacyZkX509CertificateV1,
    materialized: IrohaZkX509StarkP256StatementV1,
    intent: OrderedIntentDigestV1,
    action: ActionStatementV1,
    action_digest: ActionStatementDigestV1,
}

fn declared(
    request: &SubmitPrivacyZkX509CertificateV1,
    proof: Vec<u8>,
) -> Result<SignedAttachmentsV1> {
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
        public_inputs_digest: request.public_inputs_digest_v1().map_err(invalid)?,
        attachments: vec![PlannedAttachmentV1 {
            kind: AttachmentKindV1::Proof,
            role: AttachmentRoleV1::ActionProof,
            bytes: proof,
        }],
    });
    Ok(plan.declare())
}

impl DirectPlan {
    fn prepare(
        mut payload: TransactionPayload,
        request: SubmitPrivacyZkX509CertificateV1,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<Self> {
        request.validate_proof_free_v1(limits).map_err(invalid)?;
        if request.proof_ref.action_index != 0
            || request.statement.context.action_index != 0
            || request.statement.wallet_account != payload.authority
            || payload.network_id() != Some(&request.statement.context.network_id)
            || payload.attachments.is_some()
        {
            return Err(invalid(
                "request network, authority, ordinal or original attachment differs",
            ));
        }
        let Executable::Instructions(instructions) = &payload.instructions else {
            return Err(invalid("one direct instruction is required"));
        };
        let [instruction] = instructions.as_ref() else {
            return Err(invalid("one direct instruction is required"));
        };
        if instruction
            .as_any()
            .downcast_ref::<SubmitPrivacyZkX509CertificateV1>()
            != Some(&request)
        {
            return Err(invalid("original instruction differs from request"));
        }
        // Empty bytes exist only in this private planning owner, never a signed transaction.
        payload.attachments = Some(declared(&request, Vec::new())?);
        payload.bind_attachment_statements_v1().map_err(invalid)?;
        let intent = payload.ordered_intent_digest_v1().map_err(invalid)?;
        let action = payload
            .attachments
            .as_ref()
            .and_then(|a| a.action(0))
            .ok_or_else(|| invalid("original action is absent"))?;
        let materialized = request
            .materialize_statement_v1(action, intent, limits)
            .map_err(invalid)?;
        let action_digest = action.statement_digest_for_v1(intent).map_err(invalid)?;
        let action = action.statement_for_v1(intent);
        Ok(Self {
            payload,
            request,
            materialized,
            intent,
            action,
            action_digest,
        })
    }

    fn complete(
        &self,
        proof: &[u8],
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<TransactionPayload> {
        require_proof_bound(proof, limits)?;
        let mut payload = self.payload.clone();
        let original = payload
            .attachments
            .as_mut()
            .ok_or_else(|| invalid("original occurrence is absent"))?;
        if original.actions().len() != 1
            || original.occurrences().len() != 1
            || !original.occurrences()[0].bytes.is_empty()
        {
            return Err(invalid("original incomplete occurrence was replaced"));
        }
        original
            .set_occurrence_bytes(0, proof.to_vec())
            .map_err(invalid)?;
        payload.bind_attachment_statements_v1().map_err(invalid)?;
        payload.validate_signed_attachments_v1().map_err(invalid)?;
        let intent = payload.ordered_intent_digest_v1().map_err(invalid)?;
        let action = payload
            .attachments
            .as_ref()
            .and_then(|a| a.action(0))
            .ok_or_else(|| invalid("original action is absent"))?;
        if intent != self.intent
            || action.statement_for_v1(intent) != self.action
            || action.statement_digest_for_v1(intent).map_err(invalid)? != self.action_digest
            || self
                .request
                .materialize_statement_v1(action, intent, limits)
                .map_err(invalid)?
                != self.materialized
        {
            return Err(invalid(
                "completed occurrence changed original intent or independent inputs",
            ));
        }
        Ok(payload)
    }
}

fn unavailable(reason: PrivacyCompiledProfileUnavailableReasonV1) -> c_int {
    match reason {
        PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable => -20,
        PrivacyCompiledProfileUnavailableReasonV1::ProfileInitializationFailed => -21,
        PrivacyCompiledProfileUnavailableReasonV1::StatementSchemaInvalid(
            PrivacyCompiledStatementSchemaErrorV1::ConflictingStableTypeId,
        ) => -22,
        PrivacyCompiledProfileUnavailableReasonV1::StatementSchemaInvalid(
            PrivacyCompiledStatementSchemaErrorV1::MissingTypeReference,
        ) => -23,
        PrivacyCompiledProfileUnavailableReasonV1::JointSecurityReductionIncomplete => {
            JOINT_SECURITY
        }
    }
}
fn policy(
    manifest: &PrivacyExact12CapabilityManifestV1,
    network: NetworkId,
) -> Result<(
    PrivacyCompiledProfileSnapshotV1,
    PrivacyProtocolLimitsV1,
    PrivacyConsensusLimitsV1,
)> {
    manifest.validate().map_err(|_| POLICY)?;
    if manifest.network_id != network {
        return Err(NETWORK);
    }
    let local = match compiled_privacy_profile_snapshot_result_v1(PROTOCOL) {
        PrivacyCompiledProfileResultV1::Available(profile) => profile,
        PrivacyCompiledProfileResultV1::Unavailable(reason) => return Err(unavailable(reason)),
    };
    let row = manifest
        .protocols
        .iter()
        .find(|r| r.protocol_id == PROTOCOL)
        .ok_or(POLICY)?;
    if let PrivacyCapabilityReadinessV1::Unavailable(
        PrivacyCapabilityUnavailableReasonV1::CompiledProfile(reason),
    ) = row.readiness
    {
        return Err(unavailable(reason));
    }
    if row.compiled_profile != PrivacyCompiledProfileResultV1::Available(local) {
        return Err(TUPLE);
    }
    let limits = row.effective_protocol_limits_v1().ok_or(POLICY)?;
    Ok((local, limits, manifest.consensus_policy.current_limits))
}
fn governed(
    statement: &IrohaZkX509StarkP256StatementV1,
    profile: &PrivacyCompiledProfileSnapshotV1,
    protocol_limits: &PrivacyProtocolLimitsV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<()> {
    profile.validate().map_err(|_| POLICY)?;
    protocol_limits
        .validate_with_ceiling(&profile.protocol_limits)
        .map_err(|_| POLICY)?;
    let context = &statement.context;
    if profile.protocol_id != PROTOCOL
        || profile.parameter_id != context.parameter_id
        || profile.parameter_digest != context.parameter_digest
        || profile.verifier_digest != context.verifier_digest
        || profile.statement_schema_digest != context.statement_schema_digest
        || profile.engine_manifest_digest != context.engine_manifest_digest
    {
        return Err(POLICY);
    }
    let tagged = PrivacyStatementV1::IrohaZkX509StarkP256V1(statement.clone());
    profile
        .protocol_limits
        .validate_statement(&tagged)
        .map_err(|_| POLICY)?;
    protocol_limits
        .validate_statement(&tagged)
        .map_err(|_| POLICY)?;
    tagged.validate(limits).map_err(|_| POLICY)
}
struct Owner {
    plan: DirectPlan,
    manifest: PrivacyExact12CapabilityManifestV1,
    network: NetworkId,
}
impl Owner {
    fn prepare(network: &[u8], manifest: &[u8], payload: &[u8], request: &[u8]) -> Result<Self> {
        let network = super::network_id_from_raw_bytes(network).map_err(|_| NETWORK)?;
        if !validate_privacy_capability_archive_v1(manifest).is_valid() {
            return Err(CANONICAL);
        }
        let manifest: PrivacyExact12CapabilityManifestV1 =
            norito::decode_canonical(manifest).map_err(|_| CANONICAL)?;
        let (profile, protocol_limits, limits) = policy(&manifest, network)?;
        let mut payload: TransactionPayload =
            norito::decode_canonical(payload).map_err(|_| CANONICAL)?;
        ModelBuilder::from_payload(payload.clone()).map_err(|_| SOURCE)?;
        if payload.network_id() != Some(&network) {
            return Err(NETWORK);
        }
        let Executable::Instructions(instructions) = &payload.instructions else {
            return Err(SOURCE);
        };
        if !instructions.is_empty() || payload.attachments.is_some() {
            return Err(SOURCE);
        }
        if payload
            .authority
            .try_signatory()
            .is_none_or(|key| key.algorithm() != Algorithm::Ed25519)
        {
            return Err(AUTHORITY);
        }
        let request: SubmitPrivacyZkX509CertificateV1 =
            norito::decode_canonical(request).map_err(|_| CANONICAL)?;
        payload.instructions = Executable::Instructions(vec![request.clone().into()].into());
        let plan = DirectPlan::prepare(payload, request, &limits)?;
        governed(&plan.materialized, &profile, &protocol_limits, &limits)?;
        Ok(Self {
            plan,
            manifest,
            network,
        })
    }
    fn projection(&self, kind: u32) -> Result<Vec<u8>> {
        match kind {
            0 => Ok(self.plan.intent.as_bytes().to_vec()),
            1 => Ok(self.plan.action_digest.as_bytes().to_vec()),
            2 => norito::encode_canonical(&self.plan.action).map_err(|_| INTERNAL),
            3 => norito::encode_canonical(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
                self.plan.materialized.clone(),
            ))
            .map_err(|_| INTERNAL),
            _ => Err(INVALID),
        }
    }
    fn sign(&self, seed: &[u8], proof: &[u8]) -> Result<Vec<u8>> {
        if seed.len() != 32 {
            return Err(AUTHORITY);
        }
        let seed = Zeroizing::new(seed.to_vec());
        let key = PrivateKey::from_bytes(Algorithm::Ed25519, &seed).map_err(|_| AUTHORITY)?;
        require_key(&self.plan.payload, &key)?;
        let (profile, protocol_limits, limits) = policy(&self.manifest, self.network)?;
        governed(&self.plan.materialized, &profile, &protocol_limits, &limits)?;
        require_proof_bound(proof, &limits)?;
        self.plan
            .request
            .validate_materialized_against_profile_v1(
                &self.plan.materialized,
                proof,
                &profile,
                &protocol_limits,
                &limits,
            )
            .map_err(|_| POLICY)?;
        validate_zk_x509_credential_proof_container_v1(
            &self.plan.materialized,
            *self.network.as_bytes(),
            proof,
        )
        .map_err(|_| PROOF)?;
        let payload = self.plan.complete(proof, &limits)?;
        let signed = ModelBuilder::from_payload(payload)
            .map_err(|_| SOURCE)?
            .try_sign(&key)
            .map_err(|_| AUTHORITY)?;
        signed.verify_signature().map_err(|_| AUTHORITY)?;
        let bytes = signed.encode_versioned();
        if bytes.len() > MAX_TRANSACTION {
            return Err(RESOURCE);
        }
        Ok(bytes)
    }
}
#[derive(Default)]
struct Registry {
    next: u64,
    owners: BTreeMap<u64, Arc<Owner>>,
}
impl Registry {
    fn insert(&mut self, owner: Owner) -> Result<u64> {
        if self.owners.len() >= MAX_OWNERS {
            return Err(RESOURCE);
        }
        self.next = self
            .next
            .checked_add(1)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or(RESOURCE)?;
        self.owners.insert(self.next, Arc::new(owner));
        Ok(self.next)
    }
    fn get(&self, handle: u64) -> Result<Arc<Owner>> {
        self.owners.get(&handle).cloned().ok_or(CLOSED)
    }
    fn close(&mut self, handle: u64) -> Result<()> {
        self.owners.remove(&handle).map(|_| ()).ok_or(CLOSED)
    }
}
fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}
fn retained(handle: u64) -> Result<Arc<Owner>> {
    registry().lock().map_err(|_| INTERNAL)?.get(handle)
}
fn boundary(f: impl FnOnce() -> Result<()>) -> c_int {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or(Err(INTERNAL))
        .err()
        .unwrap_or(0)
}
unsafe fn bytes<'a>(pointer: *const u8, length: c_ulong, maximum: usize) -> Result<&'a [u8]> {
    let length = usize::try_from(length).map_err(|_| RESOURCE)?;
    if length == 0 || length > maximum {
        return Err(INVALID);
    }
    if pointer.is_null() {
        return Err(INVALID);
    }
    Ok(unsafe { std::slice::from_raw_parts(pointer, length) })
}
unsafe fn output(
    out: *mut *mut u8,
    length: *mut c_ulong,
    f: impl FnOnce() -> Result<Vec<u8>>,
) -> Result<()> {
    if !out.is_null() {
        unsafe {
            *out = std::ptr::null_mut();
        }
    }
    if !length.is_null() {
        unsafe {
            *length = 0;
        }
    }
    if out.is_null() || length.is_null() {
        return Err(INVALID);
    }
    let value = f()?;
    unsafe { super::write_privacy_bytes(out, length, &value).map_err(|_| RESOURCE) }
}
/// Prepare an immutable owner. The manifest must come from caller-authenticated transport.
/// # Safety
/// Inputs must be readable for their stated lengths; `out` must be writable and disjoint.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_zk_x509_transaction_prepare_v1(
    network: *const u8,
    network_len: c_ulong,
    manifest: *const u8,
    manifest_len: c_ulong,
    payload: *const u8,
    payload_len: c_ulong,
    request: *const u8,
    request_len: c_ulong,
    out: *mut u64,
) -> c_int {
    boundary(|| unsafe {
        if out.is_null() {
            return Err(INVALID);
        }
        *out = 0;
        let owner = Owner::prepare(
            bytes(network, network_len, 32)?,
            bytes(
                manifest,
                manifest_len,
                PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1,
            )?,
            bytes(payload, payload_len, MAX_TRANSACTION)?,
            bytes(
                request,
                request_len,
                SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1(),
            )?,
        )?;
        *out = registry().lock().map_err(|_| INTERNAL)?.insert(owner)?;
        Ok(())
    })
}
/// Copy one projection: 0 ordered intent, 1 action digest, 2 ActionStatement, 3 tagged PrivacyStatement.
/// # Safety
/// Output destinations must be writable and disjoint. Free outputs with `iroha_privacy_free_buffer`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_zk_x509_transaction_projection_v1(
    handle: u64,
    kind: u32,
    out: *mut *mut u8,
    length: *mut c_ulong,
) -> c_int {
    boundary(|| unsafe { output(out, length, || retained(handle)?.projection(kind)) })
}
/// Sign only the retained payload after selected native policy and actual proof-container checks.
/// # Safety
/// Inputs must be readable for their lengths and outputs writable/disjoint. Free with `iroha_privacy_free_buffer`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_zk_x509_transaction_sign_v1(
    handle: u64,
    seed: *const u8,
    seed_len: c_ulong,
    proof: *const u8,
    proof_len: c_ulong,
    out: *mut *mut u8,
    length: *mut c_ulong,
) -> c_int {
    boundary(|| unsafe {
        output(out, length, || {
            let owner = retained(handle)?;
            owner.sign(
                bytes(seed, seed_len, 32)?,
                bytes(proof, proof_len, ZK_X509_CREDENTIAL_PROOF_MAX_BYTES_V1)?,
            )
        })
    })
}
/// Close once. Stale/invalid handles return -2. Already accepted calls retain their own Arc.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_zk_x509_transaction_close_v1(handle: u64) -> c_int {
    boundary(|| registry().lock().map_err(|_| INTERNAL)?.close(handle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;
    use iroha_data_model::{account::AccountId, transaction::TransactionDomain};
    use std::num::{NonZeroU32, NonZeroU64};
    fn fixture() -> (
        TransactionPayload,
        SubmitPrivacyZkX509CertificateV1,
        Vec<u8>,
    ) {
        use base64::Engine as _;
        let encoded =
            include_str!("../../../fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64")
                .trim();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("current fixture base64");
        assert_eq!(
            bytes,
            iroha_data_model::privacy::privacy_exact12_fixture_bundle_bytes_v1()
                .expect("current typed model fixture")
        );
        let current: iroha_data_model::privacy::PrivacyExact12FixtureBundleV1 =
            norito::decode_canonical(&bytes).expect("current dedicated request fixture");
        let row = current
            .rows
            .into_iter()
            .find(|row| row.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
            .expect("one X509 row");
        let payload = ModelBuilder::decode_payload(&row.unsigned_transaction_payload_norito)
            .expect("model payload")
            .payload()
            .clone();
        let request = norito::decode_canonical(&row.submit_proof_instruction_norito)
            .expect("current request");
        let proof = payload.attachments.as_ref().unwrap().occurrences()[0]
            .bytes
            .clone();
        (payload, request, proof)
    }

    fn plan() -> DirectPlan {
        let (mut payload, request, _) = fixture();
        payload.attachments = None;
        DirectPlan::prepare(payload, request, &PrivacyConsensusLimitsV1::taira_default()).unwrap()
    }

    fn key() -> PrivateKey {
        // The data-model fixture owns this public deterministic test seed.
        KeyPair::try_from_seed(vec![0xE7; 32], Algorithm::Ed25519)
            .unwrap()
            .private_key()
            .clone()
    }

    #[test]
    fn direct_plan_preserves_the_model_fixture_and_one_original_occurrence() {
        let (expected, request, proof) = fixture();
        let plan = plan();
        assert_eq!(plan.request, request);
        let actual = plan
            .complete(&proof, &PrivacyConsensusLimitsV1::taira_default())
            .unwrap();
        assert_eq!(actual, expected);
        let attachments = actual.attachments.as_ref().unwrap();
        assert_eq!(attachments.actions().len(), 1);
        assert_eq!(attachments.occurrences().len(), 1);
        assert_eq!(attachments.occurrences()[0].bytes, proof);
        assert_eq!(
            plan.materialized
                .context
                .transaction_intent_digest
                .as_bytes(),
            plan.action_digest.as_bytes()
        );
        // This is a structural model fixture, not an accepted engine proof.
        assert!(
            validate_zk_x509_credential_proof_container_v1(
                &plan.materialized,
                *plan.materialized.context.network_id.as_bytes(),
                &proof
            )
            .is_err()
        );
    }

    #[test]
    fn preparation_rejects_substituted_network_authority_ordinals_and_source() {
        for case in 0..8 {
            let (mut payload, mut request, _) = fixture();
            payload.attachments = None;
            match case {
                0 => {
                    request.statement.context.network_id =
                        super::super::network_id_from_raw_bytes(&[0xA7; 32]).unwrap()
                }
                1 => {
                    request.statement.wallet_account =
                        AccountId::new(KeyPair::random().public_key().clone())
                }
                2 => request.proof_ref.action_index = 1,
                3 => request.proof_ref.slot_index = 1,
                4 => request.statement.context.action_index = 1,
                5 => {
                    request.statement.context.transaction_intent_digest =
                        iroha_data_model::privacy::PrivacyTransactionIntentDigestV1::new([1; 32])
                }
                6 => {
                    payload.instructions = Executable::Instructions(
                        vec![request.clone().into(), request.clone().into()].into(),
                    )
                }
                7 => payload.attachments = Some(declared(&request, Vec::new()).unwrap()),
                _ => unreachable!(),
            }
            assert!(
                DirectPlan::prepare(payload, request, &PrivacyConsensusLimitsV1::taira_default())
                    .is_err(),
                "case {case}"
            );
        }
    }

    #[test]
    fn completion_rejects_changed_metadata_nonce_deadline_authority_network_or_occurrence() {
        let (_, _, proof) = fixture();
        for case in 0..8 {
            let mut plan = plan();
            match case {
                0 => {
                    plan.payload
                        .metadata
                        .insert("changed".parse().unwrap(), norito::json!(1));
                }
                1 => plan.payload.nonce = NonZeroU32::new(99),
                2 => plan.payload.time_to_live_ms = NonZeroU64::new(90_000),
                3 => plan.payload.creation_time_ms += 1,
                4 => {
                    plan.payload.authority = AccountId::new(KeyPair::random().public_key().clone())
                }
                5 => {
                    plan.payload.domain = TransactionDomain::Network(
                        super::super::network_id_from_raw_bytes(&[0xA7; 32]).unwrap(),
                    )
                }
                6 => plan.payload.attachments = None,
                7 => plan
                    .payload
                    .attachments
                    .as_mut()
                    .unwrap()
                    .set_occurrence_bytes(0, vec![1])
                    .unwrap(),
                _ => unreachable!(),
            }
            assert!(
                plan.complete(&proof, &PrivacyConsensusLimitsV1::taira_default())
                    .is_err(),
                "case {case}"
            );
        }
    }

    #[test]
    fn authority_and_proof_limits_are_independent_of_witness_capacity() {
        let plan = plan();
        assert!(require_key(&plan.payload, &key()).is_ok());
        assert!(require_key(&plan.payload, KeyPair::random().private_key()).is_err());
        let limits = PrivacyConsensusLimitsV1::taira_default();
        assert!(require_proof_bound(&[], &limits).is_err());
        let ceiling = vec![0xA5; ZK_X509_CREDENTIAL_PROOF_MAX_BYTES_V1];
        assert!(require_proof_bound(&ceiling, &limits).is_ok());
        assert!(require_proof_bound(&vec![0xA5; ceiling.len() + 1], &limits).is_err());
        let mut tightened = limits;
        tightened.max_proof_bytes_per_action = 1;
        assert!(require_proof_bound(&[1, 2], &tightened).is_err());
    }

    fn manifest(network: NetworkId) -> PrivacyExact12CapabilityManifestV1 {
        let catalog =
            iroha_core_privacy::privacy_profiles::compiled_privacy_profile_catalog_v1().unwrap();
        PrivacyCapabilitySnapshotV1 {
            version: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
            committed_height: 3,
            consensus_policy: PrivacyConsensusPolicyV1::taira_default(),
            network_id: network,
            protocols: catalog
                .protocols
                .into_iter()
                .map(|row| PrivacyCapabilityRowV1 {
                    protocol_id: row.protocol_id,
                    compiled_profile: row.compiled_profile,
                    policy: None,
                })
                .collect(),
        }
        .exact12_capability_manifest_v1()
        .unwrap()
    }
    fn structural_owner() -> Owner {
        let plan = plan();
        let network = plan.materialized.context.network_id;
        Owner {
            plan,
            manifest: manifest(network),
            network,
        }
    }
    #[test]
    fn private_statement_checks_bind_native_profile_and_selected_limits() {
        let plan = plan();
        let context = &plan.materialized.context;
        // Structural fixture for this check only. This never issues native availability or a handle.
        let profile = PrivacyCompiledProfileSnapshotV1 {
            protocol_id: PROTOCOL,
            proof_system_id: PROTOCOL.expected_proof_system(),
            engine_id: PROTOCOL.expected_engine(),
            parameter_id: context.parameter_id,
            parameter_digest: context.parameter_digest,
            verifier_digest: context.verifier_digest,
            statement_schema_digest: context.statement_schema_digest,
            engine_manifest_digest: context.engine_manifest_digest,
            protocol_limits: PrivacyProtocolLimitsV1::IrohaZkX509StarkP256V1,
        };
        let limits = PrivacyConsensusLimitsV1::taira_default();
        assert_eq!(
            governed(
                &plan.materialized,
                &profile,
                &profile.protocol_limits,
                &limits
            ),
            Ok(())
        );
        let mut wrong = profile;
        wrong.parameter_digest = PrivacyParameterDigestV1::new([0x19; 32]);
        assert_eq!(
            governed(
                &plan.materialized,
                &wrong,
                &profile.protocol_limits,
                &limits
            ),
            Err(POLICY)
        );
        assert_eq!(
            governed(
                &plan.materialized,
                &profile,
                &PrivacyProtocolLimitsV1::ZkAcePqAuthorizationV1,
                &limits
            ),
            Err(POLICY)
        );
    }

    #[test]
    fn native_prepare_and_sign_preserve_the_exact_joint_security_refusal() {
        let owner = structural_owner();
        assert_eq!(
            policy(&owner.manifest, owner.network).unwrap_err(),
            JOINT_SECURITY
        );
        let other = super::super::network_id_from_raw_bytes(&[0xA7; 32]).unwrap();
        assert_eq!(policy(&owner.manifest, other).unwrap_err(), NETWORK);
        let (_, request, proof) = fixture();
        let mut empty = owner.plan.payload.clone();
        empty.attachments = None;
        empty.instructions = Executable::Instructions(Vec::new().into());
        let archive = owner.manifest.canonical_bytes().unwrap();
        assert_eq!(
            Owner::prepare(
                owner.network.as_bytes(),
                &archive,
                &norito::encode_canonical(&empty).unwrap(),
                &norito::encode_canonical(&request).unwrap()
            )
            .err(),
            Some(JOINT_SECURITY)
        );
        let (_, seed) = key().to_bytes();
        assert_eq!(owner.sign(&seed, &proof).unwrap_err(), JOINT_SECURITY);
        assert_eq!(owner.sign(&[0x71; 32], &proof).unwrap_err(), AUTHORITY);
        // A canonical remote manifest is data; this module has no authenticated-origin constructor.
        assert!(validate_privacy_capability_archive_v1(&archive).is_valid());
    }
    #[test]
    fn handle_close_is_monotonic_and_retains_already_accepted_calls() {
        let mut registry = Registry::default();
        let first = registry.insert(structural_owner()).unwrap();
        let accepted = registry.get(first).unwrap();
        registry.close(first).unwrap();
        assert_eq!(registry.close(first), Err(CLOSED));
        assert!(matches!(registry.get(first), Err(CLOSED)));
        assert!(matches!(registry.get(0), Err(CLOSED)));
        let second = registry.insert(structural_owner()).unwrap();
        assert_ne!(first, second);
        let expected = accepted.projection(0).unwrap();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let retained = accepted.clone();
                    scope.spawn(move || retained.projection(0).unwrap())
                })
                .collect();
            for handle in handles {
                assert_eq!(handle.join().unwrap(), expected);
            }
        });
        assert_eq!(accepted.projection(99).unwrap_err(), INVALID);
    }
    #[test]
    fn projection_copies_cannot_substitute_the_original_tagged_statement() {
        let owner = structural_owner();
        let mut copied = owner.projection(3).unwrap();
        let decoded: PrivacyStatementV1 = norito::decode_canonical(&copied).unwrap();
        assert_eq!(
            decoded,
            PrivacyStatementV1::IrohaZkX509StarkP256V1(owner.plan.materialized.clone())
        );
        copied.fill(0);
        assert_ne!(copied, owner.projection(3).unwrap());
        let action: ActionStatementV1 =
            norito::decode_canonical(&owner.projection(2).unwrap()).unwrap();
        assert_eq!(action, owner.plan.action);
        assert_eq!(
            owner.projection(1).unwrap(),
            owner.plan.action_digest.as_bytes()
        );
    }
    #[test]
    fn ffi_invalid_handles_and_inputs_clear_outputs_without_dereferencing_handles() {
        use std::ptr;
        let mut output = 1usize as *mut u8;
        let mut length = 99;
        assert_eq!(
            unsafe {
                connect_norito_zk_x509_transaction_projection_v1(0, 0, &mut output, &mut length)
            },
            CLOSED
        );
        assert!(output.is_null());
        assert_eq!(length, 0);
        assert_eq!(
            unsafe {
                connect_norito_zk_x509_transaction_sign_v1(
                    0,
                    ptr::null(),
                    32,
                    ptr::null(),
                    1,
                    &mut output,
                    &mut length,
                )
            },
            CLOSED
        );
        assert!(output.is_null());
        assert_eq!(length, 0);
        let mut handle = 99;
        assert_eq!(
            unsafe {
                connect_norito_zk_x509_transaction_prepare_v1(
                    ptr::null(),
                    32,
                    ptr::null(),
                    1,
                    ptr::null(),
                    1,
                    ptr::null(),
                    1,
                    &mut handle,
                )
            },
            INVALID
        );
        assert_eq!(handle, 0);
        assert_eq!(connect_norito_zk_x509_transaction_close_v1(0), CLOSED);
        assert_eq!(unsafe { bytes(ptr::null(), 0, 1) }.unwrap_err(), INVALID);
        assert_eq!(unsafe { bytes(ptr::null(), 2, 1) }.unwrap_err(), INVALID);
    }
    #[test]
    fn registry_capacity_and_exhaustion_never_reuse_an_owner() {
        let mut registry = Registry::default();
        for _ in 0..MAX_OWNERS {
            registry.insert(structural_owner()).unwrap();
        }
        assert_eq!(registry.insert(structural_owner()), Err(RESOURCE));
        registry.owners.clear();
        registry.next = i64::MAX as u64;
        assert_eq!(registry.insert(structural_owner()), Err(RESOURCE));
    }
}
