package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.client.transport.UrlConnectionTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.CredentialFreeOkHttpTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.StreamingTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.transport.TransportStreamResponse
import java.util.concurrent.CompletableFuture

/** Factory for constructing the canonical JVM transport executor. */
class PlatformHttpTransportExecutor private constructor() {

    companion object {
        /** Returns the canonical executor used by default clients. */
        @JvmStatic
        fun createDefault(): HttpTransportExecutor =
            CredentialIsolatingPlatformExecutor()
    }
}

/**
 * Keeps ordinary compatibility traffic on URLConnection while routing credential-free buffered
 * reads only through a fresh, SDK-owned OkHttp corridor. A missing/incompatible OkHttp runtime is
 * surfaced explicitly; credential-free traffic never falls back to URLConnection.
 */
private class CredentialIsolatingPlatformExecutor :
    HttpTransportExecutor,
    StreamingTransportExecutor {
    private val regular = UrlConnectionTransportExecutor()
    private val credentialFree = lazy(LazyThreadSafetyMode.SYNCHRONIZED) {
        try {
            CredentialFreeOkHttpTransportExecutor()
        } catch (error: LinkageError) {
            throw IllegalStateException(
                "credential-free requests require the pinned SDK OkHttp runtime",
                error,
            )
        }
    }

    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
        if (request.allowAmbientCredentials) {
            regular.execute(request)
        } else {
            credentialFree.value.execute(request)
        }

    override fun openStream(request: TransportRequest): CompletableFuture<TransportStreamResponse> {
        if (request.allowAmbientCredentials) return regular.openStream(request)
        val future = CompletableFuture<TransportStreamResponse>()
        future.completeExceptionally(
            IllegalStateException("credential-free streaming is unsupported"),
        )
        return future
    }

    override fun invalidateAndCancel() {
        regular.invalidateAndCancel()
        if (credentialFree.isInitialized()) credentialFree.value.invalidateAndCancel()
    }
}
