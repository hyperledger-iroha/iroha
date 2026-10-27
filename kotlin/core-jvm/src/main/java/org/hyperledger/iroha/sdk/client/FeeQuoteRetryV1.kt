package org.hyperledger.iroha.sdk.client

import java.time.Duration
import java.time.ZonedDateTime
import java.time.format.DateTimeFormatter
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.ThreadLocalRandom
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.json.JsonNumber

/** One read-only quote, with fresh authentication per capacity retry and one cancellation owner. */
internal class FeeQuoteRetryV1<T>(
    private val scheduler: ScheduledExecutorService,
    timeout: Duration,
    private val freshAuthentication: Boolean,
    private val request: () -> TransportRequest,
    private val execute: (TransportRequest) -> CompletableFuture<TransportResponse>,
    private val consume: (TransportRequest, TransportResponse) -> T,
    private val failed: (TransportRequest, Throwable) -> Unit,
) {
    val result = CompletableFuture<T>()
    private val lock = Any()
    private val budgetNanos = (if (timeout.isZero) Duration.ofSeconds(15)
        else minOf(timeout, Duration.ofSeconds(15))).toNanos()
    private val started = System.nanoTime()
    private var attempts = 0
    private var lastCapacity: ToriiApiException? = null
    private var active: CompletableFuture<TransportResponse>? = null
    private var waiting: ScheduledFuture<*>? = null
    private var deadline: ScheduledFuture<*>? = null

    fun start(): CompletableFuture<T> {
        result.whenComplete { _, _ -> synchronized(lock) {
            waiting?.cancel(false)
            deadline?.cancel(false)
            active?.cancel(false)
        } }
        synchronized(lock) {
            if (result.isDone) return result
            try {
                deadline = scheduler.schedule({ synchronized(lock) {
                    result.completeExceptionally(lastCapacity ?: TimeoutException("fee quote exceeded its bounded deadline"))
                } }, budgetNanos, TimeUnit.NANOSECONDS)
                dispatch()
            } catch (error: Exception) { result.completeExceptionally(error) }
        }
        return result
    }

    private fun remainingNanos(): Long = budgetNanos - (System.nanoTime() - started)

    private fun dispatch(): Unit = synchronized(lock) {
        if (result.isDone) return@synchronized
        val remaining = remainingNanos()
        if (remaining <= 0) {
            result.completeExceptionally(lastCapacity ?: TimeoutException("fee quote exceeded its bounded deadline"))
            return@synchronized
        }
        var current: TransportRequest? = null
        try {
            val signed = request() // Never replay the prior nonce or signature.
            current = TransportRequest(signed.method, signed.uri, signed.headers, signed.body,
                Duration.ofNanos(remaining), signed.maximumResponseBytes)
            if (result.isDone) return@synchronized
            attempts++
            val sent = current
            val upstream = execute(sent)
            active = upstream
            upstream.whenComplete { response, failure -> synchronized(lock) {
                if (result.isDone) return@whenComplete
                active = null
                try {
                    if (failure != null) throw unwrap(failure)
                    result.complete(consume(sent, response))
                } catch (error: Throwable) {
                    report(sent, error)
                    if (response != null && error is ToriiApiException && error.status == 429 &&
                        freshAuthentication && attempts < 4) {
                        lastCapacity = error
                        val delay = retryDelayMillis(attempts, response, error)
                        if (delay < TimeUnit.NANOSECONDS.toMillis(remainingNanos())) {
                            try { waiting = scheduler.schedule({ dispatch() }, delay, TimeUnit.MILLISECONDS) }
                            catch (scheduleError: Exception) { result.completeExceptionally(scheduleError) }
                        } else result.completeExceptionally(error)
                    } else if (upstream.isCancelled) result.cancel(false)
                    else result.completeExceptionally(error)
                }
            } }
        } catch (error: Throwable) {
            current?.let { report(it, error) }
            result.completeExceptionally(error)
        }
    }

    private fun report(request: TransportRequest, error: Throwable) {
        try { failed(request, error) } catch (observer: Throwable) {
            if (observer !== error) error.addSuppressed(observer)
        }
    }

    private fun unwrap(error: Throwable): Throwable =
        if (error is CompletionException && error.cause != null) unwrap(error.cause!!) else error

    companion object {
        internal fun retryDelayMillis(attempt: Int, response: TransportResponse, error: ToriiApiException): Long {
            val exponential = 250L shl (attempt - 1).coerceIn(0, 2)
            val header = response.headers["Retry-After"]?.singleOrNull()?.trim()
            val headerDelay = header?.let { value ->
                if (value.isNotEmpty() && value.all { it in '0'..'9' }) secondsMillis(value)
                else try {
                    (ZonedDateTime.parse(value, DateTimeFormatter.RFC_1123_DATE_TIME).toInstant().toEpochMilli() -
                        System.currentTimeMillis()).coerceAtLeast(0)
                } catch (_: Exception) { null }
            } ?: 0
            val typedDelay = (error.details?.get("retry_after_seconds") as? JsonNumber)?.text
                ?.takeIf { it.isNotEmpty() && it.all { char -> char in '0'..'9' } }
                ?.let(::secondsMillis) ?: 0
            val minimum = maxOf(exponential, headerDelay, typedDelay)
            return if (minimum > Long.MAX_VALUE - 100) Long.MAX_VALUE
                else minimum + ThreadLocalRandom.current().nextLong(101)
        }

        private fun secondsMillis(value: String): Long = value.toLongOrNull()?.let {
            if (it <= Long.MAX_VALUE / 1_000) it * 1_000 else Long.MAX_VALUE
        } ?: Long.MAX_VALUE
    }
}
