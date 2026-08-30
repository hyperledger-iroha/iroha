package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.ByteBuffer
import java.security.SecureRandom
import java.util.Arrays
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver

/**
 * Opaque signer for one native-built Iroha query.
 *
 * The callback receives only the exact 32-byte digest produced by the ABI-22 native codec. A
 * KeyMint/Keystore implementation can sign with a non-exportable handle; this API has no private
 * key input or output.
 */
fun interface IrohaQuerySignatureProvider {
    /**
     * Return the canonical raw Iroha signature payload (for example raw Ed25519 or low-S
     * secp256k1 `r || s`, never DER) over [nativeQueryDigest].
     */
    fun signQueryDigest(nativeQueryDigest: ByteArray): ByteArray
}

/**
 * Native-checked terminal rejection from an authenticated Torii committed-state query.
 *
 * The projection verifies the external transaction signature and every hash/result/proof-index
 * binding carried by this endpoint. The response has no signed block header/finality certificate,
 * so TLS authenticates Torii; independent finality requires separately verifying the exact block.
 */
data class AuthenticatedCommittedRejectionV1(
    val transactionHashHex: String,
    val transactionAuthorityAccountId: String,
    val blockHashHex: String,
    val resultHashHex: String,
    val rejectionCode: String,
    val rejectionMessage: String,
    val committedBlockHeight: Long,
) {
    init {
        requireHash(transactionHashHex, "transactionHashHex")
        requireText(transactionAuthorityAccountId, "transactionAuthorityAccountId", 16 * 1024)
        requireHash(blockHashHex, "blockHashHex")
        requireHash(resultHashHex, "resultHashHex")
        requireText(rejectionCode, "rejectionCode", 128)
        require(rejectionCode in REJECTION_CODES_V1) {
            "rejectionCode is not one of the six ABI-22 terminal rejection kinds"
        }
        requireText(rejectionMessage, "rejectionMessage", 1_024)
        require(committedBlockHeight > 0) { "committedBlockHeight must be positive" }
    }

    private fun requireHash(value: String, field: String) {
        require(value.length == 64 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$field must be an exact lowercase 32-byte hash"
        }
    }

    private fun requireText(value: String, field: String, maximumUTF8Bytes: Int) {
        require(
            value.isNotEmpty() &&
                value.toByteArray(Charsets.UTF_8).size <= maximumUTF8Bytes &&
                value == value.trim() &&
                value.none(Char::isISOControl),
        ) { "$field violates its closed text bound" }
    }

    private companion object {
        val REJECTION_CODES_V1 = setOf(
            "account_does_not_exist",
            "limit_check",
            "validation",
            "instruction_execution",
            "ivm_execution",
            "trigger_execution",
        )
    }
}

/**
 * Additive authority-split committed rejection. [queryAuthorityAccountId] signed the one-shot
 * FindTransactions query; [transactionAuthorityAccountId] signed the exact committed transaction.
 */
data class AuthenticatedCommittedRejectionV2(
    val transactionHashHex: String,
    val queryAuthorityAccountId: String,
    val transactionAuthorityAccountId: String,
    val blockHashHex: String,
    val resultHashHex: String,
    val rejectionCode: String,
    val rejectionMessage: String,
    val committedBlockHeight: Long,
) {
    init {
        // Reuse the V1 closed result contract for every transaction-bound field.
        AuthenticatedCommittedRejectionV1(
            transactionHashHex,
            transactionAuthorityAccountId,
            blockHashHex,
            resultHashHex,
            rejectionCode,
            rejectionMessage,
            committedBlockHeight,
        )
        require(
            queryAuthorityAccountId.isNotEmpty() &&
                queryAuthorityAccountId.toByteArray(Charsets.UTF_8).size <= 16 * 1024 &&
                queryAuthorityAccountId == queryAuthorityAccountId.trim() &&
                queryAuthorityAccountId.none(Char::isISOControl),
        ) { "queryAuthorityAccountId must be canonical non-empty text" }
    }
}

/**
 * Native-verified success or rejection from an authenticated committed-state query.
 *
 * This authenticates Torii's committed-state answer; it is not a signed block or QC and does not
 * independently prove finality. Independent finality requires exact-block verification.
 */
data class AuthenticatedCommittedTransactionResultV1(
    val transactionHashHex: String,
    val transactionAuthorityAccountId: String,
    val blockHashHex: String,
    val resultHashHex: String,
    val resultOk: Boolean,
    val rejectionMessage: String?,
    val committedBlockHeight: BigInteger,
) {
    init {
        require(transactionHashHex.matches(Regex("[0-9a-f]{64}"))) {
            "transactionHashHex must be an exact lowercase 32-byte hash"
        }
        require(blockHashHex.matches(Regex("[0-9a-f]{64}"))) {
            "blockHashHex must be an exact lowercase 32-byte hash"
        }
        require(resultHashHex.matches(Regex("[0-9a-f]{64}"))) {
            "resultHashHex must be an exact lowercase 32-byte hash"
        }
        require(
            transactionAuthorityAccountId.isNotEmpty() &&
                transactionAuthorityAccountId.toByteArray(Charsets.UTF_8).size <= 16 * 1024 &&
                transactionAuthorityAccountId == transactionAuthorityAccountId.trim() &&
                transactionAuthorityAccountId.none(Char::isISOControl),
        ) { "transactionAuthorityAccountId must be canonical non-empty text" }
        require(committedBlockHeight.signum() > 0 && committedBlockHeight.bitLength() <= 64) {
            "committedBlockHeight must be a positive u64"
        }
        if (resultOk) {
            require(rejectionMessage == null) {
                "successful committed results must not carry a rejection message"
            }
        } else {
            require(
                rejectionMessage != null &&
                    rejectionMessage.isNotEmpty() &&
                    rejectionMessage.toByteArray(Charsets.UTF_8).size <= 1_024 &&
                    rejectionMessage == rejectionMessage.trim() &&
                    rejectionMessage.none(Char::isISOControl),
            ) { "rejected committed results require one canonical non-empty message" }
        }
    }
}

/**
 * Authority-split form of [AuthenticatedCommittedTransactionResultV1]. The signed query authority
 * is reported independently from the native-verified authority of the committed transaction.
 */
data class AuthenticatedCommittedTransactionResultV2(
    val transactionHashHex: String,
    val queryAuthorityAccountId: String,
    val transactionAuthorityAccountId: String,
    val blockHashHex: String,
    val resultHashHex: String,
    val resultOk: Boolean,
    val rejectionMessage: String?,
    val committedBlockHeight: BigInteger,
) {
    init {
        AuthenticatedCommittedTransactionResultV1(
            transactionHashHex,
            transactionAuthorityAccountId,
            blockHashHex,
            resultHashHex,
            resultOk,
            rejectionMessage,
            committedBlockHeight,
        )
        require(
            queryAuthorityAccountId.isNotEmpty() &&
                queryAuthorityAccountId.toByteArray(Charsets.UTF_8).size <= 16 * 1024 &&
                queryAuthorityAccountId == queryAuthorityAccountId.trim() &&
                queryAuthorityAccountId.none(Char::isISOControl),
        ) { "queryAuthorityAccountId must be canonical non-empty text" }
    }
}

/** Application-persisted Sumeragi-v2 finality checkpoint verified by ABI-22 native code. */
class AuthenticatedFinalityCheckpointV1(
    private val exactHeight: Long,
    heightContextId: ByteArray,
) {
    private val exactHeightContextId = heightContextId.copyOf()

    init {
        require(exactHeight > 0) { "height must be positive" }
        require(
            exactHeightContextId.size == CONTEXT_ID_BYTES &&
                (exactHeightContextId.last().toInt() and 1) == 1,
        ) { "heightContextId must contain one exact marked 32-byte Iroha hash" }
    }

    fun height(): Long = exactHeight

    fun heightContextId(): ByteArray = exactHeightContextId.copyOf()

    /** Exact persistence form: positive u64 big-endian followed by the marked context id. */
    fun projectionBytes(): ByteArray = ByteBuffer.allocate(PROJECTION_BYTES)
        .putLong(exactHeight)
        .put(exactHeightContextId)
        .array()

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is AuthenticatedFinalityCheckpointV1 &&
            exactHeight == other.exactHeight &&
            exactHeightContextId.contentEquals(other.exactHeightContextId)

    override fun hashCode(): Int = 31 * exactHeight.hashCode() + exactHeightContextId.contentHashCode()

    companion object {
        const val CONTEXT_ID_BYTES: Int = 32
        const val PROJECTION_BYTES: Int = 8 + CONTEXT_ID_BYTES

        internal fun fromProjection(projection: ByteArray): AuthenticatedFinalityCheckpointV1 {
            check(projection.size == PROJECTION_BYTES) {
                "native finality checkpoint projection has invalid shape"
            }
            val buffer = ByteBuffer.wrap(projection.copyOf())
            val height = buffer.long
            val contextId = ByteArray(CONTEXT_ID_BYTES)
            buffer.get(contextId)
            return AuthenticatedFinalityCheckpointV1(height, contextId)
        }
    }
}

/** Native-canonical, content-addressed page of contiguous bridge finality proofs. */
class AuthenticatedFinalityProofPageV1 internal constructor(
    evidenceArchive: ByteArray,
    private val exactHashHex: String,
) {
    private val exactEvidenceArchive = evidenceArchive.copyOf()

    init {
        require(
            exactEvidenceArchive.isNotEmpty() &&
                exactEvidenceArchive.size.toLong() <=
                AuthenticatedTransactionDetailsNativeBridge.FINALITY_PAGE_MAX_BYTES,
        ) { "evidenceArchive violates its closed byte bound" }
        requireHash(exactHashHex, "hashHex")
    }

    fun evidenceArchive(): ByteArray = exactEvidenceArchive.copyOf()

    fun hashHex(): String = exactHashHex

    companion object {
        internal fun requireHash(value: String?, field: String) {
            require(
                value != null &&
                    value.matches(Regex("[0-9a-f]{64}")) &&
                    (Character.digit(value.last(), 16) and 1) == 1,
            ) {
                "$field must be an exact lowercase marked 32-byte Iroha hash"
            }
        }
    }
}

/**
 * Signed query and structurally verified transaction-details bytes. Hints are routing-only until
 * [AuthenticatedTransactionDetailsNativeBridge.projectFinalizedKagemushaOutcomeV1] succeeds.
 */
class AuthenticatedTransactionDetailsCarrierV2 internal constructor(
    internal val signedQuery: AuthenticatedTransactionDetailsNativeBridge.SignedQueryV2,
    responseNorito: ByteArray,
    private val heightHint: Long,
    private val okHint: Boolean,
) {
    private val exactResponseNorito = responseNorito.copyOf()

    init {
        require(exactResponseNorito.isNotEmpty()) {
            "transaction-details carrier response must be nonempty"
        }
        require(heightHint > 0) { "committedBlockHeightHint must be positive" }
    }

    fun responseNorito(): ByteArray = exactResponseNorito.copyOf()

    /** Untrusted routing hint; never consume or release value from this field. */
    fun committedBlockHeightHint(): Long = heightHint

    /** Untrusted routing hint; never consume or release value from this field. */
    fun resultOkHint(): Boolean = okHint
}

/** Exact Kagemusha issuer result independently authenticated by validator finality evidence. */
class AuthenticatedFinalizedKagemushaOutcomeV1 internal constructor(
    private val exactTerminalState: TerminalState,
    operationId: ByteArray,
    private val exactOperationKind: String,
    private val exactTransactionHashHex: String,
    private val exactQueryAuthorityAccountId: String,
    private val exactTransactionAuthorityAccountId: String,
    private val exactBlockHashHex: String,
    private val exactResultHashHex: String,
    private val exactCommittedBlockHeight: Long,
    private val exactFinalizedCheckpoint: AuthenticatedFinalityCheckpointV1,
    private val exactExecutedBlockWireHashHex: String,
    private val exactRejectionCode: String?,
    private val exactRejectionMessage: String?,
    private val exactEvidenceIdHex: String,
    private val exactTransactionDetailsHashHex: String,
    private val exactFinalityPageHashHex: String,
) {
    enum class TerminalState { APPLIED, REJECTED }

    private val exactOperationId = operationId.copyOf()

    init {
        require(exactOperationId.size == 32 && exactOperationId.any { it != 0.toByte() }) {
            "operationId must contain exactly 32 nonzero bytes"
        }
        require(exactOperationKind == "top_up" || exactOperationKind == "redeem") {
            "operationKind must be top_up or redeem"
        }
        listOf(
            "transactionHashHex" to exactTransactionHashHex,
            "blockHashHex" to exactBlockHashHex,
            "resultHashHex" to exactResultHashHex,
            "executedBlockWireHashHex" to exactExecutedBlockWireHashHex,
            "evidenceIdHex" to exactEvidenceIdHex,
            "transactionDetailsHashHex" to exactTransactionDetailsHashHex,
            "finalityPageHashHex" to exactFinalityPageHashHex,
        ).forEach { (field, value) -> AuthenticatedFinalityProofPageV1.requireHash(value, field) }
        requireText(exactQueryAuthorityAccountId, "queryAuthorityAccountId", 16 * 1024)
        requireText(exactTransactionAuthorityAccountId, "transactionAuthorityAccountId", 16 * 1024)
        require(
            exactCommittedBlockHeight > 0 &&
                exactFinalizedCheckpoint.height() == exactCommittedBlockHeight,
        ) { "finalized checkpoint must equal committedBlockHeight" }
        if (exactTerminalState == TerminalState.APPLIED) {
            require(exactRejectionCode == null && exactRejectionMessage == null) {
                "APPLIED outcome must not carry rejection fields"
            }
        } else {
            require(exactRejectionCode in REJECTION_CODES_V1) {
                "rejectionCode is not one of the six ABI-22 terminal rejection kinds"
            }
            requireText(exactRejectionCode, "rejectionCode", 128)
            requireText(exactRejectionMessage, "rejectionMessage", 1_024)
        }
    }

    fun terminalState(): TerminalState = exactTerminalState
    fun operationId(): ByteArray = exactOperationId.copyOf()
    fun operationKind(): String = exactOperationKind
    fun transactionHashHex(): String = exactTransactionHashHex
    fun queryAuthorityAccountId(): String = exactQueryAuthorityAccountId
    fun transactionAuthorityAccountId(): String = exactTransactionAuthorityAccountId
    fun blockHashHex(): String = exactBlockHashHex
    fun resultHashHex(): String = exactResultHashHex
    fun committedBlockHeight(): Long = exactCommittedBlockHeight
    fun finalizedCheckpoint(): AuthenticatedFinalityCheckpointV1 = exactFinalizedCheckpoint
    fun executedBlockWireHashHex(): String = exactExecutedBlockWireHashHex
    fun rejectionCode(): String? = exactRejectionCode
    fun rejectionMessage(): String? = exactRejectionMessage
    fun evidenceIdHex(): String = exactEvidenceIdHex
    fun transactionDetailsHashHex(): String = exactTransactionDetailsHashHex
    fun finalityPageHashHex(): String = exactFinalityPageHashHex

    private fun requireText(value: String?, field: String, maximumUtf8Bytes: Int) {
        require(
            value != null &&
                value.isNotEmpty() &&
                value.toByteArray(Charsets.UTF_8).size <= maximumUtf8Bytes &&
                value == value.trim() &&
                value.none(Char::isISOControl),
        ) { "$field violates its closed text bound" }
    }

    private companion object {
        val REJECTION_CODES_V1 = setOf(
            "account_does_not_exist",
            "limit_check",
            "validation",
            "instruction_execution",
            "ivm_execution",
            "trigger_execution",
        )
    }
}

/** ABI-22 native codec and verifier for authenticated committed-transaction lookup. */
class AuthenticatedTransactionDetailsNativeBridge private constructor() {
    /** Native-bound signed body plus the private preparation needed to verify its response. */
    class SignedQueryV1 internal constructor(
        preparation: ByteArray,
        requestBody: ByteArray,
    ) {
        private val nativePreparation = preparation.copyOf()
        private val canonicalRequestBody = requestBody.copyOf()

        /** Canonical versioned `SignedQuery` bytes for `/v1/pipeline/transactions/details`. */
        fun requestBody(): ByteArray = canonicalRequestBody.copyOf()

        internal fun preparation(): ByteArray = nativePreparation.copyOf()
    }

    /** Native-bound authority-split signed body and response-verification preparation. */
    class SignedQueryV2 internal constructor(
        preparation: ByteArray,
        requestBody: ByteArray,
    ) {
        private val nativePreparation = preparation.copyOf()
        private val canonicalRequestBody = requestBody.copyOf()

        fun requestBody(): ByteArray = canonicalRequestBody.copyOf()

        internal fun preparation(): ByteArray = nativePreparation.copyOf()
    }

    companion object {
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 22
        const val RESPONSE_MAX_BYTES: Long = 64L * 1024L * 1024L
        const val FINALITY_PAGE_MAX_PROOFS: Int = 64
        const val FINALITY_PROOF_MAX_BYTES: Long = 9L * 1024L * 1024L
        const val FINALITY_PAGE_MAX_BYTES: Long = 64L * 1024L * 1024L
        const val EXECUTED_BLOCK_WIRE_MAX_BYTES: Long = 32L * 1024L * 1024L
        private const val DIGEST_BYTES = 32
        private const val NONCE_BYTES = 32
        private const val PREPARATION_MAX_BYTES = 64 * 1024
        private const val SIGNATURE_MAX_BYTES = 16 * 1024
        private const val SIGNED_QUERY_MAX_BYTES = 64 * 1024
        private val nonceRandom = SecureRandom()

        private val nativeLoadResult: Result<Unit> by lazy {
            runCatching {
                System.loadLibrary("connect_norito_bridge")
                val actual = nativeBridgeAbiVersion()
                check(actual == REQUIRED_BRIDGE_ABI_VERSION) {
                    "native authenticated query ABI mismatch: expected " +
                        "$REQUIRED_BRIDGE_ABI_VERSION, found $actual"
                }
            }
        }

        /**
         * Build and sign one fresh exact-hash `FindTransactions` request.
         *
         * [signer] receives only a defensive copy of the native 32-byte digest. Native code checks
         * its detached signature against the public key embedded in [authorityAccountId].
         */
        @JvmStatic
        fun buildSignedRejectedTransactionQueryV1(
            transactionHashHex: String,
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
                "secure query nonce generator repeatedly returned zero"
            }
            return buildSignedRejectedTransactionQueryAtV1(
                transactionHashHex,
                networkId,
                authorityAccountId,
                signer,
                System.currentTimeMillis(),
                nonce,
            )
        }

        /** Build the same exact-hash query without assuming its committed result. */
        @JvmStatic
        fun buildSignedTransactionDetailsQueryV1(
            transactionHashHex: String,
            networkId: NetworkId,
            authorityAccountId: String,
            signer: IrohaQuerySignatureProvider,
        ): SignedQueryV1 = buildSignedRejectedTransactionQueryV1(
            transactionHashHex,
            networkId,
            authorityAccountId,
            signer,
        )

        internal fun buildSignedRejectedTransactionQueryAtV1(
            transactionHashHex: String,
            networkId: NetworkId,
            authorityAccountId: String,
            signer: IrohaQuerySignatureProvider,
            creationTimeMs: Long,
            nonce: ByteArray,
        ): SignedQueryV1 {
            requireNative()
            require(nonce.size == NONCE_BYTES && nonce.any { it != 0.toByte() }) {
                "nonce must contain exactly 32 nonzero bytes"
            }
            val prepared = nativePrepareExactRejectedTransactionQueryV1(
                networkId.bytes(),
                authorityAccountId.toByteArray(Charsets.UTF_8),
                transactionHashHex.toByteArray(Charsets.UTF_8),
                creationTimeMs,
                nonce.copyOf(),
            )
            check(
                prepared.size == 2 &&
                    prepared[0].isNotEmpty() &&
                    prepared[0].size <= PREPARATION_MAX_BYTES &&
                    prepared[1].size == DIGEST_BYTES,
            ) { "native query preparation returned an invalid shape" }
            val digest = prepared[1].copyOf()
            val signature = try {
                signer.signQueryDigest(digest.copyOf())
            } finally {
                Arrays.fill(digest, 0.toByte())
                Arrays.fill(prepared[1], 0.toByte())
            }
            require(signature.isNotEmpty() && signature.size <= SIGNATURE_MAX_BYTES) {
                "opaque query signer returned invalid signature bytes"
            }
            val requestBody = nativeFinalizeExactRejectedTransactionQueryV1(
                prepared[0].copyOf(),
                signature.copyOf(),
            )
            check(requestBody.isNotEmpty() && requestBody.size <= SIGNED_QUERY_MAX_BYTES) {
                "native query finalizer violated the request byte bound"
            }
            return SignedQueryV1(prepared[0], requestBody)
        }

        /**
         * Build a fresh exact-hash query whose signer and expected transaction signer are
         * independently native-bound. This does not weaken or reinterpret the V1 same-authority
         * contract.
         */
        @JvmStatic
        fun buildSignedTransactionDetailsQueryV2(
            transactionHashHex: String,
            networkId: NetworkId,
            queryAuthorityAccountId: String,
            expectedTransactionAuthorityAccountId: String,
            signer: IrohaQuerySignatureProvider,
        ): SignedQueryV2 {
            val nonce = ByteArray(NONCE_BYTES)
            repeat(16) {
                if (!nonce.all { byte -> byte == 0.toByte() }) return@repeat
                nonceRandom.nextBytes(nonce)
            }
            check(!nonce.all { it == 0.toByte() }) {
                "secure query nonce generator repeatedly returned zero"
            }
            return buildSignedTransactionDetailsQueryAtV2(
                transactionHashHex,
                networkId,
                queryAuthorityAccountId,
                expectedTransactionAuthorityAccountId,
                signer,
                System.currentTimeMillis(),
                nonce,
            )
        }

        internal fun buildSignedTransactionDetailsQueryAtV2(
            transactionHashHex: String,
            networkId: NetworkId,
            queryAuthorityAccountId: String,
            expectedTransactionAuthorityAccountId: String,
            signer: IrohaQuerySignatureProvider,
            creationTimeMs: Long,
            nonce: ByteArray,
        ): SignedQueryV2 {
            requireNative()
            require(nonce.size == NONCE_BYTES && nonce.any { it != 0.toByte() }) {
                "nonce must contain exactly 32 nonzero bytes"
            }
            val prepared = nativePrepareExactTransactionQueryV2(
                networkId.bytes(),
                queryAuthorityAccountId.toByteArray(Charsets.UTF_8),
                expectedTransactionAuthorityAccountId.toByteArray(Charsets.UTF_8),
                transactionHashHex.toByteArray(Charsets.UTF_8),
                creationTimeMs,
                nonce.copyOf(),
            )
            check(
                prepared.size == 2 && prepared[0].isNotEmpty() &&
                    prepared[0].size <= PREPARATION_MAX_BYTES &&
                    prepared[1].size == DIGEST_BYTES,
            ) { "native V2 query preparation returned an invalid shape" }
            val digest = prepared[1].copyOf()
            val signature = try {
                signer.signQueryDigest(digest.copyOf())
            } finally {
                Arrays.fill(digest, 0.toByte())
                Arrays.fill(prepared[1], 0.toByte())
            }
            require(signature.isNotEmpty() && signature.size <= SIGNATURE_MAX_BYTES) {
                "opaque query signer returned invalid signature bytes"
            }
            val requestBody = nativeFinalizeExactTransactionQueryV2(
                prepared[0].copyOf(),
                signature.copyOf(),
            )
            check(requestBody.isNotEmpty() && requestBody.size <= SIGNED_QUERY_MAX_BYTES) {
                "native V2 query finalizer violated the request byte bound"
            }
            return SignedQueryV2(prepared[0], requestBody)
        }

        /** Natively verify the authority-split exact committed rejection. */
        @JvmStatic
        fun projectCommittedRejectionV2(
            signedQuery: SignedQueryV2,
            responseNorito: ByteArray,
        ): AuthenticatedCommittedRejectionV2 = projectCommittedRejectionFieldsV2(
            nativeProjectExactCommittedRejectionV2(
                signedQuery.preparation(),
                boundedResponseV2(responseNorito),
            ),
        )

        /**
         * Additionally require exactly one signed Kagemusha instruction carrying the retained
         * canonical request, operation id, and kind.
         */
        @JvmStatic
        fun projectKagemushaCommittedRejectionV2(
            signedQuery: SignedQueryV2,
            responseNorito: ByteArray,
            expectedOperationId: ByteArray,
            expectedKind: String,
            expectedRequestNorito: ByteArray,
        ): AuthenticatedCommittedRejectionV2 = projectCommittedRejectionFieldsV2(
            nativeProjectExactKagemushaCommittedRejectionV2(
                signedQuery.preparation(),
                boundedResponseV2(responseNorito),
                expectedOperationId.copyOf(),
                expectedKind.toByteArray(Charsets.UTF_8),
                expectedRequestNorito.copyOf(),
            ),
        )

        /** Natively verify and project either authority-split committed success or rejection. */
        @JvmStatic
        fun projectCommittedTransactionResultV2(
            signedQuery: SignedQueryV2,
            responseNorito: ByteArray,
        ): AuthenticatedCommittedTransactionResultV2 {
            val fields = nativeProjectExactCommittedTransactionResultV2(
                signedQuery.preparation(),
                boundedResponseV2(responseNorito),
            )
            check(fields.size == 8) {
                "native committed transaction-result V2 projection has invalid shape"
            }
            val resultOk = when (exactUtf8(fields[5], "resultOk")) {
                "true" -> true
                "false" -> false
                else -> error("native committed result flag is invalid")
            }
            val reasonText = exactUtf8AllowEmpty(fields[6], "rejectionMessage")
            val height = exactPositiveDecimal(fields[7], "committedBlockHeight")
                .toBigIntegerOrNull()
                ?: error("native committed block height is invalid")
            return AuthenticatedCommittedTransactionResultV2(
                transactionHashHex = exactUtf8(fields[0], "transactionHashHex"),
                queryAuthorityAccountId = exactUtf8(fields[1], "queryAuthorityAccountId"),
                transactionAuthorityAccountId = exactUtf8(
                    fields[2],
                    "transactionAuthorityAccountId",
                ),
                blockHashHex = exactUtf8(fields[3], "blockHashHex"),
                resultHashHex = exactUtf8(fields[4], "resultHashHex"),
                resultOk = resultOk,
                rejectionMessage = reasonText.ifEmpty { null },
                committedBlockHeight = height,
            )
        }

        /** Bind exact Torii proof bodies into one canonical content-addressed page archive. */
        @JvmStatic
        fun bindFinalityProofPageV1(
            finalityProofsNorito: Array<ByteArray>,
        ): AuthenticatedFinalityProofPageV1 {
            requireNative()
            require(
                finalityProofsNorito.isNotEmpty() &&
                    finalityProofsNorito.size <= FINALITY_PAGE_MAX_PROOFS,
            ) { "finalityProofsNorito must contain 1..64 proofs" }
            var aggregate = 0L
            val copies = Array(finalityProofsNorito.size) { index ->
                val proof = finalityProofsNorito[index]
                require(proof.isNotEmpty() && proof.size.toLong() <= FINALITY_PROOF_MAX_BYTES) {
                    "finalityProofsNorito[$index] violates its closed byte bound"
                }
                aggregate += proof.size.toLong()
                require(aggregate <= FINALITY_PAGE_MAX_BYTES) {
                    "finalityProofsNorito exceeds its aggregate byte bound"
                }
                proof.copyOf()
            }
            val fields = nativeBindFinalityProofPageV1(copies)
            check(fields.size == 2) { "native finality page binding returned an invalid shape" }
            val hashHex = exactUtf8(fields[1], "finalityPageHashHex")
            val page = AuthenticatedFinalityProofPageV1(fields[0], hashHex)
            check(hashHex == IrohaHash.prehash(page.evidenceArchive()).toLowerHexV1()) {
                "native finality page hash differs from its exact archive"
            }
            return page
        }

        /** Verify one bounded contiguous finality page from a persisted checkpoint. */
        @JvmStatic
        fun verifyFinalityPageV1(
            networkId: NetworkId,
            trustedCheckpoint: AuthenticatedFinalityCheckpointV1,
            page: AuthenticatedFinalityProofPageV1,
        ): AuthenticatedFinalityCheckpointV1 = AuthenticatedFinalityCheckpointV1.fromProjection(
            nativeVerifyFinalityPageV1(
                networkId.bytes(),
                trustedCheckpoint.height(),
                trustedCheckpoint.heightContextId(),
                page.evidenceArchive(),
            ),
        )

        /** Convenience overload which first creates the canonical page. */
        @JvmStatic
        fun verifyFinalityPageV1(
            networkId: NetworkId,
            trustedCheckpoint: AuthenticatedFinalityCheckpointV1,
            finalityProofsNorito: Array<ByteArray>,
        ): AuthenticatedFinalityCheckpointV1 = verifyFinalityPageV1(
            networkId,
            trustedCheckpoint,
            bindFinalityProofPageV1(finalityProofsNorito),
        )

        /** Bind a native-checked response to its private query preparation for later finality. */
        @JvmStatic
        fun bindTransactionDetailsCarrierV2(
            signedQuery: SignedQueryV2,
            responseNorito: ByteArray,
        ): AuthenticatedTransactionDetailsCarrierV2 {
            val exactResponse = boundedResponseV2(responseNorito)
            val hint = projectCommittedTransactionResultV2(signedQuery, exactResponse)
            val height = try {
                hint.committedBlockHeight.longValueExact()
            } catch (error: ArithmeticException) {
                throw IllegalStateException(
                    "committedBlockHeightHint exceeds the mobile u63 range",
                    error,
                )
            }
            return AuthenticatedTransactionDetailsCarrierV2(
                signedQuery,
                exactResponse,
                height,
                hint.resultOk,
            )
        }

        /** Verify one exact Kagemusha issuer outcome against validator finality evidence. */
        @JvmStatic
        fun projectFinalizedKagemushaOutcomeV1(
            carrier: AuthenticatedTransactionDetailsCarrierV2,
            expectedOperationId: ByteArray,
            expectedKind: String,
            expectedRequestNorito: ByteArray,
            networkId: NetworkId,
            trustedCheckpoint: AuthenticatedFinalityCheckpointV1,
            finalityPage: AuthenticatedFinalityProofPageV1,
            executedBlockWire: ByteArray,
        ): AuthenticatedFinalizedKagemushaOutcomeV1 {
            requireNative()
            require(
                executedBlockWire.isNotEmpty() &&
                    executedBlockWire.size.toLong() <= EXECUTED_BLOCK_WIRE_MAX_BYTES,
            ) { "executedBlockWire violates its closed byte bound" }
            val fields = nativeProjectFinalizedKagemushaOutcomeV1(
                carrier.signedQuery.preparation(),
                carrier.responseNorito(),
                expectedOperationId.copyOf(),
                expectedKind.toByteArray(Charsets.UTF_8),
                expectedRequestNorito.copyOf(),
                networkId.bytes(),
                trustedCheckpoint.height(),
                trustedCheckpoint.heightContextId(),
                finalityPage.evidenceArchive(),
                executedBlockWire.copyOf(),
            )
            check(fields.size == 16) { "native finalized Kagemusha outcome has invalid shape" }
            val terminalState = when (exactUtf8(fields[0], "terminalState")) {
                "applied" -> AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED
                "rejected" -> AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.REJECTED
                else -> error("native finalized terminal state is invalid")
            }
            val height = exactPositiveDecimal(fields[8], "committedBlockHeight").toLongOrNull()
                ?: error("native finalized block height is invalid")
            val rejectionCode = exactUtf8AllowEmpty(fields[11], "rejectionCode").ifEmpty { null }
            val rejectionMessage = exactUtf8AllowEmpty(
                fields[12],
                "rejectionMessage",
            ).ifEmpty { null }
            val outcome = AuthenticatedFinalizedKagemushaOutcomeV1(
                terminalState,
                fields[1],
                exactUtf8(fields[2], "operationKind"),
                exactUtf8(fields[3], "transactionHashHex"),
                exactUtf8(fields[4], "queryAuthorityAccountId"),
                exactUtf8(fields[5], "transactionAuthorityAccountId"),
                exactUtf8(fields[6], "blockHashHex"),
                exactUtf8(fields[7], "resultHashHex"),
                height,
                AuthenticatedFinalityCheckpointV1.fromProjection(fields[9]),
                exactUtf8(fields[10], "executedBlockWireHashHex"),
                rejectionCode,
                rejectionMessage,
                exactUtf8(fields[13], "evidenceIdHex"),
                exactUtf8(fields[14], "transactionDetailsHashHex"),
                exactUtf8(fields[15], "finalityPageHashHex"),
            )
            requireCarrierRoutingHintsAgreeV1(
                carrier.committedBlockHeightHint(),
                carrier.resultOkHint(),
                outcome,
            )
            check(
                outcome.finalityPageHashHex() == finalityPage.hashHex() &&
                    outcome.transactionDetailsHashHex() ==
                    IrohaHash.prehash(carrier.responseNorito()).toLowerHexV1() &&
                    outcome.executedBlockWireHashHex() ==
                    IrohaHash.prehash(executedBlockWire).toLowerHexV1(),
            ) { "native finalized evidence content hashes are inconsistent" }
            return outcome
        }

        /** Convenience overload which first creates the canonical page. */
        @JvmStatic
        fun projectFinalizedKagemushaOutcomeV1(
            carrier: AuthenticatedTransactionDetailsCarrierV2,
            expectedOperationId: ByteArray,
            expectedKind: String,
            expectedRequestNorito: ByteArray,
            networkId: NetworkId,
            trustedCheckpoint: AuthenticatedFinalityCheckpointV1,
            finalityProofsNorito: Array<ByteArray>,
            executedBlockWire: ByteArray,
        ): AuthenticatedFinalizedKagemushaOutcomeV1 = projectFinalizedKagemushaOutcomeV1(
            carrier,
            expectedOperationId,
            expectedKind,
            expectedRequestNorito,
            networkId,
            trustedCheckpoint,
            bindFinalityProofPageV1(finalityProofsNorito),
            executedBlockWire,
        )

        /** Require uniform and specialized top-up verifiers to authenticate one exact block. */
        @JvmStatic
        fun requireKagemushaTopUpFinalityAgreementV1(
            outcome: AuthenticatedFinalizedKagemushaOutcomeV1,
            specialized: KagemushaRecursiveSpendProver.VerifiedTopUpFinalityV4,
        ) {
            require(
                outcome.terminalState() ==
                    AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED &&
                    outcome.operationKind() == "top_up" &&
                    outcome.operationId().contentEquals(specialized.operationId()) &&
                    outcome.transactionHashHex() == specialized.transactionHashHex() &&
                    outcome.committedBlockHeight() == specialized.height() &&
                    outcome.blockHashHex() == specialized.blockHashHex() &&
                    outcome.finalizedCheckpoint().heightContextId()
                        .contentEquals(specialized.heightContextId()),
            ) { "uniform and specialized Kagemusha top-up finality evidence disagree" }
        }

        internal fun requireCarrierRoutingHintsAgreeV1(
            committedBlockHeightHint: Long,
            resultOkHint: Boolean,
            outcome: AuthenticatedFinalizedKagemushaOutcomeV1,
        ) {
            require(
                committedBlockHeightHint == outcome.committedBlockHeight() &&
                    resultOkHint ==
                    (outcome.terminalState() ==
                        AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED),
            ) { "transaction-details routing hints disagree with finalized native evidence" }
        }

        private fun boundedResponseV2(responseNorito: ByteArray): ByteArray {
            require(responseNorito.isNotEmpty() && responseNorito.size.toLong() <= RESPONSE_MAX_BYTES) {
                "responseNorito violates its closed byte bound"
            }
            return responseNorito.copyOf()
        }

        private fun projectCommittedRejectionFieldsV2(
            fields: Array<ByteArray>,
        ): AuthenticatedCommittedRejectionV2 {
            check(fields.size == 8) { "native committed rejection V2 projection has invalid shape" }
            val height = exactPositiveDecimal(fields[7], "committedBlockHeight").toLongOrNull()
                ?: error("native committed block height is invalid")
            return AuthenticatedCommittedRejectionV2(
                transactionHashHex = exactUtf8(fields[0], "transactionHashHex"),
                queryAuthorityAccountId = exactUtf8(fields[1], "queryAuthorityAccountId"),
                transactionAuthorityAccountId = exactUtf8(
                    fields[2],
                    "transactionAuthorityAccountId",
                ),
                blockHashHex = exactUtf8(fields[3], "blockHashHex"),
                resultHashHex = exactUtf8(fields[4], "resultHashHex"),
                rejectionCode = exactUtf8(fields[5], "rejectionCode"),
                rejectionMessage = exactUtf8(fields[6], "rejectionMessage"),
                committedBlockHeight = height,
            )
        }

        /** Natively verify and project the committed rejection bound to [signedQuery]. */
        @JvmStatic
        fun projectCommittedRejectionV1(
            signedQuery: SignedQueryV1,
            responseNorito: ByteArray,
        ): AuthenticatedCommittedRejectionV1 {
            requireNative()
            require(responseNorito.isNotEmpty() && responseNorito.size.toLong() <= RESPONSE_MAX_BYTES) {
                "responseNorito violates its closed byte bound"
            }
            val fields = nativeProjectExactCommittedRejectionV1(
                signedQuery.preparation(),
                responseNorito.copyOf(),
            )
            check(fields.size == 7) {
                "native committed rejection projection has invalid shape"
            }
            val height = exactPositiveDecimal(fields[6], "committedBlockHeight").toLongOrNull()
                ?: error("native committed block height is invalid")
            return AuthenticatedCommittedRejectionV1(
                transactionHashHex = exactUtf8(fields[0], "transactionHashHex"),
                transactionAuthorityAccountId = exactUtf8(
                    fields[1],
                    "transactionAuthorityAccountId",
                ),
                blockHashHex = exactUtf8(fields[2], "blockHashHex"),
                resultHashHex = exactUtf8(fields[3], "resultHashHex"),
                rejectionCode = exactUtf8(fields[4], "rejectionCode"),
                rejectionMessage = exactUtf8(fields[5], "rejectionMessage"),
                committedBlockHeight = height,
            )
        }

        /** Natively verify and project either committed success or rejection. */
        @JvmStatic
        fun projectCommittedTransactionResultV1(
            signedQuery: SignedQueryV1,
            responseNorito: ByteArray,
        ): AuthenticatedCommittedTransactionResultV1 {
            requireNative()
            require(responseNorito.isNotEmpty() && responseNorito.size.toLong() <= RESPONSE_MAX_BYTES) {
                "responseNorito violates its closed byte bound"
            }
            val fields = nativeProjectExactCommittedTransactionResultV1(
                signedQuery.preparation(),
                responseNorito.copyOf(),
            )
            check(fields.size == 7) {
                "native committed transaction-result projection has invalid shape"
            }
            val resultOk = when (exactUtf8(fields[4], "resultOk")) {
                "true" -> true
                "false" -> false
                else -> error("native committed result flag is invalid")
            }
            val reasonText = exactUtf8AllowEmpty(fields[5], "rejectionMessage")
            val height = exactPositiveDecimal(fields[6], "committedBlockHeight").toBigIntegerOrNull()
                ?: error("native committed block height is invalid")
            return AuthenticatedCommittedTransactionResultV1(
                transactionHashHex = exactUtf8(fields[0], "transactionHashHex"),
                transactionAuthorityAccountId = exactUtf8(
                    fields[1],
                    "transactionAuthorityAccountId",
                ),
                blockHashHex = exactUtf8(fields[2], "blockHashHex"),
                resultHashHex = exactUtf8(fields[3], "resultHashHex"),
                resultOk = resultOk,
                rejectionMessage = reasonText.ifEmpty { null },
                committedBlockHeight = height,
            )
        }

        /**
         * Natively verify and project exactly one committed offline-device registration result.
         *
         * Unlike [projectCommittedTransactionResultV1], this requires the authenticated external
         * transaction to contain exactly one `RegisterOfflineDeviceAttestation` instruction and
         * preserves the typed eligibility decision carried by a committed rejection.
         */
        @JvmStatic
        fun projectCommittedOfflineDeviceRegistrationResultV1(
            signedQuery: SignedQueryV1,
            responseNorito: ByteArray,
        ): AuthenticatedOfflineDeviceRegistrationResultV1 {
            requireNative()
            require(responseNorito.isNotEmpty() && responseNorito.size.toLong() <= RESPONSE_MAX_BYTES) {
                "responseNorito violates its closed byte bound"
            }
            val json = nativeProjectExactOfflineDeviceRegistrationResultV1(
                signedQuery.preparation(),
                responseNorito.copyOf(),
            )
            return AuthenticatedOfflineDeviceRegistrationResultV1.parseNativeJson(json)
        }

        private fun exactUtf8(value: ByteArray, field: String): String {
            check(value.isNotEmpty()) { "native $field is empty" }
            val decoded = value.toString(Charsets.UTF_8)
            check(decoded.toByteArray(Charsets.UTF_8).contentEquals(value)) {
                "native $field is not exact UTF-8"
            }
            return decoded
        }

        private fun exactUtf8AllowEmpty(value: ByteArray, field: String): String {
            val decoded = value.toString(Charsets.UTF_8)
            check(decoded.toByteArray(Charsets.UTF_8).contentEquals(value)) {
                "native $field is not exact UTF-8"
            }
            return decoded
        }

        private fun exactPositiveDecimal(value: ByteArray, field: String): String {
            val decoded = exactUtf8(value, field)
            check(decoded.first() in '1'..'9' && decoded.all { it in '0'..'9' }) {
                "native $field is not a positive canonical decimal"
            }
            return decoded
        }

        private fun ByteArray.toLowerHexV1(): String =
            joinToString(separator = "") { byte -> "%02x".format(byte.toInt() and 0xff) }

        private fun requireNative() {
            nativeLoadResult.getOrElse { failure ->
                throw IllegalStateException(
                    "native authenticated transaction-details bridge is unavailable",
                    failure,
                )
            }
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativePrepareExactRejectedTransactionQueryV1(
            networkId: ByteArray,
            authorityAccountId: ByteArray,
            transactionHashHex: ByteArray,
            creationTimeMs: Long,
            nonce: ByteArray,
        ): Array<ByteArray>

        @JvmStatic
        private external fun nativeFinalizeExactRejectedTransactionQueryV1(
            preparation: ByteArray,
            signature: ByteArray,
        ): ByteArray

        @JvmStatic
        private external fun nativeProjectExactCommittedRejectionV1(
            preparation: ByteArray,
            responseNorito: ByteArray,
        ): Array<ByteArray>

        @JvmStatic
        private external fun nativeProjectExactCommittedTransactionResultV1(
            preparation: ByteArray,
            responseNorito: ByteArray,
        ): Array<ByteArray>

        @JvmStatic
        private external fun nativeProjectExactOfflineDeviceRegistrationResultV1(
            preparation: ByteArray,
            responseNorito: ByteArray,
        ): ByteArray

        @JvmStatic
        private external fun nativePrepareExactTransactionQueryV2(
            networkId: ByteArray,
            queryAuthorityAccountId: ByteArray,
            expectedTransactionAuthorityAccountId: ByteArray,
            transactionHashHex: ByteArray,
            creationTimeMs: Long,
            nonce: ByteArray,
        ): Array<ByteArray>

        @JvmStatic
        private external fun nativeFinalizeExactTransactionQueryV2(
            preparation: ByteArray,
            signature: ByteArray,
        ): ByteArray

        @JvmStatic
        private external fun nativeProjectExactCommittedRejectionV2(
            preparation: ByteArray,
            responseNorito: ByteArray,
        ): Array<ByteArray>

        @JvmStatic
        private external fun nativeProjectExactKagemushaCommittedRejectionV2(
            preparation: ByteArray,
            responseNorito: ByteArray,
            expectedOperationId: ByteArray,
            expectedKind: ByteArray,
            expectedRequestNorito: ByteArray,
        ): Array<ByteArray>

        @JvmStatic
        private external fun nativeProjectExactCommittedTransactionResultV2(
            preparation: ByteArray,
            responseNorito: ByteArray,
        ): Array<ByteArray>

        @JvmStatic
        private external fun nativeBindFinalityProofPageV1(
            finalityProofsNorito: Array<ByteArray>,
        ): Array<ByteArray>

        @JvmStatic
        private external fun nativeVerifyFinalityPageV1(
            networkId: ByteArray,
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
            finalityPageArchive: ByteArray,
        ): ByteArray

        @JvmStatic
        private external fun nativeProjectFinalizedKagemushaOutcomeV1(
            preparation: ByteArray,
            responseNorito: ByteArray,
            expectedOperationId: ByteArray,
            expectedKind: ByteArray,
            expectedRequestNorito: ByteArray,
            networkId: ByteArray,
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
            finalityPageArchive: ByteArray,
            executedBlockWire: ByteArray,
        ): Array<ByteArray>
    }
}
