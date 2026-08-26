package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor

/** Canonical signed Offline Cash V1 top-up request retained for exact-byte retries. */
class OfflineCashTopUpRequestV1(canonicalNorito: ByteArray) {
    private val canonical = KagemushaRecursiveSpendProver.decodeTopUpRequest(canonicalNorito)
        .also(KagemushaRecursiveSpendProver::projectTopUpSubmissionRequest)
        .noritoEncoded()

    fun encodeCanonical(): ByteArray = canonical.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashTopUpRequestV1 && canonical.contentEquals(other.canonical)

    override fun hashCode(): Int = canonical.contentHashCode()

    companion object {
        const val MAX_CANONICAL_BYTES: Int =
            KagemushaRecursiveSpendProver.MAX_TORII_TOP_UP_REQUEST_BYTES_V4

        @JvmStatic
        fun decodeCanonical(canonicalNorito: ByteArray): OfflineCashTopUpRequestV1 =
            OfflineCashTopUpRequestV1(canonicalNorito)
    }
}

/** Canonical signed Offline Cash V1 redemption request retained for exact-byte retries. */
class OfflineCashRedeemRequestV1(canonicalNorito: ByteArray) {
    private val canonical = KagemushaRecursiveSpendProver
        .decodeRedeemSubmissionRequest(canonicalNorito)
        .also(KagemushaRecursiveSpendProver::projectRedeemSubmissionRequest)
        .noritoEncoded()

    fun encodeCanonical(): ByteArray = canonical.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashRedeemRequestV1 && canonical.contentEquals(other.canonical)

    override fun hashCode(): Int = canonical.contentHashCode()

    companion object {
        const val MAX_CANONICAL_BYTES: Int =
            KagemushaRecursiveSpendProver.MAX_TORII_REDEEM_REQUEST_BYTES_V4

        @JvmStatic
        fun decodeCanonical(canonicalNorito: ByteArray): OfflineCashRedeemRequestV1 =
            OfflineCashRedeemRequestV1(canonicalNorito)
    }
}

/** Canonical Offline Cash V1 operation reference returned by an accepted command. */
class OfflineCashOperationReferenceV1(canonicalNorito: ByteArray) {
    private val canonical = KagemushaRecursiveSpendProver
        .OperationReference(canonicalNorito)
        .also(KagemushaRecursiveSpendProver::projectOperationReference)
        .noritoEncoded()

    fun encodeCanonical(): ByteArray = canonical.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashOperationReferenceV1 && canonical.contentEquals(other.canonical)

    override fun hashCode(): Int = canonical.contentHashCode()

    companion object {
        const val MAX_CANONICAL_BYTES: Int =
            KagemushaRecursiveSpendProver.MAX_TORII_RESPONSE_BYTES

        @JvmStatic
        fun decodeCanonical(canonicalNorito: ByteArray): OfflineCashOperationReferenceV1 =
            OfflineCashOperationReferenceV1(canonicalNorito)
    }
}

enum class OfflineCashOperationStateV1 {
    PENDING,
    APPLIED,
    REJECTED,
}

enum class OfflineCashOperationKindV1 {
    TOP_UP,
    REDEEM,
}

/** Canonical poll response with an explicitly requested native-backed public projection. */
class OfflineCashOperationStatusV1(canonicalNorito: ByteArray) {
    private val canonical = KagemushaRecursiveSpendProver
        .OperationStatus(canonicalNorito)
        .also(KagemushaRecursiveSpendProver::projectOperationStatus)
        .noritoEncoded()

    fun encodeCanonical(): ByteArray = canonical.copyOf()

    fun project(): OfflineCashOperationStatusProjectionV1 = mapOperationProjectionV1(
        KagemushaRecursiveSpendProver.projectOperationStatus(
            KagemushaRecursiveSpendProver.OperationStatus(canonical),
        ),
    )

    override fun equals(other: Any?): Boolean =
        other is OfflineCashOperationStatusV1 && canonical.contentEquals(other.canonical)

    override fun hashCode(): Int = canonical.contentHashCode()

    companion object {
        const val MAX_CANONICAL_BYTES: Int =
            KagemushaRecursiveSpendProver.MAX_TORII_RESPONSE_BYTES

        @JvmStatic
        fun decodeCanonical(canonicalNorito: ByteArray): OfflineCashOperationStatusV1 =
            OfflineCashOperationStatusV1(canonicalNorito)
    }
}

/** Public typed client for exactly the four first-release Offline Cash V1 Torii routes. */
class OfflineCashToriiClientV1 private constructor(
    baseUri: URI,
    transport: TransportExecutor,
    localSigningContext: LocalSigningContext,
) {
    private val delegate = KagemushaRecursiveSpendProver.newToriiClient(
        baseUri,
        transport,
        localSigningContext,
    )

    companion object {
        const val READINESS_PATH: String =
            KagemushaRecursiveSpendProver.ToriiClient.READINESS_PATH
        const val TOP_UP_PATH: String = KagemushaRecursiveSpendProver.ToriiClient.TOP_UP_PATH
        const val REDEEM_PATH: String = KagemushaRecursiveSpendProver.ToriiClient.REDEEM_PATH
        const val OPERATIONS_PATH: String =
            KagemushaRecursiveSpendProver.ToriiClient.OPERATIONS_PATH
        const val JSON_MEDIA_TYPE: String =
            KagemushaRecursiveSpendProver.ToriiClient.JSON_MEDIA_TYPE
        const val NORITO_MEDIA_TYPE: String =
            KagemushaRecursiveSpendProver.ToriiClient.NORITO_MEDIA_TYPE

        /**
         * Create a client bound to the caller-selected genesis network identity.
         *
         * [localSigningContext] intentionally remains mandatory: the SDK must never infer a
         * network identity from Torii. Top-up and redemption inputs are opaque signed canonical
         * bodies whose public authorization binding, operation id, and genesis network are checked
         * locally; registered-device signature authenticity remains Torii's admission decision.
         * Repeated submissions retain their exact bytes and idempotency key.
         */
        @JvmStatic
        fun create(
            baseUri: URI,
            transport: TransportExecutor,
            localSigningContext: LocalSigningContext,
        ): OfflineCashToriiClientV1 = OfflineCashToriiClientV1(
            baseUri,
            transport,
            localSigningContext,
        )
    }

    fun getReadiness(): CompletableFuture<OfflineCashReadinessV1> =
        delegate.getOfflineCapability().thenApply { status ->
            OfflineCashReadinessV1.fromValidatedProjection(
                status.mandatory,
                status.cashHandoffCapability,
                status.requiredBridgeAbiVersion,
                status.maximumHops,
                status.ready,
                status.blockers.map { blocker ->
                    OfflineCashReadinessBlockerV1.fromValidatedProjection(
                        blocker.code,
                        blocker.message,
                    )
                },
            )
        }

    fun submitTopUp(
        request: OfflineCashTopUpRequestV1,
        operationId: String,
    ): CompletableFuture<OfflineCashOperationReferenceV1> = delegate
        .submitTopUp(
            KagemushaRecursiveSpendProver.decodeTopUpRequest(request.encodeCanonical()),
            operationId,
        )
        .thenApply { reference -> OfflineCashOperationReferenceV1(reference.noritoEncoded()) }

    fun submitRedeem(
        request: OfflineCashRedeemRequestV1,
        operationId: String,
    ): CompletableFuture<OfflineCashOperationReferenceV1> = delegate
        .submitRedeem(
            KagemushaRecursiveSpendProver.decodeRedeemSubmissionRequest(request.encodeCanonical()),
            operationId,
        )
        .thenApply { reference -> OfflineCashOperationReferenceV1(reference.noritoEncoded()) }

    fun getOperation(operationId: String): CompletableFuture<OfflineCashOperationStatusV1> =
        delegate.getOperation(operationId).thenApply { status ->
            OfflineCashOperationStatusV1(status.noritoEncoded())
        }
}

private fun mapOperationProjectionV1(
    projection: KagemushaRecursiveSpendProver.OperationStatusProjection,
): OfflineCashOperationStatusProjectionV1 =
    OfflineCashOperationStatusProjectionV1.fromValidatedProjection(
        when (projection.state) {
            KagemushaRecursiveSpendProver.OperationState.PENDING ->
                OfflineCashOperationStateV1.PENDING
            KagemushaRecursiveSpendProver.OperationState.APPLIED ->
                OfflineCashOperationStateV1.APPLIED
            KagemushaRecursiveSpendProver.OperationState.REJECTED ->
                OfflineCashOperationStateV1.REJECTED
        },
        when (projection.kind) {
            KagemushaRecursiveSpendProver.OperationKind.TOP_UP -> OfflineCashOperationKindV1.TOP_UP
            KagemushaRecursiveSpendProver.OperationKind.REDEEM -> OfflineCashOperationKindV1.REDEEM
        },
        projection.operationId(),
        projection.transactionHash(),
        projection.submittedAtMilliseconds,
        projection.finalizedBlockHeight,
        projection.serverTimeMilliseconds,
        projection.finalizedTopUp?.let { topUp ->
            OfflineCashFinalizedTopUpV1.fromValidatedProjection(
                topUp.anchor.noritoEncoded(),
                topUp.finalityProof.noritoEncoded(),
                topUp.finalizedBlockHeight,
                topUp.serverTimeMilliseconds,
            )
        },
        projection.rejection?.let { rejection ->
            OfflineCashOperationRejectionV1.fromValidatedProjection(
                rejection.code,
                rejection.message,
            )
        },
    )
