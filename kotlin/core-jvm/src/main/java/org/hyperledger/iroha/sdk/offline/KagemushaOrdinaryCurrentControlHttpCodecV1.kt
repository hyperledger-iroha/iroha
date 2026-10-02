// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

/** Bounded data encoding only. Native authenticates the full issuer/World originals separately. */
object KagemushaOrdinaryCurrentControlHttpCodecV1 {
    const val MAXIMUM_REQUEST_BYTES: Int = 8192
    const val MAXIMUM_SIGNED_CONTROL_BYTES: Int = 64 * 1024
    const val MAXIMUM_AUTHORITY_BYTES: Int = 128 * 1024 * 1024
    const val MAXIMUM_RESPONSE_BYTES: Int =
        ((MAXIMUM_SIGNED_CONTROL_BYTES + 2) / 3) * 4 +
        ((MAXIMUM_AUTHORITY_BYTES + 2) / 3) * 4 + 1024

    /** Sole original request and its already Native-retained Ed64. No managed key is invoked. */
    fun requestBody(request: ByteArray, nativeAccountSignature: ByteArray): ByteArray {
        require(request.size in 1..MAXIMUM_REQUEST_BYTES && nativeAccountSignature.size == 64)
        return JsonEncoder.encode(linkedMapOf(
            "schema" to "iroha.kagemusha.ordinary-current-fi-control-request.v1",
            "canonical_request_base64" to Base64.getEncoder().encodeToString(request),
            "account_signature_base64" to Base64.getEncoder().encodeToString(nativeAccountSignature),
        )).toByteArray(Charsets.UTF_8)
    }

    /** Stable HTTP correlation from the complete unchanged Native request, not FI authority. */
    fun requestId(request: ByteArray): String {
        require(request.size in 1..MAXIMUM_REQUEST_BYTES)
        val digest = MessageDigest.getInstance("SHA-256").digest(
            "iroha:kagemusha:v1:ordinary-current-fi-control-http\u0000".toByteArray(Charsets.US_ASCII) + request)
        return KagemushaOrdinaryIdentityHttpCodecV1.rawAttestationRequestId(digest)
    }

    /** Return only the two bounded full originals. Unknown/duplicate keys and alternate Base64
     * encodings are refused. Decoding does not verify a signature, finality, FI status or a clock.
     */
    fun responseOriginals(raw: ByteArray): List<ByteArray> {
        require(raw.size in 1..MAXIMUM_RESPONSE_BYTES)
        val text = Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(raw)).toString()
        @Suppress("UNCHECKED_CAST")
        val fields = JsonParser.parse(text) as? Map<String, Any?>
            ?: error("Current FI response is not an object")
        require(fields.keys == setOf("signed_control_original_base64", "authority_original_base64"))
        fun original(key: String, maximum: Int): ByteArray {
            val encoded = fields.getValue(key) as? String ?: error("Current FI original is not text")
            require(encoded.length <= ((maximum + 2) / 3) * 4)
            return Base64.getDecoder().decode(encoded).also {
                require(it.size in 1..maximum && Base64.getEncoder().encodeToString(it) == encoded)
            }
        }
        return listOf(original("signed_control_original_base64", MAXIMUM_SIGNED_CONTROL_BYTES),
            original("authority_original_base64", MAXIMUM_AUTHORITY_BYTES))
    }
}
