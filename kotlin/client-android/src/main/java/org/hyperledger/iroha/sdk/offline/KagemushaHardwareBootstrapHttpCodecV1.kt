// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

/** Sole protected hardware-evidence JSON boundary; canonical issuer archives remain Native input. */
internal object KagemushaHardwareBootstrapHttpCodecV1 {
    const val MAX_ORIGINAL = 192 * 1024
    fun requireOrigin(origin: String) {
        val uri = URI(origin)
        require(origin.length <= 2048 && uri.scheme == "https" && uri.host != null
            && uri.rawUserInfo == null && uri.rawQuery == null && uri.rawFragment == null
            && uri.rawPath.isEmpty() && (uri.port == -1 || uri.port in 1..65535)
            && uri.toASCIIString() == origin && origin.none { it.isWhitespace() || it.isISOControl() })
    }
    private fun original(value: ByteArray): String {
        require(value.isNotEmpty() && value.size <= MAX_ORIGINAL)
        return Base64.getEncoder().encodeToString(value)
    }
    fun prepare(reservation: ByteArray, token: ByteArray): ByteArray {
        require(token.isNotEmpty() && token.size <= 64 * 1024 && token.all { it.toInt() in 33..126 })
        return JsonEncoder.encode(mapOf("reservation_original_base64" to original(reservation),
            "google_id_token" to token.toString(Charsets.US_ASCII))).toByteArray(Charsets.UTF_8)
    }
    fun raw(fields: Array<ByteArray>): ByteArray {
        require(fields.size == 2)
        return JsonEncoder.encode(mapOf("signed_challenge_original_base64" to original(fields[0]),
            "platform_attestation_original_base64" to original(fields[1]))).toByteArray(Charsets.UTF_8)
    }
    fun finish(fields: Array<ByteArray>): ByteArray {
        require(fields.size == 6 && fields[5].isNotEmpty() && fields[5].size <= 64 * 1024
            && fields[5].all { it.toInt() in 33..126 })
        val names = listOf("signed_challenge_original_base64", "platform_attestation_original_base64",
            "signed_raw_admission_original_base64", "possession_message_original_base64", "possession_signature_der_base64")
        val values = names.indices.associate { names[it] to original(fields[it]) }.toMutableMap()
        values["play_integrity_token"] = fields[5].toString(Charsets.US_ASCII)
        return JsonEncoder.encode(values).toByteArray(Charsets.UTF_8)
    }
    fun response(stage: KagemushaHardwareBootstrapHttpOriginalV1.Stage, bytes: ByteArray): ByteArray {
        require(bytes.isNotEmpty() && bytes.size <= ((MAX_ORIGINAL + 2) / 3) * 4 + 128)
        val text = Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes)).toString()
        val parsed = JsonParser.parse(text) as? Map<*, *> ?: error("Issuer reply must be an exact object")
        val key = when(stage) {
            KagemushaHardwareBootstrapHttpOriginalV1.Stage.PREPARE -> "signed_challenge_original_base64"
            KagemushaHardwareBootstrapHttpOriginalV1.Stage.RAW_ATTESTATION -> "signed_raw_admission_original_base64"
            KagemushaHardwareBootstrapHttpOriginalV1.Stage.RECEIPT -> "signed_hardware_receipt_original_base64"
        }
        require(parsed.keys == setOf(key)) { "Issuer reply purpose/fields differ" }
        val encoded = parsed[key] as? String ?: error("Issuer original must be base64")
        require(encoded.isNotEmpty() && encoded.length <= ((MAX_ORIGINAL+2)/3)*4)
        val original = Base64.getDecoder().decode(encoded)
        require(original.isNotEmpty() && original.size <= MAX_ORIGINAL
            && Base64.getEncoder().encodeToString(original) == encoded) { "Noncanonical issuer original" }
        return original
    }
}
