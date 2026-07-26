package org.hyperledger.iroha.samples.operator.env

import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.time.Duration
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.suspendCancellableCoroutine

data class ToriiStatus(val reachable: Boolean?, val message: String) {
    companion object {
        fun pending(message: String = "pending check"): ToriiStatus = ToriiStatus(null, message)
    }
}

object ToriiHealthProbe {

    suspend fun check(client: HttpClient, endpoint: String): ToriiStatus {
        val uri = resolveUri(endpoint)
        return runCatching {
            val request =
                HttpRequest.newBuilder(uri)
                    .timeout(Duration.ofSeconds(5))
                    .GET()
                    .build()
            val response = client.await(request)
            val status = response.statusCode()
            val message = "HTTP $status"
            val reachable = status < 500
            ToriiStatus(reachable, message)
        }.getOrElse { ex ->
            ToriiStatus(false, ex.message ?: "health probe failed")
        }
    }

    private fun resolveUri(endpoint: String): URI {
        val raw = endpoint.trim().ifEmpty { SampleEnvironment.DEFAULT_TORII }
        val parsed = try {
            URI.create(raw)
        } catch (_: IllegalArgumentException) {
            URI.create(SampleEnvironment.DEFAULT_TORII)
        }
        return if (parsed.path.isNullOrEmpty()) {
            parsed.resolve("/")
        } else {
            parsed
        }
    }

    private suspend fun <T> HttpClient.await(
        request: HttpRequest,
        bodyHandler: HttpResponse.BodyHandler<T> = HttpResponse.BodyHandlers.discarding()
    ): HttpResponse<T> =
        suspendCancellableCoroutine { continuation ->
            val future = sendAsync(request, bodyHandler)
            continuation.invokeOnCancellation { future.cancel(true) }
            future.whenComplete { response, throwable ->
                if (throwable != null) {
                    continuation.resumeWithException(throwable)
                } else {
                    continuation.resume(response)
                }
            }
        }
}
