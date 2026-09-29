package org.hyperledger.iroha.sdk.client

import java.math.BigInteger

/** Public programmed RAM-FHE shape and initializer identity advertised by a policy. */
class RamFheProfile(
    @JvmField val profileVersion: Int,
    @JvmField val registerCount: Int,
    @JvmField val memoryLaneCount: Int,
    @JvmField val ciphertextMulPerStep: Int,
    @JvmField val encryptedInputMode: RamFheEncryptedInputMode,
    /** Unsigned 64-bit minimum, represented without signed-long truncation. */
    @JvmField val minCiphertextModulus: BigInteger,
    /** Exact lowercase Iroha hash; its final byte has the marker bit set. */
    @JvmField val initializerDescriptorHash: String,
) {
    init {
        require(profileVersion in 1..255) { "profileVersion must be a positive uint8" }
        require(registerCount in 1..65535) { "registerCount must be a positive uint16" }
        require(memoryLaneCount in 1..65535) { "memoryLaneCount must be a positive uint16" }
        require(ciphertextMulPerStep in 1..255) { "ciphertextMulPerStep must be a positive uint8" }
        require(minCiphertextModulus.signum() > 0 && minCiphertextModulus.bitLength() <= 64) {
            "minCiphertextModulus must be a positive uint64"
        }
        require(isCanonicalInitializerHash(initializerDescriptorHash)) {
            "initializerDescriptorHash must be 64 lowercase hex digits with the Iroha Hash marker bit set"
        }
    }
}

/** Sole encrypted-input representation admitted by the programmed RAM-FHE profile. */
enum class RamFheEncryptedInputMode(@JvmField val wireValue: String) {
    ENCRYPTED_ENVELOPE_V1("encrypted_envelope_v1"),
}

private fun isCanonicalInitializerHash(value: String): Boolean =
    value.length == 64 && value.all { it in '0'..'9' || it in 'a'..'f' } &&
        value.last() in "13579bdf"

/** Shared strict wire decoder for both RAM-LFE and identifier policy summaries. */
internal object RamFheProfileJsonParser {
    private val fields = setOf(
        "profile_version", "register_count", "memory_lane_count", "ciphertext_mul_per_step",
        "encrypted_input_mode", "min_ciphertext_modulus", "initializer_descriptor_hash",
    )

    fun parseOptional(value: Any?, path: String): RamFheProfile? {
        if (value == null) return null
        check(value is Map<*, *>) { "$path must be a JSON object" }
        check(value.keys.all { it in fields }) { "$path contains an unknown field" }
        val hash = value["initializer_descriptor_hash"]
        check(hash is String && isCanonicalInitializerHash(hash)) {
            "$path.initializer_descriptor_hash must be 64 lowercase hex digits with the Iroha Hash marker bit set"
        }
        check(value["encrypted_input_mode"] == RamFheEncryptedInputMode.ENCRYPTED_ENVELOPE_V1.wireValue) {
            "$path.encrypted_input_mode must be encrypted_envelope_v1"
        }
        return RamFheProfile(
            unsignedShape(value["profile_version"], 255, "$path.profile_version"),
            unsignedShape(value["register_count"], 65535, "$path.register_count"),
            unsignedShape(value["memory_lane_count"], 65535, "$path.memory_lane_count"),
            unsignedShape(value["ciphertext_mul_per_step"], 255, "$path.ciphertext_mul_per_step"),
            RamFheEncryptedInputMode.ENCRYPTED_ENVELOPE_V1,
            unsignedModulus(value["min_ciphertext_modulus"], "$path.min_ciphertext_modulus"),
            hash,
        )
    }

    private fun unsignedShape(value: Any?, maximum: Int, path: String): Int {
        val integer = JsonNumbers.asInt(value, path)
        check(integer in 1..maximum) { "$path must be an integer in 1..$maximum" }
        return integer
    }

    private fun unsignedModulus(value: Any?, path: String): BigInteger {
        val integer = when (value) {
            is BigInteger -> value
            is Byte, is Short, is Int, is Long -> BigInteger.valueOf((value as Number).toLong())
            else -> error("$path must be an unsigned integer JSON number")
        }
        check(integer.signum() > 0 && integer.bitLength() <= 64) {
            "$path must be a positive uint64"
        }
        return integer
    }
}
