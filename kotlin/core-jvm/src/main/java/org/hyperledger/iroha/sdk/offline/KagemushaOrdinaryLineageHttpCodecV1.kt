// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.io.FilterOutputStream
import java.io.OutputStream
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonParser

/** Exact existing service envelope. Neither decoding nor a carrier path installs a Core service. */
object KagemushaOrdinaryLineageHttpCodecV1 {
    const val MAXIMUM_RESPONSE_BYTES =
        ((KagemushaOrdinaryOutgoingFrameV1.MAXIMUM_SIGNED_BYTES + 2) / 3) * 4 +
        ((KagemushaOrdinaryOutgoingFrameV1.MAXIMUM_DATA_BYTES + 2) / 3) * 4 +
        ((KagemushaOrdinaryOutgoingFrameV1.MAXIMUM_AUTHORITY_BYTES + 2) / 3) * 4 + 1024

    /** Stream the sole fixed service schema without another full proof JSON allocation. */
    fun writeRequestBody(nativeRequest: ByteArray, nativeSignature: ByteArray, nativeServiceOriginal: ByteArray,
        output: OutputStream) {
        require(nativeRequest.size in 1..KagemushaOrdinaryOutgoingFrameV1.MAXIMUM_REQUEST_BYTES && nativeSignature.size == 64 &&
            nativeServiceOriginal.size in 1..KagemushaOrdinaryOutgoingFrameV1.MAXIMUM_PROOF_BYTES)
        fun text(value: String) = output.write(value.toByteArray(Charsets.US_ASCII))
        fun original(raw: ByteArray) {
            val noClose = object : FilterOutputStream(output) {
                override fun write(value: ByteArray, offset: Int, length: Int) = out.write(value, offset, length)
                override fun close() = flush()
            }
            Base64.getEncoder().wrap(noClose).use { it.write(raw) }
        }
        text("{\"schema\":\"iroha.kagemusha.ordinary-lineage-cas-request.v1\",\"canonical_request_base64\":\"")
        original(nativeRequest); text("\",\"account_signature_base64\":\""); original(nativeSignature)
        text("\",\"proof_bundle_original_base64\":\""); original(nativeServiceOriginal); text("\"}")
    }
    fun requestId(nativeRequest: ByteArray): String {
        require(nativeRequest.size in 1..KagemushaOrdinaryOutgoingFrameV1.MAXIMUM_REQUEST_BYTES)
        val digest = MessageDigest.getInstance("SHA-256").digest(
            "iroha:kagemusha:v1:ordinary-lineage-http\u0000".toByteArray(Charsets.US_ASCII) + nativeRequest)
        return KagemushaOrdinaryIdentityHttpCodecV1.rawAttestationRequestId(digest)
    }
    fun responseOriginals(raw: ByteArray): List<ByteArray> {
        require(raw.size in 1..MAXIMUM_RESPONSE_BYTES)
        val text = Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(raw)).toString()
        @Suppress("UNCHECKED_CAST") val fields = JsonParser.parse(text) as? Map<String, Any?> ?: error("Lineage response must be an object")
        require(fields.keys == setOf("signed_result_original_base64", "data_record_original_base64", "authority_original_base64"))
        fun original(name: String, max: Int): ByteArray {
            val encoded = fields.getValue(name) as? String ?: error("Lineage original must be text")
            require(encoded.length <= ((max + 2) / 3) * 4)
            return Base64.getDecoder().decode(encoded).also {
                require(it.size in 1..max && base64(it) == encoded)
            }
        }
        return listOf(original("signed_result_original_base64", KagemushaOrdinaryOutgoingFrameV1.MAXIMUM_SIGNED_BYTES),
            original("data_record_original_base64", KagemushaOrdinaryOutgoingFrameV1.MAXIMUM_DATA_BYTES),
            original("authority_original_base64", KagemushaOrdinaryOutgoingFrameV1.MAXIMUM_AUTHORITY_BYTES))
    }
    private fun base64(raw: ByteArray) = Base64.getEncoder().encodeToString(raw)
}
