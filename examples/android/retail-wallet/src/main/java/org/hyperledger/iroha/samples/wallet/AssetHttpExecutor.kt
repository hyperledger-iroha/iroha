package org.hyperledger.iroha.samples.wallet

import android.content.Context
import java.io.IOException
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor

/**
 * Minimal [HttpTransportExecutor] that serves canned JSON responses from the app's assets.
 *
 * The executor is only intended for sample/demo usage where we want to exercise the SDK surfaces
 * without connecting to a live Torii endpoint.
 */
class AssetHttpExecutor(
    context: Context,
    private val routes: Map<String, Route>
) : HttpTransportExecutor {

    private val applicationContext = context.applicationContext

    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
        val path = request.uri.path
        val route = routes[path]
            ?: return failed(
                IOException("no canned response registered for ${request.uri}")
            )
        if (request.method != "GET") {
            return failed(
                IOException("asset executor only supports GET requests")
            )
        }
        return CompletableFuture.supplyAsync {
            val body = applicationContext.assets.open(route.assetName).use { it.readBytes() }
            TransportResponse(route.statusCode, body, "asset fixture", emptyMap(), request.uri, false)
        }
    }

    private fun <T> failed(error: Throwable): CompletableFuture<T> =
        CompletableFuture<T>().also { it.completeExceptionally(error) }

    data class Route(
        val assetName: String,
        val statusCode: Int = 200
    )
}
