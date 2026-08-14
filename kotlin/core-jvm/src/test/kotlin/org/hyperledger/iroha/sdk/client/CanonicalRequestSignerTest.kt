package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.security.Signature
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.testing.TestNetworkIds
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class CanonicalRequestSignerTest {
    private val networkId = TestNetworkIds.canonical()

    @Test
    fun canonicalQueryMatchesRustFormEncodingAndUtf8Ordering() {
        assertEquals(
            "a=1&b=%21*%28%29%7E%27",
            CanonicalRequestSigner.canonicalQueryString("b=!*()~'&a=1"),
        )
        assertEquals(
            "x=A%25zz%EF%BF%BD",
            CanonicalRequestSigner.canonicalQueryString("x=%41%zz%FF"),
        )
        assertEquals(
            "%EE%80%80=bmp&%F0%90%80%80=supplementary",
            CanonicalRequestSigner.canonicalQueryString("\uE000=bmp&\uD800\uDC00=supplementary"),
        )
        assertEquals(
            "k=%EE%80%80&k=%F0%90%80%80",
            CanonicalRequestSigner.canonicalQueryString("k=\uD800\uDC00&k=\uE000"),
        )
    }

    @Test
    fun canonicalQueryEnforcesV1PairAndByteLimits() {
        val exactPairs = (0 until CanonicalRequestSigner.CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1)
            .joinToString("&") { "k$it=v" }
        CanonicalRequestSigner.canonicalQueryString(exactPairs)
        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.canonicalQueryString("$exactPairs&overflow=v")
        }

        val exactBytes = "k=" + "x".repeat(
            CanonicalRequestSigner.CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 - 2,
        )
        assertEquals(
            CanonicalRequestSigner.CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1,
            exactBytes.toByteArray(StandardCharsets.UTF_8).size,
        )
        CanonicalRequestSigner.canonicalQueryString(exactBytes)
        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.canonicalQueryString(exactBytes + "x")
        }

        assertEquals("a=1&b=2", CanonicalRequestSigner.canonicalQueryString("&&b=2&&a=1&"))
    }

    @Test
    fun canonicalRequestEnforcesV1MethodLimit() {
        val uri = URI.create("https://torii.example/v1/test")
        CanonicalRequestSigner.canonicalRequestMessage(
            "A".repeat(CanonicalRequestSigner.CANONICAL_REQUEST_MAX_METHOD_BYTES_V1),
            uri,
            ByteArray(0),
        )
        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.canonicalRequestMessage(
                "A".repeat(CanonicalRequestSigner.CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 + 1),
                uri,
                ByteArray(0),
            )
        }
    }

    @Test
    fun canonicalRequestEnforcesV1PathLimit() {
        val root = CanonicalRequestSigner.canonicalRequestMessage(
            "GET",
            URI.create("/"),
            ByteArray(0),
        )
        val originWithoutPath = CanonicalRequestSigner.canonicalRequestMessage(
            "GET",
            URI.create("https://torii.example"),
            ByteArray(0),
        )
        assertEquals(
            String(root, StandardCharsets.UTF_8),
            String(originWithoutPath, StandardCharsets.UTF_8),
        )

        val exact = URI.create(
            "/" + "x".repeat(CanonicalRequestSigner.CANONICAL_REQUEST_MAX_PATH_BYTES_V1 - 1),
        )
        CanonicalRequestSigner.canonicalRequestMessage("GET", exact, ByteArray(0))
        val excessive = URI.create(
            "/" + "x".repeat(CanonicalRequestSigner.CANONICAL_REQUEST_MAX_PATH_BYTES_V1),
        )
        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.canonicalRequestMessage("GET", excessive, ByteArray(0))
        }
    }

    @Test
    fun canonicalRequestRejectsNegativeTimestamp() {
        val uri = URI.create("/v1/test")
        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.canonicalRequestSignatureMessage(
                networkId,
                "GET",
                uri,
                ByteArray(0),
                -1,
                "negative-timestamp",
            )
        }
    }

    @Test
    fun canonicalAuthEnforcesV1AccountAndNonceLimits() {
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val uri = URI.create("https://torii.example/v1/accounts")
        val timestampMs = 1_717_171_717_005L
        val exactAccount = "a".repeat(
            CanonicalRequestSigner.CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1,
        )
        assertEquals(
            CanonicalRequestSigner.CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1,
            exactAccount.toByteArray(StandardCharsets.UTF_8).size,
        )
        CanonicalRequestSigner.buildHeaders(
            networkId,
            "get",
            uri,
            ByteArray(0),
            exactAccount,
            keyPair.private,
            timestampMs,
            "account-limit",
        )
        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.buildHeaders(
                networkId,
                "get",
                uri,
                ByteArray(0),
                exactAccount + "a",
                keyPair.private,
                timestampMs,
                "account-limit-plus-one",
            )
        }

        val exactNonce = "n".repeat(256)
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            networkId,
            "get",
            uri,
            ByteArray(0),
            timestampMs,
            exactNonce,
        )
        listOf(exactNonce + "n", "internal space", "control\u0001", "nönce").forEach { nonce ->
            assertFailsWith<IllegalArgumentException>(nonce) {
                CanonicalRequestSigner.canonicalRequestSignatureMessage(
                    networkId,
                    "get",
                    uri,
                    ByteArray(0),
                    timestampMs,
                    nonce,
                )
            }
        }
    }

    @Test
    fun canonicalHeadersUseAsciiHexForI105AndPreserveAlias() {
        val i105 = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        val canonicalHex =
            "0x02000120ce7fa46c9dce7ea4b125e2e36bdb63ea33073e7590ac92816ae1e861b7048b03"
        assertEquals(
            canonicalHex,
            AccountAddress.parseEncodedIgnoringCurveSupport(i105, null).address.canonicalHex(),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val uri = URI.create("https://torii.example/v1/accounts")
        val timestampMs = 1_717_171_717_006L
        val i105Headers = CanonicalRequestSigner.buildHeaders(
            networkId,
            "get",
            uri,
            ByteArray(0),
            i105,
            keyPair.private,
            timestampMs,
            "i105-header-hex",
        )

        assertEquals(canonicalHex, i105Headers[CanonicalRequestSigner.HEADER_ACCOUNT])
        assertTrue(canonicalHex.matches(Regex("0x[0-9a-f]+")))

        val alias = "alice-1@wonderland"
        val aliasHeaders = CanonicalRequestSigner.buildHeaders(
            networkId,
            "get",
            uri,
            ByteArray(0),
            alias,
            keyPair.private,
            timestampMs,
            "alias-header",
        )
        assertEquals(alias, aliasHeaders[CanonicalRequestSigner.HEADER_ACCOUNT])

        val signedBody = CanonicalRequestSigner.withBodySignature(
            networkId,
            "post",
            uri,
            emptyMap(),
            i105,
            keyPair.private,
            timestampMs,
            "i105-body",
        )
        assertEquals(i105, signedBody[CanonicalRequestSigner.BODY_ACCOUNT_ID])
    }

    @Test
    fun unsignedBodyAuthJsonRemovesOnlyTopLevelProofFields() {
        val body = linkedMapOf<String, Any?>(
            "z" to "last",
            CanonicalRequestSigner.BODY_SIGNATURE_BASE64 to "remove",
            "nested" to linkedMapOf(CanonicalRequestSigner.BODY_SIGNATURE_BASE64 to "keep"),
            "witness_base64" to "remove-too",
            CanonicalRequestSigner.BODY_ACCOUNT_ID to "alice",
            CanonicalRequestSigner.BODY_TIMESTAMP_MS to 7L,
            CanonicalRequestSigner.BODY_NONCE to "n",
        )

        val unsigned = String(CanonicalRequestSigner.unsignedBodyAuthJson(body), StandardCharsets.UTF_8)

        assertEquals(
            """{"account_id":"alice","nested":{"signature_base64":"keep"},"nonce":"n","timestamp_ms":7,"z":"last"}""",
            unsigned,
        )
    }

    @Test
    fun bodySignatureFieldsCarryVerifiableSignature() {
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val uri = URI.create("https://torii.example/v1/offline/top-up?b=2&a=1")
        val timestampMs = 1_717_171_717_000L
        val nonce = "offline-body-nonce"
        val body = linkedMapOf<String, Any?>("operation_id" to "operation-1")

        val signed = CanonicalRequestSigner.withBodySignature(
            networkId,
            "post",
            uri,
            body,
            "alice",
            keyPair.private,
            timestampMs,
            nonce,
        )

        assertEquals("alice", signed[CanonicalRequestSigner.BODY_ACCOUNT_ID])
        assertEquals(timestampMs, signed[CanonicalRequestSigner.BODY_TIMESTAMP_MS])
        assertEquals(nonce, signed[CanonicalRequestSigner.BODY_NONCE])
        assertFalse(signed.containsKey("witness_base64"))

        val signatureBytes = Base64.getDecoder()
            .decode(signed[CanonicalRequestSigner.BODY_SIGNATURE_BASE64] as String)
        val message = CanonicalRequestSigner.canonicalBodyAuthSignatureMessage(
            networkId,
            "post",
            uri,
            signed,
            timestampMs,
            nonce,
        )
        val verifier = Signature.getInstance("Ed25519")
        verifier.initVerify(keyPair.public)
        verifier.update(message)
        assertTrue(verifier.verify(signatureBytes))
    }

    @Test
    fun canonicalAuthRejectsPaddedFreshnessAndAccountFields() {
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val uri = URI.create("https://torii.example/v1/offline/top-up")
        val bodyBytes = """{"operation_id":"operation-1"}""".toByteArray(StandardCharsets.UTF_8)
        val body = linkedMapOf<String, Any?>("operation_id" to "operation-1")
        val timestampMs = 1_717_171_717_003L

        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.canonicalRequestSignatureMessage(
                networkId,
                "post",
                uri,
                bodyBytes,
                timestampMs,
                " nonce",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.buildHeaders(
                networkId,
                "post",
                uri,
                bodyBytes,
                "alice ",
                keyPair.private,
                timestampMs,
                "nonce",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.buildHeaders(
                networkId,
                "post",
                uri,
                bodyBytes,
                "alice",
                keyPair.private,
                timestampMs,
                "\nnonce",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.withBodySignature(
                networkId,
                "post",
                uri,
                body,
                " alice",
                keyPair.private,
                timestampMs,
                "nonce",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            CanonicalRequestSigner.withBodySignature(
                networkId,
                "post",
                uri,
                body,
                "alice",
                keyPair.private,
                timestampMs,
                "nonce ",
            )
        }
    }

    @Test
    fun canonicalAuthCannotReplayAcrossSameLabelNetworks() {
        val uri = URI.create("https://torii.example/v1/accounts?label=same")
        val canonical = CanonicalRequestSigner.canonicalRequestSignatureMessage(
            networkId,
            "GET",
            uri,
            ByteArray(0),
            1_717_171_717_003L,
            "network-bound-nonce",
        )
        val foreign = CanonicalRequestSigner.canonicalRequestSignatureMessage(
            TestNetworkIds.fromSeed(7),
            "GET",
            uri,
            ByteArray(0),
            1_717_171_717_003L,
            "network-bound-nonce",
        )

        assertFalse(canonical.contentEquals(foreign))
    }
}
