package org.hyperledger.iroha.sdk.privacy

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.core.model.instructions.ConfidentialEncryptedPayload
import org.hyperledger.iroha.sdk.testing.TestNetworkIds

class ConfidentialNoteTest {
    private val networkId = TestNetworkIds.canonical()
    private val otherNetworkId = TestNetworkIds.fromSeed(0x42L)

    @Test
    fun derivesCanonicalNativeConfidentialV3Values() {
        assertTrue(PrivacyNativeBridge.isNativeAvailable())
        assertEquals(1, PrivacyNativeBridge.CONFIDENTIAL_DERIVATION_CONTRACT_REVISION_V3)
        val spendKey = repeated(0x11, 32)
        val rho = repeated(0x22, 32)

        val ownerTag = ConfidentialOwnerTag.deriveFromSpendKey(spendKey)
        val opening = ConfidentialNoteOpening(
            rho,
            spendKey,
            ownerTag,
            "rose#wonderland",
            networkId,
            "7",
        )

        val commitment = ConfidentialNoteCommitment.deriveFromOpening(opening)
        val nullifier = ConfidentialNoteNullifier.deriveFromOpening(opening)
        val assetTag = ConfidentialNoteTags.deriveAssetTag("rose#wonderland")
        val networkTag = ConfidentialNoteTags.deriveNetworkTag(networkId)
        listOf(ownerTag, commitment, nullifier, assetTag, networkTag).forEach {
            assertEquals(32, it.size)
            assertTrue(it.any { byte -> byte != 0.toByte() })
        }
        assertNotEquals(
            hexLower(ConfidentialNoteTags.deriveNetworkTag(networkId)),
            hexLower(ConfidentialNoteTags.deriveNetworkTag(otherNetworkId)),
        )

        val diversifier = ConfidentialOwnerTag.deriveDiversifier("recipient".encodeToByteArray())
        assertEquals(32, diversifier.size)
        assertTrue(diversifier.any { it != 0.toByte() })
        assertContentEquals(
            ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(spendKey, diversifier),
            ConfidentialNoteOpening.fromSpendKeyWithDiversifier(
                rho,
                spendKey,
                diversifier,
                "rose#wonderland",
                networkId,
                "7",
            ).ownerTag,
        )
    }

    @Test
    fun constructorsAndAccessorsAreDefensive() {
        val spendKey = repeated(0x11, 32)
        val rho = repeated(0x22, 32)
        val ownerTag = ConfidentialOwnerTag.deriveFromSpendKey(spendKey)
        val opening = ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", networkId, "1")

        rho[0] = 0x55
        spendKey[0] = 0x66
        ownerTag[0] = 0x77
        val exposedRho = opening.rho
        val exposedSpendKey = opening.spendKey
        val exposedOwnerTag = opening.ownerTag
        exposedRho[0] = 0x44
        exposedSpendKey[0] = 0x33
        exposedOwnerTag[0] = 0x22

        assertContentEquals(repeated(0x22, 32), opening.rho)
        assertContentEquals(repeated(0x11, 32), opening.spendKey)
        assertContentEquals(ConfidentialOwnerTag.deriveFromSpendKey(repeated(0x11, 32)), opening.ownerTag)
    }

    @Test
    fun rejectsMalformedAndAmbiguousInputs() {
        val spendKey = repeated(0x11, 32)
        val rho = repeated(0x22, 32)
        val ownerTag = ConfidentialOwnerTag.deriveFromSpendKey(spendKey)
        val opening = ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", networkId, "1")

        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(ByteArray(31), spendKey, ownerTag, "rose#wonderland", networkId, "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(ByteArray(32), spendKey, ownerTag, "rose#wonderland", networkId, "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, ByteArray(0), ownerTag, "rose#wonderland", networkId, "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, ByteArray(32), ownerTag, "rose#wonderland", networkId, "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, spendKey, ByteArray(32), "rose#wonderland", networkId, "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, spendKey, ByteArray(32) { 0xff.toByte() }, "rose#wonderland", networkId, "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, spendKey, ownerTag, " rose#wonderland", networkId, "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", networkId, "01")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", networkId, "0")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", networkId, U128_OVERFLOW)
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteEncryption.publicKeyFromPrivateKey(ByteArray(32))
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteEncryption.encryptNote(
                opening,
                ByteArray(32).also { it[0] = 1 },
                repeated(0x66, 32),
                repeated(0x77, 24),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteEncryption.encryptNote(
                opening,
                ConfidentialNoteEncryption.publicKeyFromPrivateKey(repeated(0x55, 32)),
                ByteArray(32),
                repeated(0x77, 24),
            )
        }
    }

    @Test
    fun derivationsAreDomainSeparated() {
        val first = ConfidentialNoteOpening.fromSpendKey(
            repeated(0x22, 32),
            repeated(0x11, 32),
            "rose#wonderland",
            networkId,
            "7",
        )
        val second = ConfidentialNoteOpening.fromSpendKey(
            repeated(0x23, 32),
            repeated(0x11, 32),
            "rose#wonderland",
            otherNetworkId,
            "7",
        )

        assertNotEquals(
            hexLower(ConfidentialNoteCommitment.deriveFromOpening(first)),
            hexLower(ConfidentialNoteCommitment.deriveFromOpening(second)),
        )
        assertNotEquals(
            hexLower(ConfidentialNoteNullifier.deriveFromOpening(first)),
            hexLower(ConfidentialNoteNullifier.deriveFromOpening(second)),
        )
    }

    @Test
    fun encryptsAndDecryptsPlaintextContract() {
        val spendKey = repeated(0x11, 32)
        val opening = ConfidentialNoteOpening.fromSpendKey(
            repeated(0x22, 32),
            spendKey,
            "rose#wonderland",
            networkId,
            "7",
        )
        val recipientPrivateKey = repeated(0x55, 32)
        val recipientPublicKey =
            ConfidentialNoteEncryption.publicKeyFromPrivateKey(recipientPrivateKey)
        val ephemeralPrivateKey = repeated(0x66, 32)
        val nonce = repeated(0x77, 24)

        val payload = ConfidentialNoteEncryption.encryptNote(
            opening,
            recipientPublicKey,
            ephemeralPrivateKey,
            nonce,
        )
        val decrypted = ConfidentialNoteDecryption.decryptNote(
            payload,
            recipientPrivateKey,
            spendKey,
            networkId,
        )

        assertEquals(ConfidentialEncryptedPayload.VERSION_V1, payload.version)
        assertContentEquals(
            hex("38ab664bd86f77d7e66bdd9ae0792913a94fd8b33a1260027e4b46c1f4884c67"),
            recipientPublicKey,
        )
        assertContentEquals(
            ConfidentialNoteEncryption.publicKeyFromPrivateKey(ephemeralPrivateKey),
            payload.ephemeralPublicKey,
        )
        assertContentEquals(
            hex("219e4d800da968d2a5fcb009c784f4746c7138edb9ee4844b739e830b05cf424"),
            payload.ephemeralPublicKey,
        )
        assertContentEquals(nonce, payload.nonce)
        assertTrue(payload.ciphertext.isNotEmpty())
        assertOpeningEquals(opening, decrypted)
        assertContentEquals(
            ConfidentialNoteCommitment.deriveFromOpening(opening),
            ConfidentialNoteCommitment.deriveFromOpening(decrypted),
        )
        assertContentEquals(
            ConfidentialNoteNullifier.deriveFromOpening(opening),
            ConfidentialNoteNullifier.deriveFromOpening(decrypted),
        )

        val tamperedCiphertext = payload.ciphertext
        tamperedCiphertext[tamperedCiphertext.lastIndex] =
            (tamperedCiphertext.last().toInt() xor 0x01).toByte()
        val tamperedPayload = ConfidentialEncryptedPayload(
            ephemeralPublicKey = payload.ephemeralPublicKey,
            nonce = payload.nonce,
            ciphertext = tamperedCiphertext,
        )
        assertFailsWith<SecurityException> {
            ConfidentialNoteDecryption.decryptNote(
                tamperedPayload,
                recipientPrivateKey,
                spendKey,
                networkId,
            )
        }
        assertFailsWith<SecurityException> {
            ConfidentialNoteDecryption.decryptNote(payload, repeated(0x56, 32), spendKey, networkId)
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteDecryption.decryptNote(payload, recipientPrivateKey, spendKey, otherNetworkId)
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteDecryption.decryptNote(payload, ByteArray(32), spendKey, networkId)
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteDecryption.decryptNote(
                payload,
                recipientPrivateKey,
                repeated(0x12, 32),
                networkId,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteDecryption.decryptNoteWithOwnerTag(
                payload,
                recipientPrivateKey,
                spendKey,
                ConfidentialOwnerTag.deriveFromSpendKey(repeated(0x12, 32)),
                networkId,
            )
        }

        val diversifier = ConfidentialOwnerTag.deriveDiversifier("invoice-1".encodeToByteArray())
        val diversifiedOpening = ConfidentialNoteOpening.fromSpendKeyWithDiversifier(
            repeated(0x24, 32),
            spendKey,
            diversifier,
            "rose#wonderland",
            networkId,
            "11",
        )
        val diversifiedPayload = ConfidentialNoteEncryption.encryptNote(
            diversifiedOpening,
            recipientPublicKey,
            repeated(0x68, 32),
            repeated(0x79, 24),
        )
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteDecryption.decryptNote(
                diversifiedPayload,
                recipientPrivateKey,
                spendKey,
                networkId,
            )
        }
        assertOpeningEquals(
            diversifiedOpening,
            ConfidentialNoteDecryption.decryptNoteWithOwnerTag(
                diversifiedPayload,
                recipientPrivateKey,
                spendKey,
                diversifiedOpening.ownerTag,
                networkId,
            ),
        )
    }

    private fun assertOpeningEquals(expected: ConfidentialNoteOpening, actual: ConfidentialNoteOpening) {
        assertContentEquals(expected.rho, actual.rho)
        assertContentEquals(expected.spendKey, actual.spendKey)
        assertContentEquals(expected.ownerTag, actual.ownerTag)
        assertEquals(expected.asset, actual.asset)
        assertEquals(expected.networkId, actual.networkId)
        assertEquals(expected.amount, actual.amount)
    }

    private fun repeated(value: Int, len: Int): ByteArray = ByteArray(len) { value.toByte() }

    private fun hex(value: String): ByteArray {
        require(value.length % 2 == 0)
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun hexLower(bytes: ByteArray): String =
        bytes.joinToString("") { ((it.toInt() and 0xff) + 0x100).toString(16).substring(1) }

    companion object {
        private const val U128_OVERFLOW = "340282366920938463463374607431768211456"
    }
}
