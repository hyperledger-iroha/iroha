package org.hyperledger.iroha.sdk.address

import java.io.ByteArrayOutputStream
import org.hyperledger.iroha.sdk.crypto.MlDsaPublicKey
import org.hyperledger.iroha.sdk.crypto.MlDsaPublicKeyAdmission
import org.hyperledger.iroha.sdk.norito.Varint
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.tx.MultisigSignature
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MlDsa65KeyShapeTest {
    @Test
    fun sharedValidatorAndRawKeyWrapperEnforceProtocolShape() {
        val valid = validMlDsa65Key()
        assertTrue(MlDsaPublicKeyAdmission.isValid(valid))
        assertContentEquals(valid, MlDsaPublicKey(valid).encoded)

        for ((name, invalid) in invalidMlDsa65Keys()) {
            assertFalse(MlDsaPublicKeyAdmission.isValid(invalid), name)
            assertFailsWith<IllegalArgumentException>(name) {
                MlDsaPublicKey(invalid)
            }
        }
    }

    @Test
    fun accountAddressUsesCanonicalExtendedSingleKeyWireAndRejectsMalformedKeys() {
        withMlDsaSupport {
            val valid = validMlDsa65Key()
            val address = AccountAddress.fromAccount(valid, "ml-dsa")
            val canonical = address.canonicalBytes

            assertEquals(0x02, canonical[0].toInt() and 0xFF)
            assertEquals(0x02, canonical[1].toInt() and 0xFF)
            assertEquals(0x02, canonical[2].toInt() and 0xFF)
            assertEquals(valid.size, readU16Be(canonical, 3))
            assertContentEquals(valid, canonical.copyOfRange(5, canonical.size))

            val fromCanonical = AccountAddress.fromCanonicalBytes(canonical)
            assertContentEquals(canonical, fromCanonical.canonicalBytes)
            assertContentEquals(valid, assertNotNull(fromCanonical.singleKeyPayload()).publicKey)

            val i105 = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
            val fromI105 = AccountAddress.fromI105(i105, AccountAddress.DEFAULT_I105_DISCRIMINANT)
            assertContentEquals(canonical, fromI105.canonicalBytes)
            assertEquals(i105, fromI105.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT))

            val validPolicy = MultisigPolicyPayload.of(
                1,
                1,
                listOf(MultisigMemberPayload(0x02, 1, valid)),
            )
            val multisigAddress = AccountAddress.fromMultisigPolicy(validPolicy)
            assertEquals(0x0A, multisigAddress.canonicalBytes[0].toInt() and 0xFF)
            val decodedPolicy = assertNotNull(multisigAddress.multisigPolicyPayload())
            assertContentEquals(valid, decodedPolicy.members.single().publicKey)
            assertContentEquals(
                multisigAddress.canonicalBytes,
                AccountAddress.fromCanonicalBytes(multisigAddress.canonicalBytes).canonicalBytes,
            )

            for ((name, invalid) in invalidMlDsa65Keys()) {
                val constructionError = assertFailsWith<AccountAddressException>(name) {
                    AccountAddress.fromAccount(invalid, "ml-dsa")
                }
                assertEquals(AccountAddressErrorCode.INVALID_PUBLIC_KEY, constructionError.code, name)

                assertFailsWith<AccountAddressException>(name) {
                    AccountAddress.fromCanonicalBytes(singleKeyCanonical(invalid))
                }
                assertFailsWith<AccountAddressException>(name) {
                    AccountAddress.fromMultisigPolicy(
                        MultisigPolicyPayload.of(
                            1,
                            1,
                            listOf(MultisigMemberPayload(0x02, 1, invalid)),
                        ),
                    )
                }
                assertFailsWith<AccountAddressException>(name) {
                    AccountAddress.fromCanonicalBytes(multisigCanonical(invalid))
                }
            }

            val ed25519 = TestEd25519Keys.publicKey(0x42)
            assertEquals(
                0x00,
                AccountAddress.fromAccount(ed25519, "ed25519").canonicalBytes[1].toInt() and 0xFF,
            )
            val nonCanonicalExtended = singleKeyCanonical(ed25519, curveId = 0x01, forceExtended = true)
            val extendedError = assertFailsWith<AccountAddressException> {
                AccountAddress.fromCanonicalBytes(nonCanonicalExtended)
            }
            assertEquals(AccountAddressErrorCode.INVALID_LENGTH, extendedError.code)
        }
    }

    @Test
    fun canonicalDecodeRejectsHeaderControllerClassMismatches() {
        withMlDsaSupport {
            val ed25519Single = AccountAddress
                .fromAccount(TestEd25519Keys.publicKey(0x43), "ed25519")
                .canonicalBytes
            val mlDsaExtended = AccountAddress
                .fromAccount(validMlDsa65Key(), "ml-dsa")
                .canonicalBytes
            val multisig = AccountAddress
                .fromMultisigPolicy(
                    MultisigPolicyPayload.of(
                        1,
                        1,
                        listOf(MultisigMemberPayload(0x02, 1, validMlDsa65Key())),
                    ),
                )
                .canonicalBytes

            assertEquals(0x02, ed25519Single[0].toInt() and 0xFF)
            assertEquals(0x02, mlDsaExtended[0].toInt() and 0xFF)
            assertEquals(0x0A, multisig[0].toInt() and 0xFF)

            assertHeaderClassMismatch(ed25519Single.copyOf().apply { this[0] = 0x0A })
            assertHeaderClassMismatch(mlDsaExtended.copyOf().apply { this[0] = 0x0A })
            assertHeaderClassMismatch(multisig.copyOf().apply { this[0] = 0x02 })
        }
    }

    @Test
    fun publicKeyLiteralAndCompactCodecEnforceProtocolShape() {
        val valid = validMlDsa65Key()
        val literal = encodePublicKeyMultihash(0x02, valid)
        val decodedLiteral = assertNotNull(decodePublicKeyLiteral(literal))
        assertEquals(0x02, decodedLiteral.curveId)
        assertContentEquals(valid, decodedLiteral.keyBytes)
        assertContentEquals(valid, assertNotNull(decodePublicKeyLiteral("ml-dsa:$literal")).keyBytes)

        val compact = compactPublicKeyPayload(0x02, valid)
        assertEquals(4, compact[0].toInt() and 0xFF)
        val decodedCompact = assertNotNull(decodeCompactPublicKeyPayload(compact))
        assertEquals(0x02, decodedCompact.curveId)
        assertContentEquals(valid, decodedCompact.keyBytes)

        for ((name, invalid) in invalidMlDsa65Keys()) {
            assertFailsWith<IllegalArgumentException>(name) {
                encodePublicKeyMultihash(0x02, invalid)
            }
            assertFailsWith<IllegalArgumentException>(name) {
                compactPublicKeyPayload(0x02, invalid)
            }
            assertNull(decodePublicKeyLiteral(rawMlDsaLiteral(invalid)), name)
            assertNull(decodeCompactPublicKeyPayload(byteArrayOf(4) + invalid), name)
        }
    }

    @Test
    fun multisigSignatureConstructionEnforcesProtocolShape() {
        val valid = validMlDsa65Key()
        val signature = ByteArray(64) { (it + 1).toByte() }
        val direct = MultisigSignature.fromCurveId(0x02, valid, signature)
        assertContentEquals(valid, direct.publicKey())
        assertContentEquals(byteArrayOf(4) + valid, direct.publicKeyNoritoPayload())

        val literal = encodePublicKeyMultihash(0x02, valid)
        assertContentEquals(
            valid,
            MultisigSignature.fromPublicKeyLiteral(literal, signature).publicKey(),
        )

        for ((name, invalid) in invalidMlDsa65Keys()) {
            assertFailsWith<IllegalArgumentException>(name) {
                MultisigSignature.fromCurveId(0x02, invalid, signature)
            }
            assertFailsWith<IllegalArgumentException>(name) {
                MultisigSignature.fromPublicKeyLiteral(rawMlDsaLiteral(invalid), signature)
            }
        }
    }

    private fun withMlDsaSupport(block: () -> Unit) {
        try {
            AccountAddress.configureCurveSupport(
                CurveSupportConfig.builder().allowMlDsa(true).build(),
            )
            block()
        } finally {
            AccountAddress.configureCurveSupport(CurveSupportConfig.ed25519Only())
        }
    }

    private fun singleKeyCanonical(
        publicKey: ByteArray,
        curveId: Int = 0x02,
        forceExtended: Boolean = false,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(0x02)
        val extended = forceExtended || publicKey.size > 0xFF
        out.write(if (extended) 0x02 else 0x00)
        out.write(curveId)
        if (extended) {
            out.write((publicKey.size shr 8) and 0xFF)
            out.write(publicKey.size and 0xFF)
        } else {
            out.write(publicKey.size)
        }
        out.write(publicKey, 0, publicKey.size)
        return out.toByteArray()
    }

    private fun multisigCanonical(publicKey: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(byteArrayOf(0x0A, 0x01, 0x01, 0x00, 0x01, 0x00, 0x01))
        out.write(0x02)
        out.write(byteArrayOf(0x00, 0x01))
        out.write((publicKey.size shr 8) and 0xFF)
        out.write(publicKey.size and 0xFF)
        out.write(publicKey, 0, publicKey.size)
        return out.toByteArray()
    }

    private fun assertHeaderClassMismatch(canonical: ByteArray) {
        val error = assertFailsWith<AccountAddressException> {
            AccountAddress.fromCanonicalBytes(canonical)
        }
        assertEquals(AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT, error.code)
    }

    private fun rawMlDsaLiteral(publicKey: ByteArray): String {
        val encoded =
            Varint.encode(0xEEL) +
                Varint.encode(publicKey.size.toLong()) +
                publicKey
        return encoded.joinToString("") { "%02X".format(it.toInt() and 0xFF) }
    }

    private fun readU16Be(bytes: ByteArray, offset: Int): Int =
        ((bytes[offset].toInt() and 0xFF) shl 8) or
            (bytes[offset + 1].toInt() and 0xFF)

    private fun validMlDsa65Key(): ByteArray =
        ByteArray(MlDsaPublicKeyAdmission.PUBLIC_KEY_LENGTH) { 0x65 }

    private fun invalidMlDsa65Keys(): List<Pair<String, ByteArray>> = listOf(
        "empty" to ByteArray(0),
        "32-byte" to ByteArray(32) { 0x20 },
        "ML-DSA-44-sized" to ByteArray(1_312) { 0x44 },
        "one byte short" to ByteArray(1_951) { 0x64 },
        "one byte long" to ByteArray(1_953) { 0x66 },
        "ML-DSA-87-sized" to ByteArray(2_592) { 0x87.toByte() },
        "all-zero" to ByteArray(MlDsaPublicKeyAdmission.PUBLIC_KEY_LENGTH),
    )
}
