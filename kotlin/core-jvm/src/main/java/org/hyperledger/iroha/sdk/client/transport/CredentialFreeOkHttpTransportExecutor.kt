package org.hyperledger.iroha.sdk.client.transport

import java.io.ByteArrayInputStream
import java.io.IOException
import java.net.Proxy
import java.util.Locale
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import okhttp3.Authenticator
import okhttp3.Call
import okhttp3.Callback
import okhttp3.CookieJar
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor

/**
 * SDK-owned buffered corridor for requests that forbid ambient credentials.
 *
 * The client is created from a fresh builder and cannot inherit a caller cookie jar,
 * authenticator, proxy, redirect policy, or interceptor. Retrying failed connections is also
 * disabled. Streaming is deliberately absent because the caller-owned lifetime would weaken the
 * bounded, auditable public-read contract.
 */
internal class CredentialFreeOkHttpTransportExecutor(
    private val maximumResponseBytes: Long =
        UrlConnectionTransportExecutor.DEFAULT_MAXIMUM_RESPONSE_BYTES,
) : HttpTransportExecutor {
    private val client = OkHttpClient.Builder()
        .cookieJar(CookieJar.NO_COOKIES)
        .authenticator(Authenticator.NONE)
        .proxyAuthenticator(Authenticator.NONE)
        .proxy(Proxy.NO_PROXY)
        .followRedirects(false)
        .followSslRedirects(false)
        .retryOnConnectionFailure(false)
        .build()
    private val trackedCalls = ConcurrentHashMap.newKeySet<Call>()

    init {
        require(maximumResponseBytes in 1..Int.MAX_VALUE.toLong()) {
            "maximumResponseBytes must be between 1 and ${Int.MAX_VALUE}"
        }
    }

    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
        require(!request.allowAmbientCredentials) {
            "CredentialFreeOkHttpTransportExecutor accepts credential-free requests only"
        }
        require(request.uri.rawUserInfo == null) {
            "credential-free requests reject URI user-info"
        }
        val call = client.newCall(buildRequest(request))
        trackedCalls.add(call)
        request.timeout?.let { timeout ->
            call.timeout().timeout(maxOf(0L, timeout.toMillis()), TimeUnit.MILLISECONDS)
        }
        val future = CompletableFuture<TransportResponse>()
        call.enqueue(
            object : Callback {
                override fun onFailure(call: Call, e: IOException) {
                    future.completeExceptionally(e)
                }

                override fun onResponse(call: Call, response: Response) {
                    try {
                        response.use {
                            val headers = it.headers.toMultimap()
                            val responseLimit = minOf(
                                maximumResponseBytes,
                                request.maximumResponseBytes ?: maximumResponseBytes,
                            )
                            val body = readBody(
                                it,
                                request.method,
                                headers,
                                responseLimit,
                            )
                            future.complete(
                                TransportResponse(
                                    it.code,
                                    body,
                                    it.message,
                                    headers,
                                ),
                            )
                        }
                    } catch (error: IOException) {
                        future.completeExceptionally(error)
                    }
                }
            },
        )
        future.whenComplete { _, _ ->
            trackedCalls.remove(call)
            if (future.isCancelled) call.cancel()
        }
        return future
    }

    override fun invalidateAndCancel() {
        trackedCalls.forEach(Call::cancel)
        trackedCalls.clear()
        client.dispatcher.cancelAll()
        client.connectionPool.evictAll()
        client.cache?.close()
        client.dispatcher.executorService.shutdown()
    }

    private fun buildRequest(request: TransportRequest): Request {
        val builder = Request.Builder().url(request.uri.toString())
        for ((name, values) in request.headers) {
            for (value in values) builder.addHeader(name, value)
        }
        val payload = request.body
        val normalizedMethod = request.method.trim().uppercase(Locale.ROOT)
        val body = if (payload.isEmpty() && (normalizedMethod == "GET" || normalizedMethod == "HEAD")) {
            null
        } else {
            payload.toRequestBody(null)
        }
        return builder.method(request.method, body).build()
    }

    private fun readBody(
        response: Response,
        requestMethod: String,
        headers: Map<String, List<String>>,
        maximumBytes: Long,
    ): ByteArray {
        val declaredLength = canonicalContentLength(headers)
        if (declaredLength != null && response.header("Transfer-Encoding") != null) {
            throw IOException(
                "HTTP response must not combine Content-Length with Transfer-Encoding",
            )
        }
        if (!responseMayHaveBody(requestMethod, response.code)) {
            response.body?.close()
            return ByteArray(0)
        }
        val bufferedLength = if (hasIdentityContentEncoding(response)) declaredLength else null
        if (bufferedLength != null && bufferedLength > maximumBytes) {
            throw IOException(
                "HTTP response Content-Length $bufferedLength exceeds the " +
                    "$maximumBytes-byte limit",
            )
        }
        val input = response.body?.byteStream() ?: ByteArrayInputStream(ByteArray(0))
        return input.use {
            UrlConnectionTransportExecutor.readBoundedBody(it, maximumBytes, bufferedLength)
        }
    }

    private fun canonicalContentLength(headers: Map<String, List<String>>): Long? {
        val values = headers.entries
            .asSequence()
            .filter { (name, _) -> name.equals("Content-Length", ignoreCase = true) }
            .flatMap { (_, values) -> values.asSequence() }
            .toList()
        if (values.isEmpty()) return null
        if (values.size != 1) {
            throw IOException("HTTP response contains multiple Content-Length values")
        }
        val value = values.single()
        if (value.isEmpty() || (value.length > 1 && value[0] == '0') ||
            value.any { it !in '0'..'9' }
        ) {
            throw IOException("HTTP response Content-Length must be a canonical unsigned decimal")
        }
        return value.toLongOrNull()
            ?: throw IOException("HTTP response Content-Length exceeds the supported range")
    }

    private fun hasIdentityContentEncoding(response: Response): Boolean {
        val encodings = response.headers.values("Content-Encoding")
        return encodings.isEmpty() || encodings.all { value ->
            value.split(',').all { it.trim().equals("identity", ignoreCase = true) }
        }
    }

    private fun responseMayHaveBody(method: String, status: Int): Boolean =
        !method.equals("HEAD", ignoreCase = true) &&
            status !in 100..199 && status != 204 && status != 304
}
