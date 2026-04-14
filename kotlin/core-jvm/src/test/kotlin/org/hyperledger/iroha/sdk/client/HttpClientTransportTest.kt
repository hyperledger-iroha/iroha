package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
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
        ).join()

        assertEquals(
            "https://torii.example/api/v1/accounts/alice%40wonderland.dataspace/identifiers/claim-receipt",
            executor.lastRequest.uri.toString(),
        )
    }

    @Test
    fun deployContractPostsAliasFirstPayloadAndParsesResponse() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "ok": true,
                  "contract_alias": "router::universal",
                  "contract_address": "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                  "previous_contract_address": null,
                  "upgraded": false,
                  "dataspace": "router",
                  "deploy_nonce": 7,
                  "tx_hash_hex": "${"11".repeat(32)}",
                  "code_hash_hex": "${"22".repeat(32)}",
                  "abi_hash_hex": "${"33".repeat(32)}"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.deployContract(
            authority = "alice",
            privateKey = "privkey",
            codeB64 = "AQID",
            contractAlias = "router::universal",
        ).join()

        assertTrue(response.isPresent)
        val parsed = response.get()
        assertTrue(parsed.ok)
        assertEquals("router::universal", parsed.contractAlias)
        assertEquals("router", parsed.dataspace)
        assertEquals(7L, parsed.deployNonce)
        assertEquals("11".repeat(32), parsed.txHashHex)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/contracts/deploy", request.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val payload = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals("alice", payload["authority"])
        assertEquals("privkey", payload["private_key"])
        assertEquals("AQID", payload["code_b64"])
        assertEquals("router::universal", payload["contract_alias"])
        assertFalse(payload.containsKey("lease_expiry_ms"))
    }

    @Test
    fun callContractPostsSelectorPayloadAndParsesResponse() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "ok": true,
                  "submitted": true,
                  "dataspace": "router",
                  "code_hash_hex": "${"44".repeat(32)}",
                  "abi_hash_hex": "${"55".repeat(32)}",
                  "creation_time_ms": 1712345678901,
                  "contract_address": "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                  "tx_hash_hex": "${"66".repeat(32)}",
                  "entrypoint": "contribute",
                  "transaction_scaffold_b64": "AQID",
                  "signed_transaction_b64": "BAUG",
                  "signing_message_b64": "BwgJ"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.callContract(
            authority = "alice",
            privateKey = "privkey",
            gasLimit = 5_000L,
            contractAlias = "router::universal",
            entrypoint = "contribute",
            payload = linkedMapOf("buyer" to "alice", "payment_amount" to 1L),
            gasAssetId = "xor#sora",
        ).join()

        assertTrue(response.ok)
        assertTrue(response.submitted)
        assertEquals("router", response.dataspace)
        assertEquals("contribute", response.entrypoint)
        assertEquals("AQID", response.transactionScaffoldB64)
        assertEquals("BAUG", response.signedTransactionB64)
        assertEquals("BwgJ", response.signingMessageB64)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/contracts/call", request.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val payload = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals("alice", payload["authority"])
        assertEquals("privkey", payload["private_key"])
        assertEquals("router::universal", payload["contract_alias"])
        assertFalse(payload.containsKey("contract_address"))
        assertEquals(5000L, (payload["gas_limit"] as Number).toLong())
        assertEquals("contribute", payload["entrypoint"])
        assertEquals("xor#sora", payload["gas_asset_id"])
        @Suppress("UNCHECKED_CAST")
        val args = payload["payload"] as Map<String, Any?>
        assertEquals("alice", args["buyer"])
        assertEquals(1L, (args["payment_amount"] as Number).toLong())
    }

    @Test
    fun callContractRejectsAmbiguousSelector() {
        val transport = HttpClientTransport.withExecutor(
            executor = CapturingExecutor(),
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val error = assertFailsWith<IllegalArgumentException> {
            transport.callContract(
                authority = "alice",
                privateKey = "privkey",
                gasLimit = 5_000L,
                contractAddress = "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
                contractAlias = "router::universal",
            )
        }

        assertTrue(error.message?.contains("Exactly one") == true)
    }

    @Test
    fun getGovernanceContractParsesResponse() {
        val contractAddress = "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "found": true,
                  "contract_address": "$contractAddress",
                  "dataspace": "router",
                  "code_hash_hex": "${"77".repeat(32)}"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.getGovernanceContract(contractAddress).join()

        assertTrue(response.found)
        assertEquals(contractAddress, response.contractAddress)
        assertEquals("router", response.dataspace)
        assertEquals("77".repeat(32), response.codeHashHex)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("GET", request.method)
        assertEquals(
            "https://torii.example/api/v1/gov/contracts/$contractAddress",
            request.uri.toString(),
        )
        assertEquals(0, request.body.size)
    }

    private fun readBody(request: TransportRequest): String =
        String(request.body, StandardCharsets.UTF_8)

    private open class CapturingExecutor : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            return CompletableFuture.completedFuture(
                TransportResponse.builder().setStatusCode(404).setBody(byteArrayOf()).build(),
            )
        }
    }

    private class StubResponseExecutor(
        private val statusCode: Int,
        private val body: ByteArray,
    ) : CapturingExecutor() {
        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            return CompletableFuture.completedFuture(
                TransportResponse.builder().setStatusCode(statusCode).setBody(body).build(),
            )
        }
    }
}
