package org.hyperledger.iroha.sdk.client

import java.net.URI
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.ValueSource

class TransportSecurityLocalDevelopmentHttpTest {
    private val signedHeaders = mapOf(
        "X-Iroha-Account" to "alice",
        "X-Iroha-Signature" to "deadbeef",
        "X-Iroha-Timestamp-Ms" to "123",
        "X-Iroha-Nonce" to "456",
    )

    @ParameterizedTest
    @ValueSource(strings = ["http://127.0.0.1:29080", "http://10.0.2.2:29080", "http://localhost:29080"])
    fun localDevelopmentHttpIsRefusedWithoutOptIn(baseUri: String) {
        val error = assertFailsWith<IllegalArgumentException> {
            requireAllowed(baseUri, "$baseUri/v1/fees/quote", allowLocalDevelopmentHttp = false)
        }

        assertContains(error.message.orEmpty(), "use https")
    }

    @ParameterizedTest
    @ValueSource(
        strings = [
            "http://127.0.0.1:29080",
            "http://127.10.20.30:29080",
            "http://localhost:29080",
            "http://LOCALHOST:29080",
            "http://[::1]:29080",
            "http://10.0.2.2:29080",
            "http://10.0.2.2",
            "http://10.1.2.3:29080",
            "http://172.16.0.5:29080",
            "http://172.31.255.254:29080",
            "http://192.168.1.10:29080",
        ],
    )
    fun localDevelopmentHttpIsAllowedWithOptIn(baseUri: String) {
        requireAllowed(baseUri, "$baseUri/v1/fees/quote", allowLocalDevelopmentHttp = true)
    }

    @ParameterizedTest
    @ValueSource(
        strings = [
            "http://example.com:29080",
            "http://172.15.0.1:29080",
            "http://172.32.0.1:29080",
            "http://192.169.1.10:29080",
            "http://11.0.0.1:29080",
            "http://169.254.1.1:29080",
            "http://128.0.0.1:29080",
            "http://127.0.0.1.example.com:29080",
            "http://localhost.example.com:29080",
        ],
    )
    fun remoteHttpIsRefusedEvenWithOptIn(baseUri: String) {
        val error = assertFailsWith<IllegalArgumentException> {
            requireAllowed(baseUri, "$baseUri/v1/fees/quote", allowLocalDevelopmentHttp = true)
        }

        assertContains(error.message.orEmpty(), "use https")
    }

    @Test
    fun optInDoesNotAllowHttpToDifferentLocalAuthority() {
        assertFailsWith<IllegalArgumentException> {
            requireAllowed(
                "http://127.0.0.1:29080",
                "http://127.0.0.1:8080/v1/fees/quote",
                allowLocalDevelopmentHttp = true,
            )
        }
    }

    @Test
    fun optInDoesNotAllowHttpTargetFromHttpsBase() {
        assertFailsWith<IllegalArgumentException> {
            requireAllowed(
                "https://127.0.0.1:29080",
                "http://127.0.0.1:29080/v1/fees/quote",
                allowLocalDevelopmentHttp = true,
            )
        }
    }

    @ParameterizedTest
    @ValueSource(booleans = [false, true])
    fun httpsIsUnaffectedByOptIn(allowLocalDevelopmentHttp: Boolean) {
        requireAllowed(
            "https://torii.example",
            "https://torii.example/v1/fees/quote",
            allowLocalDevelopmentHttp = allowLocalDevelopmentHttp,
        )
        assertFailsWith<IllegalArgumentException> {
            requireAllowed(
                "https://torii.example",
                "https://evil.example/v1/fees/quote",
                allowLocalDevelopmentHttp = allowLocalDevelopmentHttp,
            )
        }
    }

    @Test
    fun clientConfigOptInIsOffByDefaultAndSurvivesToBuilder() {
        val defaultConfig = ClientConfig.builder().build()
        val optedIn = ClientConfig.builder().setAllowLocalDevelopmentHttp(true).build()

        assertFalse(defaultConfig.allowLocalDevelopmentHttp())
        assertTrue(optedIn.allowLocalDevelopmentHttp())
        assertTrue(optedIn.toBuilder().build().allowLocalDevelopmentHttp())
    }

    private fun requireAllowed(baseUri: String, targetUri: String, allowLocalDevelopmentHttp: Boolean) {
        TransportSecurity.requireHttpRequestAllowed(
            context = "HttpClientTransport",
            baseUri = URI.create(baseUri),
            targetUri = URI.create(targetUri),
            headers = signedHeaders,
            body = null,
            allowLocalDevelopmentHttp = allowLocalDevelopmentHttp,
        )
    }
}
