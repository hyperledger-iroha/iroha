package org.hyperledger.iroha.sdk.sorafs

import org.hyperledger.iroha.sdk.client.CanonicalRequestSigner
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.client.PlatformHttpTransportExecutor
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.TransportSecurity
import org.hyperledger.iroha.sdk.client.stream.ServerSentEvent
import org.hyperledger.iroha.sdk.client.stream.ToriiEventStream
import org.hyperledger.iroha.sdk.client.stream.ToriiEventStreamClient
import org.hyperledger.iroha.sdk.client.stream.ToriiEventStreamListener
import org.hyperledger.iroha.sdk.client.stream.ToriiEventStreamOptions
import org.hyperledger.iroha.sdk.client.transport.StreamingTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.core.model.NetworkId
import java.math.BigInteger
import java.net.URI
import java.time.Duration
import java.util.LinkedHashMap
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.function.Function

/**
 * Closed authenticated client for committed SoraFS reputation V1 projections.
 *
 * Every method performs exactly one GET signed over the final path, canonical query, and empty
 * body. Raw authentication headers and witness authentication are intentionally not exposed.
 */
class SorafsReputationClient(
    @JvmField val baseUri: URI,
    @JvmField val networkId: NetworkId,
    private val transport: TransportExecutor,
    @JvmField val timeout: Duration?,
) {
    /** Creates a client with executor defaults for request timeouts. */
    constructor(baseUri: URI, networkId: NetworkId, transport: TransportExecutor) :
        this(baseUri, networkId, transport, null)

    /** Creates a client backed by the canonical platform transport. */
    constructor(baseUri: URI, networkId: NetworkId) :
        this(baseUri, networkId, PlatformHttpTransportExecutor.createDefault(), null)

    init {
        require(baseUri.isAbsolute && baseUri.rawQuery == null && baseUri.rawFragment == null) {
            "baseUri must be an absolute URI without query or fragment"
        }
        require(timeout == null || !timeout.isNegative) { "timeout must be non-negative" }
    }

    /** Fetches the latest committed snapshot, or `null` when no snapshot exists. */
    @JvmOverloads
    fun getLatest(
        canonicalAuth: ToriiCanonicalRequestAuth,
        limit: Int? = null,
    ): CompletableFuture<SorafsReputationSnapshotSummaryV1?> =
        fetchOptional(
            path = LATEST_PATH,
            query = snapshotQuery(limit),
            canonicalAuth = canonicalAuth,
            parser = Function { payload ->
                SorafsReputationJsonParser.parseSnapshot(payload).also { snapshot ->
                    check(limit == null || snapshot.limit == limit) {
                        "SoraFS reputation latest response limit does not match the request"
                    }
                }
            },
            context = "SoraFS reputation latest",
        )

    /** Fetches one provider and its Merkle proof from the latest committed snapshot. */
    fun getProvider(
        providerId: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<SorafsReputationProviderResponseV1?> {
        val canonicalProvider = normalizeProviderId(providerId)
        return fetchOptional(
            path = "$PROVIDER_PATH_PREFIX$canonicalProvider",
            query = emptyMap(),
            canonicalAuth = canonicalAuth,
            parser = Function { payload ->
                SorafsReputationJsonParser.parseProviderResponse(payload).also { response ->
                    check(response.provider.providerId == canonicalProvider) {
                        "SoraFS reputation provider response does not match the requested provider"
                    }
                }
            },
            context = "SoraFS reputation provider",
        )
    }

    /** Fetches one immutable committed snapshot by its exact nonzero lowercase identifier. */
    @JvmOverloads
    fun getSnapshot(
        snapshotIdHex: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
        limit: Int? = null,
    ): CompletableFuture<SorafsReputationSnapshotSummaryV1?> {
        val canonicalSnapshot = normalizeSnapshotId(snapshotIdHex)
        return fetchOptional(
            path = "$SNAPSHOT_PATH_PREFIX$canonicalSnapshot",
            query = snapshotQuery(limit),
            canonicalAuth = canonicalAuth,
            parser = Function { payload ->
                SorafsReputationJsonParser.parseSnapshot(payload).also { snapshot ->
                    check(snapshot.snapshotIdHex == canonicalSnapshot) {
                        "SoraFS reputation snapshot response does not match the requested snapshot"
                    }
                    check(limit == null || snapshot.limit == limit) {
                        "SoraFS reputation snapshot response limit does not match the request"
                    }
                }
            },
            context = "SoraFS reputation snapshot",
        )
    }

    /** Fetches active scoring weights from the latest committed snapshot. */
    fun getWeights(
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<SorafsReputationWeightsResponseV1> =
        fetchRequired(
            path = WEIGHTS_PATH,
            query = emptyMap(),
            canonicalAuth = canonicalAuth,
            parser = Function(SorafsReputationJsonParser::parseWeightsResponse),
            context = "SoraFS reputation weights",
        )

    /** Fetches a bounded finalized event page. */
    @JvmOverloads
    fun listEvents(
        canonicalAuth: ToriiCanonicalRequestAuth,
        since: String? = null,
        limit: Int? = null,
    ): CompletableFuture<SorafsReputationEventsResponseV1> {
        val query = eventQuery(since, limit)
        return fetchRequired(
            path = EVENTS_PATH,
            query = query,
            canonicalAuth = canonicalAuth,
            parser = Function { payload ->
                SorafsReputationJsonParser.parseEventPage(payload).also { page ->
                    check(page.since == query["since"]) {
                        "SoraFS reputation events response since does not match the request"
                    }
                    check(limit == null || page.limit == limit) {
                        "SoraFS reputation events response limit does not match the request"
                    }
                }
            },
            context = "SoraFS reputation events",
        )
    }

    /**
     * Opens the authenticated live event stream using a true streaming executor.
     *
     * The call performs one transport attempt and never reconnects, so a fixed caller nonce is
     * never replayed. `Last-Event-ID`, resume cursors, raw headers, and buffered fallback are not
     * part of this API.
     */
    @JvmOverloads
    fun openEventStream(
        canonicalAuth: ToriiCanonicalRequestAuth,
        listener: SorafsReputationEventStreamListener,
        since: String? = null,
        limit: Int? = null,
    ): ToriiEventStream {
        require(transport is StreamingTransportExecutor) {
            "SoraFS reputation SSE requires a StreamingTransportExecutor; buffered fallback is unsupported"
        }
        val query = eventQuery(since, limit)
        val options = ToriiEventStreamOptions(
            queryParameters = query,
            timeout = timeout,
        )
        val streamClient = ToriiEventStreamClient.builder()
            .setBaseUri(baseUri)
            .setTransportExecutor(transport)
            .canonicalRequestAuth(LocalSigningContext(networkId), canonicalAuth)
            .build()
        return streamClient.openSseStream(
            EVENTS_STREAM_PATH,
            options,
            object : ToriiEventStreamListener {
                override fun onOpen() {
                    listener.onOpen()
                }

                override fun onEvent(event: ServerSentEvent) {
                    when (event.event) {
                        "reputation_snapshot" -> {
                            val parsed = SorafsReputationJsonParser.parseEventJson(event.data)
                            val id = normalizeU64(event.id, "SoraFS reputation SSE event id", false)
                            check(id == parsed.sequence) {
                                "SoraFS reputation SSE event id must equal data.sequence"
                            }
                            listener.onSnapshot(parsed)
                        }
                        "lagged" -> {
                            check(event.id == null) {
                                "SoraFS reputation lagged SSE frames must not carry an id"
                            }
                            listener.onLagged(
                                normalizeU64(
                                    event.data,
                                    "SoraFS reputation SSE lagged count",
                                    false,
                                ),
                            )
                        }
                        else -> throw IllegalStateException(
                            "unsupported SoraFS reputation SSE event `${event.event}`",
                        )
                    }
                }

                override fun onClosed() {
                    listener.onClosed()
                }

                override fun onError(error: Throwable) {
                    listener.onError(error)
                }
            },
        )
    }

    private fun <T> fetchOptional(
        path: String,
        query: Map<String, String>,
        canonicalAuth: ToriiCanonicalRequestAuth,
        parser: Function<ByteArray, T>,
        context: String,
    ): CompletableFuture<T?> =
        execute(path, query, canonicalAuth, parser, context, true)

    private fun <T> fetchRequired(
        path: String,
        query: Map<String, String>,
        canonicalAuth: ToriiCanonicalRequestAuth,
        parser: Function<ByteArray, T>,
        context: String,
    ): CompletableFuture<T> =
        execute(path, query, canonicalAuth, parser, context, false)
            .thenApply { value -> checkNotNull(value) }

    private fun <T> execute(
        path: String,
        query: Map<String, String>,
        canonicalAuth: ToriiCanonicalRequestAuth,
        parser: Function<ByteArray, T>,
        context: String,
        nullableOnNotFound: Boolean,
    ): CompletableFuture<T?> {
        val target = buildTarget(path, query)
        val headers = canonicalHeaders(target, canonicalAuth)
        TransportSecurity.requireHttpRequestAllowed(
            "SorafsReputationClient",
            baseUri,
            target,
            headers,
            null,
        )
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", "application/json")
            .setMaximumResponseBytes(MAX_RESPONSE_BYTES)
        if (timeout != null) {
            builder.setTimeout(timeout)
        }
        for ((name, value) in headers) {
            builder.addHeader(name, value)
        }
        val request = builder.build()
        val future = CompletableFuture<T?>()
        transport.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                future.completeExceptionally(
                    RuntimeException("$context request failed", unwrapCompletion(throwable)),
                )
                return@whenComplete
            }
            try {
                response!!
                if (nullableOnNotFound && response.statusCode == 404) {
                    future.complete(null)
                    return@whenComplete
                }
                check(response.statusCode == 200) {
                    "$context request failed with status ${response.statusCode}"
                }
                future.complete(parser.apply(response.body))
            } catch (error: RuntimeException) {
                future.completeExceptionally(error)
            }
        }
        return future
    }

    private fun canonicalHeaders(
        target: URI,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): Map<String, String> {
        val timestampMs = canonicalAuth.timestampMs
        val nonce = canonicalAuth.nonce
        require((timestampMs == null) == (nonce == null)) {
            "timestampMs and nonce must be provided together"
        }
        return if (timestampMs == null) {
            CanonicalRequestSigner.buildHeaders(
                networkId,
                "GET",
                target,
                EMPTY_BODY,
                canonicalAuth.accountId,
                canonicalAuth.privateKey,
            )
        } else {
            require(timestampMs >= 0) { "timestampMs must be non-negative" }
            CanonicalRequestSigner.buildHeaders(
                networkId,
                "GET",
                target,
                EMPTY_BODY,
                canonicalAuth.accountId,
                canonicalAuth.privateKey,
                timestampMs,
                nonce!!,
            )
        }
    }

    private fun buildTarget(path: String, query: Map<String, String>): URI {
        val base = baseUri.toString()
        val normalizedPath = if (path.startsWith("/")) path.substring(1) else path
        val resolved = if (base.endsWith("/")) "$base$normalizedPath" else "$base/$normalizedPath"
        if (query.isEmpty()) {
            return URI.create(resolved)
        }
        return URI.create(
            resolved + "?" + query.entries.joinToString("&") { (name, value) -> "$name=$value" },
        )
    }

    private fun snapshotQuery(limit: Int?): Map<String, String> {
        if (limit == null) return emptyMap()
        return linkedMapOf("limit" to normalizeLimit(limit))
    }

    private fun eventQuery(since: String?, limit: Int?): Map<String, String> {
        val query = LinkedHashMap<String, String>()
        if (since != null) {
            query["since"] = normalizeU64(since, "since", true)
        }
        if (limit != null) {
            query["limit"] = normalizeLimit(limit)
        }
        return query
    }

    private fun normalizeLimit(limit: Int): String {
        require(limit in 1..500) { "limit must be between 1 and 500" }
        return limit.toString()
    }

    private fun normalizeSnapshotId(snapshotIdHex: String): String {
        require(
            snapshotIdHex.length == 32 &&
                snapshotIdHex.all { it in '0'..'9' || it in 'a'..'f' },
        ) {
            "snapshotIdHex must be exactly 32 lowercase hexadecimal characters"
        }
        require(snapshotIdHex.any { it != '0' }) { "snapshotIdHex must be nonzero" }
        return snapshotIdHex
    }

    private fun normalizeProviderId(providerId: String): String {
        require(
            providerId.isNotEmpty() &&
                providerId.length <= 256 &&
                providerId != "." &&
                providerId != "..",
        ) {
            "providerId must contain 1..256 ASCII characters from [A-Za-z0-9_.:-] and not be a dot segment"
        }
        require(providerId.all(::isProviderCharacter)) {
            "providerId must contain 1..256 ASCII characters from [A-Za-z0-9_.:-] and not be a dot segment"
        }
        return providerId
    }

    private fun isProviderCharacter(value: Char): Boolean =
        value in 'A'..'Z' ||
            value in 'a'..'z' ||
            value in '0'..'9' ||
            value == '_' ||
            value == '.' ||
            value == ':' ||
            value == '-'

    private fun normalizeU64(value: String?, context: String, allowZero: Boolean): String {
        require(value != null && value.matches(Regex("(?:0|[1-9][0-9]*)"))) {
            "$context must be a canonical unsigned decimal integer"
        }
        val parsed = BigInteger(value)
        require(parsed <= U64_MAX && (allowZero || parsed.signum() > 0)) {
            "$context must fit ${if (allowZero) "u64" else "positive u64"}"
        }
        return value
    }

    private fun unwrapCompletion(error: Throwable): Throwable =
        if (error is CompletionException && error.cause != null) error.cause!! else error

    private companion object {
        private const val LATEST_PATH = "/v1/sorafs/reputation/latest"
        private const val PROVIDER_PATH_PREFIX = "/v1/sorafs/reputation/providers/"
        private const val SNAPSHOT_PATH_PREFIX = "/v1/sorafs/reputation/snapshots/"
        private const val WEIGHTS_PATH = "/v1/sorafs/reputation/weights"
        private const val EVENTS_PATH = "/v1/sorafs/reputation/events"
        private const val EVENTS_STREAM_PATH = "/v1/sorafs/reputation/events/stream"
        private const val MAX_RESPONSE_BYTES = 4_194_304L
        private val U64_MAX = BigInteger("18446744073709551615")
        private val EMPTY_BODY = ByteArray(0)
    }
}
