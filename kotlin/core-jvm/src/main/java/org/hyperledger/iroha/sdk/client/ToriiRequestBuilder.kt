package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.time.Duration
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.norito.NoritoException
import org.hyperledger.iroha.sdk.tx.norito.SignedTransactionEncoder

/** Builds Torii HTTP requests for submitting signed transactions. */
internal object ToriiRequestBuilder {
    private const val SUBMIT_PATH = "/transaction"
    private const val STATUS_PATH = "/v1/pipeline/transactions/status"

    @JvmStatic
    fun buildSubmitRequest(
        baseUri: URI,
        transaction: SignedTransaction,
        timeout: Duration?,
        extraHeaders: Map<String, String>?
    ): TransportRequest {
        val target = resolve(baseUri, SUBMIT_PATH)
        val norito: ByteArray
        try {
            norito = SignedTransactionEncoder.encodeVersioned(transaction)
        } catch (ex: NoritoException) {
            throw IllegalStateException("Failed to encode signed transaction", ex)
        }
        TransportSecurity.requireHttpRequestAllowed(
            "HttpClientTransport",
            baseUri,
            target,
            extraHeaders,
            norito,
        )
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .addHeader("Content-Type", "application/x-norito")
            .addHeader("Accept", "application/x-norito, application/json")
            .setBody(norito)
        applyHeaders(builder, extraHeaders)
        applyTimeout(builder, timeout)
        return builder.build()
    }

    @JvmStatic
    fun buildStatusRequest(
        baseUri: URI,
        hashHex: String,
        timeout: Duration?,
        extraHeaders: Map<String, String>?
    ): TransportRequest {
        val normalizedHash = hashHex.trim()
        require(normalizedHash.isNotEmpty()) { "hashHex must not be blank" }
        val target = resolve(baseUri, "$STATUS_PATH?hash=$normalizedHash")
        TransportSecurity.requireHttpRequestAllowed(
            "HttpClientTransport",
            baseUri,
            target,
            extraHeaders,
            null,
        )
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", "application/json")
        applyHeaders(builder, extraHeaders)
        applyTimeout(builder, timeout)
        return builder.build()
    }

    private fun resolve(baseUri: URI, path: String): URI {
        val base = baseUri.toString()
        val normalizedPath = if (path.startsWith("/")) path.substring(1) else path
        val joined = if (base.endsWith("/")) base + normalizedPath else "$base/$normalizedPath"
        return URI.create(joined)
    }

    private fun applyHeaders(builder: TransportRequest.Builder, headers: Map<String, String>?) {
        if (headers.isNullOrEmpty()) return
        for ((key, value) in headers) {
            builder.addHeader(key, value)
        }
    }

    private fun applyTimeout(builder: TransportRequest.Builder, timeout: Duration?) {
        if (timeout == null || timeout.isZero || timeout.isNegative) return
        builder.setTimeout(timeout)
    }
}
