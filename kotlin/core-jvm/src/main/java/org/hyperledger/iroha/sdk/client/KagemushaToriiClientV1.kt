// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.time.Duration
import java.util.Collections
import java.util.LinkedHashMap
import java.util.Locale
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionRequestV1
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.norito.SignedTransactionEncoder

/** Exact-route client for the sole first-release KAGEMUSHA reserve API. */
class KagemushaToriiClientV1 private constructor(builder: Builder) {
    private val executor: HttpTransportExecutor =
        builder.executor ?: PlatformHttpTransportExecutor.createDefault()
    private val baseUri: URI = requireBaseUri(builder.baseUri)
    private val timeout: Duration? = builder.timeout
    private val defaultHeaders: Map<String, String> =
        Collections.unmodifiableMap(LinkedHashMap(builder.defaultHeaders))

    init {
        defaultHeaders.keys.forEach { name ->
            require(name.lowercase(Locale.ROOT) !in RESERVED_HEADERS) {
                "KagemushaToriiClientV1 default header $name is route-owned"
            }
        }
    }

    /** Read the exact `/v1/kagemusha/readiness` contract. */
    fun getReadiness(): CompletableFuture<KagemushaReadinessV1> =
        execute("GET", READINESS_PATH, null, null, setOf(200), READINESS_MAX_BYTES) {
            parseReadiness(it.body)
        }

    /**
     * Submit one exact canonical payer-signed KAGEMUSHA top-up transaction.
     * [operationId] must be the transaction's explicit nonzero 32-byte operation identifier.
     */
    fun submitTopUp(
        transaction: SignedTransaction,
        operationId: ByteArray,
    ): CompletableFuture<UnverifiedKagemushaOperationStatusV1> {
        val expected = requireNonzero32(operationId, "operationId")
        val body = SignedTransactionEncoder.encodeVersioned(transaction)
        return submit(TOP_UP_PATH, KagemushaReserveOperationKindV1.TOP_UP, expected, body)
    }

    /** Submit one exact canonical full or partial redemption request. */
    fun submitRedemption(
        request: KagemushaRedemptionRequestV1,
    ): CompletableFuture<UnverifiedKagemushaOperationStatusV1> {
        val body = KagemushaNoritoV1.encodeRedemptionRequestShape(request)
        return submit(
            REDEEM_PATH,
            KagemushaReserveOperationKindV1.REDEMPTION,
            request.operationId(),
            body,
        )
    }

    /** Poll one operation while withholding every unverified applied result. */
    fun getOperation(
        operationId: ByteArray,
    ): CompletableFuture<UnverifiedKagemushaOperationStatusV1> {
        val expected = requireNonzero32(operationId, "operationId")
        val path = OPERATION_PATH_PREFIX + expected.toLowerHex()
        return execute("GET", path, null, null, setOf(200), STATUS_MAX_BYTES) { response ->
            parseStatus(response.body).also {
                require(it.operationId().contentEquals(expected)) {
                    "KAGEMUSHA V1 response operation ID does not match the requested resource"
                }
            }
        }
    }

    private fun submit(
        path: String,
        kind: KagemushaReserveOperationKindV1,
        operationId: ByteArray,
        body: ByteArray,
    ): CompletableFuture<UnverifiedKagemushaOperationStatusV1> {
        val expected = requireNonzero32(operationId, "operationId")
        return execute(
            "POST",
            path,
            body,
            expected.toLowerHex(),
            setOf(200, 202),
            STATUS_MAX_BYTES,
        ) { response ->
            parseStatus(response.body).also {
                require(it.operationId().contentEquals(expected) && it.kind == kind) {
                    "KAGEMUSHA V1 response does not match the submitted operation"
                }
                requireCanonicalSubmissionResponse(response, it, expected)
            }
        }
    }

    private fun <T> execute(
        method: String,
        path: String,
        body: ByteArray?,
        idempotencyKey: String?,
        statuses: Set<Int>,
        maximumResponseBytes: Int,
        parser: (TransportResponse) -> T,
    ): CompletableFuture<T> {
        val request = buildRequest(method, path, body, idempotencyKey, maximumResponseBytes)
        return executor.execute(request).handle { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                throw CompletionException(KagemushaToriiExceptionV1("KAGEMUSHA V1 request failed", cause ?: throwable))
            }
            try {
                parseResponse(request, response, statuses, maximumResponseBytes, parser)
            } catch (error: RuntimeException) {
                throw CompletionException(
                    if (error is KagemushaToriiExceptionV1) error
                    else KagemushaToriiExceptionV1("KAGEMUSHA V1 response is invalid", error),
                )
            }
        }
    }

    private fun buildRequest(
        method: String,
        path: String,
        body: ByteArray?,
        idempotencyKey: String?,
        maximumResponseBytes: Int,
    ): TransportRequest {
        val target = resolvePath(path)
        val headers = LinkedHashMap(defaultHeaders)
        headers["Accept"] = JSON_MEDIA_TYPE
        if (body != null) headers["Content-Type"] = NORITO_MEDIA_TYPE
        if (idempotencyKey != null) headers["Idempotency-Key"] = idempotencyKey
        TransportSecurity.requireHttpRequestAllowed(
            "KagemushaToriiClientV1",
            baseUri,
            target,
            headers,
            body,
        )
        val request = TransportRequest.builder()
            .setUri(target)
            .setMethod(method)
            .setTimeout(timeout)
            .setMaximumResponseBytes(maximumResponseBytes.toLong())
        if (body != null) request.setBody(body)
        headers.forEach { (name, value) -> request.addHeader(name, value) }
        return request.build()
    }

    private fun <T> parseResponse(
        request: TransportRequest,
        response: TransportResponse,
        statuses: Set<Int>,
        maximumResponseBytes: Int,
        parser: (TransportResponse) -> T,
    ): T {
        require(response.statusCode in statuses) {
            "KAGEMUSHA V1 request failed with HTTP ${response.statusCode}"
        }
        require(!response.redirected && (response.finalUri == null || response.finalUri == request.uri)) {
            "KAGEMUSHA V1 routes do not permit redirects"
        }
        require(response.body.size <= maximumResponseBytes) {
            "KAGEMUSHA V1 response exceeds $maximumResponseBytes bytes"
        }
        require(response.headers.entries.any { (name, values) ->
            name.equals("Content-Type", ignoreCase = true) &&
                values.any { it.substringBefore(';').trim().equals(JSON_MEDIA_TYPE, ignoreCase = true) }
        }) { "KAGEMUSHA V1 response must use application/json" }
        return parser(response)
    }

    private fun resolvePath(path: String): URI {
        val normalized = path.removePrefix("/")
        val base = baseUri.toString()
        return URI.create(if (base.endsWith('/')) base + normalized else "$base/$normalized")
    }

    /** Builder for [KagemushaToriiClientV1]. */
    class Builder internal constructor() {
        internal var executor: HttpTransportExecutor? = null
        internal var baseUri: URI = URI.create("http://localhost:8080")
        internal var timeout: Duration? = Duration.ofSeconds(15)
        internal val defaultHeaders = LinkedHashMap<String, String>()

        fun executor(executor: HttpTransportExecutor): Builder = apply { this.executor = executor }
        fun baseUri(baseUri: URI): Builder = apply { this.baseUri = baseUri }
        fun timeout(timeout: Duration?): Builder = apply { this.timeout = timeout }
        fun addHeader(name: String, value: String): Builder = apply { defaultHeaders[name] = value }
        fun build(): KagemushaToriiClientV1 = KagemushaToriiClientV1(this)
    }

    companion object {
        const val READINESS_PATH: String = "/v1/kagemusha/readiness"
        const val TOP_UP_PATH: String = "/v1/kagemusha/top-up"
        const val REDEEM_PATH: String = "/v1/kagemusha/redeem"
        const val OPERATION_PATH_PREFIX: String = "/v1/kagemusha/operations/"

        private const val READINESS_MAX_BYTES = 4 * 1024
        private const val STATUS_MAX_BYTES = 16 * 1024 * 1024
        private const val JSON_MEDIA_TYPE = "application/json"
        private const val NORITO_MEDIA_TYPE = "application/x-norito"
        private val RESERVED_HEADERS = setOf(
            "accept",
            "content-type",
            "content-length",
            "idempotency-key",
            "host",
        )

        @JvmStatic fun builder(): Builder = Builder()

        private fun requireBaseUri(value: URI): URI {
            require(value.isAbsolute && value.rawQuery == null && value.rawFragment == null) {
                "baseUri must be one absolute HTTP(S) URI without query or fragment"
            }
            require(value.scheme.equals("http", true) || value.scheme.equals("https", true))
            return value
        }

        private fun parseReadiness(body: ByteArray): KagemushaReadinessV1 {
            val root = body.parseJsonObject("readiness")
            requireExactFields(
                root,
                setOf("kagemusha_handoff_capability", "wire_version", "device_lifecycle_version", "ready"),
                "readiness",
            )
            val capability = root["kagemusha_handoff_capability"] as? String
            val wire = root["wire_version"].exactInt("wire_version")
            val lifecycle = root["device_lifecycle_version"].exactInt("device_lifecycle_version")
            val ready = root["ready"] as? Boolean
                ?: throw IllegalArgumentException("ready must be a boolean")
            require(capability == "kagemusha_handoff_v1" && wire == 1 && lifecycle == 1) {
                "readiness must advertise the sole KAGEMUSHA V1 contract"
            }
            return KagemushaReadinessV1(capability, wire, lifecycle, ready)
        }

        private fun parseStatus(body: ByteArray): UnverifiedKagemushaOperationStatusV1 {
            val root = body.parseJsonObject("operation status")
            requireExactFields(
                root,
                setOf("version", "operation_id", "kind", "state", "result", "rejection"),
                "operation status",
            )
            require(root["version"].exactInt("version") == 1)
            val operationId = root["operation_id"].fixed32("operation_id")
            val kind = taggedUnit(root["kind"], "kind").let { wire ->
                KagemushaReserveOperationKindV1.values().singleOrNull { it.wireName == wire }
                    ?: throw IllegalArgumentException("unsupported KAGEMUSHA V1 operation kind")
            }
            val state = taggedUnit(root["state"], "state").let { wire ->
                KagemushaReserveOperationStateV1.values().singleOrNull { it.wireName == wire }
                    ?: throw IllegalArgumentException("unsupported KAGEMUSHA V1 operation state")
            }
            val result = root["result"]
            val rejectionValue = root["rejection"]
            val rejection = when (state) {
                KagemushaReserveOperationStateV1.PENDING -> {
                    require(result == null && rejectionValue == null)
                    null
                }
                KagemushaReserveOperationStateV1.APPLIED -> {
                    require(result is Map<*, *> && rejectionValue == null)
                    null
                }
                KagemushaReserveOperationStateV1.REJECTED -> {
                    require(result == null && rejectionValue is Map<*, *>)
                    @Suppress("UNCHECKED_CAST")
                    val record = rejectionValue as Map<String, Any?>
                    requireExactFields(record, setOf("code", "detail_digest"), "rejection")
                    val codeName = taggedUnit(record["code"], "code")
                    val code = KagemushaOperationRejectionCodeV1.values().singleOrNull {
                        it.wireName == codeName
                    } ?: throw IllegalArgumentException("unsupported KAGEMUSHA V1 rejection code")
                    KagemushaOperationRejectionV1(code, record["detail_digest"].fixed32("detail_digest"))
                }
            }
            return UnverifiedKagemushaOperationStatusV1(
                operationId,
                kind,
                state,
                rejection,
                body,
            )
        }

        private fun ByteArray.parseJsonObject(context: String): Map<String, Any?> {
            val decoder = StandardCharsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
            val parsed = JsonParser.parse(decoder.decode(ByteBuffer.wrap(this)).toString())
            require(parsed is Map<*, *>) { "KAGEMUSHA V1 $context must be a JSON object" }
            @Suppress("UNCHECKED_CAST")
            return parsed as Map<String, Any?>
        }

        private fun requireExactFields(
            value: Map<String, Any?>,
            expected: Set<String>,
            context: String,
        ) {
            require(value.keys == expected) { "KAGEMUSHA V1 $context has missing or unknown fields" }
        }

        private fun taggedUnit(value: Any?, tag: String): String {
            require(value is Map<*, *>) { "$tag must be one tagged unit enum" }
            @Suppress("UNCHECKED_CAST")
            val record = value as Map<String, Any?>
            requireExactFields(record, setOf(tag, "value"), tag)
            require(record["value"] == null)
            return record[tag] as? String
                ?: throw IllegalArgumentException("$tag must carry one string tag")
        }

        private fun Any?.exactInt(field: String): Int {
            val value = this as? Long ?: throw IllegalArgumentException("$field must be an integer")
            require(value in 0..Int.MAX_VALUE.toLong()) { "$field is out of range" }
            return value.toInt()
        }

        private fun Any?.fixed32(field: String): ByteArray {
            require(this is List<*> && size == 32) { "$field must be one 32-byte array" }
            return ByteArray(32) { index ->
                val value = this[index] as? Long
                    ?: throw IllegalArgumentException("$field contains a non-integer byte")
                require(value in 0..255) { "$field contains an out-of-range byte" }
                value.toByte()
            }.also { requireNonzero32(it, field) }
        }

        private fun ByteArray.toLowerHex(): String =
            joinToString(separator = "") { "%02x".format(it.toInt() and 0xff) }

        private fun requireCanonicalSubmissionResponse(
            response: TransportResponse,
            status: UnverifiedKagemushaOperationStatusV1,
            operationId: ByteArray,
        ) {
            val expectedLocation = OPERATION_PATH_PREFIX + operationId.toLowerHex()
            require(headerValues(response.headers, "Location") == listOf(expectedLocation)) {
                "KAGEMUSHA V1 submission Location must name the canonical operation resource"
            }
            val retryAfter = headerValues(response.headers, "Retry-After")
            when (response.statusCode) {
                202 -> {
                    require(status.state == KagemushaReserveOperationStateV1.PENDING) {
                        "KAGEMUSHA V1 HTTP 202 submission response must be pending"
                    }
                    val seconds = retryAfter.singleOrNull()
                    require(
                        seconds != null &&
                            seconds.isNotEmpty() &&
                            seconds.all { it in '0'..'9' } &&
                            seconds.toLongOrNull()?.let { it > 0L } == true,
                    ) { "KAGEMUSHA V1 HTTP 202 submission response needs positive Retry-After" }
                }
                200 -> {
                    require(
                        status.state == KagemushaReserveOperationStateV1.APPLIED ||
                            status.state == KagemushaReserveOperationStateV1.REJECTED,
                    ) { "KAGEMUSHA V1 HTTP 200 submission response must be terminal" }
                    require(retryAfter.isEmpty()) {
                        "KAGEMUSHA V1 HTTP 200 submission response must not contain Retry-After"
                    }
                }
            }
        }

        private fun headerValues(
            headers: Map<String, List<String>>,
            name: String,
        ): List<String> = headers.entries
            .asSequence()
            .filter { (headerName, _) -> headerName.equals(name, ignoreCase = true) }
            .flatMap { (_, values) -> values.asSequence() }
            .toList()
    }
}

/** Failure at the bounded generic KAGEMUSHA V1 Torii boundary. */
class KagemushaToriiExceptionV1(message: String, cause: Throwable? = null) :
    RuntimeException(message, cause)
