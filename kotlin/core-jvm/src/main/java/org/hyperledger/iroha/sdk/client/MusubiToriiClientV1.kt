package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.time.Duration
import java.util.Collections
import java.util.LinkedHashMap
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.musubi.MusubiAliasHistoryEntryV1
import org.hyperledger.iroha.sdk.musubi.MusubiAliasQueryV1
import org.hyperledger.iroha.sdk.musubi.MusubiAliasRecordV1
import org.hyperledger.iroha.sdk.musubi.MusubiArchiveLocationPageV1
import org.hyperledger.iroha.sdk.musubi.MusubiArchiveLocationQueryV1
import org.hyperledger.iroha.sdk.musubi.MusubiArchiveRetentionPageV1
import org.hyperledger.iroha.sdk.musubi.MusubiArchiveRetentionQueryV1
import org.hyperledger.iroha.sdk.musubi.MusubiExactPackageQueryV1
import org.hyperledger.iroha.sdk.musubi.MusubiExactReleaseSnapshotV1
import org.hyperledger.iroha.sdk.musubi.MusubiExactReleaseQueryV1
import org.hyperledger.iroha.sdk.musubi.MusubiJsonV1
import org.hyperledger.iroha.sdk.musubi.MusubiMaintainerDirectoryEntryV1
import org.hyperledger.iroha.sdk.musubi.MusubiOrderedPrefixPageV1
import org.hyperledger.iroha.sdk.musubi.MusubiOrderedPrefixQueryV1
import org.hyperledger.iroha.sdk.musubi.MusubiPackagePageQueryV1
import org.hyperledger.iroha.sdk.musubi.MusubiPackageRecordV1
import org.hyperledger.iroha.sdk.musubi.MusubiPageV1
import org.hyperledger.iroha.sdk.musubi.MusubiProviderBundleAttestationKeyV1
import org.hyperledger.iroha.sdk.musubi.MusubiProviderBundleAttestationRecordV1
import org.hyperledger.iroha.sdk.musubi.MusubiResolverIndexQueryV1
import org.hyperledger.iroha.sdk.musubi.MusubiResolverIndexPageV1
import org.hyperledger.iroha.sdk.musubi.MusubiSearchPageV1
import org.hyperledger.iroha.sdk.musubi.MusubiSearchQueryV1
import org.hyperledger.iroha.sdk.musubi.MusubiVersionV1

/** Read-only client for the twelve typed first-release Musubi registry queries. */
class MusubiToriiClientV1 private constructor(builder: Builder) {
    private val executor: HttpTransportExecutor =
        builder.executor ?: PlatformHttpTransportExecutor.createDefault()
    private val baseUri: URI = builder.baseUri
    private val timeout: Duration? = builder.timeout
    private val defaultHeaders: Map<String, String> =
        Collections.unmodifiableMap(LinkedHashMap(builder.defaultHeaders))
    private val observers: List<ClientObserver> = builder.observers.toList()

    /** Fetches one exact structural package record. */
    fun findExactPackage(
        request: MusubiExactPackageQueryV1,
    ): CompletableFuture<MusubiPackageRecordV1> =
        executePost(EXACT_PACKAGE_PATH, request.toJsonBytes()) { payload ->
            MusubiJsonV1.parseExactPackage(payload).also { it.requireMatches(request) }
        }

    /** Fetches paired home and universal projections for one exact release at finality. */
    fun findExactRelease(
        request: MusubiExactReleaseQueryV1,
    ): CompletableFuture<MusubiExactReleaseSnapshotV1> =
        executePost(EXACT_RELEASE_PATH, request.toJsonBytes()) { payload ->
            MusubiJsonV1.parseExactRelease(payload).also { it.requireMatches(request) }
        }

    /** Fetches one immutable provider proof by its archive/order/provider identity. */
    fun findProviderBundleAttestation(
        request: MusubiProviderBundleAttestationKeyV1,
    ): CompletableFuture<MusubiProviderBundleAttestationRecordV1> =
        executePost(PROVIDER_BUNDLE_ATTESTATION_PATH, request.toJsonBytes()) { payload ->
            MusubiJsonV1.parseProviderBundleAttestation(payload).also {
                it.requireMatches(request)
            }
        }

    /** Reads the finalized universal sparse resolver index. */
    fun findResolverIndex(
        request: MusubiResolverIndexQueryV1,
    ): CompletableFuture<MusubiResolverIndexPageV1> =
        executePost(RESOLVER_INDEX_PATH, request.toJsonBytes()) { payload ->
            MusubiJsonV1.parseResolverPage(payload).also { it.requireMatches(request) }
        }

    /** Lists exact structured versions for a package. */
    fun findVersions(
        request: MusubiPackagePageQueryV1,
    ): CompletableFuture<MusubiPageV1<MusubiVersionV1>> =
        executePost(VERSIONS_PATH, request.toJsonBytes()) { payload ->
            MusubiJsonV1.parseVersionPage(payload).also { it.requireVersionMatches(request) }
        }

    /** Lists accepted owners/maintainers and pending invitations for a package. */
    fun findMaintainers(
        request: MusubiPackagePageQueryV1,
    ): CompletableFuture<MusubiPageV1<MusubiMaintainerDirectoryEntryV1>> =
        executePost(MAINTAINERS_PATH, request.toJsonBytes()) { payload ->
            MusubiJsonV1.parseMaintainerPage(payload).also { page ->
                page.requireMaintainerMatches(request)
            }
        }

    /** Lists renewable SoraFS locations for an archive. */
    fun findArchiveLocations(
        request: MusubiArchiveLocationQueryV1,
    ): CompletableFuture<MusubiArchiveLocationPageV1> =
        executePost(
            ARCHIVE_LOCATIONS_PATH,
            request.toJsonBytes(),
        ) { payload ->
            MusubiJsonV1.parseArchiveLocationPage(payload).also { it.requireMatches(request) }
        }

    /** Classifies a bounded exact archive batch for fail-closed cache retention. */
    fun findArchiveRetention(
        request: MusubiArchiveRetentionQueryV1,
    ): CompletableFuture<MusubiArchiveRetentionPageV1> =
        executePost(
            ARCHIVE_RETENTION_PATH,
            request.toJsonBytes(),
        ) { payload ->
            MusubiJsonV1.parseArchiveRetentionPage(payload).also { it.requireMatches(request) }
        }

    /** Resolves one paid permanent global alias. */
    fun findAlias(request: MusubiAliasQueryV1): CompletableFuture<MusubiAliasRecordV1> =
        executePost(ALIAS_PATH, request.toJsonBytes()) { payload ->
            MusubiJsonV1.parseAlias(payload).also { it.requireMatches(request) }
        }

    /** Lists immutable history for one permanent global alias. */
    fun findAliasHistory(
        request: MusubiAliasQueryV1,
    ): CompletableFuture<MusubiPageV1<MusubiAliasHistoryEntryV1>> =
        executePost(ALIAS_HISTORY_PATH, request.toJsonBytes()) { payload ->
            MusubiJsonV1.parseAliasHistoryPage(payload).also { page ->
                page.requireAliasHistoryMatches(request)
            }
        }

    /** Scans the deterministic public package directory by byte prefix. */
    fun findOrderedPrefix(
        request: MusubiOrderedPrefixQueryV1,
    ): CompletableFuture<MusubiOrderedPrefixPageV1> =
        executePost(
            ORDERED_PREFIX_PATH,
            request.toJsonBytes(),
        ) { payload ->
            MusubiJsonV1.parseOrderedPackagePage(payload).also { it.requireMatches(request) }
        }

    /** Searches package metadata by bounded exact normalized terms. */
    fun search(request: MusubiSearchQueryV1): CompletableFuture<MusubiSearchPageV1> =
        executePost(SEARCH_PATH, request.toJsonBytes()) { payload ->
            MusubiJsonV1.parseSearchPage(payload).also { it.requireMatches(request) }
        }

    fun executor(): HttpTransportExecutor = executor

    private fun <T> executePost(
        path: String,
        body: ByteArray,
        parser: (ByteArray) -> T,
    ): CompletableFuture<T> {
        require(body.size <= REQUEST_MAX_BYTES) {
            "Musubi request exceeds the $REQUEST_MAX_BYTES-byte route limit"
        }
        val request = buildRequest(path, body)
        notifyRequest(request)
        return executor.execute(request).handle { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                val error = MusubiToriiException(
                    "Musubi V1 request failed",
                    cause ?: throwable,
                )
                notifyFailure(request, error)
                throw CompletionException(error)
            }
            try {
                val bodyBytes = response.body
                val clientResponse = ClientResponse(
                    response.statusCode,
                    bodyBytes,
                    response.message,
                    null,
                    HttpErrorMessageExtractor.extractRejectCode(
                        response.headers,
                        "x-iroha-reject-code",
                        bodyBytes,
                    ),
                )
                if (response.statusCode !in 200..299) {
                    throw MusubiToriiException(
                        "Musubi V1 request failed with HTTP ${response.statusCode}: " +
                            (HttpErrorMessageExtractor.extractMessage(bodyBytes) ?: response.message),
                    )
                }
                if (bodyBytes.size > RESPONSE_MAX_BYTES) {
                    throw MusubiToriiException(
                        "Musubi response exceeds the $RESPONSE_MAX_BYTES-byte client limit",
                    )
                }
                if (!hasJsonContentType(response.headers)) {
                    throw MusubiToriiException("Musubi response must use application/json")
                }
                parser(bodyBytes).also { notifyResponse(request, clientResponse) }
            } catch (error: RuntimeException) {
                val wrapped = if (error is MusubiToriiException) {
                    error
                } else {
                    MusubiToriiException("Failed to decode strict Musubi V1 response", error)
                }
                notifyFailure(request, wrapped)
                throw CompletionException(wrapped)
            }
        }
    }

    private fun buildRequest(path: String, body: ByteArray): TransportRequest {
        val normalized = path.removePrefix("/")
        val base = baseUri.toString()
        val target = URI.create(if (base.endsWith('/')) base + normalized else "$base/$normalized")
        val headers = LinkedHashMap(defaultHeaders)
        ensureHeader(headers, "Accept", "application/json")
        ensureHeader(headers, "Content-Type", "application/json")
        TransportSecurity.requireHttpRequestAllowed(
            "MusubiToriiClientV1",
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
            .setMaximumResponseBytes(RESPONSE_MAX_BYTES.toLong())
        headers.forEach { (name, value) -> builder.addHeader(name, value) }
        return builder.build()
    }

    private fun notifyRequest(request: TransportRequest) = observers.forEach { it.onRequest(request) }

    private fun notifyResponse(request: TransportRequest, response: ClientResponse) =
        observers.forEach { it.onResponse(request, response) }

    private fun notifyFailure(request: TransportRequest, error: Throwable) =
        observers.forEach { it.onFailure(request, error) }

    class Builder internal constructor() {
        internal var executor: HttpTransportExecutor? = null
        internal var baseUri: URI = URI.create("http://localhost:8080")
        internal var timeout: Duration? = Duration.ofSeconds(15)
        internal val defaultHeaders = LinkedHashMap<String, String>()
        internal val observers = ArrayList<ClientObserver>()

        fun executor(executor: HttpTransportExecutor): Builder = apply { this.executor = executor }
        fun baseUri(baseUri: URI): Builder = apply { this.baseUri = baseUri }
        fun timeout(timeout: Duration?): Builder = apply { this.timeout = timeout }
        fun addHeader(name: String, value: String): Builder = apply { defaultHeaders[name] = value }
        fun defaultHeaders(headers: Map<String, String>?): Builder = apply {
            defaultHeaders.clear()
            if (headers != null) defaultHeaders.putAll(headers)
        }
        fun addObserver(observer: ClientObserver?): Builder = apply {
            if (observer != null) observers.add(observer)
        }
        fun build(): MusubiToriiClientV1 = MusubiToriiClientV1(this)
    }

    companion object {
        const val EXACT_PACKAGE_PATH = "/v1/musubi/queries/exact-package"
        const val EXACT_RELEASE_PATH = "/v1/musubi/queries/exact-release"
        const val PROVIDER_BUNDLE_ATTESTATION_PATH =
            "/v1/musubi/queries/provider-bundle-attestation"
        const val RESOLVER_INDEX_PATH = "/v1/musubi/queries/resolver-index"
        const val VERSIONS_PATH = "/v1/musubi/queries/versions"
        const val MAINTAINERS_PATH = "/v1/musubi/queries/maintainers"
        const val ARCHIVE_LOCATIONS_PATH = "/v1/musubi/queries/archive-locations"
        const val ARCHIVE_RETENTION_PATH = "/v1/musubi/queries/archive-retention"
        const val ALIAS_PATH = "/v1/musubi/queries/alias"
        const val ALIAS_HISTORY_PATH = "/v1/musubi/queries/alias-history"
        const val ORDERED_PREFIX_PATH = "/v1/musubi/queries/ordered-prefix"
        const val SEARCH_PATH = "/v1/musubi/queries/search"

        private const val REQUEST_MAX_BYTES = 64 * 1024
        // Exact-release JSON repeats the bounded dependency vector in both registry projections.
        private const val RESPONSE_MAX_BYTES = 32 * 1024 * 1024

        @JvmStatic fun builder(): Builder = Builder()

        private fun ensureHeader(
            headers: MutableMap<String, String>,
            name: String,
            value: String,
        ) {
            val existing = headers.keys.firstOrNull { it.equals(name, ignoreCase = true) }
            headers[existing ?: name] = value
        }

        private fun hasJsonContentType(headers: Map<String, List<String>>): Boolean =
            headers.entries
                .firstOrNull { it.key.equals("Content-Type", ignoreCase = true) }
                ?.value
                ?.any { it.substringBefore(';').trim().equals("application/json", true) }
                ?: false
    }
}

/** Failure returned by [MusubiToriiClientV1]. */
class MusubiToriiException : RuntimeException {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)
}
