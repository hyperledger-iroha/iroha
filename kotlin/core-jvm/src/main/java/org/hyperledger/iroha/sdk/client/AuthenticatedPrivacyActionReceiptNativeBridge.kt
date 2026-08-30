// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.Arrays
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.privacy.PrivacyActionOperationViewV1
import org.hyperledger.iroha.sdk.privacy.PrivacyLedgerEffectKindV1
import org.hyperledger.iroha.sdk.privacy.PrivacyOperationSchemaV1
import org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1
import org.hyperledger.iroha.sdk.privacy.ledgerEffectKind
import org.hyperledger.iroha.sdk.privacy.protocolId

/** Native-authenticated, finalized ID105 execution receipt for one Exact12 action. */
class AuthenticatedPrivacyActionExecutionReceiptV1 internal constructor(
    @JvmField val networkIdHex: String,
    @JvmField val protocolId: PrivacyProtocolIdV1,
    @JvmField val operationSchema: PrivacyOperationSchemaV1,
    @JvmField val ledgerEffectKind: PrivacyLedgerEffectKindV1,
    @JvmField val transactionHashHex: String,
    @JvmField val actionIndex: Int,
    transactionIntentDigest: ByteArray,
    statementDigest: ByteArray,
    proofEnvelopeHash: ByteArray,
    capabilityManifestDigest: ByteArray,
    @JvmField val capabilityCommittedHeight: BigInteger,
    @JvmField val admittedAtHeight: BigInteger,
    @JvmField val finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
) {
    private val transactionIntentDigestValue = transactionIntentDigest.copyOf()
    private val statementDigestValue = statementDigest.copyOf()
    private val proofEnvelopeHashValue = proofEnvelopeHash.copyOf()
    private val capabilityManifestDigestValue = capabilityManifestDigest.copyOf()
    private val finalizedBlockHashValue = finalizedBlockHash.copyOf()

    init {
        require(networkIdHex.isExactNonzeroLowerHashV1()) {
            "networkIdHex must be an exact non-zero lowercase 32-byte hash"
        }
        require(transactionHashHex.isExactNonzeroLowerHashV1()) {
            "transactionHashHex must be an exact non-zero lowercase 32-byte hash"
        }
        require(protocolId == operationSchema.protocolId) {
            "receipt protocol does not match its operation"
        }
        require(ledgerEffectKind == operationSchema.ledgerEffectKind) {
            "receipt ledger effect does not match its operation"
        }
        require(actionIndex == 0) { "Exact12 V1 receipt actionIndex must be zero" }
        requireNonzero32V1(transactionIntentDigestValue, "transactionIntentDigest")
        requireNonzero32V1(statementDigestValue, "statementDigest")
        requireNonzero32V1(proofEnvelopeHashValue, "proofEnvelopeHash")
        requireNonzero32V1(capabilityManifestDigestValue, "capabilityManifestDigest")
        requireNonzero32V1(finalizedBlockHashValue, "finalizedBlockHash")
        requirePositiveU64V1(capabilityCommittedHeight, "capabilityCommittedHeight")
        requirePositiveU64V1(admittedAtHeight, "admittedAtHeight")
        requirePositiveU64V1(finalizedHeight, "finalizedHeight")
        require(capabilityCommittedHeight <= admittedAtHeight && admittedAtHeight <= finalizedHeight) {
            "receipt capability, admission, and finality heights are contradictory"
        }
    }

    val transactionIntentDigest: ByteArray get() = transactionIntentDigestValue.copyOf()
    val statementDigest: ByteArray get() = statementDigestValue.copyOf()
    val proofEnvelopeHash: ByteArray get() = proofEnvelopeHashValue.copyOf()
    val capabilityManifestDigest: ByteArray get() = capabilityManifestDigestValue.copyOf()
    val finalizedBlockHash: ByteArray get() = finalizedBlockHashValue.copyOf()

    fun transactionIntentDigestBytes(): ByteArray = transactionIntentDigest
    fun statementDigestBytes(): ByteArray = statementDigest
    fun proofEnvelopeHashBytes(): ByteArray = proofEnvelopeHash
    fun capabilityManifestDigestBytes(): ByteArray = capabilityManifestDigest
    fun finalizedBlockHashBytes(): ByteArray = finalizedBlockHash
}

/** Closed ABI-22 classification of a finalized Exact12 transaction rejection. */
enum class AuthenticatedPrivacyActionRejectionCodeV1(val canonicalLabel: String) {
    ACCOUNT_DOES_NOT_EXIST("account_does_not_exist"),
    LIMIT_CHECK("limit_check"),
    VALIDATION("validation"),
    INSTRUCTION_EXECUTION("instruction_execution"),
    IVM_EXECUTION("ivm_execution"),
    TRIGGER_EXECUTION("trigger_execution"),
    ;

    companion object {
        @JvmStatic
        fun fromCanonicalLabel(label: String): AuthenticatedPrivacyActionRejectionCodeV1 =
            values().firstOrNull { it.canonicalLabel == label }
                ?: throw IllegalArgumentException("unknown finalized Exact12 rejection code")
    }
}

/** Exact rejected Exact12 action independently authenticated by block and Sumeragi-v2 finality. */
class AuthenticatedFinalizedPrivacyActionRejectionV1 internal constructor(
    @JvmField val networkIdHex: String,
    @JvmField val protocolId: PrivacyProtocolIdV1,
    @JvmField val operationSchema: PrivacyOperationSchemaV1,
    @JvmField val ledgerEffectKind: PrivacyLedgerEffectKindV1,
    @JvmField val transactionHashHex: String,
    @JvmField val actionIndex: Int,
    transactionIntentDigest: ByteArray,
    statementDigest: ByteArray,
    proofEnvelopeHash: ByteArray,
    @JvmField val queryAuthorityAccountId: String,
    @JvmField val transactionAuthorityAccountId: String,
    @JvmField val blockHashHex: String,
    @JvmField val resultHashHex: String,
    @JvmField val rejectionCode: AuthenticatedPrivacyActionRejectionCodeV1,
    @JvmField val rejectionMessage: String,
    @JvmField val committedBlockHeight: Long,
    @JvmField val finalizedCheckpoint: AuthenticatedFinalityCheckpointV1,
    @JvmField val executedBlockWireHashHex: String,
    @JvmField val evidenceIdHex: String,
    @JvmField val transactionDetailsHashHex: String,
    @JvmField val finalityPageHashHex: String,
) {
    private val transactionIntentDigestValue = transactionIntentDigest.copyOf()
    private val statementDigestValue = statementDigest.copyOf()
    private val proofEnvelopeHashValue = proofEnvelopeHash.copyOf()

    init {
        requireMarkedHashV1(networkIdHex, "networkIdHex")
        require(protocolId == operationSchema.protocolId) {
            "finalized rejection protocol does not match its operation"
        }
        require(ledgerEffectKind == operationSchema.ledgerEffectKind) {
            "finalized rejection ledger effect does not match its operation"
        }
        requireMarkedHashV1(transactionHashHex, "transactionHashHex")
        require(actionIndex == 0) { "Exact12 V1 finalized rejection actionIndex must be zero" }
        requireNonzero32V1(transactionIntentDigestValue, "transactionIntentDigest")
        requireNonzero32V1(statementDigestValue, "statementDigest")
        requireNonzero32V1(proofEnvelopeHashValue, "proofEnvelopeHash")
        requireCanonicalTextV1(queryAuthorityAccountId, "queryAuthorityAccountId", 16 * 1024)
        requireCanonicalTextV1(
            transactionAuthorityAccountId,
            "transactionAuthorityAccountId",
            16 * 1024,
        )
        requireMarkedHashV1(blockHashHex, "blockHashHex")
        requireMarkedHashV1(resultHashHex, "resultHashHex")
        requireCanonicalTextV1(rejectionMessage, "rejectionMessage", 1_024)
        require(
            committedBlockHeight > 0 && finalizedCheckpoint.height() == committedBlockHeight,
        ) { "finalized checkpoint must equal committedBlockHeight" }
        requireMarkedHashV1(executedBlockWireHashHex, "executedBlockWireHashHex")
        requireMarkedHashV1(evidenceIdHex, "evidenceIdHex")
        requireMarkedHashV1(transactionDetailsHashHex, "transactionDetailsHashHex")
        requireMarkedHashV1(finalityPageHashHex, "finalityPageHashHex")
    }

    val transactionIntentDigest: ByteArray get() = transactionIntentDigestValue.copyOf()
    val statementDigest: ByteArray get() = statementDigestValue.copyOf()
    val proofEnvelopeHash: ByteArray get() = proofEnvelopeHashValue.copyOf()

    fun transactionIntentDigestBytes(): ByteArray = transactionIntentDigest
    fun statementDigestBytes(): ByteArray = statementDigest
    fun proofEnvelopeHashBytes(): ByteArray = proofEnvelopeHash
}

/** ABI-22 native codec and verifier for authenticated finalized Exact12 action receipts. */
class AuthenticatedPrivacyActionReceiptNativeBridge private constructor() {
    /** Native-bound signed ID105 body plus the private preparation used to verify its response. */
    class SignedQueryV1 internal constructor(
        preparation: ByteArray,
        requestBody: ByteArray,
        internal val networkIdHex: String,
        internal val protocolId: PrivacyProtocolIdV1,
        internal val operationSchema: PrivacyOperationSchemaV1,
        internal val ledgerEffectKind: PrivacyLedgerEffectKindV1,
        internal val transactionHashHex: String,
        internal val actionIndex: Int,
        transactionIntentDigest: ByteArray,
        statementDigest: ByteArray,
        proofEnvelopeHash: ByteArray,
    ) {
        private val nativePreparation = preparation.copyOf()
        private val canonicalRequestBody = requestBody.copyOf()
        internal val transactionIntentDigest = transactionIntentDigest.copyOf()
        internal val statementDigest = statementDigest.copyOf()
        internal val proofEnvelopeHash = proofEnvelopeHash.copyOf()

        /** Canonical versioned `SignedQuery` bytes for `POST /v1/query`. */
        fun requestBody(): ByteArray = canonicalRequestBody.copyOf()

        internal fun preparation(): ByteArray = nativePreparation.copyOf()
    }

    companion object {
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 22
        const val RESPONSE_MAX_BYTES: Long = 256L * 1024L
        private const val ACTION_INDEX_V1 = 0
        private const val DIGEST_BYTES = 32
        private const val NONCE_BYTES = 32
        private const val REQUEST_BINDING_BYTES = 96
        private const val PREPARATION_MAX_BYTES = 64 * 1024
        private const val SIGNATURE_MAX_BYTES = 16 * 1024
        private const val SIGNED_QUERY_MAX_BYTES = 64 * 1024
        private val nonceRandom = SecureRandom()

        private val nativeLoadResult: Result<Unit> by lazy {
            runCatching {
                System.loadLibrary("connect_norito_bridge")
                val actual = nativeBridgeAbiVersion()
                check(actual == REQUIRED_BRIDGE_ABI_VERSION) {
                    "native authenticated receipt ABI mismatch: expected " +
                        "$REQUIRED_BRIDGE_ABI_VERSION, found $actual"
                }
            }
        }

        /** Build and sign one fresh ID105 query bound to every inspected action digest. */
        @JvmStatic
        fun buildSignedPrivacyActionReceiptQueryV1(
            operation: PrivacyActionOperationViewV1,
            networkId: NetworkId,
            authorityAccountId: String,
            signer: IrohaQuerySignatureProvider,
        ): SignedQueryV1 {
            val nonce = ByteArray(NONCE_BYTES)
            repeat(16) {
                if (!nonce.all { byte -> byte == 0.toByte() }) return@repeat
                nonceRandom.nextBytes(nonce)
            }
            check(!nonce.all { it == 0.toByte() }) {
                "secure receipt-query nonce generator repeatedly returned zero"
            }
            return buildSignedPrivacyActionReceiptQueryAtV1(
                operation,
                networkId,
                authorityAccountId,
                signer,
                System.currentTimeMillis(),
                nonce,
            )
        }

        internal fun buildSignedPrivacyActionReceiptQueryAtV1(
            operation: PrivacyActionOperationViewV1,
            networkId: NetworkId,
            authorityAccountId: String,
            signer: IrohaQuerySignatureProvider,
            creationTimeMs: Long,
            nonce: ByteArray,
        ): SignedQueryV1 {
            requireNative()
            require(creationTimeMs > 0) { "creationTimeMs must be positive" }
            require(nonce.size == NONCE_BYTES && nonce.any { it != 0.toByte() }) {
                "nonce must contain exactly 32 non-zero bytes"
            }
            val transactionHashHex = operation.transactionHash.toLowerHexV1()
            val networkIdHex = networkId.bytes().toLowerHexV1()
            val binding = ByteArray(REQUEST_BINDING_BYTES)
            operation.transactionIntentDigest.copyInto(binding, 0)
            operation.statementDigest.copyInto(binding, 32)
            operation.proofEnvelopeHash.copyInto(binding, 64)
            val prepared = nativePreparePrivacyActionReceiptQueryV1(
                networkId.bytes(),
                authorityAccountId.toByteArray(Charsets.UTF_8),
                operation.operationSchema.ordinal,
                transactionHashHex.toByteArray(Charsets.US_ASCII),
                ACTION_INDEX_V1,
                binding,
                creationTimeMs,
                nonce.copyOf(),
            )
            check(
                prepared.size == 2 &&
                    prepared[0].isNotEmpty() &&
                    prepared[0].size <= PREPARATION_MAX_BYTES &&
                    prepared[1].size == DIGEST_BYTES,
            ) { "native receipt-query preparation returned an invalid shape" }
            val digest = prepared[1].copyOf()
            val signature = try {
                signer.signQueryDigest(digest.copyOf())
            } finally {
                Arrays.fill(digest, 0.toByte())
                Arrays.fill(prepared[1], 0.toByte())
                Arrays.fill(binding, 0.toByte())
            }
            require(signature.isNotEmpty() && signature.size <= SIGNATURE_MAX_BYTES) {
                "opaque query signer returned invalid signature bytes"
            }
            val requestBody = nativeFinalizePrivacyActionReceiptQueryV1(
                prepared[0].copyOf(),
                signature.copyOf(),
            )
            check(requestBody.isNotEmpty() && requestBody.size <= SIGNED_QUERY_MAX_BYTES) {
                "native receipt-query finalizer violated the request byte bound"
            }
            return SignedQueryV1(
                preparation = prepared[0],
                requestBody = requestBody,
                networkIdHex = networkIdHex,
                protocolId = operation.protocolId,
                operationSchema = operation.operationSchema,
                ledgerEffectKind = operation.ledgerEffectKind,
                transactionHashHex = transactionHashHex,
                actionIndex = ACTION_INDEX_V1,
                transactionIntentDigest = operation.transactionIntentDigest,
                statementDigest = operation.statementDigest,
                proofEnvelopeHash = operation.proofEnvelopeHash,
            )
        }

        /** Natively verify and project the exact finalized receipt bound to [signedQuery]. */
        @JvmStatic
        fun projectPrivacyActionReceiptV1(
            signedQuery: SignedQueryV1,
            responseNorito: ByteArray,
        ): AuthenticatedPrivacyActionExecutionReceiptV1 {
            requireNative()
            require(responseNorito.isNotEmpty() && responseNorito.size.toLong() <= RESPONSE_MAX_BYTES) {
                "responseNorito violates its closed byte bound"
            }
            val fields = nativeProjectPrivacyActionReceiptV1(
                signedQuery.preparation(),
                responseNorito.copyOf(),
            )
            check(fields.size == 15) {
                "native authenticated receipt projection has invalid shape"
            }
            check(exactUtf8V1(fields[0], "version") == "1") {
                "native authenticated receipt version is invalid"
            }
            val networkIdHex = exactNonzeroLowerHashV1(fields[1], "networkId")
            val protocolId = PrivacyProtocolIdV1.fromCanonicalLabel(
                exactUtf8V1(fields[2], "protocolId"),
            )
            val operationSchema = PrivacyOperationSchemaV1.fromCanonicalLabel(
                exactUtf8V1(fields[3], "operationSchema"),
            )
            val ledgerEffectKind = PrivacyLedgerEffectKindV1.fromCanonicalLabel(
                exactUtf8V1(fields[4], "ledgerEffectKind"),
            )
            val transactionHashHex = exactNonzeroLowerHashV1(fields[5], "transactionHash")
            val actionIndex = exactUnsignedDecimalV1(fields[6], "actionIndex").intValueExact()
            val transactionIntentDigest = exactNonzeroLowerHashBytesV1(
                fields[7],
                "transactionIntentDigest",
            )
            val statementDigest = exactNonzeroLowerHashBytesV1(fields[8], "statementDigest")
            val proofEnvelopeHash = exactNonzeroLowerHashBytesV1(fields[9], "proofEnvelopeHash")
            val capabilityManifestDigest = exactNonzeroLowerHashBytesV1(
                fields[10],
                "capabilityManifestDigest",
            )
            val capabilityCommittedHeight = exactPositiveU64V1(
                fields[11],
                "capabilityCommittedHeight",
            )
            val admittedAtHeight = exactPositiveU64V1(fields[12], "admittedAtHeight")
            val finalizedHeight = exactPositiveU64V1(fields[13], "finalizedHeight")
            val finalizedBlockHash = exactNonzeroLowerHashBytesV1(
                fields[14],
                "finalizedBlockHash",
            )

            check(
                networkIdHex == signedQuery.networkIdHex &&
                    protocolId == signedQuery.protocolId &&
                    operationSchema == signedQuery.operationSchema &&
                    ledgerEffectKind == signedQuery.ledgerEffectKind &&
                    protocolId == operationSchema.protocolId &&
                    ledgerEffectKind == operationSchema.ledgerEffectKind &&
                    transactionHashHex == signedQuery.transactionHashHex &&
                    actionIndex == signedQuery.actionIndex &&
                    MessageDigest.isEqual(
                        transactionIntentDigest,
                        signedQuery.transactionIntentDigest,
                    ) &&
                    MessageDigest.isEqual(statementDigest, signedQuery.statementDigest) &&
                    MessageDigest.isEqual(proofEnvelopeHash, signedQuery.proofEnvelopeHash),
            ) { "native authenticated receipt changed its requested action binding" }

            return AuthenticatedPrivacyActionExecutionReceiptV1(
                networkIdHex = networkIdHex,
                protocolId = protocolId,
                operationSchema = operationSchema,
                ledgerEffectKind = ledgerEffectKind,
                transactionHashHex = transactionHashHex,
                actionIndex = actionIndex,
                transactionIntentDigest = transactionIntentDigest,
                statementDigest = statementDigest,
                proofEnvelopeHash = proofEnvelopeHash,
                capabilityManifestDigest = capabilityManifestDigest,
                capabilityCommittedHeight = capabilityCommittedHeight,
                admittedAtHeight = admittedAtHeight,
                finalizedHeight = finalizedHeight,
                finalizedBlockHash = finalizedBlockHash,
            )
        }

        /** Verify one rejected Exact12 action against its exact binding, block, and QC page. */
        @JvmStatic
        fun projectFinalizedPrivacyActionRejectionV1(
            carrier: AuthenticatedTransactionDetailsCarrierV2,
            operation: PrivacyActionOperationViewV1,
            networkId: NetworkId,
            trustedCheckpoint: AuthenticatedFinalityCheckpointV1,
            finalityPage: AuthenticatedFinalityProofPageV1,
            executedBlockWire: ByteArray,
        ): AuthenticatedFinalizedPrivacyActionRejectionV1 {
            requireNative()
            require(!carrier.resultOkHint()) {
                "finalized Exact12 rejection requires a rejected transaction-details carrier"
            }
            require(
                executedBlockWire.isNotEmpty() &&
                    executedBlockWire.size.toLong() <=
                    AuthenticatedTransactionDetailsNativeBridge.EXECUTED_BLOCK_WIRE_MAX_BYTES,
            ) { "executedBlockWire violates its closed byte bound" }
            val requestedActionBinding = ByteArray(REQUEST_BINDING_BYTES)
            operation.transactionIntentDigest.copyInto(requestedActionBinding, 0)
            operation.statementDigest.copyInto(requestedActionBinding, 32)
            operation.proofEnvelopeHash.copyInto(requestedActionBinding, 64)
            val fields = nativeProjectFinalizedPrivacyActionRejectionV1(
                carrier.signedQuery.preparation(),
                carrier.responseNorito(),
                operation.operationSchema.ordinal,
                ACTION_INDEX_V1,
                requestedActionBinding,
                networkId.bytes(),
                trustedCheckpoint.height(),
                trustedCheckpoint.heightContextId(),
                finalityPage.evidenceArchive(),
                executedBlockWire.copyOf(),
            )
            Arrays.fill(requestedActionBinding, 0.toByte())
            check(fields.size == 22) {
                "native finalized Exact12 rejection projection has invalid shape"
            }
            check(exactUtf8V1(fields[0], "version") == "1") {
                "native finalized Exact12 rejection version is invalid"
            }
            val networkIdHex = exactNonzeroLowerHashV1(fields[1], "networkId")
            val protocolId = PrivacyProtocolIdV1.fromCanonicalLabel(
                exactUtf8V1(fields[2], "protocolId"),
            )
            val operationSchema = PrivacyOperationSchemaV1.fromCanonicalLabel(
                exactUtf8V1(fields[3], "operationSchema"),
            )
            val ledgerEffectKind = PrivacyLedgerEffectKindV1.fromCanonicalLabel(
                exactUtf8V1(fields[4], "ledgerEffectKind"),
            )
            val transactionHashHex = exactNonzeroLowerHashV1(fields[5], "transactionHash")
            val actionIndex = exactUnsignedDecimalV1(fields[6], "actionIndex").intValueExact()
            val transactionIntentDigest = exactNonzeroLowerHashBytesV1(
                fields[7],
                "transactionIntentDigest",
            )
            val statementDigest = exactNonzeroLowerHashBytesV1(fields[8], "statementDigest")
            val proofEnvelopeHash = exactNonzeroLowerHashBytesV1(
                fields[9],
                "proofEnvelopeHash",
            )
            val queryAuthority = exactUtf8V1(fields[10], "queryAuthorityAccountId")
            val transactionAuthority = exactUtf8V1(
                fields[11],
                "transactionAuthorityAccountId",
            )
            val blockHashHex = exactNonzeroLowerHashV1(fields[12], "blockHashHex")
            val resultHashHex = exactNonzeroLowerHashV1(fields[13], "resultHashHex")
            val rejectionCode = AuthenticatedPrivacyActionRejectionCodeV1.fromCanonicalLabel(
                exactUtf8V1(fields[14], "rejectionCode"),
            )
            val rejectionMessage = exactUtf8V1(fields[15], "rejectionMessage")
            val committedBlockHeight = try {
                exactPositiveU64V1(fields[16], "committedBlockHeight").longValueExact()
            } catch (error: ArithmeticException) {
                throw IllegalStateException(
                    "native finalized Exact12 rejection height exceeds the mobile u63 range",
                    error,
                )
            }
            val rejection = AuthenticatedFinalizedPrivacyActionRejectionV1(
                networkIdHex = networkIdHex,
                protocolId = protocolId,
                operationSchema = operationSchema,
                ledgerEffectKind = ledgerEffectKind,
                transactionHashHex = transactionHashHex,
                actionIndex = actionIndex,
                transactionIntentDigest = transactionIntentDigest,
                statementDigest = statementDigest,
                proofEnvelopeHash = proofEnvelopeHash,
                queryAuthorityAccountId = queryAuthority,
                transactionAuthorityAccountId = transactionAuthority,
                blockHashHex = blockHashHex,
                resultHashHex = resultHashHex,
                rejectionCode = rejectionCode,
                rejectionMessage = rejectionMessage,
                committedBlockHeight = committedBlockHeight,
                finalizedCheckpoint = AuthenticatedFinalityCheckpointV1.fromProjection(fields[17]),
                executedBlockWireHashHex = exactNonzeroLowerHashV1(
                    fields[18],
                    "executedBlockWireHashHex",
                ),
                evidenceIdHex = exactNonzeroLowerHashV1(fields[19], "evidenceIdHex"),
                transactionDetailsHashHex = exactNonzeroLowerHashV1(
                    fields[20],
                    "transactionDetailsHashHex",
                ),
                finalityPageHashHex = exactNonzeroLowerHashV1(
                    fields[21],
                    "finalityPageHashHex",
                ),
            )
            check(
                rejection.networkIdHex == networkId.bytes().toLowerHexV1() &&
                    rejection.protocolId == operation.protocolId &&
                    rejection.operationSchema == operation.operationSchema &&
                    rejection.ledgerEffectKind == operation.ledgerEffectKind &&
                    rejection.transactionHashHex == operation.transactionHash.toLowerHexV1() &&
                    rejection.actionIndex == ACTION_INDEX_V1 &&
                    MessageDigest.isEqual(
                        rejection.transactionIntentDigest,
                        operation.transactionIntentDigest,
                    ) &&
                    MessageDigest.isEqual(rejection.statementDigest, operation.statementDigest) &&
                    MessageDigest.isEqual(
                        rejection.proofEnvelopeHash,
                        operation.proofEnvelopeHash,
                    ) &&
                    rejection.committedBlockHeight == carrier.committedBlockHeightHint() &&
                    rejection.finalizedCheckpoint.height() > trustedCheckpoint.height() &&
                    rejection.finalityPageHashHex == finalityPage.hashHex() &&
                    rejection.finalityPageHashHex ==
                    IrohaHash.prehash(finalityPage.evidenceArchive()).toLowerHexV1() &&
                    rejection.transactionDetailsHashHex ==
                    IrohaHash.prehash(carrier.responseNorito()).toLowerHexV1() &&
                    rejection.executedBlockWireHashHex ==
                    IrohaHash.prehash(executedBlockWire).toLowerHexV1(),
            ) { "native finalized Exact12 rejection changed its requested evidence binding" }
            return rejection
        }

        /** Convenience overload which first creates the canonical QC page. */
        @JvmStatic
        fun projectFinalizedPrivacyActionRejectionV1(
            carrier: AuthenticatedTransactionDetailsCarrierV2,
            operation: PrivacyActionOperationViewV1,
            networkId: NetworkId,
            trustedCheckpoint: AuthenticatedFinalityCheckpointV1,
            finalityProofsNorito: Array<ByteArray>,
            executedBlockWire: ByteArray,
        ): AuthenticatedFinalizedPrivacyActionRejectionV1 =
            projectFinalizedPrivacyActionRejectionV1(
                carrier,
                operation,
                networkId,
                trustedCheckpoint,
                AuthenticatedTransactionDetailsNativeBridge.bindFinalityProofPageV1(
                    finalityProofsNorito,
                ),
                executedBlockWire,
            )

        private fun requireNative() {
            nativeLoadResult.getOrElse { error ->
                throw IllegalStateException(
                    "ABI-22 native authenticated Exact12 receipt bridge is unavailable",
                    error,
                )
            }
        }

        @JvmStatic private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic private external fun nativePreparePrivacyActionReceiptQueryV1(
            networkId: ByteArray,
            authorityAccountId: ByteArray,
            operationIndex: Int,
            transactionHashHex: ByteArray,
            actionIndex: Int,
            requestedActionBinding: ByteArray,
            creationTimeMs: Long,
            nonce: ByteArray,
        ): Array<ByteArray>

        @JvmStatic private external fun nativeFinalizePrivacyActionReceiptQueryV1(
            preparation: ByteArray,
            signature: ByteArray,
        ): ByteArray

        @JvmStatic private external fun nativeProjectPrivacyActionReceiptV1(
            preparation: ByteArray,
            responseNorito: ByteArray,
        ): Array<ByteArray>

        @JvmStatic private external fun nativeProjectFinalizedPrivacyActionRejectionV1(
            preparation: ByteArray,
            responseNorito: ByteArray,
            operationIndex: Int,
            actionIndex: Int,
            requestedActionBinding: ByteArray,
            networkId: ByteArray,
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
            finalityPageArchive: ByteArray,
            executedBlockWire: ByteArray,
        ): Array<ByteArray>
    }
}

private fun exactUtf8V1(value: ByteArray?, field: String): String {
    check(value != null && value.isNotEmpty()) { "native $field is empty" }
    return try {
        Charsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
            .decode(ByteBuffer.wrap(value))
            .toString()
    } catch (error: Exception) {
        throw IllegalStateException("native $field is not exact UTF-8", error)
    }
}

private fun exactUnsignedDecimalV1(value: ByteArray?, field: String): BigInteger {
    val text = exactUtf8V1(value, field)
    check(text.isNotEmpty() && text.all { it in '0'..'9' } &&
        (text.length == 1 || text[0] != '0')) {
        "native $field is not a canonical unsigned decimal"
    }
    return try {
        BigInteger(text)
    } catch (error: NumberFormatException) {
        throw IllegalStateException("native $field is invalid", error)
    }
}

private fun exactPositiveU64V1(value: ByteArray?, field: String): BigInteger =
    exactUnsignedDecimalV1(value, field).also { requirePositiveU64V1(it, field) }

private fun requirePositiveU64V1(value: BigInteger, field: String) {
    check(value.signum() > 0 && value.bitLength() <= 64) {
        "native $field is not a positive u64"
    }
}

private fun exactNonzeroLowerHashV1(value: ByteArray?, field: String): String =
    exactUtf8V1(value, field).also {
        check(it.isExactNonzeroLowerHashV1()) {
            "native $field is not an exact non-zero lowercase 32-byte hash"
        }
    }

private fun exactNonzeroLowerHashBytesV1(value: ByteArray?, field: String): ByteArray {
    val text = exactNonzeroLowerHashV1(value, field)
    return ByteArray(32) { index ->
        ((text[index * 2].digitToInt(16) shl 4) or text[index * 2 + 1].digitToInt(16)).toByte()
    }
}

private fun String.isExactNonzeroLowerHashV1(): Boolean =
    length == 64 &&
        all { it in '0'..'9' || it in 'a'..'f' } &&
        any { it != '0' }

private fun requireMarkedHashV1(value: String, field: String) {
    require(
        value.isExactNonzeroLowerHashV1() &&
            (value.last().digitToInt(16) and 1) == 1,
    ) { "$field must be an exact lowercase marked 32-byte Iroha hash" }
}

private fun requireCanonicalTextV1(value: String, field: String, maximumUtf8Bytes: Int) {
    require(
        value.isNotEmpty() &&
            value.toByteArray(Charsets.UTF_8).size <= maximumUtf8Bytes &&
            value == value.trim() &&
            value.none(Char::isISOControl),
    ) { "$field violates its closed text bound" }
}

private fun ByteArray.toLowerHexV1(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }

private fun requireNonzero32V1(value: ByteArray, field: String) {
    require(value.size == 32 && value.any { it != 0.toByte() }) {
        "$field must contain exactly 32 non-zero bytes"
    }
}
