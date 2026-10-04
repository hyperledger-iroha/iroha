package org.hyperledger.iroha.sdk.client.collections

import java.util.concurrent.CancellationException
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.ExecutionException
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference
import org.hyperledger.iroha.sdk.client.ToriiApiException
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.ToriiProtocolException
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.query.ListQuery
import org.hyperledger.iroha.sdk.query.ListQueryException
import org.hyperledger.iroha.sdk.query.Page

/** Executes one `POST <collection>/query`; implemented by the HTTP client. */
internal fun interface CollectionQueryTransport {
    fun query(
        path: String,
        body: ByteArray,
        auth: ToriiCanonicalRequestAuth?,
    ): CompletableFuture<TransportResponse>
}

/**
 * One Torii collection (`specs/torii/collection_queries.md`), read with [ListQuery] through
 * `POST <path>/query`.
 *
 * ```kotlin
 * val page = client.assetDefinitions.page(listQuery { filter(field("owned_by") eq me); limit(50) }).join()
 * for (definition in client.assetDefinitions.iterate(query)) println(definition.id)  // follows next_cursor
 * val everything = client.domains.fetchAll().join()
 * ```
 *
 * Requests carry a canonical account signature when the client is configured with one (or for a
 * view returned by [signedBy]); a signature only widens visibility into restricted dataspaces.
 * Typed rows require complete items: use [json] for `select` projections and aggregates.
 *
 * Failures complete futures with [ToriiApiException] (`code` such as `invalid_filter`), or with
 * [ToriiProtocolException] when a response does not follow the contract.
 *
 * Paging always follows `next_cursor` until it is `null`: history collections scan a bounded
 * budget per page, so a page may hold fewer items than `limit`, or none, and still continue.
 * History collections ([org.hyperledger.iroha.sdk.client.HttpClientTransport.transactions] and
 * `accountTransactions`) reject `sort`, `include_total` and `aggregate` with the matching
 * [ListQueryException.code] before sending.
 */
class ToriiCollection<T> internal constructor(
    private val transport: CollectionQueryTransport,
    /** The collection path, e.g. `/v1/assets/definitions`. */
    @JvmField val path: String,
    private val typedRows: Boolean,
    private val auth: ToriiCanonicalRequestAuth?,
    /** History collections (transactions) reject `sort`, `include_total` and `aggregate`. */
    private val history: Boolean,
    private val project: (JsonObject) -> T,
) {
    /**
     * Fetch one page.
     *
     * @throws IllegalArgumentException when a typed collection is asked for a `select` or aggregate
     */
    @JvmOverloads
    fun page(query: ListQuery = ListQuery.EMPTY): CompletableFuture<Page<T>> {
        requireFullRows(query)
        return fetch(query)
    }

    /**
     * Fetch every matching item, following `next_cursor` until the last page. Cancelling the
     * returned future stops paging and cancels the request in flight.
     */
    @JvmOverloads
    fun fetchAll(query: ListQuery = ListQuery.EMPTY): CompletableFuture<List<T>> {
        requireFullRows(query)
        val result = CompletableFuture<List<T>>()
        val items = ArrayList<T>()
        val inFlight = AtomicReference<CompletableFuture<Page<T>>?>()
        result.whenComplete { _, _ -> if (result.isCancelled) inFlight.get()?.cancel(true) }
        drive(query, items, result, inFlight)
        return result
    }

    /**
     * Lazily iterate every matching item, fetching the next page when the current one is used up.
     * The iteration blocks the calling thread while a page is in flight; [PagedResults.close]
     * cancels it. Iterate it once, like [java.nio.file.DirectoryStream].
     */
    @JvmOverloads
    fun iterate(query: ListQuery = ListQuery.EMPTY): PagedResults<T> {
        requireFullRows(query)
        val pages = PageCursor(query, ::fetch)
        return PagedResults(pages) { page -> page.items.iterator() }
    }

    /** Lazily iterate pages (for [Page.total] or cursor bookkeeping). */
    @JvmOverloads
    fun pages(query: ListQuery = ListQuery.EMPTY): PagedResults<Page<T>> {
        requireFullRows(query)
        val pages = PageCursor(query, ::fetch)
        return PagedResults(pages) { page -> listOf(page).iterator() }
    }

    /** The same collection with raw JSON rows, for `select` projections and aggregate rows. */
    fun json(): ToriiCollection<JsonObject> = ToriiCollection(transport, path, false, auth, history) { it }

    /** A view of this collection whose requests are signed by [auth]. */
    fun signedBy(auth: ToriiCanonicalRequestAuth): ToriiCollection<T> {
        require(auth.timestampMs == null && auth.nonce == null) {
            "collection reads issue one request per page; use per-request freshness (no explicit nonce)"
        }
        return ToriiCollection(transport, path, typedRows, auth, history, project)
    }

    override fun toString(): String = "ToriiCollection($path)"

    private fun requireFullRows(query: ListQuery) {
        if (history) requireHistoryControls(query)
        if (!typedRows) return
        require(query.select == null && query.aggregate == null) {
            "typed $path rows need complete items; use json() for `select` projections and aggregates"
        }
    }

    /** Transaction history is read newest first in bounded scans; Torii rejects these controls. */
    private fun requireHistoryControls(query: ListQuery) {
        if (query.sort.isNotEmpty()) {
            throw ListQueryException(
                "sort",
                "$path is history ordered newest first by (block_height, block_index); `sort` is not supported",
            )
        }
        if (query.includeTotal) {
            throw ListQueryException("include_total", "$path is a history collection; `include_total` is not supported")
        }
        if (query.aggregate != null) {
            throw ListQueryException("aggregate", "$path is a history collection; `aggregate` is not supported")
        }
    }

    private fun fetch(query: ListQuery): CompletableFuture<Page<T>> {
        val body = query.toJson().toJsonBytes()
        val response = try {
            transport.query(path, body, auth)
        } catch (error: RuntimeException) {
            return CompletableFuture<Page<T>>().also { it.completeExceptionally(error) }
        }
        val decoded = response.thenApply { decodePage(it) }
        decoded.whenComplete { _, _ -> if (decoded.isCancelled) response.cancel(true) }
        return decoded
    }

    private fun decodePage(response: TransportResponse): Page<T> {
        val body = response.body
        if (response.statusCode != 200) {
            throw ToriiApiException.fromResponse(response.statusCode, response.headers, body)
        }
        val json = try {
            Json.parse(body)
        } catch (error: IllegalArgumentException) {
            throw ToriiProtocolException(response.statusCode, "$path page is not valid JSON: ${error.message}", error)
        }
        val page = try {
            Page.fromJson(json)
        } catch (error: IllegalArgumentException) {
            throw ToriiProtocolException(response.statusCode, "$path page: ${error.message}", error)
        }
        return page.map { item ->
            val row = item as? JsonObject
                ?: throw ToriiProtocolException(response.statusCode, "$path page items must be JSON objects")
            try {
                project(row)
            } catch (error: IllegalArgumentException) {
                throw ToriiProtocolException(response.statusCode, "$path row: ${error.message}", error)
            } catch (error: IllegalStateException) {
                throw ToriiProtocolException(response.statusCode, "$path row: ${error.message}", error)
            }
        }
    }

    /**
     * Iterative page driver: synchronous completions loop here instead of nesting callbacks, so
     * long result sets cannot overflow the stack.
     */
    private fun drive(
        start: ListQuery,
        items: MutableList<T>,
        result: CompletableFuture<List<T>>,
        inFlight: AtomicReference<CompletableFuture<Page<T>>?>,
    ) {
        var query = start
        while (!result.isDone) {
            val future = fetch(query)
            inFlight.set(future)
            if (result.isCancelled) {
                future.cancel(true)
                return
            }
            if (!future.isDone) {
                future.whenComplete { page, error ->
                    val next = advance(query, page, error, items, result)
                    if (next != null) drive(next, items, result, inFlight)
                }
                return
            }
            val outcome = try {
                Result.success(future.join())
            } catch (error: Throwable) {
                Result.failure(error)
            }
            query = advance(query, outcome.getOrNull(), outcome.exceptionOrNull(), items, result) ?: return
        }
    }

    private fun advance(
        query: ListQuery,
        page: Page<T>?,
        error: Throwable?,
        items: MutableList<T>,
        result: CompletableFuture<List<T>>,
    ): ListQuery? {
        if (error != null) {
            result.completeExceptionally(unwrap(error))
            return null
        }
        page!!
        items.addAll(page.items)
        val next = try {
            nextQuery(query, page)
        } catch (failure: RuntimeException) {
            result.completeExceptionally(failure)
            return null
        }
        if (next == null) result.complete(java.util.Collections.unmodifiableList(ArrayList(items)))
        return next
    }

    internal companion object {
        /** The query for the page after [page], or `null` at the end; rejects a stuck cursor. */
        fun nextQuery(query: ListQuery, page: Page<*>): ListQuery? {
            val cursor = page.nextCursor ?: return null
            if (cursor == query.cursor) {
                throw ToriiProtocolException(200, "Torii returned the same next_cursor again; paging would not advance")
            }
            return query.withCursor(cursor)
        }

        fun unwrap(error: Throwable): Throwable {
            var current = error
            while ((current is CompletionException || current is ExecutionException) && current.cause != null) {
                current = current.cause!!
            }
            return current
        }
    }
}

/** Blocking, cancellable walk over the pages of one query. */
internal class PageCursor<T>(
    start: ListQuery,
    private val fetch: (ListQuery) -> CompletableFuture<Page<T>>,
) {
    private var next: ListQuery? = start
    private val closed = AtomicBoolean(false)
    private val inFlight = AtomicReference<CompletableFuture<Page<T>>?>()

    /** The next page, or `null` after the last one or after [close]. */
    fun nextPage(): Page<T>? {
        val query = next ?: return null
        if (closed.get()) return null
        val future = fetch(query)
        inFlight.set(future)
        if (closed.get()) future.cancel(true)
        val page = try {
            future.get()
        } catch (error: InterruptedException) {
            future.cancel(true)
            Thread.currentThread().interrupt()
            throw CancellationException("interrupted while fetching a page").also { it.initCause(error) }
        } catch (error: CancellationException) {
            if (closed.get()) return null
            throw error
        } catch (error: ExecutionException) {
            val cause = ToriiCollection.unwrap(error)
            if (cause is CancellationException && closed.get()) return null
            throw cause as? RuntimeException ?: CompletionException(cause)
        } finally {
            inFlight.set(null)
        }
        next = ToriiCollection.nextQuery(query, page)
        return page
    }

    fun close() {
        if (closed.compareAndSet(false, true)) inFlight.getAndSet(null)?.cancel(true)
    }
}

/**
 * Lazily fetched results of one collection query: an [Iterable] that can be iterated once and an
 * [AutoCloseable] whose [close] stops paging and cancels the request in flight.
 *
 * Kotlin: `for (row in results)`, `results.asSequence().take(10)`. Java: try-with-resources plus
 * an enhanced `for` loop. Request failures surface from [Iterator.hasNext] as
 * [ToriiApiException] / [ToriiProtocolException]; interruption cancels the request and throws
 * [CancellationException].
 */
class PagedResults<E> internal constructor(
    private val pages: PageCursor<*>,
    private val explode: (Page<*>) -> Iterator<*>,
) : Iterable<E>, AutoCloseable {
    private val started = AtomicBoolean(false)

    override fun iterator(): Iterator<E> {
        check(started.compareAndSet(false, true)) { "PagedResults can be iterated only once; run the query again" }
        return object : Iterator<E> {
            private var current: Iterator<*> = emptyList<E>().iterator()

            override fun hasNext(): Boolean {
                while (!current.hasNext()) {
                    val page = pages.nextPage() ?: return false
                    current = explode(page)
                }
                return true
            }

            @Suppress("UNCHECKED_CAST")
            override fun next(): E {
                if (!hasNext()) throw NoSuchElementException()
                return current.next() as E
            }
        }
    }

    /** Stop paging; a request in flight is cancelled and iteration ends. */
    override fun close() {
        pages.close()
    }
}
