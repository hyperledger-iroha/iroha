// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
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
import org.hyperledger.iroha.sdk.core.util.HashLiteral

/** Authentication class required by an atomic-private-settlement Torii operation. */
enum class AtomicPrivateSettlementAuthV1 {
    /** Exact sponsor account request signature. */
    SPONSOR,

    /** Exact validator or auditor identity request signature. */
    ROLE_IDENTITY,

    /** Public redacted query with no identity material. */
    PUBLIC,
}

/**
 * Closed V1 request catalog for atomic private settlement.
 *
 * The request body is constructed by the native Rust wallet/coordinator. Mobile SDKs route and
 * authenticate the exact prepared object, but never receive a proof witness or capsule plaintext.
 */
enum class AtomicPrivateSettlementOperationV1(
    @JvmField val path: String,
    @JvmField val auth: AtomicPrivateSettlementAuthV1,
    internal val topLevelFields: Set<String>,
    internal val maximumRequestBytes: Int,
) {
    /** Persist provisional encrypted material and obtain one availability share. */
    AVAILABILITY_SHARE(
        "/v1/nexus/private-settlements/legs/availability-shares",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        setOf("material"),
        32 * 1024 * 1024,
    ),

    /** Ask one exact four-member committee validator for a Prepare vote. */
    PREPARE_VOTE(
        "/v1/nexus/private-settlements/phases/prepare-votes",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        setOf("manifest", "payload_digest"),
        8 * 1024 * 1024,
    ),

    /** Ask one exact four-member committee validator for a Commit vote. */
    COMMIT_VOTE(
        "/v1/nexus/private-settlements/phases/commit-votes",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        setOf("payload_digest", "barrier"),
        8 * 1024 * 1024,
    ),

    /** Persist an aggregate Prepare or Commit certificate on one participant node. */
    PHASE_CERTIFICATE(
        "/v1/nexus/private-settlements/phases/certificates",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        setOf("manifest", "payload_digest", "certificate"),
        8 * 1024 * 1024,
    ),

    /** Promote one availability-certified encrypted leg. */
    LEG_UPLOAD(
        "/v1/nexus/private-settlements/legs",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        setOf("manifest", "audit_policy", "committee_authority", "payload"),
        32 * 1024 * 1024,
    ),

    /** Submit one purpose-separated governed auditor approval. */
    AUDIT_APPROVAL(
        "/v1/nexus/private-settlements/legs/{payload_digest}/audit-approvals",
        AtomicPrivateSettlementAuthV1.ROLE_IDENTITY,
        setOf("approval"),
        2 * 1024 * 1024,
    ),

    /** Submit one sponsor-signed exact global finalization carrier. */
    BUNDLE_SUBMIT(
        "/v1/nexus/private-settlements/bundles",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        setOf("transaction"),
        8 * 1024 * 1024,
    ),
}

/** Immutable 32-byte Iroha hash identity used in settlement paths and response binding. */
class AtomicPrivateSettlementIdentifierV1 private constructor(bytes: ByteArray) {
    private val value: ByteArray = bytes.copyOf()

    init {
        require(value.size == HASH_BYTES) { "settlement identifier must contain exactly 32 bytes" }
        require(value[value.lastIndex].toInt() and 1 == 1) {
            "settlement identifier must satisfy the Iroha hash marker invariant"
        }
    }

    /** Lowercase raw hexadecimal path component accepted by Torii. */
    fun pathComponent(): String = value.joinToString("") { "%02x".format(it.toInt() and 0xff) }

    /** Canonical Norito JSON hash literal used in response objects. */
    fun jsonLiteral(): String = HashLiteral.canonicalize(value)

    /** Defensive copy of the exact 32-byte identity. */
    fun bytes(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        this === other || other is AtomicPrivateSettlementIdentifierV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()

    override fun toString(): String = pathComponent()

    companion object {
        private const val HASH_BYTES = 32
        private val RAW_HEX = Regex("^[0-9A-Fa-f]{64}$")

        /** Validate and copy an exact marked Iroha hash. */
        @JvmStatic
        fun fromBytes(bytes: ByteArray): AtomicPrivateSettlementIdentifierV1 =
            AtomicPrivateSettlementIdentifierV1(bytes)

        /** Parse either a raw 64-digit hash or one canonical Norito JSON hash literal. */
        @JvmStatic
        fun parse(value: String): AtomicPrivateSettlementIdentifierV1 {
            require(value == value.trim() && value.isNotEmpty()) {
                "settlement identifier must be exact and non-empty"
            }
            if (RAW_HEX.matches(value)) {
                val decoded = ByteArray(HASH_BYTES)
                decoded.indices.forEach { index ->
                    decoded[index] = value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
                }
                return AtomicPrivateSettlementIdentifierV1(decoded)
            }
            val decoded = HashLiteral.decode(value)
            require(HashLiteral.canonicalize(decoded) == value) {
                "settlement identifier must use the canonical Norito JSON hash literal"
            }
            return AtomicPrivateSettlementIdentifierV1(decoded)
        }
    }
}

/**
 * Bounded, operation-tagged request body produced by the native settlement coordinator.
 *
 * [close] erases the SDK copy. Rendering never includes proof, capsule, carrier, or signature
 * bytes.
 */
class AtomicPrivateSettlementPreparedRequestV1 private constructor(
    @JvmField val operation: AtomicPrivateSettlementOperationV1,
    canonicalJson: ByteArray,
) : AutoCloseable {
    private val body: ByteArray = canonicalJson.copyOf()
    private var closed: Boolean = false

    /** Obtain a defensive body copy for an exact one-shot transport request. */
    fun bytes(): ByteArray = synchronized(this) {
        check(!closed) { "atomic private settlement request is closed" }
        body.copyOf()
    }

    override fun close() = synchronized(this) {
        if (!closed) {
            body.fill(0)
            closed = true
        }
    }

    override fun toString(): String =
        "AtomicPrivateSettlementPreparedRequestV1(operation=$operation, body=[REDACTED])"

    companion object {
        /**
         * Validate one native-prepared JSON object and bind it to a single Torii operation.
         *
         * Duplicate keys, malformed UTF-8, unexpected top-level fields, excessive nesting, and
         * operation/body substitution fail before request signing.
         */
        @JvmStatic
        fun fromNativePreparedJson(
            operation: AtomicPrivateSettlementOperationV1,
            json: ByteArray,
        ): AtomicPrivateSettlementPreparedRequestV1 {
            require(json.isNotEmpty()) { "atomic private settlement request must not be empty" }
            require(json.size <= operation.maximumRequestBytes) {
                "atomic private settlement ${operation.name} request exceeds " +
                    "${operation.maximumRequestBytes} bytes"
            }
            val parsed = parseExactJsonObject(json, "atomic private settlement request")
            require(parsed.keys == operation.topLevelFields) {
                "atomic private settlement ${operation.name} request has the wrong top-level fields"
            }
            val canonical = JsonEncoder.encode(parsed).toByteArray(StandardCharsets.UTF_8)
            require(canonical.size <= operation.maximumRequestBytes) {
                "canonical atomic private settlement request exceeds the operation limit"
            }
            return AtomicPrivateSettlementPreparedRequestV1(operation, canonical)
        }
    }
}

/**
 * Bounded exact JSON response from a settlement route.
 *
 * Restricted proof and encrypted-capsule responses remain opaque. Callers may pass [bytes] to the
 * native wallet/auditor worker; this class never parses or renders secret-bearing inner material.
 */
class AtomicPrivateSettlementJsonResponseV1 internal constructor(
    private val route: String,
    responseBytes: ByteArray,
) : AutoCloseable {
    private val body: ByteArray = responseBytes.copyOf()
    private var closed: Boolean = false

    /** Defensive copy suitable for a native decoder. */
    fun bytes(): ByteArray = synchronized(this) {
        check(!closed) { "atomic private settlement response is closed" }
        body.copyOf()
    }

    override fun close() = synchronized(this) {
        if (!closed) {
            body.fill(0)
            closed = true
        }
    }

    override fun toString(): String =
        "AtomicPrivateSettlementJsonResponseV1(route=$route, body=[REDACTED])"
}

/** Exact-route V1 client for prepared-leg, audit, coordination, and redacted query workflows. */
class AtomicPrivateSettlementToriiClientV1 private constructor(builder: Builder) {
    private val executor: HttpTransportExecutor =
        builder.executor ?: PlatformHttpTransportExecutor.createDefault()
    private val baseUri: URI = requireBaseUri(builder.baseUri)
    private val localSigningContext: LocalSigningContext = checkNotNull(builder.localSigningContext) {
        "localSigningContext must be configured before building a settlement client"
    }
    private val timeout: Duration? = builder.timeout
    private val defaultHeaders: Map<String, String> =
        Collections.unmodifiableMap(LinkedHashMap(builder.defaultHeaders))

    init {
        requireSafeDefaultHeaders(defaultHeaders)
    }

    /** Request one durable availability share with exact sponsor authentication. */
    fun requestAvailabilityShare(
        request: AtomicPrivateSettlementPreparedRequestV1,
        sponsorAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeSponsorMutation(request, AtomicPrivateSettlementOperationV1.AVAILABILITY_SHARE, sponsorAuth)

    /** Request one independently verified Prepare vote with exact sponsor authentication. */
    fun requestPrepareVote(
        request: AtomicPrivateSettlementPreparedRequestV1,
        sponsorAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeSponsorMutation(request, AtomicPrivateSettlementOperationV1.PREPARE_VOTE, sponsorAuth)

    /** Request one complete-barrier Commit vote with exact sponsor authentication. */
    fun requestCommitVote(
        request: AtomicPrivateSettlementPreparedRequestV1,
        sponsorAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeSponsorMutation(request, AtomicPrivateSettlementOperationV1.COMMIT_VOTE, sponsorAuth)

    /** Persist an aggregate Prepare or Commit certificate on one participant node. */
    fun persistPhaseCertificate(
        request: AtomicPrivateSettlementPreparedRequestV1,
        sponsorAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeSponsorMutation(request, AtomicPrivateSettlementOperationV1.PHASE_CERTIFICATE, sponsorAuth)

    /** Promote one complete availability-certified encrypted leg. */
    fun uploadLeg(
        request: AtomicPrivateSettlementPreparedRequestV1,
        sponsorAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeSponsorMutation(request, AtomicPrivateSettlementOperationV1.LEG_UPLOAD, sponsorAuth)

    /** Submit one purpose-separated approval as the exact governed auditor identity. */
    fun submitAuditApproval(
        payloadDigest: AtomicPrivateSettlementIdentifierV1,
        request: AtomicPrivateSettlementPreparedRequestV1,
        auditorSigningContext: OperatorSigningContext,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> {
        requireOperation(request, AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL)
        val path = operationPath(request.operation, payloadDigest)
        return executeMutation(
            path,
            request.bytes(),
            identityHeaders = { target, body ->
                OperatorRequestSigner.buildHeaders(
                    auditorSigningContext,
                    "POST",
                    target,
                    body,
                )
            },
            expectedIdentifier = payloadDigest,
            expectedIdentifierField = "payload_digest",
        )
    }

    /** Submit one sponsor-signed global carrier exactly once. */
    fun submitBundle(
        request: AtomicPrivateSettlementPreparedRequestV1,
        sponsorAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeSponsorMutation(request, AtomicPrivateSettlementOperationV1.BUNDLE_SUBMIT, sponsorAuth)

    /** Read one sponsor-authenticated, redacted local leg lifecycle. */
    fun getLegStatus(
        payloadDigest: AtomicPrivateSettlementIdentifierV1,
        sponsorAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeGet(
            "/v1/nexus/private-settlements/legs/${payloadDigest.pathComponent()}/status",
            RESPONSE_SMALL_MAX_BYTES,
            identityHeaders = { target, body -> sponsorHeaders("GET", target, body, sponsorAuth) },
            expectedIdentifier = payloadDigest,
            expectedIdentifierField = "payload_digest",
        )

    /** Recover the persisted Prepare and Commit certificates for one local leg. */
    fun getPhaseCertificates(
        payloadDigest: AtomicPrivateSettlementIdentifierV1,
        sponsorAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeGet(
            "/v1/nexus/private-settlements/legs/${payloadDigest.pathComponent()}/phase-certificates",
            RESPONSE_SMALL_MAX_BYTES,
            identityHeaders = { target, body -> sponsorHeaders("GET", target, body, sponsorAuth) },
            expectedIdentifier = payloadDigest,
            expectedIdentifierField = "payload_digest",
        )

    /** Fetch proof and opaque delta material as one exact participant validator. */
    fun getCommitteeProof(
        payloadDigest: AtomicPrivateSettlementIdentifierV1,
        validatorSigningContext: OperatorSigningContext,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeGet(
            "/v1/nexus/private-settlements/legs/${payloadDigest.pathComponent()}/committee-proof",
            RESPONSE_RESTRICTED_MAX_BYTES,
            identityHeaders = { target, body ->
                OperatorRequestSigner.buildHeaders(
                    validatorSigningContext,
                    "GET",
                    target,
                    body,
                )
            },
            expectedIdentifier = payloadDigest,
            expectedIdentifierField = null,
        )

    /** Fetch one padded encrypted capsule as one exact governed local auditor. */
    fun getAuditorCapsule(
        payloadDigest: AtomicPrivateSettlementIdentifierV1,
        auditorSigningContext: OperatorSigningContext,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeGet(
            "/v1/nexus/private-settlements/legs/${payloadDigest.pathComponent()}/audit-capsule",
            RESPONSE_RESTRICTED_MAX_BYTES,
            identityHeaders = { target, body ->
                OperatorRequestSigner.buildHeaders(
                    auditorSigningContext,
                    "GET",
                    target,
                    body,
                )
            },
            expectedIdentifier = payloadDigest,
            expectedIdentifierField = null,
        )

    /** Read the public allowlisted lifecycle for one bundle. */
    fun getBundleStatus(
        bundleId: AtomicPrivateSettlementIdentifierV1,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeGet(
            "/v1/nexus/private-settlements/bundles/${bundleId.pathComponent()}",
            RESPONSE_PUBLIC_BUNDLE_MAX_BYTES,
            identityHeaders = null,
            expectedIdentifier = bundleId,
            expectedIdentifierField = null,
        )

    /** Read the public terminal receipt or pending marker for one bundle. */
    fun getBundleReceipt(
        bundleId: AtomicPrivateSettlementIdentifierV1,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> =
        executeGet(
            "/v1/nexus/private-settlements/bundles/${bundleId.pathComponent()}/receipt",
            RESPONSE_RESTRICTED_MAX_BYTES,
            identityHeaders = null,
            expectedIdentifier = bundleId,
            expectedIdentifierField = null,
        )

    private fun executeSponsorMutation(
        request: AtomicPrivateSettlementPreparedRequestV1,
        expectedOperation: AtomicPrivateSettlementOperationV1,
        sponsorAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> {
        requireOperation(request, expectedOperation)
        val body = request.bytes()
        return executeMutation(
            expectedOperation.path,
            body,
            identityHeaders = { target, signedBody ->
                sponsorHeaders("POST", target, signedBody, sponsorAuth)
            },
            expectedIdentifier = null,
            expectedIdentifierField = null,
        )
    }

    private fun executeMutation(
        path: String,
        body: ByteArray,
        identityHeaders: (URI, ByteArray) -> Map<String, String>,
        expectedIdentifier: AtomicPrivateSettlementIdentifierV1?,
        expectedIdentifierField: String?,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> {
        val target = resolvePath(path)
        val headers = requestHeaders(includeContentType = true)
        headers.putAll(identityHeaders(target, body))
        val request = buildRequest("POST", target, body, headers, RESPONSE_RESTRICTED_MAX_BYTES)
        return execute(
            request,
            path,
            RESPONSE_RESTRICTED_MAX_BYTES,
            expectedIdentifier,
            expectedIdentifierField,
        )
    }

    private fun executeGet(
        path: String,
        maximumResponseBytes: Int,
        identityHeaders: ((URI, ByteArray) -> Map<String, String>)?,
        expectedIdentifier: AtomicPrivateSettlementIdentifierV1?,
        expectedIdentifierField: String?,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> {
        val target = resolvePath(path)
        val body = ByteArray(0)
        val headers = requestHeaders(includeContentType = false)
        if (identityHeaders != null) headers.putAll(identityHeaders(target, body))
        val request = buildRequest("GET", target, body, headers, maximumResponseBytes)
        return execute(
            request,
            path,
            maximumResponseBytes,
            expectedIdentifier,
            expectedIdentifierField,
        )
    }

    private fun execute(
        request: TransportRequest,
        route: String,
        maximumResponseBytes: Int,
        expectedIdentifier: AtomicPrivateSettlementIdentifierV1?,
        expectedIdentifierField: String?,
    ): CompletableFuture<AtomicPrivateSettlementJsonResponseV1> {
        val result = CompletableFuture<AtomicPrivateSettlementJsonResponseV1>()
        val execution = try {
            executor.execute(request)
        } catch (error: RuntimeException) {
            result.completeExceptionally(
                AtomicPrivateSettlementToriiExceptionV1("atomic private settlement request failed", error),
            )
            return result
        }
        execution.whenComplete { response, throwable ->
            if (throwable != null) {
                result.completeExceptionally(
                    AtomicPrivateSettlementToriiExceptionV1(
                        "atomic private settlement request failed",
                        unwrapCompletion(throwable),
                    ),
                )
                return@whenComplete
            }
            try {
                result.complete(
                    validateResponse(
                        request,
                        checkNotNull(response),
                        route,
                        maximumResponseBytes,
                        expectedIdentifier,
                        expectedIdentifierField,
                    ),
                )
            } catch (error: RuntimeException) {
                result.completeExceptionally(
                    if (error is AtomicPrivateSettlementToriiExceptionV1) {
                        error
                    } else {
                        AtomicPrivateSettlementToriiExceptionV1(
                            "atomic private settlement response is invalid",
                            error,
                        )
                    },
                )
            }
        }
        return result
    }

    private fun validateResponse(
        request: TransportRequest,
        response: TransportResponse,
        route: String,
        maximumResponseBytes: Int,
        expectedIdentifier: AtomicPrivateSettlementIdentifierV1?,
        expectedIdentifierField: String?,
    ): AtomicPrivateSettlementJsonResponseV1 {
        require(
            !response.redirected &&
                response.finalUri?.toASCIIString() == request.uri.toASCIIString(),
        ) {
            "atomic private settlement response must come from the exact request URL without redirects"
        }
        if (response.statusCode !in 200..299) {
            val rejectCode = HttpErrorMessageExtractor.extractRejectCode(
                response.headers,
                "x-iroha-reject-code",
                ByteArray(0),
            )
            val suffix = if (rejectCode.isNullOrBlank()) "" else "; reject_code=$rejectCode"
            throw AtomicPrivateSettlementToriiExceptionV1(
                "atomic private settlement request failed with HTTP ${response.statusCode}$suffix",
            )
        }
        val body = response.body
        require(body.isNotEmpty()) { "atomic private settlement response must not be empty" }
        require(body.size <= maximumResponseBytes) {
            "atomic private settlement response exceeds $maximumResponseBytes bytes"
        }
        requireExactJsonContentType(response.headers)
        val parsed = parseExactJsonObject(body, "atomic private settlement response")
        validateRouteShape(route, parsed, expectedIdentifier, expectedIdentifierField)
        val canonical = JsonEncoder.encode(parsed).toByteArray(StandardCharsets.UTF_8)
        return AtomicPrivateSettlementJsonResponseV1(route, canonical)
    }

    private fun validateRouteShape(
        route: String,
        parsed: Map<String, Any?>,
        expectedIdentifier: AtomicPrivateSettlementIdentifierV1?,
        expectedIdentifierField: String?,
    ) {
        val expectedFields = responseFields(route)
        require(parsed.keys == expectedFields) {
            "atomic private settlement response has unexpected public fields"
        }
        if (expectedIdentifier != null && expectedIdentifierField != null) {
            require(parsed[expectedIdentifierField] == expectedIdentifier.jsonLiteral()) {
                "atomic private settlement response identifier is substituted"
            }
        }
        if (route.endsWith("/phase-certificates")) {
            require(
                parsed["prepare_certificate"] == null ||
                    parsed["prepare_certificate"] is Map<*, *>,
            ) { "settlement Prepare certificate must be null or an opaque object" }
            require(
                parsed["commit_certificate"] == null ||
                    parsed["commit_certificate"] is Map<*, *>,
            ) { "settlement Commit certificate must be null or an opaque object" }
        }
        if (route.endsWith("/receipt")) {
            validateReceiptIdentity(parsed, checkNotNull(expectedIdentifier))
        } else if (route.contains("/bundles/") && !route.endsWith("/receipt")) {
            validateBundleStatusIdentity(parsed, checkNotNull(expectedIdentifier))
        }
    }

    @Suppress("UNCHECKED_CAST")
    private fun validateBundleStatusIdentity(
        parsed: Map<String, Any?>,
        expected: AtomicPrivateSettlementIdentifierV1,
    ) {
        val manifest = parsed["manifest"] ?: return
        require(manifest is Map<*, *>) { "settlement bundle status manifest must be an object" }
        require((manifest as Map<String, Any?>)["bundle_id"] == expected.jsonLiteral()) {
            "atomic private settlement bundle status is substituted"
        }
    }

    @Suppress("UNCHECKED_CAST")
    private fun validateReceiptIdentity(
        parsed: Map<String, Any?>,
        expected: AtomicPrivateSettlementIdentifierV1,
    ) {
        val status = parsed["status"]
        require(status in setOf("pending", "finalized", "aborted")) {
            "settlement receipt has an unknown terminal tag"
        }
        val value = parsed["value"]
        require(value is Map<*, *>) { "settlement receipt value must be an object" }
        require((value as Map<String, Any?>)["bundle_id"] == expected.jsonLiteral()) {
            "atomic private settlement receipt is substituted"
        }
    }

    private fun sponsorHeaders(
        method: String,
        target: URI,
        body: ByteArray,
        sponsorAuth: ToriiCanonicalRequestAuth,
    ): Map<String, String> {
        val timestampMs = sponsorAuth.timestampMs
        val nonce = sponsorAuth.nonce
        require((timestampMs == null) == (nonce == null)) {
            "timestampMs and nonce must be provided together"
        }
        return if (timestampMs == null) {
            CanonicalRequestSigner.buildHeaders(
                localSigningContext.networkId(),
                method,
                target,
                body,
                sponsorAuth.accountId,
                sponsorAuth.privateKey,
            )
        } else {
            CanonicalRequestSigner.buildHeaders(
                localSigningContext.networkId(),
                method,
                target,
                body,
                sponsorAuth.accountId,
                sponsorAuth.privateKey,
                timestampMs,
                checkNotNull(nonce),
            )
        }
    }

    private fun buildRequest(
        method: String,
        target: URI,
        body: ByteArray,
        headers: Map<String, String>,
        maximumResponseBytes: Int,
    ): TransportRequest {
        TransportSecurity.requireHttpRequestAllowed(
            "AtomicPrivateSettlementToriiClientV1",
            baseUri,
            target,
            headers,
            if (body.isEmpty()) null else body,
        )
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod(method)
            .setTimeout(timeout)
            .setMaximumResponseBytes(maximumResponseBytes.toLong())
        if (body.isNotEmpty()) builder.setBody(body)
        headers.forEach { (name, value) -> builder.addHeader(name, value) }
        return builder.build()
    }

    private fun requestHeaders(includeContentType: Boolean): LinkedHashMap<String, String> {
        val headers = LinkedHashMap(defaultHeaders)
        headers["Accept"] = JSON_MEDIA_TYPE
        headers["Accept-Encoding"] = "identity"
        headers["Cache-Control"] = "no-store"
        headers["Pragma"] = "no-cache"
        if (includeContentType) headers["Content-Type"] = JSON_MEDIA_TYPE
        return headers
    }

    private fun resolvePath(path: String): URI {
        val normalized = path.removePrefix("/")
        val base = baseUri.toString()
        return URI.create(if (base.endsWith('/')) base + normalized else "$base/$normalized")
    }

    /** Builder for an exact-network settlement client. */
    class Builder internal constructor() {
        internal var executor: HttpTransportExecutor? = null
        internal var baseUri: URI = URI.create("http://localhost:8080")
        internal var localSigningContext: LocalSigningContext? = null
        internal var timeout: Duration? = Duration.ofSeconds(30)
        internal val defaultHeaders = LinkedHashMap<String, String>()

        /** Override the HTTP executor. */
        fun executor(executor: HttpTransportExecutor): Builder = apply { this.executor = executor }

        /** Set the exact Torii base, including any deployment path prefix. */
        fun baseUri(baseUri: URI): Builder = apply { this.baseUri = baseUri }

        /** Bind sponsor signatures to one exact genesis-derived network identity. */
        fun localSigningContext(context: LocalSigningContext): Builder = apply {
            this.localSigningContext = context
        }

        /** Set the one-shot request timeout; `null` delegates to the executor. */
        fun timeout(timeout: Duration?): Builder = apply { this.timeout = timeout }

        /** Add one non-authentication, non-content-negotiation deployment header. */
        fun addHeader(name: String, value: String): Builder = apply { defaultHeaders[name] = value }

        /** Replace non-authentication deployment headers. */
        fun defaultHeaders(headers: Map<String, String>?): Builder = apply {
            defaultHeaders.clear()
            if (headers != null) defaultHeaders.putAll(headers)
        }

        /** Build the exact-route client. */
        fun build(): AtomicPrivateSettlementToriiClientV1 =
            AtomicPrivateSettlementToriiClientV1(this)
    }

    companion object {
        private const val JSON_MEDIA_TYPE = "application/json"
        private const val RESPONSE_SMALL_MAX_BYTES = 1024 * 1024
        private const val RESPONSE_PUBLIC_BUNDLE_MAX_BYTES = 8 * 1024 * 1024
        private const val RESPONSE_RESTRICTED_MAX_BYTES = 32 * 1024 * 1024

        private val FORBIDDEN_DEFAULT_HEADERS = setOf(
            "authorization",
            "x-api-token",
            "x-iroha-witness",
            CanonicalRequestSigner.HEADER_ACCOUNT.lowercase(Locale.ROOT),
            CanonicalRequestSigner.HEADER_SIGNATURE.lowercase(Locale.ROOT),
            CanonicalRequestSigner.HEADER_TIMESTAMP_MS.lowercase(Locale.ROOT),
            CanonicalRequestSigner.HEADER_NONCE.lowercase(Locale.ROOT),
            OperatorRequestSigner.HEADER_PUBLIC_KEY.lowercase(Locale.ROOT),
            OperatorRequestSigner.HEADER_TIMESTAMP_MS.lowercase(Locale.ROOT),
            OperatorRequestSigner.HEADER_NONCE.lowercase(Locale.ROOT),
            OperatorRequestSigner.HEADER_SIGNATURE.lowercase(Locale.ROOT),
            "accept",
            "accept-encoding",
            "content-type",
            "content-length",
            "cache-control",
            "pragma",
            "host",
        )

        /** Create a settlement client builder. */
        @JvmStatic
        fun builder(): Builder = Builder()

        private fun requireOperation(
            request: AtomicPrivateSettlementPreparedRequestV1,
            expected: AtomicPrivateSettlementOperationV1,
        ) {
            require(request.operation == expected) {
                "prepared ${request.operation} request cannot be sent to $expected"
            }
        }

        private fun operationPath(
            operation: AtomicPrivateSettlementOperationV1,
            payloadDigest: AtomicPrivateSettlementIdentifierV1,
        ): String = operation.path.replace("{payload_digest}", payloadDigest.pathComponent())

        private fun requireBaseUri(uri: URI): URI {
            require(uri.scheme.equals("http", true) || uri.scheme.equals("https", true)) {
                "settlement Torii base URI must use HTTP or HTTPS"
            }
            require(uri.rawQuery == null && uri.rawFragment == null) {
                "settlement Torii base URI must not contain a query or fragment"
            }
            return uri
        }

        private fun requireSafeDefaultHeaders(headers: Map<String, String>) {
            val forbidden = headers.keys.firstOrNull {
                FORBIDDEN_DEFAULT_HEADERS.contains(it.lowercase(Locale.ROOT))
            }
            require(forbidden == null) {
                "settlement header $forbidden is generated by the exact-route client"
            }
        }

        private fun requireExactJsonContentType(headers: Map<String, List<String>>) {
            val values = headers.entries
                .filter { it.key.equals("Content-Type", true) }
                .flatMap { it.value }
            require(values == listOf(JSON_MEDIA_TYPE)) {
                "atomic private settlement response must have one exact application/json content type"
            }
        }

        private fun responseFields(route: String): Set<String> = when {
            route.endsWith("/availability-shares") ->
                setOf("bundle_id", "payload_digest", "leg_ordinal", "disposition", "share")
            route.endsWith("/prepare-votes") || route.endsWith("/commit-votes") ->
                setOf("bundle_id", "payload_digest", "leg_ordinal", "vote")
            route.endsWith("/phase-certificates") ->
                setOf(
                    "bundle_id",
                    "payload_digest",
                    "leg_ordinal",
                    "lifecycle",
                    "prepare_certificate",
                    "commit_certificate",
                )
            route.endsWith("/certificates") ->
                setOf("bundle_id", "payload_digest", "leg_ordinal", "phase", "lifecycle")
            route.endsWith("/legs") ->
                setOf(
                    "bundle_id",
                    "payload_digest",
                    "leg_ordinal",
                    "disposition",
                    "lifecycle",
                )
            route.endsWith("/audit-approvals") ->
                setOf(
                    "bundle_id",
                    "payload_digest",
                    "leg_ordinal",
                    "collected",
                    "required",
                    "newly_recorded",
                    "lifecycle",
                )
            route.endsWith("/bundles") ->
                setOf("bundle_id", "accepted_at_height", "carrier_id")
            route.endsWith("/status") ->
                setOf(
                    "bundle_id",
                    "payload_digest",
                    "leg_ordinal",
                    "route",
                    "stored_at_height",
                    "lifecycle_height",
                    "expiry_height",
                    "lifecycle",
                )
            route.endsWith("/committee-proof") ->
                setOf(
                    "manifest",
                    "audit_policy",
                    "committee_authority",
                    "statement",
                    "proof",
                    "delta",
                    "audit_approvals",
                    "audit_capsule_digest",
                    "availability",
                    "lifecycle",
                )
            route.endsWith("/audit-capsule") ->
                setOf(
                    "manifest",
                    "audit_policy",
                    "committee_authority",
                    "statement",
                    "delta",
                    "audit_capsule",
                    "availability",
                    "lifecycle",
                )
            route.endsWith("/receipt") -> setOf("status", "value")
            route.contains("/bundles/") -> setOf("manifest", "lifecycle", "finalized_height")
            else -> throw IllegalArgumentException("unknown atomic private settlement route")
        }

        private fun unwrapCompletion(error: Throwable): Throwable =
            if (error is CompletionException && error.cause != null) checkNotNull(error.cause) else error
    }
}

/** Redacted failure from the exact-route settlement client. */
class AtomicPrivateSettlementToriiExceptionV1 : RuntimeException {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)
}

@Suppress("UNCHECKED_CAST")
private fun parseExactJsonObject(bytes: ByteArray, label: String): Map<String, Any?> {
    val decoder = StandardCharsets.UTF_8.newDecoder()
        .onMalformedInput(CodingErrorAction.REPORT)
        .onUnmappableCharacter(CodingErrorAction.REPORT)
    val text = try {
        decoder.decode(ByteBuffer.wrap(bytes)).toString()
    } catch (error: Exception) {
        throw IllegalArgumentException("$label must be exact UTF-8", error)
    }
    val parsed = try {
        JsonParser.parse(text)
    } catch (error: RuntimeException) {
        throw IllegalArgumentException("$label must be one strict JSON object", error)
    }
    require(parsed is Map<*, *>) { "$label must be one strict JSON object" }
    require(parsed.keys.all { it is String }) { "$label object keys must be strings" }
    requireJsonIntegersAreBounded(parsed, label)
    return parsed as Map<String, Any?>
}

private fun requireJsonIntegersAreBounded(value: Any?, label: String) {
    when (value) {
        is Map<*, *> -> value.values.forEach { requireJsonIntegersAreBounded(it, label) }
        is List<*> -> value.forEach { requireJsonIntegersAreBounded(it, label) }
        is BigInteger -> require(value.bitLength() <= 256) { "$label contains an oversized integer" }
    }
}
