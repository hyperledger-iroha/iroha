package org.hyperledger.iroha.sdk.client

import java.io.IOException
import java.net.Authenticator
import java.net.CookieHandler
import java.net.InetSocketAddress
import java.net.PasswordAuthentication
import java.net.Proxy
import java.net.ProxySelector
import java.net.SocketAddress
import java.net.URI
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.hyperledger.iroha.sdk.client.transport.StreamingTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest

class PlatformHttpTransportExecutorCredentialFreeTest {
    @Test
    fun credentialFreeCorridorIgnoresAmbientCookiesAuthenticationAndProxy() {
        val previousCookieHandler = CookieHandler.getDefault()
        val previousAuthenticator = currentAuthenticator()
        val previousProxySelector = ProxySelector.getDefault()
        val authenticatorCalls = AtomicInteger()
        try {
            CookieHandler.setDefault(
                object : CookieHandler() {
                    override fun get(
                        uri: URI,
                        requestHeaders: Map<String, List<String>>,
                    ): Map<String, List<String>> =
                        mapOf("Cookie" to listOf("ambient-session=must-not-leak"))

                    override fun put(uri: URI, responseHeaders: Map<String, List<String>>) = Unit
                },
            )
            Authenticator.setDefault(
                object : Authenticator() {
                    override fun getPasswordAuthentication(): PasswordAuthentication {
                        authenticatorCalls.incrementAndGet()
                        return PasswordAuthentication("ambient", "must-not-leak".toCharArray())
                    }
                },
            )
            MockWebServer().use { destination ->
                MockWebServer().use { proxy ->
                    destination.enqueue(
                        MockResponse()
                            .setResponseCode(401)
                            .setHeader("WWW-Authenticate", "Basic realm=ambient"),
                    )
                    destination.start()
                    proxy.start()
                    ProxySelector.setDefault(
                        object : ProxySelector() {
                            override fun select(uri: URI): List<Proxy> = listOf(
                                Proxy(
                                    Proxy.Type.HTTP,
                                    InetSocketAddress("127.0.0.1", proxy.port),
                                ),
                            )

                            override fun connectFailed(
                                uri: URI,
                                socketAddress: SocketAddress,
                                error: IOException,
                            ) = Unit
                        },
                    )
                    val executor = PlatformHttpTransportExecutor.createDefault()
                    try {
                        val request = TransportRequest.builder()
                            .setMethod("GET")
                            .setUri(destination.url("/v1/zk/vk").toUri())
                            .addHeader("X-Trace-Id", "credential-free-test")
                            .setAllowAmbientCredentials(false)
                            .build()

                        val response = executor.execute(request).get(5, TimeUnit.SECONDS)

                        assertEquals(401, response.statusCode)
                        val received = assertNotNull(
                            destination.takeRequest(5, TimeUnit.SECONDS),
                        )
                        assertNull(received.getHeader("Authorization"))
                        assertNull(received.getHeader("Proxy-Authorization"))
                        assertNull(received.getHeader("Cookie"))
                        assertEquals("credential-free-test", received.getHeader("X-Trace-Id"))
                        assertEquals(1, destination.requestCount)
                        assertEquals(0, proxy.requestCount)
                        assertEquals(0, authenticatorCalls.get())
                    } finally {
                        executor.invalidateAndCancel()
                    }
                }
            }
        } finally {
            ProxySelector.setDefault(previousProxySelector)
            Authenticator.setDefault(previousAuthenticator)
            CookieHandler.setDefault(previousCookieHandler)
        }
    }

    @Test
    fun credentialFreeCorridorDoesNotFollowRedirects() {
        MockWebServer().use { server ->
            server.start()
            server.enqueue(
                MockResponse()
                    .setResponseCode(302)
                    .setHeader("Location", server.url("/redirected")),
            )
            server.enqueue(MockResponse().setResponseCode(200))
            val executor = PlatformHttpTransportExecutor.createDefault()
            try {
                val request = TransportRequest.builder()
                    .setMethod("GET")
                    .setUri(server.url("/v1/zk/vk").toUri())
                    .setAllowAmbientCredentials(false)
                    .build()

                val response = executor.execute(request).get(5, TimeUnit.SECONDS)

                assertEquals(302, response.statusCode)
                assertEquals(1, server.requestCount)
            } finally {
                executor.invalidateAndCancel()
            }
        }
    }

    @Test
    fun credentialFreeCorridorEnforcesPerRequestBodyLimit() {
        MockWebServer().use { server ->
            server.enqueue(MockResponse().setResponseCode(200).setBody("123456789"))
            server.start()
            val executor = PlatformHttpTransportExecutor.createDefault()
            try {
                val request = TransportRequest.builder()
                    .setMethod("GET")
                    .setUri(server.url("/v1/zk/vk").toUri())
                    .setMaximumResponseBytes(8L)
                    .setAllowAmbientCredentials(false)
                    .build()

                val failure = assertFailsWith<ExecutionException> {
                    executor.execute(request).get(5, TimeUnit.SECONDS)
                }

                assertTrue(failure.hasCauseMessage("Content-Length 9 exceeds the 8-byte limit"))
                assertEquals(1, server.requestCount)
            } finally {
                executor.invalidateAndCancel()
            }
        }
    }

    @Test
    fun credentialFreeCorridorRejectsUriUserInfoBeforeNetwork() {
        MockWebServer().use { server ->
            server.start()
            val endpoint = server.url("/v1/zk/vk")
            val uri = URI(
                endpoint.scheme,
                "ambient:must-not-leak",
                endpoint.host,
                endpoint.port,
                endpoint.encodedPath,
                null,
                null,
            )
            val executor = PlatformHttpTransportExecutor.createDefault()
            try {
                val request = TransportRequest.builder()
                    .setMethod("GET")
                    .setUri(uri)
                    .setAllowAmbientCredentials(false)
                    .build()

                val failure = assertFailsWith<IllegalArgumentException> {
                    executor.execute(request)
                }

                assertTrue(failure.message?.contains("reject URI user-info") == true)
                assertEquals(0, server.requestCount)
            } finally {
                executor.invalidateAndCancel()
            }
        }
    }

    @Test
    fun credentialFreeStreamingIsRejectedWithoutNetwork() {
        val executor = PlatformHttpTransportExecutor.createDefault()
        try {
            val streaming = assertNotNull(executor as? StreamingTransportExecutor)
            val request = TransportRequest.builder()
                .setMethod("GET")
                .setUri(URI.create("http://127.0.0.1:1/v1/zk/vk"))
                .setAllowAmbientCredentials(false)
                .build()
            val failure = assertFailsWith<ExecutionException> {
                streaming.openStream(request).get()
            }
            assertTrue(failure.hasCauseMessage("credential-free streaming is unsupported"))
        } finally {
            executor.invalidateAndCancel()
        }
    }

    private fun currentAuthenticator(): Authenticator? =
        Authenticator::class.java.getMethod("getDefault").invoke(null) as Authenticator?

    private fun Throwable.hasCauseMessage(fragment: String): Boolean =
        generateSequence(this) { it.cause }
            .any { it.message?.contains(fragment) == true }
}
