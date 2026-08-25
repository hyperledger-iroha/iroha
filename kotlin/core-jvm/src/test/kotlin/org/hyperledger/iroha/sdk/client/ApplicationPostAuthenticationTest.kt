package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertNotEquals
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.NetworkId

class ApplicationPostAuthenticationTest {
    @Test
    fun ramLfeSignatureSeparatesSameAccountAcrossForeignGenesis() {
        val canonical = NetworkId.parse(
            "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149",
        )
        val foreign = NetworkId.parse(
            "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a9",
        )
        val auth = applicationAuth()
        val requests = mutableListOf<TransportRequest>()
        val executor = object : HttpTransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                requests.add(request)
                return CompletableFuture.completedFuture(
                    TransportResponse.builder().setStatusCode(404).setBody(byteArrayOf()).build(),
                )
            }
        }

        for (networkId in listOf(canonical, foreign)) {
            val config = ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .setLocalSigningContext(LocalSigningContext(networkId))
                .build()
            HttpClientTransport.withExecutor(executor, config)
                .executeRamLfeProgram("lookup", RamLfeExecuteRequest.encrypted("ABCD"), auth)
                .join()
        }

        assertNotEquals(
            requests[0].headers[CanonicalRequestSigner.HEADER_SIGNATURE]?.single(),
            requests[1].headers[CanonicalRequestSigner.HEADER_SIGNATURE]?.single(),
        )
    }
}
