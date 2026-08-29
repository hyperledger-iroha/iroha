package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.MessageDigest
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class BootleLanternIssuanceClientV1Test {
    @Test
    fun sharedClientContractFixtureBindsExactWireBytes() {
        val fixture = Json.parseToJsonElement(
            String(Files.readAllBytes(clientContractFixture()), StandardCharsets.UTF_8),
        ).jsonObject
        assertEquals(
            "iroha.bootle_lantern.issuance_client_contract",
            fixture.getValue("schema").jsonPrimitive.content,
        )
        assertEquals(1, fixture.getValue("version").jsonPrimitive.int)
        assertEquals(
            "public-synthetic-test-data",
            fixture.getValue("classification").jsonPrimitive.content,
        )

        val transport = fixture.getValue("transport").jsonObject
        assertEquals("POST", transport.getValue("method").jsonPrimitive.content)
        assertEquals(
            BootleLanternIssuanceClientV1.AUTHORIZE_PATH,
            transport.getValue("authorize_path").jsonPrimitive.content,
        )
        assertEquals(
            BootleLanternIssuanceClientV1.ISSUE_PATH,
            transport.getValue("issue_path").jsonPrimitive.content,
        )
        assertEquals(
            BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
            transport.getValue("norito_media_type").jsonPrimitive.content,
        )
        assertEquals(
            "Bearer realm=\"iroha-bootle-lantern-issuance\"",
            transport.getValue("unauthorized_www_authenticate").jsonPrimitive.content,
        )

        val credentialContract = fixture.getValue("credential").jsonObject
        assertEquals(
            "base64url-unpadded-canonical",
            credentialContract.getValue("encoding").jsonPrimitive.content,
        )
        assertEquals(1, credentialContract.getValue("minimum_decoded_bytes").jsonPrimitive.int)
        assertEquals(
            BootleLanternIssuanceCredentialV1.MAX_BYTES,
            credentialContract.getValue("maximum_decoded_bytes").jsonPrimitive.int,
        )
        val examples = credentialContract.getValue("examples").jsonArray
        assertEquals(3, examples.size)
        examples.forEach { value ->
            val example = value.jsonObject
            val decoded = example.getValue("decoded_hex").jsonPrimitive.content.hexBytes()
            val encoded = example.getValue("encoded").jsonPrimitive.content
            assertEquals(
                encoded,
                java.util.Base64.getUrlEncoder().withoutPadding().encodeToString(decoded),
            )
            val admitted = BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(encoded)
            assertEquals("Bearer $encoded", admitted.authorizationHeaderValue())
            admitted.close()
        }

        val bodies = fixture.getValue("bodies").jsonObject
        assertEquals(
            "byte-at-index-equals-index-modulo-256-with-canonical-wire-magics",
            bodies.getValue("pattern").jsonPrimitive.content,
        )
        listOf(
            Triple(
                "authorization_response",
                "ILA1",
                BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES,
            ),
            Triple("issue_request", "ILA1+ILQ1", BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES),
            Triple("issue_response", "ILR1", BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES),
        ).forEach { (name, wire, length) ->
            val body = bodies.getValue(name).jsonObject
            assertEquals(wire, body.getValue("wire").jsonPrimitive.content)
            assertEquals(length, body.getValue("length_bytes").jsonPrimitive.int)
            assertEquals(
                body.getValue("pattern_sha256_hex").jsonPrimitive.content,
                sha256Hex(patterned(length)),
            )
        }
        assertContentEquals("ILA1".toByteArray(StandardCharsets.US_ASCII), patterned(320).copyOfRange(0, 4))
        assertContentEquals("ILA1".toByteArray(StandardCharsets.US_ASCII), patterned(71_896).copyOfRange(0, 4))
        assertContentEquals("ILQ1".toByteArray(StandardCharsets.US_ASCII), patterned(71_896).copyOfRange(320, 324))
        assertContentEquals("ILR1".toByteArray(StandardCharsets.US_ASCII), patterned(3_176).copyOfRange(0, 4))
        val componentLengths = bodies.getValue("issue_request").jsonObject
            .getValue("component_lengths_bytes").jsonArray
            .map { it.jsonPrimitive.int }
        assertEquals(listOf(320, 71_576), componentLengths)
        assertEquals(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES, componentLengths.sum())

        val errors = fixture.getValue("errors").jsonObject
        assertEquals(
            BootleLanternIssuanceClientV1.ERROR_RESPONSE_MAX_BYTES,
            errors.getValue("maximum_body_bytes").jsonPrimitive.int,
        )
        val envelope = errors.getValue("norito_envelope").jsonObject
        assertEquals(
            "iroha_torii_shared::ErrorEnvelope",
            envelope.getValue("schema_type_name").jsonPrimitive.content,
        )
        assertEquals(
            "793f11768076bfe270a17aeb86752cd9",
            envelope.getValue("schema_hash_hex").jsonPrimitive.content,
        )
        assertEquals("02", envelope.getValue("flags_hex").jsonPrimitive.content)
        val responses = errors.getValue("responses").jsonArray.map { it.jsonObject }
        assertEquals(8, responses.size)
        responses.forEach { contract ->
            assertEquals(
                if (contract.getValue("status").jsonPrimitive.int == 401) {
                    transport.getValue("unauthorized_www_authenticate").jsonPrimitive.content
                } else {
                    null
                },
                contract["www_authenticate"]?.jsonPrimitive?.content,
            )
            val failure = assertClientFailure(
                client(ScriptedExecutor(errorResponse(contract))).authorize(credential()),
            )
            assertEquals(contract.getValue("status").jsonPrimitive.int, failure.statusCode)
            assertEquals(contract.getValue("code").jsonPrimitive.content, failure.code)
            assertEquals(
                contract["retry_after_seconds"]?.jsonPrimitive?.int?.toLong(),
                failure.retryAfterSeconds,
            )
        }
    }

    @Test
    fun authorizeUsesCanonicalExactEmptySingleAttemptRequest() {
        val responseBytes = patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES)
        val executor = ScriptedExecutor(success(responseBytes))
        val credential = BootleLanternIssuanceCredentialV1.fromOpaqueBytes(byteArrayOf(0x61))

        val result = client(executor).authorize(credential).join()

        assertContentEquals(responseBytes, result)
        assertEquals(1, executor.calls)
        val request = executor.lastRequest
        assertEquals("POST", request.method)
        assertEquals(BootleLanternIssuanceClientV1.AUTHORIZE_PATH, request.uri.rawPath)
        assertContentEquals(ByteArray(0), request.body)
        assertEquals(
            BootleLanternIssuanceClientV1.ERROR_RESPONSE_MAX_BYTES.toLong(),
            request.maximumResponseBytes,
        )
        assertEquals("Bearer YQ", exactHeader(request, "Authorization"))
        assertEquals(
            BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
            exactHeader(request, "Content-Type"),
        )
        assertEquals(
            BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE,
            exactHeader(request, "Accept"),
        )
        assertEquals("identity", exactHeader(request, "Accept-Encoding"))
        assertTrue(headerValues(request, "Content-Encoding").isEmpty())
        assertEquals("no-store", exactHeader(request, "Cache-Control"))
        assertEquals("no-cache", exactHeader(request, "Pragma"))
    }

    @Test
    fun issueUsesExactDefensiveBodyAndResponseLimits() {
        val requestBytes = patterned(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES)
        val original = requestBytes.copyOf()
        val responseBytes = patterned(BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES)
        val executor = ScriptedExecutor(success(responseBytes))
        val credential = BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url("AQID")

        val future = client(executor).issue(credential, requestBytes)
        requestBytes.fill(0)
        val result = future.join()

        assertContentEquals(responseBytes, result)
        assertEquals(1, executor.calls)
        assertEquals(BootleLanternIssuanceClientV1.ISSUE_PATH, executor.lastRequest.uri.rawPath)
        assertContentEquals(original, executor.lastRequest.body)
        assertEquals("Bearer AQID", exactHeader(executor.lastRequest, "Authorization"))
        assertEquals(
            BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES.toLong(),
            executor.lastRequest.maximumResponseBytes,
        )
    }

    @Test
    fun issueRejectsZeroTruncatedExtendedAndOversizedBodiesBeforeExecution() {
        val executor = ScriptedExecutor(success(patterned(BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES)))
        val credential = credential()
        val invalidSizes = listOf(
            0,
            1,
            BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES - 1,
            BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES + 1,
            BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES * 2,
        )

        invalidSizes.forEach { size ->
            assertFailsWith<IllegalArgumentException> {
                client(executor).issue(credential, ByteArray(size))
            }
        }
        assertEquals(0, executor.calls)
    }

    @Test
    fun issueRejectsSameLengthWrongTruncatedShiftedAndSubstitutedIla1Magic() {
        val executor = ScriptedExecutor(success(patterned(BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES)))
        listOf(
            byteArrayOf(0, 0, 0, 0),
            "ILA0".toByteArray(StandardCharsets.US_ASCII),
            byteArrayOf(0x49, 0x4c, 0x41, 0),
            "XLA1".toByteArray(StandardCharsets.US_ASCII),
        ).forEach { prefix ->
            val request = patterned(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES)
            prefix.copyInto(request)
            assertFailsWith<IllegalArgumentException> {
                client(executor).issue(credential(), request)
            }
        }
        listOf(
            byteArrayOf(0, 0, 0, 0),
            "ILQ0".toByteArray(StandardCharsets.US_ASCII),
            byteArrayOf(0x49, 0x4c, 0x51, 0),
            "XLQ1".toByteArray(StandardCharsets.US_ASCII),
        ).forEach { prefix ->
            val request = patterned(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES)
            prefix.copyInto(request, BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES)
            assertFailsWith<IllegalArgumentException> {
                client(executor).issue(credential(), request)
            }
        }
        assertEquals(0, executor.calls)
    }

    @Test
    fun credentialAdmissionIsCanonicalBoundedDefensiveAndRedacted() {
        assertFailsWith<IllegalArgumentException> {
            BootleLanternIssuanceCredentialV1.fromOpaqueBytes(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            BootleLanternIssuanceCredentialV1.fromOpaqueBytes(
                ByteArray(BootleLanternIssuanceCredentialV1.MAX_BYTES + 1),
            )
        }
        listOf(
            "",
            "A",
            "YQ==",
            "YR",
            "Y Q",
            "YQ\n",
            "Bearer YQ",
            "+w",
            java.util.Base64.getUrlEncoder().withoutPadding().encodeToString(
                ByteArray(BootleLanternIssuanceCredentialV1.MAX_BYTES + 1),
            ),
            "A".repeat(((BootleLanternIssuanceCredentialV1.MAX_BYTES + 2) / 3) * 4 + 1),
        ).forEach { malformed ->
            assertFailsWith<IllegalArgumentException>("credential `$malformed` must fail") {
                BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(malformed)
            }
        }

        val source = byteArrayOf(0x61)
        val credential = BootleLanternIssuanceCredentialV1.fromOpaqueBytes(source)
        source[0] = 0x62
        val executor = ScriptedExecutor(success(patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES)))
        client(executor).authorize(credential).join()
        assertEquals("Bearer YQ", exactHeader(executor.lastRequest, "Authorization"))
        assertFalse(credential.toString().contains("YQ"))
        assertFalse(credential.toString().contains("61"))
        assertTrue(credential.toString().contains("REDACTED"))

        val exactMaximum = ByteArray(BootleLanternIssuanceCredentialV1.MAX_BYTES) { 0xff.toByte() }
        val exactMaximumEncoded = java.util.Base64.getUrlEncoder().withoutPadding()
            .encodeToString(exactMaximum)
        val maximumCredential =
            BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(exactMaximumEncoded)
        assertEquals("Bearer $exactMaximumEncoded", maximumCredential.authorizationHeaderValue())
        maximumCredential.close()

        credential.close()
        credential.close()
        assertFailsWith<IllegalStateException> {
            client(executor).authorize(credential)
        }
        assertEquals(1, executor.calls)
    }

    @Test
    fun authorizeRejectsZeroTruncatedAndExtendedResponses() {
        listOf(
            0,
            1,
            BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES - 1,
            BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES + 1,
        ).forEach { size ->
            val executor = ScriptedExecutor(success(ByteArray(size), includeLength = false))
            assertClientFailure(client(executor).authorize(credential()))
            assertEquals(1, executor.calls)
        }
    }

    @Test
    fun issueRejectsZeroTruncatedAndExtendedResponses() {
        listOf(
            0,
            1,
            BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES - 1,
            BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES + 1,
        ).forEach { size ->
            val executor = ScriptedExecutor(success(ByteArray(size), includeLength = false))
            assertClientFailure(
                client(executor).issue(
                    credential(),
                    patterned(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES),
                ),
            )
            assertEquals(1, executor.calls)
        }
    }

    @Test
    fun successfulResponsesRequireExactIla1AndIlr1Magic() {
        listOf(
            byteArrayOf(0, 0, 0, 0),
            "ILA0".toByteArray(StandardCharsets.US_ASCII),
            byteArrayOf(0x49, 0x4c, 0x41, 0),
            "XLA1".toByteArray(StandardCharsets.US_ASCII),
        ).forEach { prefix ->
            val body = patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES)
            prefix.copyInto(body)
            assertClientFailure(client(ScriptedExecutor(success(body))).authorize(credential()))
        }
        listOf(
            byteArrayOf(0, 0, 0, 0),
            "ILR0".toByteArray(StandardCharsets.US_ASCII),
            byteArrayOf(0x49, 0x4c, 0x52, 0),
            "XLR1".toByteArray(StandardCharsets.US_ASCII),
        ).forEach { prefix ->
            val body = patterned(BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES)
            prefix.copyInto(body)
            assertClientFailure(
                client(ScriptedExecutor(success(body))).issue(
                    credential(),
                    patterned(BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES),
                ),
            )
        }
    }

    @Test
    fun responsesRequireExactStatusAndNeverRetryRedirectOrFailure() {
        listOf(201, 204, 301, 302, 307, 308, 418, 500).forEach { status ->
            val executor = ScriptedExecutor(
                response(
                    status = status,
                    body = patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES),
                    headers = mapOf(
                        "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                    ),
                ),
            )
            assertClientFailure(client(executor).authorize(credential()))
            assertEquals(1, executor.calls, "HTTP $status must not be retried")
        }

        val asynchronousFailure = ScriptedExecutor(failure = IllegalStateException("network down"))
        assertClientFailure(client(asynchronousFailure).authorize(credential()))
        assertEquals(1, asynchronousFailure.calls)

        val synchronousFailure = ScriptedExecutor(throwSynchronously = true)
        assertClientFailure(client(synchronousFailure).authorize(credential()))
        assertEquals(1, synchronousFailure.calls)
    }

    @Test
    fun transportFailuresAfterAuthorizationExposureDiscardSecretBearingCauses() {
        val leaked = "Bearer secret-that-must-not-survive"
        val asynchronous = ScriptedExecutor(failure = IllegalStateException(leaked))
        val asynchronousFailure = assertClientFailure(client(asynchronous).authorize(credential()))
        assertEquals("Bootle/Lantern issuance authorization request failed", asynchronousFailure.message)
        assertEquals(null, asynchronousFailure.cause)
        assertFalse(asynchronousFailure.toString().contains(leaked))

        val synchronous = ScriptedExecutor(
            failure = IllegalStateException(leaked),
            throwSynchronously = true,
        )
        val synchronousFailure = assertClientFailure(client(synchronous).authorize(credential()))
        assertEquals("Bootle/Lantern issuance authorization request failed", synchronousFailure.message)
        assertEquals(null, synchronousFailure.cause)
        assertFalse(synchronousFailure.toString().contains(leaked))
    }

    @Test
    fun structuredErrorsBindStatusMediaCodeAndRetryHint() {
        errorContracts().forEach { contract ->
            val executor = ScriptedExecutor(errorResponse(contract))
            val failure = assertClientFailure(client(executor).authorize(credential()))
            assertEquals(contract.getValue("status").jsonPrimitive.int, failure.statusCode)
            assertEquals(contract.getValue("code").jsonPrimitive.content, failure.code)
            assertEquals(
                contract["retry_after_seconds"]?.jsonPrimitive?.int?.toLong(),
                failure.retryAfterSeconds,
            )
            assertEquals(1, executor.calls)
        }
    }

    @Test
    fun allSevenNoritoErrorsRejectLegacyMalformedTruncatedAndTrailingFrames() {
        val contracts = errorContracts().filter {
            it.getValue("media_type").jsonPrimitive.content ==
                BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE
        }
        assertEquals(7, contracts.size)
        contracts.forEach { contract ->
            val canonical = errorBody(contract)
            val variants = listOf(
                rejectedLegacyNoritoErrorFrame(
                    canonical,
                    contract.getValue("code").jsonPrimitive.content,
                ),
                malformedNoritoFieldFrame(canonical),
                canonical.copyOf(canonical.size - 1),
                canonical.copyOf(canonical.size + 1),
            )
            variants.forEach { body ->
                val failure = assertClientFailure(
                    client(ScriptedExecutor(errorResponse(contract, body = body)))
                        .authorize(credential()),
                )
                assertEquals(null, failure.statusCode)
                assertEquals(null, failure.code)
                assertEquals(null, failure.retryAfterSeconds)
            }
        }
    }

    @Test
    fun structuredErrorsRejectMalformedSubstitutedAndOversizedEnvelopes() {
        val contracts = errorContracts().associateBy { it.getValue("status").jsonPrimitive.int }
        val badRequest = requireNotNull(contracts[400])
        val unauthorized = requireNotNull(contracts[401])
        val notAcceptable = requireNotNull(contracts[406])
        val capacity = requireNotNull(contracts[429])
        val unavailable = requireNotNull(contracts[503])
        val corrupted = errorBody(badRequest).also { it[0] = (it[0].toInt() xor 1).toByte() }
        val adversarial = listOf(
            errorResponse(badRequest, body = corrupted),
            errorResponse(
                badRequest,
                headers = mapOf(
                    "Content-Type" to listOf("application/json"),
                    "Content-Length" to listOf(errorBody(badRequest).size.toString()),
                ),
            ),
            errorResponse(
                badRequest,
                headers = mapOf(
                    "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                    "Content-Encoding" to listOf("identity"),
                ),
            ),
            errorResponse(
                badRequest,
                headers = mapOf(
                    "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                    "Content-Length" to listOf("0107"),
                ),
            ),
            errorResponse(badRequest, body = errorBody(unauthorized)),
            errorResponse(
                notAcceptable,
                body = (notAcceptable.getValue("body_utf8").jsonPrimitive.content + " ")
                    .toByteArray(StandardCharsets.UTF_8),
            ),
            errorResponse(
                capacity,
                headers = mapOf(
                    "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                    "Retry-After" to listOf("2"),
                ),
            ),
            errorResponse(
                unavailable,
                headers = mapOf(
                    "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                    "Retry-After" to listOf("1"),
                ),
            ),
            errorResponse(
                unauthorized,
                headers = mapOf(
                    "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                    "Content-Length" to listOf(errorBody(unauthorized).size.toString()),
                ),
            ),
            errorResponse(
                unauthorized,
                headers = mapOf(
                    "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                    "Content-Length" to listOf(errorBody(unauthorized).size.toString()),
                    "WWW-Authenticate" to listOf(
                        unauthorized.getValue("www_authenticate").jsonPrimitive.content,
                        unauthorized.getValue("www_authenticate").jsonPrimitive.content,
                    ),
                ),
            ),
            errorResponse(
                unauthorized,
                headers = mapOf(
                    "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                    "Content-Length" to listOf(errorBody(unauthorized).size.toString()),
                    "WWW-Authenticate" to listOf("Bearer realm=\"attacker\""),
                ),
            ),
            errorResponse(
                badRequest,
                headers = mapOf(
                    "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                    "Content-Length" to listOf(errorBody(badRequest).size.toString()),
                    "WWW-Authenticate" to listOf(
                        unauthorized.getValue("www_authenticate").jsonPrimitive.content,
                    ),
                ),
            ),
            errorResponse(
                badRequest,
                body = ByteArray(BootleLanternIssuanceClientV1.ERROR_RESPONSE_MAX_BYTES + 1),
            ),
        )
        adversarial.forEach { response ->
            val failure = assertClientFailure(
                client(ScriptedExecutor(response)).authorize(credential()),
            )
            assertEquals(null, failure.statusCode)
            assertEquals(null, failure.code)
            assertEquals(null, failure.retryAfterSeconds)
        }
    }

    @Test
    fun responsesRequireOneExactNoritoContentType() {
        val bytes = patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES)
        val variants = listOf(
            emptyList(),
            listOf("application/json"),
            listOf("application/x-norito; charset=binary"),
            listOf("Application/X-Norito"),
            listOf("application/x-norito", "application/x-norito"),
        )
        variants.forEach { values ->
            val headers = if (values.isEmpty()) emptyMap() else mapOf("Content-Type" to values)
            val executor = ScriptedExecutor(response(200, bytes, headers))
            assertClientFailure(client(executor).authorize(credential()))
            assertEquals(1, executor.calls)
        }
    }

    @Test
    fun responsesRejectCompressionIncludingIdentityEncoding() {
        listOf("gzip", "br", "deflate", "identity").forEach { encoding ->
            val executor = ScriptedExecutor(
                response(
                    200,
                    patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES),
                    mapOf(
                        "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                        "Content-Encoding" to listOf(encoding),
                    ),
                ),
            )
            assertClientFailure(client(executor).authorize(credential()))
            assertEquals(1, executor.calls)
        }

        val challenged = ScriptedExecutor(
            response(
                200,
                patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES),
                mapOf(
                    "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                    "WWW-Authenticate" to listOf(
                        "Bearer realm=\"iroha-bootle-lantern-issuance\"",
                    ),
                ),
            ),
        )
        assertClientFailure(client(challenged).authorize(credential()))
    }

    @Test
    fun responseContentLengthMustBeUniqueCanonicalAndExactWhenPresent() {
        val bytes = patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES)
        listOf(
            listOf("319"),
            listOf("321"),
            listOf("0320"),
            listOf("+320"),
            listOf("320 "),
            listOf("320", "320"),
        ).forEach { values ->
            val executor = ScriptedExecutor(
                response(
                    200,
                    bytes,
                    mapOf(
                        "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
                        "Content-Length" to values,
                    ),
                ),
            )
            assertClientFailure(client(executor).authorize(credential()))
        }

        val absent = ScriptedExecutor(success(bytes, includeLength = false))
        assertContentEquals(bytes, client(absent).authorize(credential()).join())
        val exact = ScriptedExecutor(success(bytes, includeLength = true))
        assertContentEquals(bytes, client(exact).authorize(credential()).join())
    }

    @Test
    fun errorsNeverRenderCredentialOrResponseBody() {
        val secret = "credential-secret"
        val encoded = java.util.Base64.getUrlEncoder().withoutPadding()
            .encodeToString(secret.toByteArray(StandardCharsets.UTF_8))
        val credential = BootleLanternIssuanceCredentialV1.fromCanonicalBase64Url(encoded)
        val executor = ScriptedExecutor(
            response(
                401,
                ("server-echo:$secret:$encoded").toByteArray(StandardCharsets.UTF_8),
                emptyMap(),
            ),
        )

        val error = assertClientFailure(client(executor).authorize(credential))
        val rendered = error.toString()
        assertFalse(rendered.contains(secret))
        assertFalse(rendered.contains(encoded))
        assertFalse(rendered.contains("server-echo"))
    }

    @Test
    fun clientRejectsInsecureOrNonOriginBaseUrisBeforeSendingCredentials() {
        listOf(
            "http://taira.sora.org",
            "https://user@taira.sora.org",
            "https://taira.sora.org/proxy",
            "https://taira.sora.org?route=privacy",
            "https://taira.sora.org#privacy",
            "/relative",
        ).forEach { uri ->
            val executor = ScriptedExecutor(success(patterned(BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES)))
            assertFailsWith<IllegalArgumentException>(uri) {
                BootleLanternIssuanceClientV1.builder()
                    .baseUri(URI.create(uri))
                    .executor(executor)
                    .build()
            }
            assertEquals(0, executor.calls)
        }
    }

    private fun client(executor: HttpTransportExecutor): BootleLanternIssuanceClientV1 =
        BootleLanternIssuanceClientV1.builder()
            .baseUri(URI.create("https://taira.sora.org"))
            .executor(executor)
            .build()

    private fun credential(): BootleLanternIssuanceCredentialV1 =
        BootleLanternIssuanceCredentialV1.fromOpaqueBytes(byteArrayOf(1, 2, 3))

    private fun exactHeader(request: TransportRequest, name: String): String {
        val values = headerValues(request, name)
        assertEquals(1, values.size, "$name must occur exactly once")
        return values.single()
    }

    private fun headerValues(request: TransportRequest, name: String): List<String> =
        request.headers.entries
            .filter { (headerName, _) -> headerName.equals(name, ignoreCase = true) }
            .flatMap { (_, values) -> values }

    private fun assertClientFailure(
        future: CompletableFuture<ByteArray>,
    ): BootleLanternIssuanceClientExceptionV1 {
        val failure = assertFailsWith<CompletionException> { future.join() }
        assertTrue(failure.cause is BootleLanternIssuanceClientExceptionV1)
        return failure.cause as BootleLanternIssuanceClientExceptionV1
    }

    private fun success(body: ByteArray, includeLength: Boolean = true): TransportResponse {
        val headers = linkedMapOf(
            "Content-Type" to listOf(BootleLanternIssuanceClientV1.NORITO_MEDIA_TYPE),
        )
        if (includeLength) headers["Content-Length"] = listOf(body.size.toString())
        return response(200, body, headers)
    }

    private fun errorContracts(): List<JsonObject> {
        val fixture = Json.parseToJsonElement(
            String(Files.readAllBytes(clientContractFixture()), StandardCharsets.UTF_8),
        ).jsonObject
        return fixture.getValue("errors").jsonObject
            .getValue("responses").jsonArray
            .map { it.jsonObject }
    }

    private fun errorBody(contract: JsonObject): ByteArray =
        contract["body_hex"]?.jsonPrimitive?.content?.hexBytes()
            ?: contract.getValue("body_utf8").jsonPrimitive.content
                .toByteArray(StandardCharsets.UTF_8)

    private fun malformedNoritoFieldFrame(body: ByteArray): ByteArray {
        val malformed = body.copyOf()
        assertContentEquals(
            "NRT0".toByteArray(StandardCharsets.US_ASCII),
            malformed.copyOfRange(0, 4),
        )
        val frame = ByteBuffer.wrap(malformed).order(ByteOrder.LITTLE_ENDIAN)
        val payloadLength = frame.getLong(23)
        assertEquals(40L + payloadLength, malformed.size.toLong())
        assertTrue((malformed[40].toInt() and 0xff) < 0x7f)
        malformed[40] = (malformed[40].toInt() + 1).toByte()
        frame.putLong(31, crc64(malformed.copyOfRange(40, malformed.size)))
        return malformed
    }

    private fun rejectedLegacyNoritoErrorFrame(template: ByteArray, code: String): ByteArray {
        val encoded = code.toByteArray(StandardCharsets.UTF_8)
        assertTrue(encoded.size < 0x80)
        val payload = ByteArray(encoded.size * 2 + 3)
        var offset = 0
        payload[offset++] = encoded.size.toByte()
        encoded.copyInto(payload, offset)
        offset += encoded.size
        payload[offset++] = encoded.size.toByte()
        encoded.copyInto(payload, offset)
        offset += encoded.size
        payload[offset] = 0
        return noritoFrameWithPayload(template, payload)
    }

    private fun noritoFrameWithPayload(template: ByteArray, payload: ByteArray): ByteArray {
        val frameBytes = template.copyOf(40 + payload.size)
        payload.copyInto(frameBytes, 40)
        val frame = ByteBuffer.wrap(frameBytes).order(ByteOrder.LITTLE_ENDIAN)
        frame.putLong(23, payload.size.toLong())
        frame.putLong(31, crc64(payload))
        return frameBytes
    }

    private fun crc64(payload: ByteArray): Long {
        val polynomial = 0xC96C_5795_D787_0F42uL
        var value = ULong.MAX_VALUE
        payload.forEach { raw ->
            value = value xor (raw.toInt() and 0xff).toULong()
            repeat(8) {
                value = if (value and 1uL == 0uL) {
                    value shr 1
                } else {
                    polynomial xor (value shr 1)
                }
            }
        }
        return (value xor ULong.MAX_VALUE).toLong()
    }

    private fun errorResponse(
        contract: JsonObject,
        body: ByteArray = errorBody(contract),
        headers: Map<String, List<String>>? = null,
    ): TransportResponse {
        val canonicalHeaders = linkedMapOf(
            "Content-Type" to listOf(contract.getValue("media_type").jsonPrimitive.content),
            "Content-Length" to listOf(body.size.toString()),
        )
        contract["retry_after_seconds"]?.jsonPrimitive?.int?.let { retry ->
            canonicalHeaders["Retry-After"] = listOf(retry.toString())
        }
        contract["www_authenticate"]?.jsonPrimitive?.content?.let { challenge ->
            canonicalHeaders["WWW-Authenticate"] = listOf(challenge)
        }
        return response(
            contract.getValue("status").jsonPrimitive.int,
            body,
            headers ?: canonicalHeaders,
        )
    }

    private fun response(
        status: Int,
        body: ByteArray,
        headers: Map<String, List<String>>,
    ): TransportResponse = TransportResponse(status, body, "scripted", headers, null, false)

    private fun patterned(size: Int): ByteArray = ByteArray(size) { index -> index.toByte() }.also { body ->
        when (size) {
            BootleLanternIssuanceClientV1.AUTHORIZATION_RESPONSE_BYTES ->
                "ILA1".toByteArray(StandardCharsets.US_ASCII).copyInto(body)
            BootleLanternIssuanceClientV1.ISSUE_REQUEST_BYTES -> {
                "ILA1".toByteArray(StandardCharsets.US_ASCII).copyInto(body)
                "ILQ1".toByteArray(StandardCharsets.US_ASCII).copyInto(body, 320)
            }
            BootleLanternIssuanceClientV1.ISSUE_RESPONSE_BYTES ->
                "ILR1".toByteArray(StandardCharsets.US_ASCII).copyInto(body)
        }
    }

    private fun clientContractFixture(): Path {
        var current = Paths.get("").toAbsolutePath().normalize()
        while (true) {
            val candidate = current.resolve(
                "fixtures/privacy/bootle_lantern_issuance_client_v1.json",
            )
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent
                ?: error("shared Bootle/Lantern issuance client fixture was not found")
        }
    }

    private fun sha256Hex(bytes: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") {
            "%02x".format(it.toInt() and 0xff)
        }

    private fun String.hexBytes(): ByteArray =
        chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    private class ScriptedExecutor(
        private val response: TransportResponse? = null,
        private val failure: Throwable? = null,
        private val throwSynchronously: Boolean = false,
    ) : HttpTransportExecutor {
        var calls: Int = 0
        lateinit var lastRequest: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            calls += 1
            lastRequest = request
            if (throwSynchronously) {
                throw failure as? RuntimeException
                    ?: IllegalStateException("synchronous transport failure")
            }
            val future = CompletableFuture<TransportResponse>()
            if (failure != null) {
                future.completeExceptionally(failure)
            } else {
                future.complete(requireNotNull(response))
            }
            return future
        }
    }
}
