package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.time.Duration
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class ClientConfigManifestLoaderTest {
    @Test
    fun rejectsNonStringScalarFields() {
        val malformed = listOf(
            manifest(torii = """{"base_uri":123}"""),
            manifest(torii = """{"base_uri":"https://torii.example","sorafs_gateway_uri":false}"""),
            manifest(torii = """{"base_uri":"https://torii.example","sorafs_gateway_uri":""}"""),
            manifest(torii = """{"base_uri":"https://torii.example","default_headers":{"X-Test":7}}"""),
            manifest(torii = """{"base_uri":"https://torii.example","default_headers":{"X-Test":null}}"""),
            manifest(extra = ""","pending_queue":{"kind":true}"""),
            manifest(telemetry = """{"enabled":false,"exporter_name":[]}"""),
        )

        malformed.forEach { json -> assertRejected(json) }
    }

    @Test
    fun rejectsMalformedPresentNumericAndBooleanFields() {
        val malformed = listOf(
            manifest(torii = """{"base_uri":"https://torii.example","timeout_ms":true}"""),
            manifest(torii = """{"base_uri":"https://torii.example","timeout_ms":""}"""),
            manifest(extra = ""","retry":{"max_attempts":false}"""),
            manifest(extra = ""","retry":{"base_delay_ms":{}}"""),
            manifest(extra = ""","retry":{"retry_on_network_error":1}"""),
            manifest(extra = ""","retry":{"retry_status_codes":[null]}"""),
        )

        malformed.forEach { json -> assertRejected(json) }
    }

    @Test
    fun rejectsOutOfDomainNumericValuesInsteadOfUsingDefaults() {
        val malformed = listOf(
            manifest(torii = """{"base_uri":"https://torii.example","timeout_ms":-1}"""),
            manifest(extra = ""","retry":{"max_attempts":0}"""),
            manifest(extra = ""","retry":{"base_delay_ms":-1}"""),
            manifest(extra = ""","retry":{"max_delay_ms":-1}"""),
        )

        malformed.forEach { json -> assertRejected(json) }
    }

    @Test
    fun preservesIntegerAndBooleanStringCompatibility() {
        val config = load(
            manifest(
                torii = """{"base_uri":"https://torii.example","timeout_ms":"7000"}""",
                extra =
                    ""","retry":{"max_attempts":"3","base_delay_ms":"250","retry_on_network_error":"no"}""",
            ),
        )

        assertEquals(Duration.ofMillis(7_000), config.requestTimeout())
        assertTrue(config.retryPolicy().allowsRetry(2))
        assertFalse(config.retryPolicy().allowsRetry(3))
        assertFalse(config.retryPolicy().shouldRetryError(1))
    }

    private fun assertRejected(json: String) {
        assertFailsWith<IllegalStateException> { load(json) }
    }

    private fun load(json: String): ClientConfig {
        val manifest = Files.createTempFile("client-config-manifest", ".json")
        return try {
            Files.write(manifest, json.toByteArray(StandardCharsets.UTF_8))
            ClientConfigManifestLoader.load(manifest).clientConfig()
        } finally {
            Files.deleteIfExists(manifest)
        }
    }

    private fun manifest(
        torii: String = """{"base_uri":"https://torii.example"}""",
        telemetry: String = """{"enabled":false}""",
        extra: String = "",
    ): String = """{"torii":$torii,"telemetry":$telemetry$extra}"""
}
