package org.hyperledger.iroha.sdk.client.transport

import java.util.concurrent.CompletableFuture

/** Platform-neutral transport executor interface to decouple from `java.net.http.HttpClient`. */
interface TransportExecutor {

    fun execute(request: TransportRequest): CompletableFuture<TransportResponse>
}
