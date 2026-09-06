package org.hyperledger.iroha.sdk.client.websocket

import io.netty.channel.MultiThreadIoEventLoopGroup
import io.netty.channel.nio.NioIoHandler
import java.net.URI
import java.security.KeyFactory
import java.security.KeyStore
import java.security.cert.CertificateFactory
import java.security.spec.PKCS8EncodedKeySpec
import java.time.Duration
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import javax.net.ssl.KeyManagerFactory
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManagerFactory
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportWebSocket
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Test-only self-signed localhost certificate verifies explicit trust and mandatory DNS identity. */
class NettyWebSocketTlsTest {
    @Test
    fun `TLS verifies hostname and trust while preserving credential headers`() {
        val contexts = contexts()
        val group = MultiThreadIoEventLoopGroup(1, NioIoHandler.newFactory())
        try {
            MockWebServer().use { server ->
                server.useHttps(contexts.first.socketFactory, false)
                repeat(3) { server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
                    override fun onClosing(webSocket: WebSocket, code: Int, reason: String) { webSocket.close(code, reason) }
                })) }
                NettyWebSocketConnector(group, contexts.second).use { connector ->
                    val opened = CompletableFuture<ToriiWebSocketSession>()
                    val client = ToriiWebSocketClient.builder().setBaseUri(server.url("/").toUri())
                        .setWebSocketConnector(connector).putDefaultHeader("X-Iroha-Signature", "exact-test-signature").build()
                    client.connect("events", null, object : ToriiWebSocketListener {
                        override fun onOpen(session: ToriiWebSocketSession) { opened.complete(session) }
                        override fun onError(session: ToriiWebSocketSession, error: Throwable) { opened.completeExceptionally(error) }
                    })
                    opened.get(5, TimeUnit.SECONDS).close(1000, "done").get(5, TimeUnit.SECONDS)
                    val request = assertNotNull(server.takeRequest(2, TimeUnit.SECONDS))
                    assertEquals("exact-test-signature", request.getHeader("X-Iroha-Signature"))
                    val wrongHost = TransportRequest.builder().setUri(URI.create("wss://127.0.0.1:${server.port}/events"))
                        .setTimeout(Duration.ofSeconds(3)).build()
                    assertFailsWith<ExecutionException> {
                        connector.connect(wrongHost, object : TransportWebSocket.Listener {}).get(5, TimeUnit.SECONDS)
                    }
                }
                NettyWebSocketConnector.create().use { untrusted ->
                    assertFailsWith<ExecutionException> {
                        untrusted.connect(TransportRequest.builder().setUri(URI.create("wss://localhost:${server.port}/events"))
                            .setTimeout(Duration.ofSeconds(3)).build(), object : TransportWebSocket.Listener {}).get(5, TimeUnit.SECONDS)
                    }
                }
                assertEquals(1, server.requestCount, "TLS identity/trust failures must occur before the HTTP upgrade")
            }
        } finally { group.shutdownGracefully(0, 2, TimeUnit.SECONDS).syncUninterruptibly() }
    }

    private fun contexts(): Pair<SSLContext, SSLContext> {
        val certificate = CertificateFactory.getInstance("X.509").generateCertificate(CERTIFICATE.byteInputStream())
        val encoded = PRIVATE_KEY.lineSequence().filter { !it.startsWith("---") }.joinToString("")
        val key = KeyFactory.getInstance("RSA").generatePrivate(PKCS8EncodedKeySpec(Base64.getDecoder().decode(encoded)))
        val store = KeyStore.getInstance("PKCS12").apply {
            load(null, null)
            setKeyEntry("fixture", key, "fixture".toCharArray(), arrayOf(certificate))
        }
        val managers = KeyManagerFactory.getInstance(KeyManagerFactory.getDefaultAlgorithm()).apply { init(store, "fixture".toCharArray()) }
        val trust = KeyStore.getInstance("PKCS12").apply { load(null, null); setCertificateEntry("fixture", certificate) }
        val trustManagers = TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm()).apply { init(trust) }
        val server = SSLContext.getInstance("TLS").apply { init(managers.keyManagers, null, null) }
        val client = SSLContext.getInstance("TLS").apply { init(null, trustManagers.trustManagers, null) }
        return server to client
    }

    companion object {
        // Public test material only; this key never authenticates a deployed endpoint.
        private val CERTIFICATE = """-----BEGIN CERTIFICATE-----
MIIDITCCAgmgAwIBAgIUYhT2el7KfI/aXnz3M8w7JeisAwQwDQYJKoZIhvcNAQEL
BQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MCAXDTI2MDkwNTE4MDU1NloYDzIxMjYw
ODEyMTgwNTU2WjAUMRIwEAYDVQQDDAlsb2NhbGhvc3QwggEiMA0GCSqGSIb3DQEB
AQUAA4IBDwAwggEKAoIBAQC/ppQcQLUOOlXDcTSSYwd2qxujrIP0zzWZX24o8nhr
z6HVrvWHlYSGg9qwhpHavDculp5nvgXny+daBO+vwLtsMNsZS++i6gbhflzb5m84
3kwKVwksMuXJZLDKaSCXgBbTn4vgb+tnac2gAiwEnRerJn09zQqSiVmFO39KvHA9
k+VdygUC6PN2hzyDMif9RtWeztFYG/mw0OCUB7b9GYRAbH3PHHvGE0Y8KVdHwMrP
DY5EZcN0SJ083yGqWyvLFpbIPkEf5Fa5wJuIoyQqT7ycfUGSX3v4VCG5ZCqvkyul
eWXOgrAF7vqlzhyomwsjrXbNu+IjicwYrG2YfArrZOYzAgMBAAGjaTBnMB0GA1Ud
DgQWBBQgABfOa6CMjRSwATy/T2euAjnISzAfBgNVHSMEGDAWgBQgABfOa6CMjRSw
ATy/T2euAjnISzAPBgNVHRMBAf8EBTADAQH/MBQGA1UdEQQNMAuCCWxvY2FsaG9z
dDANBgkqhkiG9w0BAQsFAAOCAQEAgD5s8T635riAAEtTLB4XtKcLXX7lkhC5eMvE
TBg+0YdoGZ8ZZLjUuwt1gTrR328vtJCLKFUW9qBYRb5kbZ1s12ql5mOpNsZWqLmS
lV1ptZYY9dEDFgF9zrUBoMMkYUAyOBTxf+Ve85g+/GBbtnIJFtLcJdK2Lbldtmkp
tfV/qyWGtTWgR6NOt/NlDasCJrLXubmHwxfkLF8TEtSJMDNif9i3p6QsPLMt32vx
JCEeH666lnRlpo3SvSecu0AMXQpwlBKuMXV1lQ0AjS1KkXRKjfPmWmDLCLED1TSS
uNeD2GY0DE63pWeb9tjNQC2gX9+TivTwxBhxDgHVK3L1mhFsOA==
-----END CERTIFICATE-----""".trimIndent()
        private val PRIVATE_KEY = """-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQC/ppQcQLUOOlXD
cTSSYwd2qxujrIP0zzWZX24o8nhrz6HVrvWHlYSGg9qwhpHavDculp5nvgXny+da
BO+vwLtsMNsZS++i6gbhflzb5m843kwKVwksMuXJZLDKaSCXgBbTn4vgb+tnac2g
AiwEnRerJn09zQqSiVmFO39KvHA9k+VdygUC6PN2hzyDMif9RtWeztFYG/mw0OCU
B7b9GYRAbH3PHHvGE0Y8KVdHwMrPDY5EZcN0SJ083yGqWyvLFpbIPkEf5Fa5wJuI
oyQqT7ycfUGSX3v4VCG5ZCqvkyuleWXOgrAF7vqlzhyomwsjrXbNu+IjicwYrG2Y
fArrZOYzAgMBAAECggEAJijLyshThI627uBGgHM5VDaDnVZHO+JaILywmXSV55mC
9qIMfz+VEJeGXqmctvnM3vjcd3mNgXbHDNR4yPzOFJ+xsFq/Tyfb0OAxKxO5x4/z
ggeMawGDYVMsJFFETQYTBXX6Cukd7QxTBe4Ix65jvQ8/1qNR3JV+fpm3IbFdg9Tr
Udoh/0YztRei1bj0ZCcJDl/iFGdOu/aGiIVa/p47z7AI9dkVBYEgefg+JeOKBGiw
y3wHYdUnSx4U/FKNHJiROiqbTymM/tFExyBqQtYfJN1MngaByfHk+spsVFJeOc6L
ygn/LL1ZiLRW162Q4dlkndp/Xgybj9Bm2pz6c6jdYQKBgQDwQzUOrHX1hppxiDMn
7CtxqQ8Y9+aDMSEFQPa6Zv75ysdTlB7RxjQsYLtDkDFMImmYp7ua1d0dOhPqaH+e
/5y6FclIu2jxkWAPMYSm8IH3Vf+F1pwLWjJoHHbB6aYh0EEO1+vts3QOZg+l1cdZ
0qPzWj9LxLtNDivldyORADqEowKBgQDMNDvbzkEuaI0xrPBS0KH35NvTDRkNg/91
U+R7a2jeWWXS/WhzCXZCx3bJBwBccPycXVcO0spaM3WofIuTt5viQRrzsEpEhnEn
PTgVYYD8Ft5iMFpZFh+0sek/cThDmwRR2dop34GbeqkRxN7JIAZsnEpmN1XAvBGO
pKyZN42hMQKBgD+2YsSAYUt5pU0EBCTLEP40CafiXUNe7NW603K8y1KsPk1gkwen
2sAF6sLQ4vHAkmYD3NEDc35Dn3Jiwa0FNad3DYh3Ai5FEccVp4qpbp2LNZZlQb4U
7hcDrU5gykhfNFFeWtcO5nDHCdE9Ln8YR5fJz80k31Jgtq1D+a+C8wGnAoGARBzY
KoUtsLEnB37L2pPEss8fk9I2nQ9+UkBdYd196UygbjQgdt6dF8E4me0/7ZWybOWl
eEhPPq8Te9OvKuJ/mIRm3Qnce+bsL054Ool/YJawLsg6GqUKhlchmgvF3KcEVdj4
sCbhMF9FrauhNCz+d5PaLSYf8F3K7W14NNMW5sECgYAWGrH2UmFdsX5eWBIykhpX
KP4+ZlQIzAL4+QPR0r7sS6kv+eRX/kTVE+gf/PxwtlezrkNbnzOplYBPslE8EHUn
TgdsuRZd54SNKAo6JHeWwCupGFDQTWFs/FSbPwoJBHnAuAAzf9VV9abk6WeQj5Tp
Udgpd1gJ4lvEf+Ww4+CUaQ==
-----END PRIVATE KEY-----""".trimIndent()
    }
}
