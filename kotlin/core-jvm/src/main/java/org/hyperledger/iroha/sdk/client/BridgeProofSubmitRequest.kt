package org.hyperledger.iroha.sdk.client

import java.util.Base64
import org.hyperledger.iroha.sdk.norito.NoritoHeader

/** Request payload for `POST /v1/bridge/proofs/submit`. */
class BridgeProofSubmitRequest(
    authority: String,
    messageBundleB64: String,
    publicKeyHex: String? = null,
    signatureB64: String? = null,
    networkIdHex: String? = null,
    verifierAddressHex: String? = null,
    bridgeAddressHex: String? = null,
    verifierCodeHashHex: String? = null,
    verifierKeyHashHex: String? = null,
    tronVerifierAddress: String? = null,
    proofBytesHex: String? = null,
    creationTimeMs: Long? = null,
) {
    val authority: String = requireNonBlank(authority, "authority")
    val publicKeyHex: String? = normalizeOptionalPublicKeyHex(publicKeyHex)
    val signatureB64: String? = normalizeOptionalExactBase64(signatureB64, "signatureB64")
    val messageBundleB64: String = messageBundleB64.also {
        validateCanonicalSccpNoritoBase64(it, "messageBundleB64", SCCP_MAX_MESSAGE_BUNDLE_BYTES)
    }
    val networkIdHex: String? = normalizeOptionalHex(networkIdHex, 32, "networkIdHex")
    val verifierAddressHex: String? = normalizeOptionalHex(verifierAddressHex, 20, "verifierAddressHex")
    val bridgeAddressHex: String? = normalizeOptionalHex(bridgeAddressHex, 20, "bridgeAddressHex")
    val verifierCodeHashHex: String? = normalizeOptionalHex(verifierCodeHashHex, 32, "verifierCodeHashHex")
    val verifierKeyHashHex: String? = normalizeOptionalHex(verifierKeyHashHex, 32, "verifierKeyHashHex")
    val tronVerifierAddress: String? = normalizeOptional(tronVerifierAddress)
    val proofBytesHex: String? = normalizeOptional(proofBytesHex)
    val creationTimeMs: Long? = creationTimeMs?.also {
        require(it > 0) { "creationTimeMs must be positive" }
    }

    init {
        require((this.publicKeyHex == null) == (this.signatureB64 == null)) {
            "publicKeyHex and signatureB64 must be supplied together"
        }
        val destinationPresent = hasDestinationMaterial()
        require((this.proofBytesHex == null) == !destinationPresent) {
            "proofBytesHex and complete destination material must be supplied together"
        }
        if (destinationPresent) requireCompleteDestinationTuple()
    }

    fun toJsonMap(): Map<String, Any?> = buildMap {
        put("authority", authority)
        publicKeyHex?.let { put("public_key_hex", it) }
        signatureB64?.let { put("signature_b64", it) }
        put("message_bundle_b64", messageBundleB64)
        networkIdHex?.let { put("network_id_hex", it) }
        verifierAddressHex?.let { put("verifier_address_hex", it) }
        bridgeAddressHex?.let { put("bridge_address_hex", it) }
        verifierCodeHashHex?.let { put("verifier_code_hash_hex", it) }
        verifierKeyHashHex?.let { put("verifier_key_hash_hex", it) }
        tronVerifierAddress?.let { put("tron_verifier_address", it) }
        proofBytesHex?.let { put("proof_bytes_hex", it) }
        creationTimeMs?.let { put("creation_time_ms", it) }
    }

    fun toJsonBytes(): ByteArray = JsonEncoder.encode(toJsonMap()).toByteArray(Charsets.UTF_8)

    private fun hasDestinationMaterial(): Boolean =
        networkIdHex != null || verifierAddressHex != null || bridgeAddressHex != null ||
            verifierCodeHashHex != null || verifierKeyHashHex != null || tronVerifierAddress != null

    private fun requireCompleteDestinationTuple() {
        val evm = verifierAddressHex != null || bridgeAddressHex != null
        val tron = tronVerifierAddress != null
        require(evm != tron) { "destination material must select exactly one EVM or TRON family" }
        val commonPresent = networkIdHex != null && verifierCodeHashHex != null && verifierKeyHashHex != null
        require(commonPresent) { "complete SCCP destination material is required" }
        if (evm) {
            require(verifierAddressHex != null && bridgeAddressHex != null) {
                "complete EVM SCCP destination material is required"
            }
        } else {
            require(!tronVerifierAddress.isNullOrBlank()) { "complete TRON SCCP destination material is required" }
        }
    }
}

/** Native-proof-only settlement request for `POST /v1/bridge/messages`. */
class BridgeMessageSubmitRequest(
    authority: String,
    nativeProofB64: String,
    publicKeyHex: String? = null,
    signatureB64: String? = null,
    creationTimeMs: Long? = null,
) {
    val authority: String = requireNonBlank(authority, "authority")
    val publicKeyHex: String? = normalizeOptionalPublicKeyHex(publicKeyHex)
    val signatureB64: String? = normalizeOptionalExactBase64(signatureB64, "signatureB64")
    val nativeProofB64: String = nativeProofB64.also {
        validateCanonicalSccpNoritoBase64(it, "nativeProofB64", SCCP_MAX_NATIVE_PROOF_BYTES)
    }
    val creationTimeMs: Long? = creationTimeMs?.also {
        require(it > 0) { "creationTimeMs must be positive" }
    }

    init {
        require((this.publicKeyHex == null) == (this.signatureB64 == null)) {
            "publicKeyHex and signatureB64 must be supplied together"
        }
    }

    fun toJsonMap(): Map<String, Any?> = buildMap {
        put("authority", authority)
        publicKeyHex?.let { put("public_key_hex", it) }
        signatureB64?.let { put("signature_b64", it) }
        put("native_proof_b64", nativeProofB64)
        creationTimeMs?.let { put("creation_time_ms", it) }
    }

    fun toJsonBytes(): ByteArray = JsonEncoder.encode(toJsonMap()).toByteArray(Charsets.UTF_8)
}

private const val SCCP_MAX_MESSAGE_BUNDLE_BYTES = 16 * 1024 * 1024
private const val SCCP_MAX_NATIVE_PROOF_BYTES = 16 * 1024 * 1024

internal fun validateCanonicalSccpNoritoBase64(value: String, field: String, maximum: Int): ByteArray {
    require(value.isNotEmpty() && value == value.trim()) { "$field must be canonical padded base64" }
    val decoded = try {
        Base64.getDecoder().decode(value)
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("$field must be valid base64", ex)
    }
    require(decoded.isNotEmpty() && decoded.size <= maximum) { "$field exceeds its canonical size bound" }
    require(Base64.getEncoder().encodeToString(decoded) == value) { "$field must be canonical padded base64" }
    val result = try {
        NoritoHeader.decode(decoded, null)
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("$field must contain a canonical Norito envelope", ex)
    }
    val header = result.header
    require(header.compression == NoritoHeader.COMPRESSION_NONE) { "$field must use uncompressed canonical Norito" }
    val headerPadding = decoded.size - NoritoHeader.HEADER_LENGTH - header.payloadLength
    require(headerPadding == 0 || headerPadding == 8) {
        "$field must use canonical Norito header alignment padding"
    }
    require(header.schemaHash.any { it.toInt() != 0 }) { "$field must advertise a nonzero Norito schema" }
    require(header.encode().contentEquals(decoded.copyOfRange(0, NoritoHeader.HEADER_LENGTH))) {
        "$field contains a non-canonical Norito header"
    }
    header.validateChecksum(result.payload)
    return decoded
}

private fun requireNonBlank(value: String, field: String): String {
    require(value.isNotBlank() && value == value.trim()) { "$field is required and must be canonical" }
    return value
}

private fun normalizeOptional(value: String?): String? {
    if (value == null) return null
    require(value == value.trim()) { "optional string fields must not contain surrounding whitespace" }
    return value.ifEmpty { null }
}

private fun normalizeOptionalHex(value: String?, bytes: Int, field: String): String? {
    val normalized = normalizeOptional(value) ?: return null
    require(normalized.startsWith("0x") && normalized.length == 2 + bytes * 2) {
        "$field must be canonical lowercase 0x-prefixed $bytes-byte hex"
    }
    require(normalized.substring(2).all { it in '0'..'9' || it in 'a'..'f' }) {
        "$field must be canonical lowercase 0x-prefixed $bytes-byte hex"
    }
    require(normalized.substring(2).any { it != '0' }) { "$field must be nonzero" }
    return normalized
}

private fun normalizeOptionalPublicKeyHex(value: String?): String? {
    val normalized = normalizeOptional(value) ?: return null
    require(Regex("[0-9a-f]{64}").matches(normalized) && normalized.any { it != '0' }) {
        "publicKeyHex must be exactly 32 nonzero lowercase hexadecimal bytes"
    }
    return normalized
}

private fun normalizeOptionalExactBase64(value: String?, field: String): String? {
    if (value == null) return null
    require(value.isNotEmpty() && value == value.trim()) { "$field must be exact standard-base64" }
    val decoded = try {
        Base64.getDecoder().decode(value)
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("$field must be valid base64", ex)
    }
    require(decoded.isNotEmpty()) { "$field must not decode to empty bytes" }
    if (field == "signatureB64") require(decoded.size == 64) { "$field must contain a 64-byte Ed25519 signature" }
    require(Base64.getEncoder().encodeToString(decoded) == value) { "$field must be exact standard-base64" }
    return value
}
