package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.ClientConfig
import org.hyperledger.iroha.sdk.client.HttpClientTransport
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class PrivacyCapabilitiesV1Test {
    @Test
    fun parsesClosedSnapshotWithoutLosingU64Height() {
        val snapshot = PrivacyCapabilitySnapshotJsonV1.parse(
            unavailableSnapshot("18446744073709551615"),
        )
        assertEquals(BigInteger("18446744073709551615"), snapshot.committedHeight)
        assertEquals(PrivacyProtocolIdV1.values().toList(), snapshot.protocols.map { it.protocolId })
        assertEquals(18 * 1024 * 1024, snapshot.consensusPolicy.currentLimits.maxPrivacyBytesPerBlock)
        assertNull(snapshot.consensusPolicy.pendingTightening)
        assertIs<PrivacyCompiledProfileResultV1.Unavailable>(snapshot.protocols[0].compiledProfile)
    }

    @Test
    fun rejectsAliasesRetiredRowsAmbiguousJsonAndNonCanonicalIntegers() {
        val canonical = unavailableSnapshot("42")
        val hostile = listOf(
            canonical.replaceFirst("{", "{\"committed_height\":41,"),
            canonical.replace("\"zk-ace-pq-authorization-v0\"", "\"sis-with-hints\""),
            canonical.replace("\"zk-ace-pq-authorization-v0\"", "\"sis-hints-anoncred-pq-v0\""),
            canonical.replace("\"zk-ace-pq-authorization-v0\"", "\"ZK-ACE-PQ-AUTHORIZATION-V0\""),
            unavailableSnapshot("-0"),
            unavailableSnapshot("01"),
            unavailableSnapshot("1.0"),
            unavailableSnapshot("1e3"),
            unavailableSnapshot("18446744073709551616"),
            canonical.replaceFirst("\"activation\":null", "\"activation\":null,\"legacy\":true"),
        )
        for (payload in hostile) {
            assertFailsWith<PrivacyCapabilitySnapshotException> {
                PrivacyCapabilitySnapshotJsonV1.parse(payload)
            }
        }
    }

    @Test
    fun validatesAllGovernedBindingsAndRejectsCrossProfileSubstitution() {
        val valid = PrivacyCapabilitySnapshotJsonV1.parse(activeAnonymousPgcSnapshot(false))
        val row = valid.protocols[1]
        val compiled = assertIs<PrivacyCompiledProfileResultV1.Available>(row.compiledProfile)
        assertEquals(PrivacyEngineIdV1.NATIVE_ANONYMOUS_PGC_P256, compiled.profile.engineId)
        assertEquals(PrivacyProtocolLifecycleStateV1.ACTIVE, row.activation?.lifecycle?.state)
        assertEquals(PrivacyAssuranceV1.EXPERIMENTAL, row.activation?.assurance)

        assertFailsWith<PrivacyCapabilitySnapshotException> {
            PrivacyCapabilitySnapshotJsonV1.parse(activeAnonymousPgcSnapshot(true))
        }
    }

    @Test
    fun rejectsUnsignedValuesThatWouldWrapTheJvmIntModel() {
        val canonical = unavailableSnapshot("42")
        val hostile = listOf(
            canonical.replace("\"retained_root_count\":2048", "\"retained_root_count\":2147483648"),
            canonical.replace("\"retained_root_count\":2048", "\"retained_root_count\":4294967295"),
            activeAnonymousPgcSnapshot(false).replace(
                "\"max_recipient_count\":8",
                "\"max_recipient_count\":2147483648",
            ),
            activeAnonymousPgcSnapshot(false).replace(
                "\"max_recipient_count\":8",
                "\"max_recipient_count\":4294967295",
            ),
            activeAnonymousPgcSnapshot(false).replaceFirst(
                "\"parameter_id\":[1,",
                "\"parameter_id\":[4294967295,",
            ),
        )
        hostile.forEach { payload ->
            assertFailsWith<PrivacyCapabilitySnapshotException> {
                PrivacyCapabilitySnapshotJsonV1.parse(payload)
            }
        }
    }

    @Test
    fun configuredClientUsesExactNoritoRouteAndRejectsLegacySnapshot() {
        val response = TransportResponse.builder()
            .setStatusCode(200)
            .setBody(unavailableSnapshot("42").toByteArray(StandardCharsets.UTF_8))
            .addHeader("Content-Type", "application/x-norito")
            .build()
        val executor = OneResponseExecutor(response)
        val client = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )
        val legacy = assertFailsWith<CompletionException> {
            client.getPrivacyCapabilities().join()
        }
        assertTrue(legacy.cause is RuntimeException)
        assertEquals("/v1/privacy/capabilities", executor.request.uri.rawPath)
        assertEquals(listOf("application/x-norito"), executor.request.headers["Accept"])
        assertEquals(
            PrivacyExact12CapabilityManifestV1.MAX_ARCHIVE_BYTES.toLong(),
            executor.request.maximumResponseBytes,
        )

        val wrongMedia = response.let {
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(it.body)
                .addHeader("Content-Type", "application/json; charset=utf-8")
                .build()
        }
        val error = assertFailsWith<CompletionException> {
            HttpClientTransport.withExecutor(
                OneResponseExecutor(wrongMedia),
                ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
            ).getPrivacyCapabilities().join()
        }
        assertTrue(error.cause is RuntimeException)
    }

    @Test
    fun exactTransportRejectsHeaderAmbiguityLengthSmugglingAndHostileBodies() {
        val body = unavailableSnapshot("42").toByteArray(StandardCharsets.UTF_8)
        val canonicalHeaders = mapOf(
            "Content-Type" to listOf("application/x-norito"),
            "Content-Length" to listOf(body.size.toString()),
        )
        val legacy = assertFailsWith<CompletionException> {
            clientFor(response(body = body, headers = canonicalHeaders))
                .getPrivacyCapabilities()
                .join()
        }
        assertTrue(legacy.cause is RuntimeException)

        val hostile = buildList {
            add(response(statusCode = 206, body = body, headers = canonicalHeaders))
            add(response(body = body, headers = mapOf("Content-Type" to listOf("application/json; charset=utf-8"))))
            add(response(body = body, headers = mapOf("Content-Type" to listOf("application/x-norito", "application/x-norito"))))
            add(response(body = body, headers = mapOf("Content-Type" to listOf("application/x-norito"), "Content-Length" to listOf(body.size.toString(), body.size.toString()))))
            add(response(body = body, headers = mapOf("Content-Type" to listOf("application/x-norito"), "Content-Length" to listOf((body.size + 1).toString()))))
            for (length in listOf("+${body.size}", "0${body.size}", "${body.size} ", "-1", "9".repeat(1_024))) {
                add(response(body = body, headers = mapOf("Content-Type" to listOf("application/x-norito"), "Content-Length" to listOf(length))))
            }
            add(response(body = ByteArray(0), headers = mapOf("Content-Type" to listOf("application/x-norito"))))
            add(
                response(
                    body = ByteArray(PrivacyExact12CapabilityManifestV1.MAX_ARCHIVE_BYTES + 1) { 0x20 },
                    headers = mapOf("Content-Type" to listOf("application/x-norito")),
                ),
            )
        }
        hostile.forEachIndexed { index, candidate ->
            val error = assertFailsWith<CompletionException>("hostile response $index must fail closed") {
                clientFor(candidate).getPrivacyCapabilities().join()
            }
            assertTrue(error.cause is RuntimeException, "unexpected hostile response $index cause: ${error.cause}")
        }
    }

    @Test
    fun exactTransportRejectsCaseVariantDefaultAcceptBeforeDispatch() {
        val body = unavailableSnapshot("42").toByteArray(StandardCharsets.UTF_8)
        val executor = OneResponseExecutor(
            response(body = body, headers = mapOf("Content-Type" to listOf("application/x-norito"))),
        )
        val client = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .putDefaultHeader("aCcEpT", "application/x-norito")
                .build(),
        )
        assertFailsWith<IllegalArgumentException> {
            client.getPrivacyCapabilities()
        }
    }

    private fun unavailableSnapshot(height: String): String =
        """{"version":1,"committed_height":$height,"consensus_policy":{"current_limits":${consensusLimits()},"pending_tightening":null},"protocols":[${PrivacyProtocolIdV1.values().joinToString(",") { unavailableRow(it) }}]}"""

    private fun unavailableRow(protocol: PrivacyProtocolIdV1): String =
        """{"protocol_id":{"protocol":"${protocol.canonicalLabel}","value":null},"compiled_profile":{"status":"unavailable","value":{"reason":"engine-unavailable","detail":null}},"activation":null}"""

    private fun activeAnonymousPgcSnapshot(substituteActivationDigest: Boolean): String {
        val profile = profileFields("2")
        val activation = profileFields(if (substituteActivationDigest) "9" else "2") +
            ""","lifecycle":{"state":"active","record":{"proposed_at_height":1,"activated_at_height":2,"state_since_height":2}},"pending_protocol_limits_tightening":null,"assurance":{"assurance":"experimental","value":null}"""
        val rows = PrivacyProtocolIdV1.values().map { protocol ->
            if (protocol == PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1) {
                """{"protocol_id":{"protocol":"${protocol.canonicalLabel}","value":null},"compiled_profile":{"status":"available","value":{$profile}},"activation":{$activation}}"""
            } else {
                unavailableRow(protocol)
            }
        }
        return """{"version":1,"committed_height":42,"consensus_policy":{"current_limits":${consensusLimits()},"pending_tightening":null},"protocols":[${rows.joinToString(",")}]}"""
    }

    private fun profileFields(parameterDigestByte: String): String =
        """"protocol_id":{"protocol":"anonymous-pgc-k-out-of-n-v1","value":null},"proof_system_id":{"proof_system":"anonymous-pgc-p256","value":null},"engine_id":{"engine":"native-anonymous-pgc-p256","value":null},"parameter_id":${bytes("1")},"parameter_digest":${bytes(parameterDigestByte)},"verifier_digest":${bytes("3")},"statement_schema_digest":${bytes("4")},"engine_manifest_digest":${bytes("5")},"protocol_limits":{"protocol":"anonymous-pgc-k-out-of-n-v1","limits":{"max_anonymity_set_size":64,"max_recipient_count":8}}"""

    private fun bytes(value: String): String = List(32) { value }.joinToString(",", "[", "]")

    private fun consensusLimits(): String =
        """{"max_actions_per_transaction":1,"max_actions_per_block":2,"max_proof_bytes_per_action":9437184,"max_action_bytes":9437184,"max_privacy_bytes_per_transaction":9437184,"max_privacy_bytes_per_block":18874368,"max_statement_and_encrypted_output_bytes_per_transaction":262144,"max_nullifiers_per_action":8,"max_commitments_per_action":8,"retained_root_count":2048}"""

    private fun response(
        statusCode: Int = 200,
        body: ByteArray,
        headers: Map<String, List<String>>,
    ): TransportResponse = TransportResponse.builder()
        .setStatusCode(statusCode)
        .setBody(body)
        .setHeaders(headers)
        .build()

    private fun clientFor(response: TransportResponse): HttpClientTransport =
        HttpClientTransport.withExecutor(
            OneResponseExecutor(response),
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )

    private class OneResponseExecutor(
        private val response: TransportResponse,
    ) : HttpTransportExecutor {
        lateinit var request: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            this.request = request
            return CompletableFuture.completedFuture(response)
        }
    }
}
