package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.time.Duration
import java.util.Base64
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.Varint

/**
 * Opaque issuer credential for the first-release Bootle/Lantern issuance routes.
 *
 * The credential is retained as private bytes, rendered as canonical unpadded base64url only while
 * constructing a request, never exposed by [toString], and erased when [close] is called.
 */
class BootleLanternIssuanceCredentialV1 private constructor(
    credential: ByteArray,
) : AutoCloseable {
    private val credentialBytes: ByteArray = credential.copyOf()
    private var closed: Boolean = false

    internal fun authorizationHeaderValue(): String = synchronized(this) {
        check(!closed) { "Bootle/Lantern issuance credential is closed" }
        "Bearer ${Base64.getUrlEncoder().withoutPadding().encodeToString(credentialBytes)}"
    }

    override fun close() = synchronized(this) {
        if (!closed) {
            credentialBytes.fill(0)
            closed = true
        }
    }

    override fun toString(): String = "BootleLanternIssuanceCredentialV1([REDACTED])"

    companion object {
        /** Maximum decoded credential length accepted by Torii. */
        const val MAX_BYTES: Int = 4_096

        private const val MAX_ENCODED_BYTES: Int = ((MAX_BYTES + 2) / 3) * 4

        /** Copies and validates opaque credential bytes. */
        @JvmStatic
        fun fromOpaqueBytes(credential: ByteArray): BootleLanternIssuanceCredentialV1 {
            require(credential.isNotEmpty()) { "Bootle/Lantern issuance credential must not be empty" }
            require(credential.size <= MAX_BYTES) {
                "Bootle/Lantern issuance credential exceeds $MAX_BYTES bytes"
            }
            return BootleLanternIssuanceCredentialV1(credential)
        }

        /**
         * Decodes exactly one canonical, unpadded base64url credential without a `Bearer` prefix.
         */
        @JvmStatic
        fun fromCanonicalBase64Url(encoded: String): BootleLanternIssuanceCredentialV1 {
            require(encoded.isNotEmpty()) { "Bootle/Lantern issuance credential must not be empty" }
            require(encoded.length <= MAX_ENCODED_BYTES) {
                "Bootle/Lantern issuance credential encoding is too long"
            }
            require(encoded.none { it == '=' || it.isWhitespace() }) {
                "Bootle/Lantern issuance credential must be canonical unpadded base64url"
            }
            val decoded = try {
                Base64.getUrlDecoder().decode(encoded)
            } catch (error: IllegalArgumentException) {
                throw IllegalArgumentException(
                    "Bootle/Lantern issuance credential must be canonical unpadded base64url",
                    error,
                )
            }
            try {
                require(decoded.isNotEmpty() && decoded.size <= MAX_BYTES) {
                    "Bootle/Lantern issuance credential must contain 1..$MAX_BYTES bytes"
                }
                require(
                    Base64.getUrlEncoder().withoutPadding().encodeToString(decoded) == encoded,
                ) {
                    "Bootle/Lantern issuance credential must be canonical unpadded base64url"
                }
                return BootleLanternIssuanceCredentialV1(decoded)
            } finally {
                decoded.fill(0)
            }
        }
    }
}

/**
 * Exact, no-retry client for native Bootle/Lantern blind issuance.
 *
 * This client deliberately has no observer or generic-header surface: bearer material cannot enter
 * SDK telemetry, and callers cannot weaken the canonical media type or identity-only encoding.
 */
class BootleLanternIssuanceClientV1 private constructor(builder: Builder) {
    private val executor: HttpTransportExecutor =
        builder.executor ?: PlatformHttpTransportExecutor.createDefault()
    private val baseUri: URI = validateBaseUri(builder.baseUri)
    private val timeout: Duration? = builder.timeout

    /**
     * Mints one exact 320-byte `ILA1` authorization with an exact empty request body.
     *
     * The operation is submitted exactly once. Transport and HTTP failures are never retried.
     */
    fun authorize(
        credential: BootleLanternIssuanceCredentialV1,
    ): CompletableFuture<ByteArray> = executeExact(
        operation = "Bootle/Lantern issuance authorization",
        path = AUTHORIZE_PATH,
        credential = credential,
        body = ByteArray(0),
        expectedResponseBytes = AUTHORIZATION_RESPONSE_BYTES,
        expectedResponseMagic = AUTHORIZATION_MAGIC,
    )

    /**
     * Submits exactly `ILA1 || ILQ1` and returns one exact 3,176-byte `ILR1` response.
     *
     * The operation is submitted exactly once. Transport and HTTP failures are never retried.
     */
    fun issue(
        credential: BootleLanternIssuanceCredentialV1,
        canonicalRequest: ByteArray,
    ): CompletableFuture<ByteArray> {
        require(canonicalRequest.size == ISSUE_REQUEST_BYTES) {
            "Bootle/Lantern issue request must be exactly $ISSUE_REQUEST_BYTES bytes"
        }
        require(
            hasExactMagic(canonicalRequest, AUTHORIZATION_MAGIC) &&
                hasExactMagic(canonicalRequest, BLIND_REQUEST_MAGIC, AUTHORIZATION_RESPONSE_BYTES),
        ) {
            "Bootle/Lantern issue request must contain canonical ILA1 || ILQ1 magics"
        }
        return executeExact(
            operation = "Bootle/Lantern blind issuance",
            path = ISSUE_PATH,
            credential = credential,
            body = canonicalRequest,
            expectedResponseBytes = ISSUE_RESPONSE_BYTES,
            expectedResponseMagic = RESPONSE_MAGIC,
        )
    }

    private fun executeExact(
        operation: String,
        path: String,
        credential: BootleLanternIssuanceCredentialV1,
        body: ByteArray,
        expectedResponseBytes: Int,
        expectedResponseMagic: ByteArray,
    ): CompletableFuture<ByteArray> {
        val request = buildRequest(path, credential, body, expectedResponseBytes)
        val result = CompletableFuture<ByteArray>()
        val execution = try {
            executor.execute(request)
        } catch (_: RuntimeException) {
            result.completeExceptionally(
                BootleLanternIssuanceClientExceptionV1("$operation request failed"),
            )
            return result
        }
        execution.whenComplete { response, throwable ->
            if (throwable != null) {
                result.completeExceptionally(
                    BootleLanternIssuanceClientExceptionV1("$operation request failed"),
                )
                return@whenComplete
            }
            try {
                result.complete(
                    validateResponse(
                        response,
                        operation,
                        expectedResponseBytes,
                        expectedResponseMagic,
                    ),
                )
            } catch (error: RuntimeException) {
                result.completeExceptionally(
                    if (error is BootleLanternIssuanceClientExceptionV1) {
                        error
                    } else {
                        BootleLanternIssuanceClientExceptionV1(
                            "$operation response is invalid",
                        )
                    },
                )
            }
        }
        return result
    }

    private fun buildRequest(
        path: String,
        credential: BootleLanternIssuanceCredentialV1,
        body: ByteArray,
        expectedResponseBytes: Int,
    ): TransportRequest {
        val target = resolvePath(path)
        val authorization = credential.authorizationHeaderValue()
        val headers = linkedMapOf(
            "Authorization" to authorization,
            "Content-Type" to NORITO_MEDIA_TYPE,
            "Accept" to NORITO_MEDIA_TYPE,
            "Accept-Encoding" to "identity",
            "Cache-Control" to "no-store",
            "Pragma" to "no-cache",
        )
        TransportSecurity.requireHttpRequestAllowed(
            "BootleLanternIssuanceClientV1",
            baseUri,
            target,
            headers,
            body,
        )
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .setBody(body)
            .setTimeout(timeout)
            .setMaximumResponseBytes(maxOf(expectedResponseBytes, ERROR_RESPONSE_MAX_BYTES).toLong())
        headers.forEach { (name, value) -> builder.addHeader(name, value) }
        return builder.build()
    }

    private fun validateResponse(
        response: TransportResponse,
        operation: String,
        expectedResponseBytes: Int,
        expectedResponseMagic: ByteArray,
    ): ByteArray {
        if (response.statusCode != 200) {
            throw decodeErrorResponse(response, operation)
        }
        requireExactHeader(response.headers, "Content-Type", NORITO_MEDIA_TYPE, operation)
        requireNoHeader(response.headers, "Content-Encoding", operation)
        requireNoHeader(response.headers, "WWW-Authenticate", operation)
        val body = response.body
        if (body.size != expectedResponseBytes) {
            throw BootleLanternIssuanceClientExceptionV1(
                "$operation response must be exactly $expectedResponseBytes bytes",
            )
        }
        if (!hasExactMagic(body, expectedResponseMagic)) {
            throw BootleLanternIssuanceClientExceptionV1(
                "$operation response wire magic is invalid",
            )
        }
        requireExactOptionalContentLength(response.headers, body.size, operation)
        return body.copyOf()
    }

    private fun decodeErrorResponse(
        response: TransportResponse,
        operation: String,
    ): BootleLanternIssuanceClientExceptionV1 {
        val contract = errorContract(response.statusCode)
            ?: return BootleLanternIssuanceClientExceptionV1(
                "$operation returned an unsupported error response",
            )
        return try {
            val body = response.body
            require(body.isNotEmpty() && body.size <= ERROR_RESPONSE_MAX_BYTES) {
                "error response body has an invalid length"
            }
            requireExactHeader(
                response.headers,
                "Content-Type",
                contract.mediaType,
                operation,
            )
            requireNoHeader(response.headers, "Content-Encoding", operation)
            requireExactOptionalContentLength(response.headers, body.size, operation)
            val retryAfter = headerValues(response.headers, "Retry-After")
            if (contract.retryAfterSeconds == 1L) {
                require(retryAfter == listOf("1")) { "invalid Retry-After" }
            } else {
                require(retryAfter.isEmpty()) { "unexpected Retry-After" }
            }
            val wwwAuthenticate = headerValues(response.headers, "WWW-Authenticate")
            if (response.statusCode == 401) {
                require(wwwAuthenticate == listOf(WWW_AUTHENTICATE_VALUE)) {
                    "invalid WWW-Authenticate"
                }
            } else {
                require(wwwAuthenticate.isEmpty()) { "unexpected WWW-Authenticate" }
            }

            val envelope = if (response.statusCode == 406) {
                val expected =
                    "{\"code\":\"${contract.code}\",\"message\":\"${contract.code}\"}"
                        .toByteArray(StandardCharsets.UTF_8)
                require(body.contentEquals(expected)) { "non-canonical JSON error envelope" }
                ErrorEnvelopeV1(contract.code, contract.code)
            } else {
                decodeNoritoErrorEnvelope(body)
            }
            require(envelope.code == contract.code && envelope.message == contract.code) {
                "error envelope does not match its HTTP status"
            }
            BootleLanternIssuanceClientExceptionV1(
                message = "$operation returned HTTP ${response.statusCode}: ${contract.code}",
                statusCode = response.statusCode,
                code = contract.code,
                retryAfterSeconds = contract.retryAfterSeconds,
            )
        } catch (_: RuntimeException) {
            BootleLanternIssuanceClientExceptionV1(
                "$operation returned an invalid error response",
            )
        }
    }

    private fun resolvePath(path: String): URI =
        URI.create(baseUri.toString().removeSuffix("/") + path)

    /** Builder for an exact issuance client. */
    class Builder internal constructor() {
        internal var executor: HttpTransportExecutor? = null
        internal var baseUri: URI = URI.create("https://localhost:8080")
        internal var timeout: Duration? = Duration.ofSeconds(15)

        /** Injects the single-attempt transport executor. */
        fun executor(executor: HttpTransportExecutor): Builder {
            this.executor = executor
            return this
        }

        /** Sets an origin-only HTTPS Torii base URI. */
        fun baseUri(baseUri: URI): Builder {
            this.baseUri = baseUri
            return this
        }

        /** Sets the per-request timeout, or `null` to use executor defaults. */
        fun timeout(timeout: Duration?): Builder {
            require(timeout == null || !timeout.isNegative) { "timeout must be non-negative" }
            this.timeout = timeout
            return this
        }

        /** Builds the issuance client. */
        fun build(): BootleLanternIssuanceClientV1 = BootleLanternIssuanceClientV1(this)
    }

    companion object {
        /** Canonical authorization route. */
        const val AUTHORIZE_PATH: String = "/v1/privacy/bootle-lantern/issuance/authorize"

        /** Canonical one-shot issuance route. */
        const val ISSUE_PATH: String = "/v1/privacy/bootle-lantern/issuance/issue"

        /** Sole request and successful-response media type. */
        const val NORITO_MEDIA_TYPE: String = "application/x-norito"

        /** Exact authorize response length. */
        const val AUTHORIZATION_RESPONSE_BYTES: Int = 320

        /** Exact `ILA1 || ILQ1` issue request length. */
        const val ISSUE_REQUEST_BYTES: Int = 71_896

        /** Exact issue response length. */
        const val ISSUE_RESPONSE_BYTES: Int = 3_176

        /** Maximum accepted structured issuance-error body length. */
        const val ERROR_RESPONSE_MAX_BYTES: Int = 512

        private const val JSON_MEDIA_TYPE: String = "application/json"
        private val AUTHORIZATION_MAGIC: ByteArray = "ILA1".toByteArray(StandardCharsets.US_ASCII)
        private val BLIND_REQUEST_MAGIC: ByteArray = "ILQ1".toByteArray(StandardCharsets.US_ASCII)
        private val RESPONSE_MAGIC: ByteArray = "ILR1".toByteArray(StandardCharsets.US_ASCII)
        private const val WWW_AUTHENTICATE_VALUE: String =
            "Bearer realm=\"iroha-bootle-lantern-issuance\""
        private const val ERROR_ENVELOPE_TYPE_NAME: String =
            "iroha_torii_shared::ErrorEnvelope"

        /** Creates a client builder. */
        @JvmStatic
        fun builder(): Builder = Builder()

        private fun validateBaseUri(baseUri: URI): URI {
            require(baseUri.isAbsolute && baseUri.scheme.equals("https", ignoreCase = true)) {
                "Bootle/Lantern issuance requires an absolute HTTPS base URI"
            }
            require(!baseUri.host.isNullOrEmpty()) {
                "Bootle/Lantern issuance base URI must contain a host"
            }
            require(baseUri.rawUserInfo == null) {
                "Bootle/Lantern issuance base URI must not contain user info"
            }
            require(baseUri.rawQuery == null && baseUri.rawFragment == null) {
                "Bootle/Lantern issuance base URI must not contain a query or fragment"
            }
            require(baseUri.rawPath.isNullOrEmpty() || baseUri.rawPath == "/") {
                "Bootle/Lantern issuance base URI must be origin-only"
            }
            return baseUri
        }

        private data class ErrorContractV1(
            val code: String,
            val mediaType: String,
            val retryAfterSeconds: Long? = null,
        )

        private data class ErrorEnvelopeV1(val code: String, val message: String)

        private fun errorContract(status: Int): ErrorContractV1? = when (status) {
            400 -> ErrorContractV1("privacy_issuance_invalid_request", NORITO_MEDIA_TYPE)
            401 -> ErrorContractV1("privacy_issuance_unauthorized", NORITO_MEDIA_TYPE)
            406 -> ErrorContractV1("privacy_issuance_not_acceptable", JSON_MEDIA_TYPE)
            409 -> ErrorContractV1("privacy_issuance_state_conflict", NORITO_MEDIA_TYPE)
            413 -> ErrorContractV1("privacy_issuance_payload_too_large", NORITO_MEDIA_TYPE)
            415 -> ErrorContractV1("privacy_issuance_unsupported_media_type", NORITO_MEDIA_TYPE)
            429 -> ErrorContractV1(
                "privacy_issuance_capacity_exhausted",
                NORITO_MEDIA_TYPE,
                1,
            )
            503 -> ErrorContractV1("privacy_issuance_unavailable", NORITO_MEDIA_TYPE)
            else -> null
        }

        private fun hasExactMagic(
            body: ByteArray,
            magic: ByteArray,
            offset: Int = 0,
        ): Boolean = body.size >= offset + magic.size &&
            magic.indices.all { body[offset + it] == magic[it] }

        private fun decodeNoritoErrorEnvelope(body: ByteArray): ErrorEnvelopeV1 {
            val decoded = NoritoHeader.decode(
                body,
                SchemaHash.hash16(ERROR_ENVELOPE_TYPE_NAME),
            )
            require(decoded.header.compression == NoritoHeader.COMPRESSION_NONE)
            require(decoded.header.flags == NoritoHeader.COMPACT_LEN)
            require(body.size == NoritoHeader.HEADER_LENGTH + decoded.header.payloadLength) {
                "error envelope must not contain alignment padding"
            }
            decoded.header.validateChecksum(decoded.payload)
            var offset = 0
            val code = readStrictNoritoStringField(decoded.payload, offset)
            offset = code.second
            val message = readStrictNoritoStringField(decoded.payload, offset)
            offset = message.second
            val details = readStrictNoritoField(decoded.payload, offset)
            offset = details.second
            require(details.first.size == 1 && details.first[0].toInt() == 0) {
                "error details must be absent"
            }
            require(offset == decoded.payload.size) { "trailing error-envelope payload" }
            return ErrorEnvelopeV1(code.first, message.first)
        }

        private fun readStrictNoritoField(payload: ByteArray, offset: Int): Pair<ByteArray, Int> {
            val length = Varint.decode(payload, offset)
            require(length.value in 0..Int.MAX_VALUE.toLong()) { "field length overflow" }
            val start = length.nextOffset
            val fieldLength = length.value.toInt()
            require(start >= 0 && start <= payload.size - fieldLength) {
                "truncated error-envelope field"
            }
            val end = start + fieldLength
            return payload.copyOfRange(start, end) to end
        }

        private fun readStrictNoritoStringField(
            payload: ByteArray,
            offset: Int,
        ): Pair<String, Int> {
            val field = readStrictNoritoField(payload, offset)
            val decoded = readStrictNoritoString(field.first, 0)
            require(decoded.second == field.first.size) {
                "trailing bytes in error-envelope string field"
            }
            return decoded.first to field.second
        }

        private fun readStrictNoritoString(payload: ByteArray, offset: Int): Pair<String, Int> {
            val length = Varint.decode(payload, offset)
            require(length.value in 0..Int.MAX_VALUE.toLong()) { "string length overflow" }
            val start = length.nextOffset
            val end = start + length.value.toInt()
            require(end >= start && end <= payload.size) { "truncated string" }
            val decoder = StandardCharsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
            val value = decoder.decode(ByteBuffer.wrap(payload, start, end - start)).toString()
            return value to end
        }

        private fun headerValues(
            headers: Map<String, List<String>>,
            name: String,
        ): List<String> = headers.entries
            .asSequence()
            .filter { (headerName, _) -> headerName.equals(name, ignoreCase = true) }
            .flatMap { (_, values) -> values.asSequence() }
            .toList()

        private fun requireExactHeader(
            headers: Map<String, List<String>>,
            name: String,
            expected: String,
            operation: String,
        ) {
            val values = headerValues(headers, name)
            if (values.size != 1 || values.single() != expected) {
                throw BootleLanternIssuanceClientExceptionV1(
                    "$operation response $name must be exactly $expected",
                )
            }
        }

        private fun requireNoHeader(
            headers: Map<String, List<String>>,
            name: String,
            operation: String,
        ) {
            if (headerValues(headers, name).isNotEmpty()) {
                throw BootleLanternIssuanceClientExceptionV1(
                    "$operation response must not contain $name",
                )
            }
        }

        private fun requireExactOptionalContentLength(
            headers: Map<String, List<String>>,
            actualBytes: Int,
            operation: String,
        ) {
            val values = headerValues(headers, "Content-Length")
            if (values.isEmpty()) return
            if (values.size != 1) {
                throw BootleLanternIssuanceClientExceptionV1(
                    "$operation response has ambiguous Content-Length",
                )
            }
            val value = values.single()
            val canonical = value == "0" ||
                (value.isNotEmpty() && value[0] in '1'..'9' && value.drop(1).all { it in '0'..'9' })
            if (!canonical || value.toLongOrNull() != actualBytes.toLong()) {
                throw BootleLanternIssuanceClientExceptionV1(
                    "$operation response Content-Length is not canonical and exact",
                )
            }
        }
    }
}

/** Fail-closed transport or response validation failure from the issuance client. */
class BootleLanternIssuanceClientExceptionV1(
    message: String,
    val statusCode: Int? = null,
    val code: String? = null,
    val retryAfterSeconds: Long? = null,
) : RuntimeException(message)
