package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull

/** Both policy endpoints must preserve the same exact initializer and bounded profile. */
class RamFheProfileTest {
    @Test
    fun bothPolicyListsPreserveEveryProfileField() {
        for (identifier in listOf(false, true)) {
            val profile = assertNotNull(parse(identifier, profileJson()))
            assertEquals(1, profile.profileVersion)
            assertEquals(4, profile.registerCount)
            assertEquals(32, profile.memoryLaneCount)
            assertEquals(16, profile.ciphertextMulPerStep)
            assertEquals(RamFheEncryptedInputMode.ENCRYPTED_ENVELOPE_V1, profile.encryptedInputMode)
            assertEquals("encrypted_envelope_v1", profile.encryptedInputMode.wireValue)
            assertEquals(BigInteger("4503599627370496"), profile.minCiphertextModulus)
            assertEquals(HASH, profile.initializerDescriptorHash)
        }
    }

    @Test
    fun optionalProfileRemainsAbsentButPresentObjectHasNoDefaults() {
        for (identifier in listOf(false, true)) {
            assertNull(parse(identifier, null))
            assertNull(parse(identifier, "null"))
            for (field in fields.keys) {
                assertRejected(identifier, profileJson(omit = field), field)
                assertRejected(identifier, profileJson(field to "null"), field)
            }
            for (malformed in listOf("[]", "false", "1", "\"profile\"")) {
                assertRejected(identifier, malformed, "ram_fhe_profile")
            }
            assertRejected(identifier, profileJson("legacy_initializer" to "1"), "unknown field")
        }
    }

    @Test
    fun initializerRequiresExactCanonicalMarkedHash() {
        val malformed = listOf(
            "\"${HASH.uppercase()}\"", "\"0x$HASH\"", "\"hash:$HASH\"", "\"$HASH#\"",
            "\" $HASH\"", "\"$HASH \"", "\"${HASH.dropLast(1)}\"", "\"${HASH}1\"",
            "\"${"0".repeat(63)}2\"", "\"${"0".repeat(63)}g\"", "7", "false",
        )
        for (identifier in listOf(false, true)) {
            for (value in malformed) {
                assertRejected(identifier, profileJson("initializer_descriptor_hash" to value), "initializer_descriptor_hash")
            }
        }
    }

    @Test
    fun modeRejectsRetiredAliasesTaggedObjectsAndCoercions() {
        val malformed = listOf(
            "\"EncryptedEnvelopeV1\"", "\"encryptedEnvelopeV1\"", "\"encrypted_envelope_v1 \"",
            "\"resolver_canonicalized\"", "{\"mode\":\"encrypted_envelope_v1\"}", "1", "false",
        )
        for (identifier in listOf(false, true)) {
            for (value in malformed) {
                assertRejected(identifier, profileJson("encrypted_input_mode" to value), "encrypted_input_mode")
            }
        }
    }

    @Test
    fun shapeBoundsRejectOverflowZeroSignedAndNonIntegerValues() {
        val limits = mapOf("profile_version" to 255, "register_count" to 65535,
            "memory_lane_count" to 65535, "ciphertext_mul_per_step" to 255)
        for (identifier in listOf(false, true)) {
            for ((field, maximum) in limits) {
                for (invalid in listOf("0", "-0", "-1", "${maximum + 1}", "18446744073709551616",
                    "1.0", "1e0", "\"1\"", "true", "[]")) {
                    assertRejected(identifier, profileJson(field to invalid), field)
                }
                assertNotNull(parse(identifier, profileJson(field to "1")))
                assertNotNull(parse(identifier, profileJson(field to maximum.toString())))
            }
        }
    }

    @Test
    fun fullUnsigned64ModulusIsPreservedWithoutTruncationOrStringFallback() {
        for (identifier in listOf(false, true)) {
            for (valid in listOf("1", "9223372036854775808", "18446744073709551615")) {
                assertEquals(BigInteger(valid), assertNotNull(parse(identifier,
                    profileJson("min_ciphertext_modulus" to valid))).minCiphertextModulus)
            }
            for (invalid in listOf("0", "-0", "-1", "18446744073709551616", "1.0", "1e0",
                "\"4503599627370496\"", "true", "[]")) {
                assertRejected(identifier, profileJson("min_ciphertext_modulus" to invalid), "min_ciphertext_modulus")
            }
        }
    }

    @Test
    fun directConstructionEnforcesTheSameShapeAndIdentity() {
        assertEquals(BigInteger.ONE, makeProfile().minCiphertextModulus)
        for (invalid in listOf(-1, 0, 256)) {
            assertFailsWith<IllegalArgumentException> { makeProfile(version = invalid) }
            assertFailsWith<IllegalArgumentException> { makeProfile(multiplications = invalid) }
        }
        for (invalid in listOf(-1, 0, 65536)) {
            assertFailsWith<IllegalArgumentException> { makeProfile(registers = invalid) }
            assertFailsWith<IllegalArgumentException> { makeProfile(memory = invalid) }
        }
        for (invalid in listOf(BigInteger.valueOf(-1), BigInteger.ZERO, BigInteger.ONE.shiftLeft(64))) {
            assertFailsWith<IllegalArgumentException> { makeProfile(modulus = invalid) }
        }
        assertFailsWith<IllegalArgumentException> { makeProfile(hash = HASH.uppercase()) }
        assertFailsWith<IllegalArgumentException> { makeProfile(hash = "0".repeat(64)) }
    }

    private fun assertRejected(identifier: Boolean, profile: String, field: String) {
        val error = assertFailsWith<IllegalStateException> { parse(identifier, profile) }
        assertContains(error.message.orEmpty(), field)
        assertContains(error.message.orEmpty(), "ram_fhe_profile")
    }

    private fun parse(identifier: Boolean, profile: String?): RamFheProfile? {
        val optional = if (profile == null) "" else ",\"ram_fhe_profile\":$profile"
        val payload = """{"items":[{"policy_id":"retail","program_id":"lookup","owner":"owner",
            "active":true,"normalization":"exact","resolver_public_key":"$KEY",
            "output_opening_public_key":"$KEY","backend":"bfv-programmed-sha3-256-v1",
            "verification_mode":"signed"$optional}]}""".toByteArray(Charsets.UTF_8)
        return if (identifier) IdentifierJsonParser.parsePolicyList(payload).items.single().ramFheProfile
        else RamLfeJsonParser.parsePolicyList(payload).items.single().ramFheProfile
    }

    private fun profileJson(replace: Pair<String, String>? = null, omit: String? = null): String {
        val values = LinkedHashMap(fields)
        if (replace != null) values[replace.first] = replace.second
        if (omit != null) values.remove(omit)
        return values.entries.joinToString(prefix = "{", postfix = "}") { "\"${it.key}\":${it.value}" }
    }

    private fun makeProfile(version: Int = 1, registers: Int = 4, memory: Int = 32,
        multiplications: Int = 16, modulus: BigInteger = BigInteger.ONE, hash: String = HASH) =
        RamFheProfile(version, registers, memory, multiplications,
            RamFheEncryptedInputMode.ENCRYPTED_ENVELOPE_V1, modulus, hash)

    companion object {
        private const val KEY = "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29"
        private val HASH = "ab".repeat(32)
        private val fields = linkedMapOf(
            "profile_version" to "1", "register_count" to "4", "memory_lane_count" to "32",
            "ciphertext_mul_per_step" to "16", "encrypted_input_mode" to "\"encrypted_envelope_v1\"",
            "min_ciphertext_modulus" to "4503599627370496", "initializer_descriptor_hash" to "\"$HASH\"",
        )
    }
}
