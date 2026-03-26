package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class HttpClientTransportTest {
    @Test
    fun issueIdentifierClaimReceiptForwardsAccountAliasPathLiteral() {
        val executor = CapturingExecutor()
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        transport.issueIdentifierClaimReceipt(
            "alice@wonderland.dataspace",
            IdentifierResolveRequest.encrypted("phone#retail", "abcd"),
        )
            .join()

        assertEquals(
            "https://torii.example/api/v1/accounts/alice%40wonderland.dataspace/identifiers/claim-receipt",
            executor.lastRequest.uri.toString(),
        )
    }

    private class CapturingExecutor : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            return CompletableFuture.completedFuture(
                TransportResponse.builder().setStatusCode(404).setBody(byteArrayOf()).build(),
            )
        }
    }
}
