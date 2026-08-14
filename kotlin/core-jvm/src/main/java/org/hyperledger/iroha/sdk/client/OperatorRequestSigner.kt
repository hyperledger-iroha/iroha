package org.hyperledger.iroha.sdk.client

import java.io.ByteArrayOutputStream
import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.SecureRandom
import java.util.Base64
import java.util.Locale

/** Exact-request header builder for operator-authenticated Torii APIs. */
object OperatorRequestSigner {
    const val HEADER_PUBLIC_KEY = "X-Iroha-Operator-Public-Key"
    const val HEADER_TIMESTAMP_MS = "X-Iroha-Operator-Timestamp-Ms"
    const val HEADER_NONCE = "X-Iroha-Operator-Nonce"
    const val HEADER_SIGNATURE = "X-Iroha-Operator-Signature"

    private val domain = "iroha.operator.http-request.network.v1\u0000"
        .toByteArray(StandardCharsets.UTF_8)
    private val nonceRandom = SecureRandom()
    private val forbiddenHeaders = setOf(
        "authorization",
        "x-api-token",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-timestamp-ms",
        "x-iroha-nonce",
        "x-iroha-witness",
        HEADER_PUBLIC_KEY.lowercase(Locale.ROOT),
        HEADER_TIMESTAMP_MS.lowercase(Locale.ROOT),
        HEADER_NONCE.lowercase(Locale.ROOT),
        HEADER_SIGNATURE.lowercase(Locale.ROOT),
    )

    /** Reject token, account, witness, and precomputed operator fallback headers. */
    @JvmStatic
    fun requireGeneratedAuth(headers: Map<String, *>?) {
        val forbidden = headers?.keys?.firstOrNull {
            forbiddenHeaders.contains(it.lowercase(Locale.ROOT))
        }
        require(forbidden == null) {
            "operator GET requires generated signing; header $forbidden is not accepted"
        }
    }

    /** Build the exact NetworkId-bound operator message for deterministic tests/signers. */
    @JvmStatic
    fun signatureMessage(
        context: OperatorSigningContext,
        method: String,
        uri: URI,
        body: ByteArray?,
        timestampMs: Long,
        nonce: String,
    ): ByteArray {
        require(timestampMs >= 0L) { "operator timestamp must be non-negative" }
        require(nonce.isNotEmpty() && nonce == nonce.trim()) {
            "operator nonce must be exact and non-empty"
        }
        val output = ByteArrayOutputStream()
        output.write(domain)
        output.write(context.networkId().bytes())
        output.write(CanonicalRequestSigner.canonicalRequestMessage(method, uri, body))
        output.write("\n$timestampMs\n$nonce".toByteArray(StandardCharsets.UTF_8))
        return output.toByteArray()
    }

    /** Build a fresh operator signature quartet for one finalized request target. */
    @JvmStatic
    fun buildHeaders(
        context: OperatorSigningContext,
        method: String,
        uri: URI,
        body: ByteArray?,
    ): Map<String, String> {
        val timestampMs = System.currentTimeMillis()
        val nonceBytes = ByteArray(16)
        nonceRandom.nextBytes(nonceBytes)
        val nonce = Base64.getUrlEncoder().withoutPadding().encodeToString(nonceBytes)
        val signature = context.sign(
            signatureMessage(context, method, uri, body, timestampMs, nonce),
        )
        return linkedMapOf(
            HEADER_PUBLIC_KEY to context.publicKey(),
            HEADER_TIMESTAMP_MS to timestampMs.toString(),
            HEADER_NONCE to nonce,
            HEADER_SIGNATURE to Base64.getEncoder().encodeToString(signature),
        )
    }
}
