// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Public action spelling for the closed 13-operation Exact12 schema. */
typealias PrivacyExact12ActionOperationV1 = PrivacyOperationSchemaV1

/** Closed ledger-effect class committed by a public Exact12 operation. */
enum class PrivacyLedgerEffectKindV1(val canonicalLabel: String) {
    VERIFICATION_ONLY("verification_only"),
    ZK_ACE_TRANSPARENT_TRANSFER("zk_ace_transparent_transfer"),
    ANONYMOUS_PGC_ACCOUNT_STATE_TRANSITION("anonymous_pgc_account_state_transition"),
    ZK_AMS_BATCH_ADMISSION("zk_ams_batch_admission"),
    ZK_AMS_PROVISION_ACCOUNT("zk_ams_provision_account"),
    ZK_X509_CERTIFICATE_NULLIFIER("zk_x509_certificate_nullifier"),
    ORCHARD_NOTE_STATE_TRANSITION("orchard_note_state_transition"),
    FCMP_MEMBERSHIP_PAYMENT("fcmp_membership_payment"),
    IVM_PRIVATE_NOTE_STATE_TRANSITION("ivm_private_note_state_transition"),
    PQ_MASP_NOTE_STATE_TRANSITION("pq_masp_note_state_transition"),
    ;

    companion object {
        @JvmStatic
        fun fromCanonicalLabel(label: String): PrivacyLedgerEffectKindV1 =
            values().firstOrNull { it.canonicalLabel == label }
                ?: throw IllegalArgumentException("unknown Exact12 ledger-effect kind")
    }
}

/** Sole retained protocol that executes this public operation. */
val PrivacyOperationSchemaV1.protocolId: PrivacyProtocolIdV1
    get() = when (this) {
        PrivacyOperationSchemaV1.ZK_ACE_AUTHORIZATION_ACTION_V1 ->
            PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V0
        PrivacyOperationSchemaV1.ANONYMOUS_PGC_PAYMENT_ACTION_V1 ->
            PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        PrivacyOperationSchemaV1.VERANGE_RANGE_PROOF_V1 ->
            PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1
        PrivacyOperationSchemaV1.ZK_AMS_BATCH_ADMISSION_ACTION_V1,
        PrivacyOperationSchemaV1.ZK_AMS_PROVISION_ACCOUNT_ACTION_V1,
        -> PrivacyProtocolIdV1.IROHA_ZK_AMS_V1
        PrivacyOperationSchemaV1.VEGA_CREDENTIAL_PRESENTATION_V1 ->
            PrivacyProtocolIdV1.VEGA_EXISTING_CREDENTIAL_ZK_V0
        PrivacyOperationSchemaV1.ZK_X509_IDENTITY_PRESENTATION_V1 ->
            PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V0
        PrivacyOperationSchemaV1.JINDO_POLYNOMIAL_EVALUATION_V1 ->
            PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0
        PrivacyOperationSchemaV1.BOOTLE_LANTERN_CREDENTIAL_PRESENTATION_V1 ->
            PrivacyProtocolIdV1.IROHA_BOOTLE_LANTERN_ANONCRED_V1
        PrivacyOperationSchemaV1.ORCHARD_NOTE_ACTION_V1 ->
            PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1
        PrivacyOperationSchemaV1.FCMP_MEMBERSHIP_PAYMENT_V1 ->
            PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1
        PrivacyOperationSchemaV1.IVM_PRIVATE_NOTE_ACTION_V1 ->
            PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1
        PrivacyOperationSchemaV1.PQ_MASP_NOTE_ACTION_V1 ->
            PrivacyProtocolIdV1.PQ_MASP_STARK_V0
    }

/** Typed ledger effect committed when this public operation succeeds. */
val PrivacyOperationSchemaV1.ledgerEffectKind: PrivacyLedgerEffectKindV1
    get() = when (this) {
        PrivacyOperationSchemaV1.ZK_ACE_AUTHORIZATION_ACTION_V1 ->
            PrivacyLedgerEffectKindV1.ZK_ACE_TRANSPARENT_TRANSFER
        PrivacyOperationSchemaV1.ANONYMOUS_PGC_PAYMENT_ACTION_V1 ->
            PrivacyLedgerEffectKindV1.ANONYMOUS_PGC_ACCOUNT_STATE_TRANSITION
        PrivacyOperationSchemaV1.VERANGE_RANGE_PROOF_V1,
        PrivacyOperationSchemaV1.VEGA_CREDENTIAL_PRESENTATION_V1,
        PrivacyOperationSchemaV1.JINDO_POLYNOMIAL_EVALUATION_V1,
        PrivacyOperationSchemaV1.BOOTLE_LANTERN_CREDENTIAL_PRESENTATION_V1,
        -> PrivacyLedgerEffectKindV1.VERIFICATION_ONLY
        PrivacyOperationSchemaV1.ZK_AMS_BATCH_ADMISSION_ACTION_V1 ->
            PrivacyLedgerEffectKindV1.ZK_AMS_BATCH_ADMISSION
        PrivacyOperationSchemaV1.ZK_AMS_PROVISION_ACCOUNT_ACTION_V1 ->
            PrivacyLedgerEffectKindV1.ZK_AMS_PROVISION_ACCOUNT
        PrivacyOperationSchemaV1.ZK_X509_IDENTITY_PRESENTATION_V1 ->
            PrivacyLedgerEffectKindV1.ZK_X509_CERTIFICATE_NULLIFIER
        PrivacyOperationSchemaV1.ORCHARD_NOTE_ACTION_V1 ->
            PrivacyLedgerEffectKindV1.ORCHARD_NOTE_STATE_TRANSITION
        PrivacyOperationSchemaV1.FCMP_MEMBERSHIP_PAYMENT_V1 ->
            PrivacyLedgerEffectKindV1.FCMP_MEMBERSHIP_PAYMENT
        PrivacyOperationSchemaV1.IVM_PRIVATE_NOTE_ACTION_V1 ->
            PrivacyLedgerEffectKindV1.IVM_PRIVATE_NOTE_STATE_TRANSITION
        PrivacyOperationSchemaV1.PQ_MASP_NOTE_ACTION_V1 ->
            PrivacyLedgerEffectKindV1.PQ_MASP_NOTE_STATE_TRANSITION
    }

/** Java-friendly access to the closed operation mappings. */
object PrivacyExact12ActionContractV1 {
    @JvmStatic
    fun protocolId(operation: PrivacyExact12ActionOperationV1): PrivacyProtocolIdV1 =
        operation.protocolId

    @JvmStatic
    fun ledgerEffectKind(
        operation: PrivacyExact12ActionOperationV1,
    ): PrivacyLedgerEffectKindV1 = operation.ledgerEffectKind
}

/** Local lifecycle projection for one Exact12 action submission. */
enum class PrivacyActionLocalStateV1(val canonicalLabel: String) {
    SUBMITTED("submitted"),
    TERMINAL("terminal"),
}

/** Authenticated terminal pipeline state for one Exact12 action submission. */
enum class PrivacyActionTerminalChainStateV1(val canonicalLabel: String) {
    COMMITTED("Committed"),
    APPLIED("Applied"),
    REJECTED("Rejected"),
    EXPIRED("Expired"),
}

/**
 * One closed Exact12 operation and its already-signed versioned transaction wire.
 *
 * This model snapshots and bounds public wire bytes. It performs no local proof acceptance and
 * grants no capability or submission authority.
 */
class PrivacyExact12ActionRequestV1 @JvmOverloads constructor(
    @JvmField val operation: PrivacyExact12ActionOperationV1,
    signedTransactionVersioned: ByteArray,
    expectedManifestDigest: ByteArray? = null,
) {
    private val signedWire = signedTransactionVersioned.copyOf()
    private val expectedDigest = expectedManifestDigest?.copyOf()

    init {
        require(signedWire.size in 1..MAX_SIGNED_TRANSACTION_BYTES) {
            "Exact12 signed transaction must contain 1..$MAX_SIGNED_TRANSACTION_BYTES bytes"
        }
        if (expectedDigest != null) {
            requireNonzeroFixed32(expectedDigest, "expectedManifestDigest")
        }
    }

    val signedTransactionVersioned: ByteArray get() = signedWire.copyOf()
    val expectedManifestDigest: ByteArray? get() = expectedDigest?.copyOf()

    fun signedTransactionVersionedBytes(): ByteArray = signedTransactionVersioned
    fun expectedManifestDigestBytes(): ByteArray? = expectedManifestDigest

    override fun equals(other: Any?): Boolean =
        other is PrivacyExact12ActionRequestV1 &&
            operation == other.operation &&
            signedWire.contentEquals(other.signedWire) &&
            nullableBytesEqual(expectedDigest, other.expectedDigest)

    override fun hashCode(): Int {
        var result = operation.hashCode()
        result = 31 * result + signedWire.contentHashCode()
        return 31 * result + (expectedDigest?.contentHashCode() ?: 0)
    }

    companion object {
        /** Taira V1 `max_tx_bytes`, shared with native Exact12 action inspection. */
        const val MAX_SIGNED_TRANSACTION_BYTES: Int = 10 * 1024 * 1024
    }
}

/** Native-authenticated public digest projection for one exact signed action wire. */
class PrivacyExact12ActionInspectionV1 internal constructor(projection: ByteArray) {
    private val bytes = projection.copyOf()

    init {
        check(bytes.size == PROJECTION_BYTES) {
            "native Exact12 action inspection must contain exactly $PROJECTION_BYTES bytes"
        }
        for (offset in 0 until PROJECTION_BYTES step HASH_BYTES) {
            check(bytes.copyOfRange(offset, offset + HASH_BYTES).any { it != 0.toByte() }) {
                "native Exact12 action inspection contains a zero digest"
            }
        }
    }

    val transactionHash: ByteArray get() = bytes.copyOfRange(0, 32)
    val transactionIntentDigest: ByteArray get() = bytes.copyOfRange(32, 64)
    val statementDigest: ByteArray get() = bytes.copyOfRange(64, 96)
    val proofEnvelopeHash: ByteArray get() = bytes.copyOfRange(96, 128)

    companion object {
        private const val HASH_BYTES = 32
        private const val PROJECTION_BYTES = 4 * HASH_BYTES
    }
}

/** Per-client identity that cannot be supplied through the public operation-view API. */
internal class PrivacyActionOperationProvenanceOwnerV1

private class PrivacyActionOperationProvenanceTokenV1(
    private val owner: PrivacyActionOperationProvenanceOwnerV1,
    private val networkId: NetworkId,
    private val protocolId: PrivacyProtocolIdV1,
    private val operationSchema: PrivacyExact12ActionOperationV1,
    transactionHash: ByteArray,
    transactionIntentDigest: ByteArray,
    statementDigest: ByteArray,
    proofEnvelopeHash: ByteArray,
    private val ledgerEffectKind: PrivacyLedgerEffectKindV1,
    capabilityManifestDigest: ByteArray,
    private val capabilityCommittedHeight: BigInteger,
) {
    private val transactionHash = transactionHash.copyOf()
    private val transactionIntentDigest = transactionIntentDigest.copyOf()
    private val statementDigest = statementDigest.copyOf()
    private val proofEnvelopeHash = proofEnvelopeHash.copyOf()
    private val capabilityManifestDigest = capabilityManifestDigest.copyOf()

    fun matches(
        expectedOwner: PrivacyActionOperationProvenanceOwnerV1,
        expectedNetworkId: NetworkId,
        view: PrivacyActionOperationViewV1,
    ): Boolean =
        owner === expectedOwner &&
            networkId == expectedNetworkId &&
            protocolId == view.protocolId &&
            operationSchema == view.operationSchema &&
            transactionHash.contentEquals(view.transactionHash) &&
            transactionIntentDigest.contentEquals(view.transactionIntentDigest) &&
            statementDigest.contentEquals(view.statementDigest) &&
            proofEnvelopeHash.contentEquals(view.proofEnvelopeHash) &&
            ledgerEffectKind == view.ledgerEffectKind &&
            capabilityManifestDigest.contentEquals(view.capabilityManifestDigest) &&
            capabilityCommittedHeight == view.capabilityCommittedHeight
}

/**
 * Immutable public state of one authenticated Exact12 action submission.
 *
 * Construction validates operation mappings, non-zero hashes, authenticated heights, and the
 * complete local/terminal state relationship. Public construction produces a detached display
 * view; authenticated status queries accept only views returned by submission.
 */
class PrivacyActionOperationViewV1 @JvmOverloads constructor(
    @JvmField val protocolId: PrivacyProtocolIdV1,
    @JvmField val operationSchema: PrivacyExact12ActionOperationV1,
    transactionHash: ByteArray,
    transactionIntentDigest: ByteArray,
    statementDigest: ByteArray,
    proofEnvelopeHash: ByteArray,
    @JvmField val localState: PrivacyActionLocalStateV1,
    @JvmField val terminalChainState: PrivacyActionTerminalChainStateV1?,
    @JvmField val committedHeight: BigInteger?,
    @JvmField val rejectionReason: String?,
    @JvmField val ledgerEffectKind: PrivacyLedgerEffectKindV1,
    capabilityManifestDigest: ByteArray,
    @JvmField val capabilityCommittedHeight: BigInteger,
    executionCapabilityManifestDigest: ByteArray? = null,
    @JvmField val executionCapabilityCommittedHeight: BigInteger? = null,
    @JvmField val executionReceiptFinalizedHeight: BigInteger? = null,
    executionReceiptFinalizedBlockHash: ByteArray? = null,
) {
    private val transactionHashValue = transactionHash.copyOf()
    private val transactionIntentDigestValue = transactionIntentDigest.copyOf()
    private val statementDigestValue = statementDigest.copyOf()
    private val proofEnvelopeHashValue = proofEnvelopeHash.copyOf()
    private val capabilityManifestDigestValue = capabilityManifestDigest.copyOf()
    private val executionCapabilityManifestDigestValue =
        executionCapabilityManifestDigest?.copyOf()
    private val executionReceiptFinalizedBlockHashValue =
        executionReceiptFinalizedBlockHash?.copyOf()
    private var authenticatedProvenance: PrivacyActionOperationProvenanceTokenV1? = null

    init {
        require(protocolId == operationSchema.protocolId) {
            "Exact12 operation does not belong to the supplied protocol"
        }
        require(ledgerEffectKind == operationSchema.ledgerEffectKind) {
            "Exact12 operation does not produce the supplied ledger-effect kind"
        }
        requireNonzeroFixed32(transactionHashValue, "transactionHash")
        requireNonzeroFixed32(transactionIntentDigestValue, "transactionIntentDigest")
        requireNonzeroFixed32(statementDigestValue, "statementDigest")
        requireNonzeroFixed32(proofEnvelopeHashValue, "proofEnvelopeHash")
        requireNonzeroFixed32(capabilityManifestDigestValue, "capabilityManifestDigest")
        executionCapabilityManifestDigestValue?.let {
            requireNonzeroFixed32(it, "executionCapabilityManifestDigest")
        }
        executionReceiptFinalizedBlockHashValue?.let {
            requireNonzeroFixed32(it, "executionReceiptFinalizedBlockHash")
        }
        requireU64(capabilityCommittedHeight, "capabilityCommittedHeight", nonzero = true)
        committedHeight?.let { requireU64(it, "committedHeight", nonzero = true) }
        executionCapabilityCommittedHeight?.let {
            requireU64(it, "executionCapabilityCommittedHeight", nonzero = true)
        }
        executionReceiptFinalizedHeight?.let {
            requireU64(it, "executionReceiptFinalizedHeight", nonzero = true)
        }

        val hasCompleteExecutionReceipt =
            executionCapabilityManifestDigestValue != null &&
                executionCapabilityCommittedHeight != null &&
                executionReceiptFinalizedHeight != null &&
                executionReceiptFinalizedBlockHashValue != null
        val hasAnyExecutionReceipt =
            executionCapabilityManifestDigestValue != null ||
                executionCapabilityCommittedHeight != null ||
                executionReceiptFinalizedHeight != null ||
                executionReceiptFinalizedBlockHashValue != null

        when (localState) {
            PrivacyActionLocalStateV1.SUBMITTED -> require(
                terminalChainState == null &&
                    committedHeight == null &&
                    rejectionReason == null &&
                    !hasAnyExecutionReceipt,
            ) { "submitted Exact12 action must not carry terminal or execution-receipt fields" }
            PrivacyActionLocalStateV1.TERMINAL -> when (terminalChainState) {
                PrivacyActionTerminalChainStateV1.COMMITTED -> require(
                    committedHeight != null && rejectionReason == null && !hasAnyExecutionReceipt,
                ) {
                    "legacy Committed Exact12 action must carry only its committed height"
                }
                PrivacyActionTerminalChainStateV1.APPLIED -> {
                    require(
                        committedHeight != null &&
                            rejectionReason == null &&
                            hasCompleteExecutionReceipt,
                    ) {
                        "Applied Exact12 action requires complete authenticated execution-receipt evidence"
                    }
                    require(
                        executionCapabilityCommittedHeight <= committedHeight &&
                            executionReceiptFinalizedHeight >= committedHeight,
                    ) {
                        "Applied Exact12 execution-receipt heights contradict its committed height"
                    }
                }
                PrivacyActionTerminalChainStateV1.REJECTED -> {
                    require(committedHeight != null) {
                        "rejected Exact12 action must carry its authenticated committed height"
                    }
                    require(isCanonicalRejectionReason(rejectionReason)) {
                        "rejected Exact12 action must carry one canonical non-empty reason"
                    }
                    require(!hasAnyExecutionReceipt) {
                        "rejected Exact12 action must not carry successful execution evidence"
                    }
                }
                PrivacyActionTerminalChainStateV1.EXPIRED -> require(
                    committedHeight == null && rejectionReason == null && !hasAnyExecutionReceipt,
                ) { "expired Exact12 action must not carry committed or execution-receipt fields" }
                null -> throw IllegalArgumentException(
                    "terminal Exact12 action must carry one terminal chain state",
                )
            }
        }
        if (localState == PrivacyActionLocalStateV1.TERMINAL && committedHeight != null) {
            require(committedHeight >= capabilityCommittedHeight) {
                "authenticated terminal height predates pre-submit capability admission"
            }
        }
    }

    val transactionHash: ByteArray get() = transactionHashValue.copyOf()
    val transactionIntentDigest: ByteArray get() = transactionIntentDigestValue.copyOf()
    val statementDigest: ByteArray get() = statementDigestValue.copyOf()
    val proofEnvelopeHash: ByteArray get() = proofEnvelopeHashValue.copyOf()
    val capabilityManifestDigest: ByteArray get() = capabilityManifestDigestValue.copyOf()
    val executionCapabilityManifestDigest: ByteArray?
        get() = executionCapabilityManifestDigestValue?.copyOf()
    val executionReceiptFinalizedBlockHash: ByteArray?
        get() = executionReceiptFinalizedBlockHashValue?.copyOf()

    fun transactionHashBytes(): ByteArray = transactionHash
    fun transactionIntentDigestBytes(): ByteArray = transactionIntentDigest
    fun statementDigestBytes(): ByteArray = statementDigest
    fun proofEnvelopeHashBytes(): ByteArray = proofEnvelopeHash
    fun capabilityManifestDigestBytes(): ByteArray = capabilityManifestDigest
    fun executionCapabilityManifestDigestBytes(): ByteArray? = executionCapabilityManifestDigest
    fun executionReceiptFinalizedBlockHashBytes(): ByteArray? =
        executionReceiptFinalizedBlockHash

    internal fun bindAuthenticatedSubmissionV1(
        owner: PrivacyActionOperationProvenanceOwnerV1,
        networkId: NetworkId,
    ): PrivacyActionOperationViewV1 {
        check(localState == PrivacyActionLocalStateV1.SUBMITTED && terminalChainState == null) {
            "only a submitted Exact12 action can receive authenticated provenance"
        }
        check(authenticatedProvenance == null) {
            "Exact12 action already carries authenticated provenance"
        }
        authenticatedProvenance = PrivacyActionOperationProvenanceTokenV1(
            owner,
            networkId,
            protocolId,
            operationSchema,
            transactionHashValue,
            transactionIntentDigestValue,
            statementDigestValue,
            proofEnvelopeHashValue,
            ledgerEffectKind,
            capabilityManifestDigestValue,
            capabilityCommittedHeight,
        )
        return this
    }

    internal fun requireAuthenticatedProvenanceV1(
        owner: PrivacyActionOperationProvenanceOwnerV1,
        networkId: NetworkId,
    ) {
        check(authenticatedProvenance?.matches(owner, networkId, this) == true) {
            "Exact12 status requires a view returned by this client's authenticated submission"
        }
    }

    internal fun withAuthenticatedTerminalStateV1(
        terminal: PrivacyActionTerminalChainStateV1,
        height: BigInteger?,
        rejection: String?,
        executionCapabilityManifestDigest: ByteArray? = null,
        executionCapabilityCommittedHeight: BigInteger? = null,
        executionReceiptFinalizedHeight: BigInteger? = null,
        executionReceiptFinalizedBlockHash: ByteArray? = null,
    ): PrivacyActionOperationViewV1 {
        check(authenticatedProvenance != null) {
            "detached Exact12 views cannot receive authenticated terminal state"
        }
        return PrivacyActionOperationViewV1(
            protocolId = protocolId,
            operationSchema = operationSchema,
            transactionHash = transactionHashValue,
            transactionIntentDigest = transactionIntentDigestValue,
            statementDigest = statementDigestValue,
            proofEnvelopeHash = proofEnvelopeHashValue,
            localState = PrivacyActionLocalStateV1.TERMINAL,
            terminalChainState = terminal,
            committedHeight = height,
            rejectionReason = rejection,
            ledgerEffectKind = ledgerEffectKind,
            capabilityManifestDigest = capabilityManifestDigestValue,
            capabilityCommittedHeight = capabilityCommittedHeight,
            executionCapabilityManifestDigest = executionCapabilityManifestDigest,
            executionCapabilityCommittedHeight = executionCapabilityCommittedHeight,
            executionReceiptFinalizedHeight = executionReceiptFinalizedHeight,
            executionReceiptFinalizedBlockHash = executionReceiptFinalizedBlockHash,
        ).also { it.authenticatedProvenance = authenticatedProvenance }
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyActionOperationViewV1 &&
            protocolId == other.protocolId &&
            operationSchema == other.operationSchema &&
            transactionHashValue.contentEquals(other.transactionHashValue) &&
            transactionIntentDigestValue.contentEquals(other.transactionIntentDigestValue) &&
            statementDigestValue.contentEquals(other.statementDigestValue) &&
            proofEnvelopeHashValue.contentEquals(other.proofEnvelopeHashValue) &&
            localState == other.localState &&
            terminalChainState == other.terminalChainState &&
            committedHeight == other.committedHeight &&
            rejectionReason == other.rejectionReason &&
            ledgerEffectKind == other.ledgerEffectKind &&
            capabilityManifestDigestValue.contentEquals(other.capabilityManifestDigestValue) &&
            capabilityCommittedHeight == other.capabilityCommittedHeight &&
            nullableBytesEqual(
                executionCapabilityManifestDigestValue,
                other.executionCapabilityManifestDigestValue,
            ) &&
            executionCapabilityCommittedHeight == other.executionCapabilityCommittedHeight &&
            executionReceiptFinalizedHeight == other.executionReceiptFinalizedHeight &&
            nullableBytesEqual(
                executionReceiptFinalizedBlockHashValue,
                other.executionReceiptFinalizedBlockHashValue,
            )

    override fun hashCode(): Int {
        var result = protocolId.hashCode()
        result = 31 * result + operationSchema.hashCode()
        result = 31 * result + transactionHashValue.contentHashCode()
        result = 31 * result + transactionIntentDigestValue.contentHashCode()
        result = 31 * result + statementDigestValue.contentHashCode()
        result = 31 * result + proofEnvelopeHashValue.contentHashCode()
        result = 31 * result + localState.hashCode()
        result = 31 * result + (terminalChainState?.hashCode() ?: 0)
        result = 31 * result + (committedHeight?.hashCode() ?: 0)
        result = 31 * result + (rejectionReason?.hashCode() ?: 0)
        result = 31 * result + ledgerEffectKind.hashCode()
        result = 31 * result + capabilityManifestDigestValue.contentHashCode()
        result = 31 * result + capabilityCommittedHeight.hashCode()
        result = 31 * result + (executionCapabilityManifestDigestValue?.contentHashCode() ?: 0)
        result = 31 * result + (executionCapabilityCommittedHeight?.hashCode() ?: 0)
        result = 31 * result + (executionReceiptFinalizedHeight?.hashCode() ?: 0)
        return 31 * result + (executionReceiptFinalizedBlockHashValue?.contentHashCode() ?: 0)
    }
}

private fun requireNonzeroFixed32(value: ByteArray, field: String) {
    require(value.size == 32 && value.any { it.toInt() != 0 }) {
        "$field must contain exactly 32 non-zero bytes"
    }
}

private fun requireU64(value: BigInteger, field: String, nonzero: Boolean) {
    require(value.signum() >= 0 && value.bitLength() <= 64 && (!nonzero || value.signum() > 0)) {
        "$field must be ${if (nonzero) "a non-zero " else "an "}u64"
    }
}

private fun isCanonicalRejectionReason(value: String?): Boolean =
    value != null &&
        value.isNotEmpty() &&
        value.toByteArray(Charsets.UTF_8).size <= 1_024 &&
        value == value.trim() &&
        value.none(Char::isISOControl)

private fun nullableBytesEqual(left: ByteArray?, right: ByteArray?): Boolean =
    when {
        left == null -> right == null
        right == null -> false
        else -> left.contentEquals(right)
    }
