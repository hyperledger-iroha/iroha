// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.time.Duration
import java.util.Collections
import java.util.LinkedHashMap
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import org.hyperledger.iroha.sdk.client.transport.HttpTransportScope
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

/**
 * Routes, limits and path validation of the anonymous SORA Parliament family
 * (`specs/parliament_private_ballot_design.md` sections 6 and 12.3, slice 5): the account- and
 * session-free ballot submission and the two public whole-record reads.
 *
 * The SDK only transports and reads. It does not prove, stamp, decode or verify a ballot: a
 * native wallet produces the canonical envelope frame, and the board and attempt records are
 * returned as their canonical Norito bytes. Send the requests with [AnonymousBallotToriiClientV1].
 */
object AnonymousBallotApiV1 {
    /** Account- and session-free submission route (`governance.anonymous_ballot.submit`). */
    const val SUBMIT_PATH: String = "/v1/gov/anonymous-ballots"

    /** Public board read; `{scope}` is the 64-byte ballot scope as 128 lowercase hex digits. */
    const val BOARD_READ_PATH: String = "/v1/gov/anonymous-ballots/boards/{scope}"

    /** Public attempt read; the 32-byte attempt identifier as 64 lowercase hex digits. */
    const val ATTEMPT_READ_PATH: String =
        "/v1/gov/anonymous-ballots/attempts/{governance_attempt_id}"

    /** Media type of a submitted envelope frame and of both read responses. */
    const val MEDIA_TYPE: String = "application/x-norito"

    /** The only success status of a submission: the node handled the ballot. */
    const val SUBMIT_HANDLED_STATUS: Int = 202

    /** Largest canonical envelope frame, `E_max` (Rust `ANONYMOUS_BALLOT_ENVELOPE_MAX_BYTES_V1`). */
    const val ENVELOPE_MAX_BYTES: Int = 262_144

    /** Largest board record the node serves (Rust `MAX_ANONYMOUS_BALLOT_BOARD_READ_BYTES_V1`). */
    const val BOARD_MAX_BYTES: Int = 1_048_576

    /**
     * Largest attempt record the node serves
     * (Rust `MAX_ANONYMOUS_PARLIAMENT_ATTEMPT_READ_BYTES_V1`).
     */
    const val ATTEMPT_MAX_BYTES: Int = 16_777_216

    /** Byte length of one ballot scope (`BallotScopeV1`). */
    const val SCOPE_BYTES: Int = 64

    /** Byte length of one governance attempt identifier (`GovernanceAttemptId`). */
    const val GOVERNANCE_ATTEMPT_ID_BYTES: Int = 32

    private const val HEX_DIGITS = "0123456789abcdef"

    /** The board-read path of one exact 64-byte [scope]. */
    @JvmStatic
    fun boardReadPath(scope: ByteArray): String {
        require(scope.size == SCOPE_BYTES) { "scope must be exactly $SCOPE_BYTES bytes" }
        return BOARD_READ_PATH.replace("{scope}", lowerHex(scope))
    }

    /** The board-read path of a scope given as exactly 128 lowercase hexadecimal characters. */
    @JvmStatic
    fun boardReadPath(scopeHex: String): String =
        BOARD_READ_PATH.replace("{scope}", requireLowerHex(scopeHex, SCOPE_BYTES, "scope"))

    /** The attempt-read path of one exact 32-byte [governanceAttemptId]. */
    @JvmStatic
    fun attemptReadPath(governanceAttemptId: ByteArray): String {
        require(governanceAttemptId.size == GOVERNANCE_ATTEMPT_ID_BYTES) {
            "governance_attempt_id must be exactly $GOVERNANCE_ATTEMPT_ID_BYTES bytes"
        }
        return ATTEMPT_READ_PATH.replace("{governance_attempt_id}", lowerHex(governanceAttemptId))
    }

    /** The attempt-read path of an identifier given as exactly 64 lowercase hexadecimal characters. */
    @JvmStatic
    fun attemptReadPath(governanceAttemptIdHex: String): String =
        ATTEMPT_READ_PATH.replace(
            "{governance_attempt_id}",
            requireLowerHex(governanceAttemptIdHex, GOVERNANCE_ATTEMPT_ID_BYTES, "governance_attempt_id"),
        )

    /** Refuse an envelope frame that is empty or above [ENVELOPE_MAX_BYTES]; never renders it. */
    internal fun requireEnvelope(envelope: ByteArray) {
        require(envelope.isNotEmpty()) { "anonymous ballot envelope must not be empty" }
        require(envelope.size <= ENVELOPE_MAX_BYTES) {
            "anonymous ballot envelope exceeds $ENVELOPE_MAX_BYTES bytes"
        }
    }

    /** Lowercase hexadecimal of [bytes]. */
    internal fun lowerHex(bytes: ByteArray): String {
        val out = StringBuilder(bytes.size * 2)
        for (byte in bytes) {
            val value = byte.toInt() and 0xff
            out.append(HEX_DIGITS[value ushr 4]).append(HEX_DIGITS[value and 0x0f])
        }
        return out.toString()
    }

    private fun requireLowerHex(value: String, bytes: Int, label: String): String {
        require(value.length == bytes * 2 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$label must be exactly ${bytes * 2} lowercase hexadecimal characters"
        }
        return value
    }
}

/**
 * Exact-route client of the anonymous SORA Parliament family
 * (`specs/parliament_private_ballot_design.md` sections 6 and 12.3).
 *
 * It only transports and reads: it does not prove, stamp, decode or verify ballots, and it
 * returns the board and attempt records as raw canonical Norito bytes.
 *
 * **Submission privacy.** [submitAnonymousBallot] sends the envelope frame with
 * `Content-Type: application/x-norito` and nothing else this SDK controls. It never carries the
 * configured default headers (an API token, `Authorization`, cookies, correlation, request-id or
 * tracing headers), canonical account authentication (`X-Iroha-Account`, `X-Iroha-Signature`,
 * `X-Iroha-Timestamp-Ms`, `X-Iroha-Nonce`), a witness, or a client-wide
 * [ClientConfig.canonicalAuth]. The client accepts no [ClientObserver] and emits no telemetry, so
 * ballot bytes never reach application logging hooks, and none of its errors renders the request
 * body or the server's text. The HTTP library still adds its own fixed framing headers
 * (such as `Host` and `Content-Length`); an application-injected executor must not attach
 * cookies or identifying headers. Network-origin hiding needs a qualified anonymity transport,
 * which this client does not provide: the node and the network path still see the connection's
 * address (design section 6).
 *
 * **Reads.** The board and attempt reads are ordinary public reads of whole records. They carry
 * the configured default headers (for example a listener API token) but no account signature,
 * and they take no credential, assignment or nullifier as a selector.
 *
 * Input validation fails synchronously with [IllegalArgumentException] before any dispatch.
 * A response with another status completes the future with [AnonymousBallotToriiExceptionV1];
 * a response that breaks the route contract completes it with [ToriiProtocolException]; a
 * transport failure completes it with a [RuntimeException] whose cause is the executor failure.
 */
class AnonymousBallotToriiClientV1 private constructor(builder: Builder) : AutoCloseable {
    private val executor: HttpTransportExecutor = HttpTransportScope.create(builder.executor)
    private val baseUri: URI = requireBaseUri(builder.baseUri)
    private val allowLocalDevelopmentHttp: Boolean = builder.allowLocalDevelopmentHttp
    private val timeout: Duration? = builder.timeout
    private val readHeaders: Map<String, String> =
        Collections.unmodifiableMap(LinkedHashMap(builder.defaultHeaders))

    /** Cancels this client's calls; an injected executor remains application-owned. */
    override fun close() {
        executor.close()
    }

    /**
     * POST [AnonymousBallotApiV1.SUBMIT_PATH]: submit one canonical anonymous-ballot envelope
     * frame produced by a native wallet. The SDK treats [envelope] as opaque bytes and sends
     * them unchanged, with none of this client's headers (see the class documentation).
     *
     * The future completes normally only on exactly `202 Accepted` with an empty body. That means
     * the node *handled* the ballot: its queue keeps it, or its nullifier is already pending or
     * committed. It is **not** finalized acceptance. Confirm acceptance from the scope's public
     * corpus ([getAnonymousBallotBoard]) and resubmit through several peers until the ballot is
     * there (design section 6).
     *
     * Any other status completes the future with [AnonymousBallotToriiExceptionV1] carrying the
     * status and the node's stable code: `400` (`anonymous_ballot_malformed` or a rejection code
     * such as `anonymous_ballot_proof_rejected`), `413` (`anonymous_ballot_oversize`), `415`
     * (`anonymous_ballot_unsupported_media_type`), `429` (throttling or local back-pressure, with
     * [AnonymousBallotToriiExceptionV1.retryAfterSeconds] when the node sent one) and `503`
     * (`anonymous_ballot_ingress_unavailable`).
     *
     * @throws IllegalArgumentException when [envelope] is empty or above
     * [AnonymousBallotApiV1.ENVELOPE_MAX_BYTES], or when the base URI is not HTTPS (plain HTTP is
     * admitted only to a local development origin that the builder explicitly allows).
     */
    fun submitAnonymousBallot(envelope: ByteArray): CompletableFuture<Void?> {
        AnonymousBallotApiV1.requireEnvelope(envelope)
        require(TransportSecurity.isHttpEndpointAllowed(baseUri, allowLocalDevelopmentHttp)) {
            "anonymous ballot submission requires an HTTPS Torii endpoint"
        }
        val request = TransportRequest.builder()
            .setUri(resolve(AnonymousBallotApiV1.SUBMIT_PATH))
            .setMethod("POST")
            .setBody(envelope)
            .addHeader("Content-Type", AnonymousBallotApiV1.MEDIA_TYPE)
            .setMaximumResponseBytes(SUBMIT_RESPONSE_MAX_BYTES.toLong())
            .setTimeout(timeout)
            .build()
        return execute<Void?>(request, SUBMIT_CONTEXT) { response ->
            requireExactProvenance(request, response, SUBMIT_CONTEXT)
            val status = response.statusCode
            if (status !in 200..299) throw refusal(response, SUBMIT_CONTEXT)
            if (status != AnonymousBallotApiV1.SUBMIT_HANDLED_STATUS) {
                throw ToriiProtocolException(status, "$SUBMIT_CONTEXT response status must be 202")
            }
            if (response.body.isNotEmpty()) {
                throw ToriiProtocolException(status, "$SUBMIT_CONTEXT response must have an empty body")
            }
            null
        }
    }

    /**
     * GET [AnonymousBallotApiV1.BOARD_READ_PATH]: the canonical Norito bytes of the public board
     * (`ParliamentBallotBoardRecordV1`) of one exact 64-byte [scope]: its sealed context, casting
     * window and whole corpus. A wallet builds its proof context from it and confirms acceptance
     * in it. The bytes are returned undecoded and bounded to [AnonymousBallotApiV1.BOARD_MAX_BYTES].
     *
     * A missing board is `404` (`anonymous_ballot_board_not_found`), reported as
     * [AnonymousBallotToriiExceptionV1].
     *
     * @throws IllegalArgumentException when [scope] is not exactly 64 bytes.
     */
    fun getAnonymousBallotBoard(scope: ByteArray): CompletableFuture<ByteArray> =
        read(
            AnonymousBallotApiV1.boardReadPath(scope),
            AnonymousBallotApiV1.BOARD_MAX_BYTES,
            BOARD_CONTEXT,
        )

    /**
     * As [getAnonymousBallotBoard], with the scope given as exactly 128 lowercase hexadecimal
     * characters.
     *
     * @throws IllegalArgumentException when [scopeHex] is not 128 lowercase hexadecimal characters.
     */
    fun getAnonymousBallotBoard(scopeHex: String): CompletableFuture<ByteArray> =
        read(
            AnonymousBallotApiV1.boardReadPath(scopeHex),
            AnonymousBallotApiV1.BOARD_MAX_BYTES,
            BOARD_CONTEXT,
        )

    /**
     * GET [AnonymousBallotApiV1.ATTEMPT_READ_PATH]: the canonical Norito bytes of one anonymous
     * Parliament attempt (its elections, assignments, bodies and committed schedule), selected by
     * one exact 32-byte [governanceAttemptId]. The bytes are returned undecoded and bounded to
     * [AnonymousBallotApiV1.ATTEMPT_MAX_BYTES].
     *
     * A missing attempt is `404` (`anonymous_parliament_attempt_not_found`), reported as
     * [AnonymousBallotToriiExceptionV1].
     *
     * @throws IllegalArgumentException when [governanceAttemptId] is not exactly 32 bytes.
     */
    fun getAnonymousParliamentAttempt(governanceAttemptId: ByteArray): CompletableFuture<ByteArray> =
        read(
            AnonymousBallotApiV1.attemptReadPath(governanceAttemptId),
            AnonymousBallotApiV1.ATTEMPT_MAX_BYTES,
            ATTEMPT_CONTEXT,
        )

    /**
     * As [getAnonymousParliamentAttempt], with the identifier given as exactly 64 lowercase
     * hexadecimal characters.
     *
     * @throws IllegalArgumentException when [governanceAttemptIdHex] is not 64 lowercase
     * hexadecimal characters.
     */
    fun getAnonymousParliamentAttempt(governanceAttemptIdHex: String): CompletableFuture<ByteArray> =
        read(
            AnonymousBallotApiV1.attemptReadPath(governanceAttemptIdHex),
            AnonymousBallotApiV1.ATTEMPT_MAX_BYTES,
            ATTEMPT_CONTEXT,
        )

    private fun read(path: String, maximumBytes: Int, context: String): CompletableFuture<ByteArray> {
        require(readHeaders.keys.none { it.equals("Accept", ignoreCase = true) }) {
            "Accept must not be overridden for anonymous-family reads"
        }
        val target = resolve(path)
        TransportSecurity.requireHttpRequestAllowed(
            "AnonymousBallotToriiClientV1",
            baseUri,
            target,
            readHeaders,
            null,
            allowLocalDevelopmentHttp = allowLocalDevelopmentHttp,
        )
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", AnonymousBallotApiV1.MEDIA_TYPE)
            .setMaximumResponseBytes(maximumBytes.toLong())
            .setTimeout(timeout)
        readHeaders.forEach { (name, value) -> builder.addHeader(name, value) }
        val request = builder.build()
        return execute(request, context) { response ->
            requireExactProvenance(request, response, context)
            val status = response.statusCode
            if (status !in 200..299) throw refusal(response, context)
            if (status != 200) throw ToriiProtocolException(status, "$context response status must be 200")
            val body = response.body
            if (body.isEmpty()) throw ToriiProtocolException(status, "$context response must not be empty")
            if (body.size > maximumBytes) {
                throw ToriiProtocolException(status, "$context response exceeds $maximumBytes bytes")
            }
            requireNoritoBody(response.headers, status, context)
            body
        }
    }

    private fun <T> execute(
        request: TransportRequest,
        context: String,
        consume: (TransportResponse) -> T,
    ): CompletableFuture<T> {
        val result = CompletableFuture<T>()
        val dispatched = try {
            executor.execute(request)
        } catch (error: RuntimeException) {
            result.completeExceptionally(RuntimeException("$context request failed", error))
            return result
        }
        result.whenComplete { _, _ -> if (result.isCancelled) dispatched.cancel(false) }
        dispatched.whenComplete { response, failure ->
            if (failure != null) {
                result.completeExceptionally(RuntimeException("$context request failed", unwrap(failure)))
                return@whenComplete
            }
            try {
                result.complete(consume(checkNotNull(response) { "$context returned no response" }))
            } catch (error: Throwable) {
                result.completeExceptionally(error)
            }
        }
        return result
    }

    private fun resolve(path: String): URI {
        val normalized = path.removePrefix("/")
        val base = baseUri.toString()
        return URI.create(if (base.endsWith('/')) base + normalized else "$base/$normalized")
    }

    /** Builder of an [AnonymousBallotToriiClientV1]. */
    class Builder internal constructor() {
        internal var executor: HttpTransportExecutor? = null
        internal var baseUri: URI = URI.create("http://localhost:8080")
        internal var allowLocalDevelopmentHttp: Boolean = false
        internal var timeout: Duration? = Duration.ofSeconds(15)
        internal val defaultHeaders = LinkedHashMap<String, String>()

        /** Borrow an HTTP executor; without one the client owns a default OkHttp backend. */
        fun executor(executor: HttpTransportExecutor): Builder = apply { this.executor = executor }

        /** Set the exact Torii base, including any deployment path prefix. */
        fun baseUri(baseUri: URI): Builder = apply { this.baseUri = baseUri }

        /**
         * Allow plain HTTP to the same loopback or private IPv4 origin; disabled by default.
         * See [ClientConfig.Builder.setAllowLocalDevelopmentHttp] for the admitted addresses.
         * Without it a ballot submission requires HTTPS.
         */
        fun setAllowLocalDevelopmentHttp(allow: Boolean): Builder = apply {
            this.allowLocalDevelopmentHttp = allow
        }

        /** Set the per-request timeout; `null` delegates to the executor. */
        fun timeout(timeout: Duration?): Builder = apply { this.timeout = timeout }

        /**
         * Add one header for the two public reads only; a ballot submission never carries it.
         * `Accept` is owned by the client.
         */
        fun addHeader(name: String, value: String): Builder = apply { defaultHeaders[name] = value }

        /** Replace the read-only headers (see [addHeader]). */
        fun defaultHeaders(headers: Map<String, String>?): Builder = apply {
            defaultHeaders.clear()
            if (headers != null) defaultHeaders.putAll(headers)
        }

        /** Build the client. */
        fun build(): AnonymousBallotToriiClientV1 = AnonymousBallotToriiClientV1(this)
    }

    companion object {
        private const val SUBMIT_CONTEXT = "anonymous ballot submission"
        private const val BOARD_CONTEXT = "anonymous-ballot board read"
        private const val ATTEMPT_CONTEXT = "anonymous Parliament attempt read"
        private const val SUBMIT_RESPONSE_MAX_BYTES = 64 * 1024
        private const val ERROR_BODY_MAX_BYTES = 64 * 1024
        private const val RETRY_AFTER_MAX_DIGITS = 9
        private val STABLE_CODE = Regex("^[A-Za-z0-9_.:-]{1,128}$")

        /** Create a client builder. */
        @JvmStatic
        fun builder(): Builder = Builder()

        private fun requireBaseUri(uri: URI): URI {
            require(uri.scheme.equals("http", true) || uri.scheme.equals("https", true)) {
                "anonymous ballot Torii base URI must use HTTP or HTTPS"
            }
            require(uri.rawQuery == null && uri.rawFragment == null) {
                "anonymous ballot Torii base URI must not contain a query or fragment"
            }
            return uri
        }

        private fun requireExactProvenance(
            request: TransportRequest,
            response: TransportResponse,
            context: String,
        ) {
            val finalUri = response.finalUri
            if (response.redirected ||
                (finalUri != null && finalUri.toASCIIString() != request.uri.toASCIIString())
            ) {
                throw ToriiProtocolException(
                    response.statusCode,
                    "$context response must come from the exact request URL without redirects",
                )
            }
        }

        /** An SDK-authored refusal: the status, a validated stable code and `Retry-After`. */
        private fun refusal(response: TransportResponse, context: String): AnonymousBallotToriiExceptionV1 {
            val status = response.statusCode
            val code = stableCode(response.body)
            val message = if (code == null) {
                "$context was refused with HTTP $status"
            } else {
                "$context was refused with HTTP $status ($code)"
            }
            return AnonymousBallotToriiExceptionV1(status, code, message, retryAfterSeconds(response.headers))
        }

        /** The `ErrorEnvelope` code (JSON or Norito) when it is one stable token, else `null`. */
        private fun stableCode(body: ByteArray): String? {
            if (body.isEmpty() || body.size > ERROR_BODY_MAX_BYTES) return null
            val code = try {
                ToriiErrorEnvelope.decode(body)?.code
            } catch (_: RuntimeException) {
                null
            }
            return code?.takeIf { STABLE_CODE.matches(it) }
        }

        /** One `Retry-After` delta-seconds value; an HTTP date or anything ambiguous is `null`. */
        private fun retryAfterSeconds(headers: Map<String, List<String>>): Long? {
            val value = headers.entries
                .filter { it.key.equals("Retry-After", ignoreCase = true) }
                .flatMap { it.value }
                .singleOrNull()
                ?.trim()
                ?: return null
            if (value.isEmpty() || value.length > RETRY_AFTER_MAX_DIGITS || value.any { it !in '0'..'9' }) {
                return null
            }
            return value.toLong()
        }

        private fun requireNoritoBody(headers: Map<String, List<String>>, status: Int, context: String) {
            val contentTypes = headerValues(headers, "Content-Type")
            if (contentTypes.size != 1 ||
                !contentTypes.single().trim().equals(AnonymousBallotApiV1.MEDIA_TYPE, ignoreCase = true)
            ) {
                throw ToriiProtocolException(
                    status,
                    "$context response Content-Type must be exactly ${AnonymousBallotApiV1.MEDIA_TYPE}",
                )
            }
            val encodings = headerValues(headers, "Content-Encoding")
            if (encodings.isNotEmpty() &&
                !(encodings.size == 1 && encodings.single().trim().equals("identity", ignoreCase = true))
            ) {
                throw ToriiProtocolException(status, "$context response Content-Encoding must be absent or identity")
            }
        }

        private fun headerValues(headers: Map<String, List<String>>, name: String): List<String> =
            headers.entries.filter { it.key.equals(name, ignoreCase = true) }.flatMap { it.value }

        private fun unwrap(error: Throwable): Throwable =
            if (error is CompletionException && error.cause != null) checkNotNull(error.cause) else error
    }
}

/**
 * A refusal from an anonymous SORA Parliament family route: any status other than the route's
 * success status (`specs/parliament_private_ballot_design.md` sections 6 and 12.3).
 *
 * [status] and [code] (the Torii `ErrorEnvelope` code, such as `anonymous_ballot_malformed`, or
 * `null` when the body carried no stable code) are the answer. The message is SDK-authored: it
 * never contains the server's text, the request body or ballot bytes. [details] and [rejectCode]
 * are always `null`.
 */
class AnonymousBallotToriiExceptionV1 internal constructor(
    status: Int,
    code: String?,
    message: String,
    /** `Retry-After` delta-seconds of a throttled or unavailable answer, when the node sent one. */
    @JvmField val retryAfterSeconds: Long?,
) : ToriiApiException(status, code, message, null, null)
