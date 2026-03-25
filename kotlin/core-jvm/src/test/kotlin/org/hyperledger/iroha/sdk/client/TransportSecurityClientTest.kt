package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.stream.ServerSentEvent
import org.hyperledger.iroha.sdk.client.stream.ToriiEventStreamClient
import org.hyperledger.iroha.sdk.client.stream.ToriiEventStreamListener
import org.hyperledger.iroha.sdk.client.stream.ToriiEventStreamOptions
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.websocket.ToriiWebSocketClient
import org.hyperledger.iroha.sdk.client.websocket.ToriiWebSocketListener
import org.hyperledger.iroha.sdk.client.websocket.ToriiWebSocketOptions
import org.hyperledger.iroha.sdk.subscriptions.SubscriptionPlanCreateRequest

class TransportSecurityClientTest {
    @Test
    fun noritoRpcRejectsInsecureAuthorizationHeader() {
        val client = NoritoRpcClient.builder()
            .setBaseUri(URI.create("http://example.com"))
            .putDefaultHeader("Authorization", "Bearer token")
            .build()

        assertFailsWith<IllegalArgumentException> {
            client.call("/rpc", byteArrayOf(0x00))
        }
    }

    @Test
    fun noritoRpcRejectsCredentialedAbsoluteHostOverride() {
        val client = NoritoRpcClient.builder()
            .setBaseUri(URI.create("https://example.com"))
            .putDefaultHeader("Authorization", "Bearer token")
            .build()

        assertFailsWith<IllegalArgumentException> {
            client.call("https://evil.example/rpc", byteArrayOf(0x00))
        }
    }

    @Test
    fun offlineClientRejectsInsecurePrivateKeyBody() {
        val client = OfflineToriiClient.builder()
            .executor(StubExecutor())
            .baseUri(URI.create("http://example.com"))
            .build()

        assertFailsWith<IllegalArgumentException> {
            client.submitSettlement(
                mapOf("bundle_id" to "deadbeef"),
                sampleAuthority(0x41),
                "deadbeef",
            )
        }
    }

    @Test
    fun subscriptionClientRejectsInsecurePrivateKeyBody() {
        val client = SubscriptionToriiClient.builder()
            .executor(StubExecutor())
            .baseUri(URI.create("http://example.com"))
            .build()

        assertFailsWith<IllegalArgumentException> {
            client.createSubscriptionPlan(
                SubscriptionPlanCreateRequest(
                    authority = sampleAuthority(0x42),
                    privateKey = "deadbeef",
                    planId = "plan#subs",
                    plan = mapOf("kind" to "fixed"),
                ),
            )
        }
    }

    @Test
    fun eventStreamRejectsInsecureAuthorizationHeader() {
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
                    CompletableFuture.completedFuture(
                        TransportResponse.builder().setStatusCode(200).setBody(byteArrayOf()).build(),
                    )
            },
            defaultHeaders = mapOf("Authorization" to "Bearer token"),
        )

        assertFailsWith<IllegalArgumentException> {
            client.openSseStream(
                "/events",
                ToriiEventStreamOptions.defaultOptions(),
                object : ToriiEventStreamListener {
                    override fun onEvent(event: ServerSentEvent) = Unit
                },
            )
        }
    }

    @Test
    fun websocketRejectsInsecureAuthorizationHeader() {
        val client = ToriiWebSocketClient.builder()
            .setBaseUri(URI.create("http://example.com"))
            .setWebSocketConnector { _, _, _, _ ->
                CompletableFuture.failedFuture(IllegalStateException("should not connect"))
            }
            .build()

        assertFailsWith<IllegalArgumentException> {
            client.connect(
                "/ws",
                ToriiWebSocketOptions(headers = mapOf("Authorization" to "Bearer token")),
                object : ToriiWebSocketListener {},
            )
        }
    }

    private class StubExecutor : HttpTransportExecutor {
        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
            CompletableFuture.completedFuture(
                TransportResponse.builder().setStatusCode(200).setBody(byteArrayOf()).build(),
            )
    }

    private fun sampleAuthority(fill: Int): String = AccountAddress
        .fromAccount(ByteArray(32) { fill.toByte() }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
}
