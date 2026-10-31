//! Frozen Python X509 preparation and the sole direct signed-attachment producer.
//!
//! Public projections are copies, never authority to replace the retained payload.
//! Wire planning does not prove cryptography or authorize execution. Both prepare
//! and sign require the authenticated selected native capability.
use crate::privacy_capability_manifest::PyPrivacyExact12CapabilityManifestV1;
use crate::privacy_native_actions::{
    PRIVACY_NATIVE_ACTION_MAX_SIGNED_TRANSACTION_BYTES_V1, PRIVACY_ZK_X509_MAX_PROOF_BYTES_V1,
};
use crate::{
    PrivacyStatementV1, PyNetworkId, SignedTransactionEnvelope, TransactionBuilder,
    ensure_ed25519_account, parse_private_key, signed_transaction_envelope_from_model_v1,
};
use iroha_core_privacy::privacy_engines::validate_zk_x509_credential_proof_container_v1;
use iroha_crypto::{PrivateKey, PublicKey};
use iroha_data_model::{
    NetworkId,
    isi::{InstructionBox, privacy::SubmitPrivacyZkX509CertificateV1},
    privacy::{IrohaZkX509StarkP256StatementV1, PrivacyConsensusLimitsV1, PrivacyProtocolIdV1},
    transaction::{
        Executable, SignedTransaction, TransactionBuilder as ModelBuilder, TransactionPayload,
        signed_attachments::*,
    },
};
use iroha_version::codec::EncodeVersioned;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyBytes};

fn invalid(error: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(format!("invalid direct X509 transaction: {error}"))
}

fn decode_request(bytes: &[u8]) -> PyResult<SubmitPrivacyZkX509CertificateV1> {
    if bytes.is_empty()
        || bytes.len() > SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1()
    {
        return Err(invalid("request archive exceeds its canonical bound"));
    }
    norito::decode_canonical(bytes).map_err(invalid)
}

fn require_key(payload: &TransactionPayload, key: &PrivateKey) -> PyResult<()> {
    ensure_ed25519_account(&payload.authority)?;
    if payload.authority.try_signatory() != Some(&PublicKey::from(key.clone())) {
        return Err(invalid("signing key differs from retained authority"));
    }
    Ok(())
}

fn require_proof_bound(proof: &[u8], limits: &PrivacyConsensusLimitsV1) -> PyResult<()> {
    if proof.is_empty()
        || proof.len() > PRIVACY_ZK_X509_MAX_PROOF_BYTES_V1
        || proof.len() > limits.max_proof_bytes_per_action as usize
    {
        return Err(invalid("proof exceeds the native or committed bound"));
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
) -> PyResult<SignedAttachmentsV1> {
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
    ) -> PyResult<Self> {
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
    ) -> PyResult<TransactionPayload> {
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

/// An immutable direct transaction prepared from one authenticated native capability.
/// There is no public constructor or caller-supplied payload at signing.
#[pyclass(
    name = "PrivacyX509PreparedTransactionV1",
    frozen,
    module = "iroha_native._crypto"
)]
pub(crate) struct PyPrivacyX509PreparedTransactionV1 {
    plan: DirectPlan,
    admission: PyPrivacyExact12CapabilityManifestV1,
    network_id: NetworkId,
    chain_discriminant: u16,
}
impl PyPrivacyX509PreparedTransactionV1 {
    pub(crate) fn prepare(builder: &TransactionBuilder, archive: &[u8]) -> PyResult<Self> {
        builder.require_empty_privacy_action_builder_v1(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
            "ZK-X509",
        )?;
        let admission = builder
            .privacy_capability_manifest
            .as_ref()
            .ok_or_else(|| invalid("authenticated native capability is absent"))?
            .clone();
        let (_, _, limits) = admission.x509_policy(builder.network_id)?;
        let request = decode_request(archive)?;
        let payload = builder
            .to_model_builder()
            .with_instructions([InstructionBox::from(request.clone())])
            .into_payload()
            .map_err(invalid)?;
        let plan = DirectPlan::prepare(payload, request, &limits)?;
        admission.require_governed_statement(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
            plan.materialized.clone(),
        ))?;
        Ok(Self {
            plan,
            admission,
            network_id: builder.network_id,
            chain_discriminant: builder.chain_discriminant,
        })
    }
}
#[pymethods]
impl PyPrivacyX509PreparedTransactionV1 {
    #[getter]
    fn ordered_intent_digest<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.plan.intent.as_bytes())
    }
    #[getter]
    fn action_statement_digest<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.plan.action_digest.as_bytes())
    }
    #[getter]
    fn action_statement_norito<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(
            py,
            &norito::encode_canonical(&self.plan.action).map_err(invalid)?,
        ))
    }
    #[getter]
    fn materialized_statement_norito<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(
            py,
            &norito::encode_canonical(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
                self.plan.materialized.clone(),
            ))
            .map_err(invalid)?,
        ))
    }
    /// Sign the original payload with exactly one completed raw proof occurrence.
    /// The holder prover consumes materialized_statement_norito; independent inputs cannot change.
    fn sign(
        &self,
        private_key: &[u8],
        credential_proof: &[u8],
    ) -> PyResult<SignedTransactionEnvelope> {
        let key = parse_private_key(private_key)?;
        require_key(&self.plan.payload, &key)?;
        let (profile, protocol_limits, limits) = self.admission.x509_policy(self.network_id)?;
        self.admission
            .require_governed_statement(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
                self.plan.materialized.clone(),
            ))?;
        self.plan
            .request
            .validate_materialized_against_profile_v1(
                &self.plan.materialized,
                credential_proof,
                &profile,
                &protocol_limits,
                &limits,
            )
            .map_err(invalid)?;
        require_proof_bound(credential_proof, &limits)?;
        validate_zk_x509_credential_proof_container_v1(
            &self.plan.materialized,
            *self.network_id.as_bytes(),
            credential_proof,
        )
        .map_err(invalid)?;
        let payload = self.plan.complete(credential_proof, &limits)?;
        let signed = ModelBuilder::from_payload(payload)
            .map_err(invalid)?
            .try_sign(&key)
            .map_err(invalid)?;
        if signed.encode_versioned().len() > PRIVACY_NATIVE_ACTION_MAX_SIGNED_TRANSACTION_BYTES_V1 {
            return Err(invalid("signed transaction exceeds the 10 MiB limit"));
        }
        signed.verify_signature().map_err(invalid)?;
        signed_transaction_envelope_from_model_v1(&signed, self.chain_discriminant)
    }
}

/// Authenticate only the sole direct current request/occurrence shape; no inline fallback.
pub(crate) fn inspect(
    signed: &SignedTransaction,
    expected: &PyNetworkId,
) -> PyResult<(
    IrohaZkX509StarkP256StatementV1,
    OrderedIntentDigestV1,
    ActionStatementDigestV1,
    usize,
)> {
    signed.verify_signature().map_err(invalid)?;
    let payload = signed.payload();
    if payload.network_id() != Some(&expected.inner) {
        return Err(invalid("signed transaction belongs to another network"));
    }
    payload.validate_signed_attachments_v1().map_err(invalid)?;
    let Executable::Instructions(instructions) = &payload.instructions else {
        return Err(invalid("one direct X509 instruction is required"));
    };
    let [instruction] = instructions.as_ref() else {
        return Err(invalid("one direct X509 instruction is required"));
    };
    let request = instruction
        .as_any()
        .downcast_ref::<SubmitPrivacyZkX509CertificateV1>()
        .ok_or_else(|| invalid("current proof-free X509 request is required"))?;
    let attachments = payload
        .attachments
        .as_ref()
        .ok_or_else(|| invalid("original occurrence is absent"))?;
    if attachments.actions().len() != 1
        || attachments.occurrences().len() != 1
        || request.proof_ref.action_index != 0
        || request.statement.context.action_index != 0
        || request.statement.wallet_account != payload.authority
        || request.statement.context.network_id != expected.inner
    {
        return Err(invalid(
            "X509 original network, authority, ordinal or occurrence count differs",
        ));
    }
    let action = &attachments.actions()[0];
    if action.authority != ActionAuthorityBindingV1::TransactionAuthority
        || action.invocation
            != (ActionInvocationBindingV1::Direct {
                instruction_index: 0,
            })
    {
        return Err(invalid("X509 original authority or invocation differs"));
    }
    let intent = payload.ordered_intent_digest_v1().map_err(invalid)?;
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let statement = request
        .materialize_statement_v1(action, intent, &limits)
        .map_err(invalid)?;
    let proof = &attachments.occurrences()[0].bytes;
    require_proof_bound(proof, &limits)?;
    validate_zk_x509_credential_proof_container_v1(&statement, *expected.inner.as_bytes(), proof)
        .map_err(invalid)?;
    Ok((
        statement,
        intent,
        action.statement_digest_for_v1(intent).map_err(invalid)?,
        proof.len(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{account::AccountId, transaction::TransactionDomain};
    use std::num::{NonZeroU32, NonZeroU64};

    fn fixture() -> (
        TransactionPayload,
        SubmitPrivacyZkX509CertificateV1,
        Vec<u8>,
    ) {
        use base64::Engine as _;
        let encoded =
            include_str!("../../../../fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64")
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
        let request =
            decode_request(&row.submit_proof_instruction_norito).expect("current request");
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
    fn request_archive_boundary_rejects_empty_trailing_retired_and_oversized_frames() {
        let (_, request, _) = fixture();
        let canonical = norito::encode_canonical(&request).unwrap();
        assert_eq!(decode_request(&canonical).unwrap(), request);
        let mut trailing = canonical;
        trailing.push(0);
        for invalid_bytes in [
            Vec::new(),
            trailing,
            b"not-norito".to_vec(),
            norito::encode_canonical(&request.statement).unwrap(),
            vec![0; SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1() + 1],
        ] {
            assert!(decode_request(&invalid_bytes).is_err());
        }
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
                        crate::PyNetworkId::from_exact_bytes(&[0xA7; 32])
                            .unwrap()
                            .inner
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
                        crate::PyNetworkId::from_exact_bytes(&[0xA7; 32])
                            .unwrap()
                            .inner,
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
        let ceiling = vec![0xA5; PRIVACY_ZK_X509_MAX_PROOF_BYTES_V1];
        assert!(
            ceiling.len()
                > crate::privacy_native_actions::PRIVACY_NATIVE_ACTION_MAX_SECRET_BUNDLE_BYTES_V1
        );
        assert!(require_proof_bound(&ceiling, &limits).is_ok());
        assert!(require_proof_bound(&vec![0xA5; ceiling.len() + 1], &limits).is_err());
        let mut tightened = limits;
        tightened.max_proof_bytes_per_action = 1;
        assert!(require_proof_bound(&[1, 2], &tightened).is_err());
    }

    #[test]
    fn public_owner_and_inspector_do_not_turn_model_fixtures_into_engine_admission() {
        let plan = plan();
        let network_id = plan.materialized.context.network_id;
        let admission = PyPrivacyExact12CapabilityManifestV1::test_fetched_binding_for_protocol(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            network_id,
        );
        let owner = PyPrivacyX509PreparedTransactionV1 {
            plan,
            admission,
            network_id,
            chain_discriminant: 753,
        };
        let (_, _, proof) = fixture();
        Python::initialize();
        let (_, private_bytes) = key().to_bytes();
        let error = owner
            .sign(&private_bytes, &proof)
            .err()
            .expect("X509 selected native profile remains unavailable");
        assert!(
            error
                .to_string()
                .contains("joint-security-reduction-incomplete"),
            "{error}"
        );
        let (payload, _, _) = fixture();
        let signed = ModelBuilder::from_payload(payload)
            .unwrap()
            .try_sign(&key())
            .unwrap();
        assert!(inspect(&signed, &PyNetworkId { inner: network_id }).is_err());
        let foreign = crate::PyNetworkId::from_exact_bytes(&[0xA7; 32]).unwrap();
        assert!(inspect(&signed, &foreign).is_err());
        let mut empty = signed.payload().clone();
        empty.instructions = Executable::Instructions(Vec::<InstructionBox>::new().into());
        empty.attachments = None;
        let empty_signed = ModelBuilder::from_payload(empty)
            .unwrap()
            .try_sign(&key())
            .unwrap();
        assert!(inspect(&empty_signed, &PyNetworkId { inner: network_id }).is_err());
    }

    #[test]
    fn public_preparation_rejects_missing_cross_network_or_unavailable_native_admission() {
        Python::initialize();
        let (payload, request, _) = fixture();
        let network_id = request.statement.context.network_id;
        let mut builder = TransactionBuilder {
            network_id,
            chain_discriminant: 753,
            authority: payload.authority,
            fee_payment: payload.fee_payment,
            creation_time: Some(std::time::Duration::from_millis(payload.creation_time_ms)),
            ttl: Some(std::time::Duration::from_millis(60_000)),
            nonce: payload.nonce,
            executable_items: Vec::new(),
            explicit_batch: false,
            metadata: payload.metadata,
            executable_override: None,
            attachments: None,
            privacy_capability_manifest: None,
        };
        let archive = norito::encode_canonical(&request).unwrap();
        assert!(
            builder
                .prepare_privacy_zk_x509_identity_presentation_action_v1(&archive)
                .is_err()
        );
        let foreign = crate::PyNetworkId::from_exact_bytes(&[0xA7; 32])
            .unwrap()
            .inner;
        builder.privacy_capability_manifest = Some(
            PyPrivacyExact12CapabilityManifestV1::test_fetched_binding_for_protocol(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                foreign,
            ),
        );
        let error = builder
            .prepare_privacy_zk_x509_identity_presentation_action_v1(&archive)
            .err()
            .unwrap();
        assert!(error.to_string().contains("network"), "{error}");
        builder.privacy_capability_manifest = Some(
            PyPrivacyExact12CapabilityManifestV1::test_fetched_binding_for_protocol(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                network_id,
            ),
        );
        let error = builder
            .prepare_privacy_zk_x509_identity_presentation_action_v1(&archive)
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains("joint-security-reduction-incomplete"),
            "{error}"
        );
        builder.explicit_batch = true;
        let error = builder
            .prepare_privacy_zk_x509_identity_presentation_action_v1(&archive)
            .err()
            .unwrap();
        assert!(error.to_string().contains("otherwise empty"), "{error}");
    }

    #[test]
    fn native_owner_is_not_constructible_or_mutable_from_python_and_returns_copies() {
        Python::initialize();
        Python::attach(|py| {
            let retained_plan = plan();
            let network_id = retained_plan.materialized.context.network_id;
            let admission = PyPrivacyExact12CapabilityManifestV1::test_fetched_binding_for_protocol(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                network_id,
            );
            let owner = Py::new(
                py,
                PyPrivacyX509PreparedTransactionV1 {
                    plan: retained_plan,
                    admission,
                    network_id,
                    chain_discriminant: 753,
                },
            )
            .unwrap();
            let binding = owner.bind(py);
            assert!(binding.get_type().call0().is_err());
            assert!(binding.setattr("network_id", vec![0xA7; 32]).is_err());
            assert!(binding.setattr("plan", "replacement").is_err());
            let first = binding
                .getattr("materialized_statement_norito")
                .unwrap()
                .extract::<Vec<u8>>()
                .unwrap();
            let tagged: PrivacyStatementV1 =
                norito::decode_canonical(&first).expect("canonical tagged public statement");
            assert_eq!(
                tagged,
                PrivacyStatementV1::IrohaZkX509StarkP256V1(plan().materialized)
            );
            let model_row = iroha_data_model::privacy::privacy_exact12_fixture_bundle_v1()
                .unwrap()
                .rows
                .into_iter()
                .find(|row| row.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
                .unwrap();
            assert_eq!(first, model_row.statement_norito);
            let mut changed = first.clone();
            changed[0] ^= 1;
            assert_eq!(
                binding
                    .getattr("materialized_statement_norito")
                    .unwrap()
                    .extract::<Vec<u8>>()
                    .unwrap(),
                first
            );
            assert_ne!(changed, first);
            assert_eq!(
                binding
                    .getattr("ordered_intent_digest")
                    .unwrap()
                    .extract::<Vec<u8>>()
                    .unwrap()
                    .len(),
                32
            );
            assert_eq!(
                binding
                    .getattr("action_statement_digest")
                    .unwrap()
                    .extract::<Vec<u8>>()
                    .unwrap()
                    .len(),
                32
            );
            assert!(
                !binding
                    .getattr("action_statement_norito")
                    .unwrap()
                    .extract::<Vec<u8>>()
                    .unwrap()
                    .is_empty()
            );
            assert!(
                binding
                    .call_method1("sign", (vec![0xE7; 32], vec![1], "replacement"))
                    .is_err()
            );
        });
    }
}
