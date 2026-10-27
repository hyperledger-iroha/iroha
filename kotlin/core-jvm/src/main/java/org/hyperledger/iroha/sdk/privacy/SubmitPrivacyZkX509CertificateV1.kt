// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.core.model.*
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

/**
 * Canonical proof-free X509 instruction request owned by the Rust model.
 *
 * The request's context keeps its privacy ordinal and a zero transaction-intent digest.
 * [proofActionIndex] is the separate signed-attachment action ordinal. Proof bytes are supplied
 * only as a [SignedAttachmentOccurrenceV1], never embedded into this request. Canonical inspection
 * and statement materialization establish binding, not proof validity or runtime authorization.
 */
class SubmitPrivacyZkX509CertificateV1 private constructor(
    private val inspection: PrivacyZkX509RequestInspectionV1,
) {
    internal val contextNetworkId: NetworkId get() = inspection.networkId
    internal fun requireCompiledContextV1(networkId: NetworkId, profile: PrivacyCompiledProfileV1) =
        inspection.requireCompiledContextV1(networkId, profile)

    /** Zero-based action ordinal in the original signed attachment descriptor. */
    val proofActionIndex: Long get() = inspection.proofActionIndex
    /** The sole original action-proof slot. */
    val proofSlotIndex: Long get() = 0L
    /** Exact complete proof-free request archive, including its canonical Norito frame. */
    fun canonicalRequestBytes(): ByteArray = inspection.canonicalRequestBytes()
    /** Digest of the complete independent statement in the model-owned X509 domain. */
    fun publicInputsDigestV1(): ActionPublicInputsDigestV1 = inspection.publicInputsDigest

    /** Plan the original action before generating its proof; derived statement fields remain zero. */
    fun actionIntentV1(
        authority: ActionAuthorityBindingV1,
        invocation: ActionInvocationBindingV1,
        proofOccurrenceIndex: Long,
    ): ActionIntentV1 {
        require(invocation != ActionInvocationBindingV1.Transaction) {
            "X509 requires an original direct or contract invocation"
        }
        return ActionIntentV1(
            proofActionIndex, ActionOperationV1.NativeInstruction(WIRE_ID), ActionRoleV1.ProofEffect,
            authority, invocation, publicInputsDigestV1(), ActionStatementDigestV1(ByteArray(32)),
            listOf(AttachmentReferenceV1(proofOccurrenceIndex, AttachmentKindV1.Proof,
                AttachmentRoleV1.ActionProof, 0L, AttachmentContentHashV1(ByteArray(32)))),
        ).also(::requireActionV1)
    }

    /** Replace only the proof metadata after generating the original action's signed proof. */
    fun withSignedProofV1(action: ActionIntentV1, signedProof: SignedAttachmentOccurrenceV1): ActionIntentV1 {
        requireActionV1(action)
        signedProof.validate()
        require(signedProof.kind == AttachmentKindV1.Proof &&
            signedProof.index == action.attachments.single().index) { "X509 signed proof occurrence mismatch" }
        return ActionIntentV1(action.actionIndex, action.operation, action.role, action.authority,
            action.invocation, action.publicInputsDigest, action.statementDigest,
            listOf(signedProof.reference(AttachmentRoleV1.ActionProof)))
    }

    /**
     * Derive the tagged native statement from the original payload, never a caller-supplied digest.
     * The account codec is explicitly bound to [chainDiscriminant]. Changing independent payload
     * fields requires a new statement and proof. This operation does not grant admission.
     */
    fun materializeStatementV1(originalPayload: TransactionPayload, chainDiscriminant: Int): ByteArray =
        NoritoJavaCodecAdapter(chainDiscriminant).materializeZkX509StatementV1(this, originalPayload)

    /** Issue the sole executing X509 wire route using fresh committed capability admission. */
    fun toInstructionBoxV1(admission: PrivacyExact12CapabilityTupleAdmissionV1): InstructionBox =
        InstructionBox.fromPrivacyZkX509CertificateV1(admission, this)

    internal fun requireActionV1(action: ActionIntentV1) {
        require(action.actionIndex == proofActionIndex &&
            action.operation == ActionOperationV1.NativeInstruction(WIRE_ID) &&
            action.role == ActionRoleV1.ProofEffect) { "X509 request action identity mismatch" }
        require(action.attachments.size == 1 && action.attachments[0].kind == AttachmentKindV1.Proof &&
            action.attachments[0].role == AttachmentRoleV1.ActionProof) { "X509 requires one Proof/ActionProof slot" }
        require(when (action.invocation) {
            is ActionInvocationBindingV1.Direct -> action.authority == ActionAuthorityBindingV1.TransactionAuthority || action.authority is ActionAuthorityBindingV1.Account
            is ActionInvocationBindingV1.Contract -> action.authority is ActionAuthorityBindingV1.ContractSubject || action.authority is ActionAuthorityBindingV1.Account
            ActionInvocationBindingV1.Transaction -> false
        }) { "X509 action authority does not match its invocation" }
        require(action.publicInputsDigest == publicInputsDigestV1()) { "X509 public-input binding mismatch" }
    }

    override fun equals(other: Any?): Boolean = other is SubmitPrivacyZkX509CertificateV1 &&
        canonicalRequestBytes().contentEquals(other.canonicalRequestBytes())
    override fun hashCode(): Int = canonicalRequestBytes().contentHashCode()
    override fun toString(): String = "SubmitPrivacyZkX509CertificateV1(proof-free request)"

    companion object {
        const val WIRE_ID: String = "iroha.privacy.submit_zk_x509_certificate.v1"

        /**
         * Explicit proof occurrence declaration for the original pre-proof payload.
         * Insert it into SignedAttachmentsV1 before materialization; final signed payloads
         * require the actual proof bytes and cannot encode this placeholder.
         */
        @JvmStatic
        fun plannedProofOccurrenceV1(index: Long): SignedAttachmentOccurrenceV1 =
            SignedAttachmentOccurrenceV1(index, AttachmentKindV1.Proof, 0L,
                AttachmentContentHashV1(ByteArray(32)), ByteArray(0))

        /** Construct a request from a canonical tagged X509 statement whose intent field is zero. */
        @JvmStatic
        fun fromStatementV1(independentStatementNorito: ByteArray, attachmentActionIndex: Long): SubmitPrivacyZkX509CertificateV1 =
            fromCanonicalRequestV1(PrivacyNativeBridge.createZkX509RequestV1(independentStatementNorito, attachmentActionIndex))

        /** Reject noncanonical requests, supplied derived intent, and nonzero proof slots natively. */
        @JvmStatic
        fun fromCanonicalRequestV1(requestArchive: ByteArray): SubmitPrivacyZkX509CertificateV1 =
            SubmitPrivacyZkX509CertificateV1(PrivacyNativeBridge.inspectZkX509RequestV1(requestArchive))
    }
}

/** Native inspection of exact request bytes; never a runtime authorization token. */
internal class PrivacyZkX509RequestInspectionV1 private constructor(
    canonicalRequest: ByteArray,
    val proofActionIndex: Long,
    val publicInputsDigest: ActionPublicInputsDigestV1,
    val networkId: NetworkId,
    private val tuple: List<PrivacyFixed32V1>,
) {
    private val request = canonicalRequest.copyOf()
    fun canonicalRequestBytes(): ByteArray = request.copyOf()

    fun requireCompiledContextV1(expectedNetwork: NetworkId, profile: PrivacyCompiledProfileV1) {
        require(networkId == expectedNetwork) { "X509 request context belongs to a different network" }
        require(profile.protocolId == PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1 &&
            tuple == listOf(profile.parameterId, profile.parameterDigest, profile.verifierDigest,
                profile.statementSchemaDigest, profile.engineManifestDigest)) {
            "X509 request context differs from the admitted five-field compiled tuple"
        }
    }

    companion object {
        fun fromNative(original: ByteArray, result: Array<ByteArray?>?): PrivacyZkX509RequestInspectionV1 {
            require(original.isNotEmpty() && original.size <= PRIVACY_X509_REQUEST_BRIDGE_MAX_BYTES_V1) {
                "invalid X509 request length"
            }
            require(result != null && result.size == 9) { "invalid native X509 inspection shape" }
            val canonical = requireNotNull(result[0]) { "missing native X509 request" }
            val ordinal = requireNotNull(result[1]) { "missing native X509 action ordinal" }
            val digest = requireNotNull(result[2]) { "missing native X509 public-input digest" }
            require(canonical.contentEquals(original)) { "native X509 inspection changed the canonical request" }
            require(ordinal.size == 4 && digest.size == 32 && digest.any { it != 0.toByte() }) {
                "invalid native X509 binding fields"
            }
            return PrivacyZkX509RequestInspectionV1(canonical,
                ByteBuffer.wrap(ordinal).order(ByteOrder.LITTLE_ENDIAN).int.toLong() and 0xffff_ffffL,
                ActionPublicInputsDigestV1(digest),
                NetworkId.fromBytes(requireNotNull(result[3]) { "missing native X509 context network" }),
                (4..8).map { PrivacyFixed32V1(requireNotNull(result[it]) { "missing native X509 context binding" }) })
        }
    }
}
