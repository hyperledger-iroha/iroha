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

/** Complete signed CAS transport data, without a proof/owner/cash constructor. */
object KagemushaOrdinaryIncomingHttpCodecV1 {
    const val MAXIMUM_RESPONSE_BYTES =
        ((KagemushaOrdinaryIncomingFrameV1.SIGNED_MAXIMUM_BYTES + 2) / 3) * 4 +
        ((KagemushaOrdinaryIncomingFrameV1.DATA_MAXIMUM_BYTES + 2) / 3) * 4 +
        ((KagemushaOrdinaryIncomingFrameV1.AUTHORITY_MAXIMUM_BYTES + 2) / 3) * 4 + 1024
    fun requestId(request: ByteArray): String {
        require(request.size in 1..KagemushaOrdinaryIncomingFrameV1.REQUEST_MAXIMUM_BYTES)
        return KagemushaOrdinaryIdentityHttpCodecV1.rawAttestationRequestId(MessageDigest.getInstance("SHA-256").digest(
            "iroha:kagemusha:v1:ordinary-incoming-lineage-http\u0000".toByteArray(Charsets.US_ASCII) + request))
    }
    /** Stream the exact fixed schema and Base64 originals. Avoid an additional full proof JSON
     * String/byte array; closing the Base64 encoder only flushes this caller-owned output.
     */
    fun writeRequestBody(request: ByteArray, signature: ByteArray, proof: ByteArray, commit: Boolean, output: OutputStream) {
        require(request.size in 1..KagemushaOrdinaryIncomingFrameV1.REQUEST_MAXIMUM_BYTES && signature.size == 64)
        require(proof.size in 1..(if (commit) KagemushaOrdinaryIncomingFrameV1.COMMIT_BUNDLE_MAXIMUM_BYTES
            else KagemushaOrdinaryIncomingFrameV1.RESERVATION_BUNDLE_MAXIMUM_BYTES))
        fun text(value: String) = output.write(value.toByteArray(Charsets.US_ASCII))
        fun base64(raw: ByteArray) {
            val noClose = object : FilterOutputStream(output) {
                override fun write(value: ByteArray, offset: Int, length: Int) = out.write(value, offset, length)
                override fun close() = flush()
            }
            Base64.getEncoder().wrap(noClose).use { it.write(raw) }
        }
        text("{\"schema\":\"iroha.kagemusha.ordinary-lineage-cas-request.v1\",\"canonical_request_base64\":\"")
        base64(request); text("\",\"account_signature_base64\":\""); base64(signature)
        text("\",\"proof_bundle_original_base64\":\""); base64(proof); text("\"}")
    }
    fun responseOriginals(raw: ByteArray): List<ByteArray> {
        require(raw.size in 1..MAXIMUM_RESPONSE_BYTES)
        val text = Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(raw)).toString()
        @Suppress("UNCHECKED_CAST")
        val fields = JsonParser.parse(text) as? Map<String, Any?> ?: error("Ordinary CAS reply is not an object")
        val names = listOf("signed_result_original_base64", "data_record_original_base64", "authority_original_base64")
        require(fields.keys == names.toSet())
        val bounds = listOf(KagemushaOrdinaryIncomingFrameV1.SIGNED_MAXIMUM_BYTES,
            KagemushaOrdinaryIncomingFrameV1.DATA_MAXIMUM_BYTES, KagemushaOrdinaryIncomingFrameV1.AUTHORITY_MAXIMUM_BYTES)
        return names.zip(bounds).map { (name, maximum) ->
            val encoded = fields.getValue(name) as? String ?: error("Ordinary CAS original is not text")
            require(encoded.length <= ((maximum + 2) / 3) * 4)
            Base64.getDecoder().decode(encoded).also {
                require(it.size in 1..maximum && Base64.getEncoder().encodeToString(it) == encoded)
            }
        }
    }
}
