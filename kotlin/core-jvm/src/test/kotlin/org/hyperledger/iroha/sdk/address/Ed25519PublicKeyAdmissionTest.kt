package org.hyperledger.iroha.sdk.address

import java.io.ByteArrayOutputStream
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.hyperledger.iroha.sdk.client.IdentifierJsonParser
import org.hyperledger.iroha.sdk.client.IdentifierNormalization
import org.hyperledger.iroha.sdk.client.IdentifierPolicySummary
import org.hyperledger.iroha.sdk.client.RamLfeJsonParser
import org.hyperledger.iroha.sdk.client.RamLfeProgramPolicySummary
import org.hyperledger.iroha.sdk.crypto.Ed25519PublicKeyAdmission
import org.hyperledger.iroha.sdk.tx.MultisigSignature

class Ed25519PublicKeyAdmissionTest {
    @Test
    fun `shared vectors enforce strict admission across all address routes`() {
        for (vector in loadVectors()) {
            assertEquals(vector.valid, Ed25519PublicKeyAdmission.isValid(vector.key), vector.name)

            val rawLiteral = rawEd25519Literal(vector.key)
            val rawCompact = byteArrayOf(0) + vector.key
            val decodedLiteral = decodePublicKeyLiteral(rawLiteral)
            val decodedCompact = decodeCompactPublicKeyPayload(rawCompact)

            if (vector.valid) {
                assertEquals(rawLiteral, encodePublicKeyMultihash(0x01, vector.key), vector.name)
                assertContentEquals(rawCompact, compactPublicKeyPayload(0x01, vector.key), vector.name)
                assertNotNull(decodePublicKeyLiteral("ed25519:$rawLiteral"), vector.name)
                assertNull(decodePublicKeyLiteral("garbage:$rawLiteral"), vector.name)
                assertNull(decodePublicKeyLiteral("secp256k1:$rawLiteral"), vector.name)
                assertNull(decodePublicKeyLiteral("ed25519:ed25519:$rawLiteral"), vector.name)
                assertNull(decodePublicKeyLiteral("ED25519:$rawLiteral"), vector.name)
                assertNull(decodePublicKeyLiteral(" $rawLiteral"), vector.name)
                assertNull(decodePublicKeyLiteral("$rawLiteral "), vector.name)
                assertNull(decodePublicKeyLiteral("\u00A0$rawLiteral"), vector.name)
                assertContentEquals(
                    rawCompact,
                    MultisigSignature.fromCurveId(0x01, vector.key, ByteArray(64) { 1 })
                        .publicKeyNoritoPayload(),
                    vector.name,
                )
                assertContentEquals(vector.key, assertNotNull(decodedLiteral, vector.name).keyBytes)
                assertContentEquals(vector.key, assertNotNull(decodedCompact, vector.name).keyBytes)
                assertContentEquals(
                    vector.canonical,
                    AccountAddress.fromAccount(vector.key, "ed25519").canonicalBytes,
                    vector.name,
                )
                assertContentEquals(
                    vector.canonical,
                    AccountAddress.fromCanonicalBytes(vector.canonical).canonicalBytes,
                    vector.name,
                )
                assertContentEquals(
                    vector.canonical,
                    AccountAddress.fromI105(vector.i105, AccountAddress.DEFAULT_I105_DISCRIMINANT).canonicalBytes,
                    vector.name,
                )
                AccountAddress.parseEncodedIgnoringCurveSupport(
                    vector.i105,
                    AccountAddress.DEFAULT_I105_DISCRIMINANT,
                )
                AccountAddress.fromMultisigPolicy(multisigPolicy(vector.key))
                AccountAddress.fromCanonicalBytes(multisigCanonical(vector.key))
                identifierPolicy(rawLiteral)
                ramLfePolicy(rawLiteral)
                IdentifierJsonParser.parsePolicyList(identifierPolicyJson(rawLiteral))
                RamLfeJsonParser.parsePolicyList(ramLfePolicyJson(rawLiteral))
            } else {
                assertFailsWith<IllegalArgumentException>(vector.name) {
                    encodePublicKeyMultihash(0x01, vector.key)
                }
                assertFailsWith<IllegalArgumentException>(vector.name) {
                    compactPublicKeyPayload(0x01, vector.key)
                }
                assertFailsWith<IllegalArgumentException>(vector.name) {
                    MultisigSignature.fromCurveId(0x01, vector.key, ByteArray(64) { 1 })
                }
                assertNull(decodedLiteral, vector.name)
                assertNull(decodedCompact, vector.name)
                assertInvalidPublicKey(vector.name) {
                    AccountAddress.fromAccount(vector.key, "ed25519")
                }
                assertInvalidPublicKey(vector.name) {
                    AccountAddress.fromCanonicalBytes(vector.canonical)
                }
                assertInvalidPublicKey(vector.name) {
                    AccountAddress.fromI105(vector.i105, AccountAddress.DEFAULT_I105_DISCRIMINANT)
                }
                assertInvalidPublicKey(vector.name) {
                    AccountAddress.parseEncodedIgnoringCurveSupport(
                        vector.i105,
                        AccountAddress.DEFAULT_I105_DISCRIMINANT,
                    )
                }
                assertInvalidPublicKey(vector.name) {
                    AccountAddress.fromMultisigPolicy(multisigPolicy(vector.key))
                }
                assertInvalidPublicKey(vector.name) {
                    AccountAddress.fromCanonicalBytes(multisigCanonical(vector.key))
                }
                assertFailsWith<IllegalArgumentException>(vector.name) {
                    identifierPolicy(rawLiteral)
                }
                assertFailsWith<IllegalArgumentException>(vector.name) {
                    ramLfePolicy(rawLiteral)
                }
                assertFailsWith<IllegalStateException>(vector.name) {
                    IdentifierJsonParser.parsePolicyList(identifierPolicyJson(rawLiteral))
                }
                assertFailsWith<IllegalStateException>(vector.name) {
                    RamLfeJsonParser.parsePolicyList(ramLfePolicyJson(rawLiteral))
                }
            }
        }

        val secpLiteral = encodePublicKeyMultihash(0x04, ByteArray(33) { (it + 1).toByte() })
        assertEquals(0x04, assertNotNull(decodePublicKeyLiteral("secp256k1:$secpLiteral")).curveId)
        assertNull(decodePublicKeyLiteral("ed25519:$secpLiteral"))
    }

    @Test
    fun `policy routes independently reject an invalid output-opening key`() {
        val vectors = loadVectors()
        val validLiteral = rawEd25519Literal(vectors.first { it.valid }.key)
        val invalidLiteral = rawEd25519Literal(vectors.first { !it.valid }.key)

        assertEquals(validLiteral, identifierPolicy(validLiteral).outputOpeningPublicKey)
        assertEquals(validLiteral, ramLfePolicy(validLiteral).outputOpeningPublicKey)
        assertEquals(
            validLiteral,
            IdentifierJsonParser.parsePolicyList(identifierPolicyJson(validLiteral)).items.single().outputOpeningPublicKey,
        )
        assertEquals(
            validLiteral,
            RamLfeJsonParser.parsePolicyList(ramLfePolicyJson(validLiteral)).items.single().outputOpeningPublicKey,
        )
        assertFailsWith<IllegalArgumentException> {
            identifierPolicy(validLiteral, invalidLiteral)
        }
        assertFailsWith<IllegalArgumentException> {
            ramLfePolicy(validLiteral, invalidLiteral)
        }
        assertFailsWith<IllegalStateException> {
            IdentifierJsonParser.parsePolicyList(identifierPolicyJson(validLiteral, invalidLiteral))
        }
        assertFailsWith<IllegalStateException> {
            RamLfeJsonParser.parsePolicyList(ramLfePolicyJson(validLiteral, invalidLiteral))
        }
        for (invalidJsonValue in listOf("null", "true")) {
            val identifierError = assertFailsWith<IllegalStateException> {
                IdentifierJsonParser.parsePolicyList(
                    replaceOutputOpeningValue(
                        identifierPolicyJson(validLiteral),
                        validLiteral,
                        invalidJsonValue,
                    ),
                )
            }
            assertTrue(identifierError.message?.contains("output_opening_public_key") == true)

            val ramLfeError = assertFailsWith<IllegalStateException> {
                RamLfeJsonParser.parsePolicyList(
                    replaceOutputOpeningValue(
                        ramLfePolicyJson(validLiteral),
                        validLiteral,
                        invalidJsonValue,
                    ),
                )
            }
            assertTrue(ramLfeError.message?.contains("output_opening_public_key") == true)
        }
    }

    private fun assertInvalidPublicKey(name: String, action: () -> Unit) {
        val error = assertFailsWith<AccountAddressException>(name, action)
        assertEquals(AccountAddressErrorCode.INVALID_PUBLIC_KEY, error.code, name)
    }

    private fun multisigPolicy(publicKey: ByteArray): MultisigPolicyPayload =
        MultisigPolicyPayload.of(
            version = 1,
            threshold = 1,
            members = listOf(MultisigMemberPayload(curveId = 0x01, weight = 1, publicKey = publicKey)),
        )

    private fun multisigCanonical(publicKey: ByteArray): ByteArray = ByteArrayOutputStream().run {
        write(0x0A)
        write(0x01)
        write(0x01)
        write(0x00)
        write(0x01)
        write(0x00)
        write(0x01)
        write(0x01)
        write(0x00)
        write(0x01)
        write((publicKey.size ushr 8) and 0xFF)
        write(publicKey.size and 0xFF)
        write(publicKey)
        toByteArray()
    }

    private fun identifierPolicy(
        publicKeyLiteral: String,
        outputOpeningPublicKeyLiteral: String = publicKeyLiteral,
    ): IdentifierPolicySummary =
        IdentifierPolicySummary(
            policyId = "key-admission#fixture",
            owner = "owner",
            active = true,
            normalization = IdentifierNormalization.EXACT,
            resolverPublicKey = publicKeyLiteral,
            backend = "signed",
            inputEncryption = null,
            inputEncryptionPublicParameters = null,
            inputEncryptionPublicParametersDecoded = null,
            note = null,
            outputOpeningPublicKey = outputOpeningPublicKeyLiteral,
        )

    private fun ramLfePolicy(
        publicKeyLiteral: String,
        outputOpeningPublicKeyLiteral: String = publicKeyLiteral,
    ): RamLfeProgramPolicySummary =
        RamLfeProgramPolicySummary(
            programId = "key_admission_fixture",
            owner = "owner",
            active = true,
            resolverPublicKey = publicKeyLiteral,
            backend = "signed",
            verificationMode = "signed",
            inputEncryption = null,
            inputEncryptionPublicParameters = null,
            inputEncryptionPublicParametersDecoded = null,
            note = null,
            outputOpeningPublicKey = outputOpeningPublicKeyLiteral,
        )

    private fun identifierPolicyJson(
        publicKeyLiteral: String,
        outputOpeningPublicKeyLiteral: String = publicKeyLiteral,
    ): ByteArray =
        """{"items":[{"policy_id":"key-admission#fixture","owner":"owner","active":true,"normalization":"exact","resolver_public_key":"$publicKeyLiteral","output_opening_public_key":"$outputOpeningPublicKeyLiteral","backend":"signed"}]}"""
            .toByteArray(StandardCharsets.UTF_8)

    private fun ramLfePolicyJson(
        publicKeyLiteral: String,
        outputOpeningPublicKeyLiteral: String = publicKeyLiteral,
    ): ByteArray =
        """{"items":[{"program_id":"key_admission_fixture","owner":"owner","active":true,"resolver_public_key":"$publicKeyLiteral","output_opening_public_key":"$outputOpeningPublicKeyLiteral","backend":"signed","verification_mode":"signed"}]}"""
            .toByteArray(StandardCharsets.UTF_8)

    private fun replaceOutputOpeningValue(
        payload: ByteArray,
        publicKeyLiteral: String,
        jsonValue: String,
    ): ByteArray =
        String(payload, StandardCharsets.UTF_8)
            .replace(
                "\"output_opening_public_key\":\"$publicKeyLiteral\"",
                "\"output_opening_public_key\":$jsonValue",
            )
            .toByteArray(StandardCharsets.UTF_8)

    private fun loadVectors(): List<AdmissionVector> {
        val root = Json.parseToJsonElement(
            String(Files.readAllBytes(resolveFixturePath()), StandardCharsets.UTF_8),
        ).jsonObject
        assertEquals(1, root.getValue("schema_version").jsonPrimitive.content.toInt())
        return root.getValue("vectors").jsonArray.map { element ->
            val vector = element.jsonObject
            AdmissionVector(
                name = vector.getValue("name").jsonPrimitive.content,
                valid = vector.getValue("valid").jsonPrimitive.boolean,
                key = hexToBytes(vector.getValue("key_hex").jsonPrimitive.content),
                canonical = hexToBytes(vector.getValue("single_canonical_hex").jsonPrimitive.content),
                i105 = vector.getValue("single_i105").jsonPrimitive.content,
            )
        }
    }

    private fun resolveFixturePath(): Path {
        var candidate = Paths.get(FIXTURE_PATH)
        repeat(6) {
            if (Files.exists(candidate)) return candidate.normalize()
            candidate = Paths.get("..").resolve(candidate)
        }
        error("unable to locate $FIXTURE_PATH")
    }

    private fun hexToBytes(hex: String): ByteArray {
        require(hex.length % 2 == 0)
        return ByteArray(hex.length / 2) { index ->
            hex.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun rawEd25519Literal(key: ByteArray): String = buildString {
        append("ed01")
        append(key.size.toString(16).padStart(2, '0'))
        key.forEach { value -> append("%02X".format(value.toInt() and 0xFF)) }
    }

    private data class AdmissionVector(
        val name: String,
        val valid: Boolean,
        val key: ByteArray,
        val canonical: ByteArray,
        val i105: String,
    )

    private companion object {
        const val FIXTURE_PATH = "fixtures/crypto/ed25519_public_key_admission_v1.json"
    }
}
