package org.hyperledger.iroha.sdk.privacy

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import org.hyperledger.iroha.sdk.core.model.instructions.ConfidentialEncryptedPayload

class ConfidentialNoteTest {
    @Test
    fun derivesRustConfidentialV2Vectors() {
        val spendKey = repeated(0x11, 32)
        val rho = repeated(0x22, 32)

        val ownerTag = ConfidentialOwnerTag.deriveFromSpendKey(spendKey)
        val opening = ConfidentialNoteOpening(
            rho,
            spendKey,
            ownerTag,
            "rose#wonderland",
            "confidential-sdk-chain",
            "7",
        )

        assertContentEquals(hex("5bd47275e203cc0f57ca4ac1b280f9cfe4709e2932f0ac2f6e78d5bcc9cc1e3a"), ownerTag)
        assertContentEquals(
            hex("2d6a7673e8120943d9ec65584117bf16c689094a98eec66a6740b677e92a3f3d"),
            ConfidentialNoteCommitment.deriveFromOpening(opening),
        )
        assertContentEquals(
            hex("35230c0fd55b2f43f23150b36663728e0fcbc62ef97e591e730c13bbc5625f25"),
            ConfidentialNoteNullifier.deriveFromOpening(opening),
        )
        assertContentEquals(
            hex("aa6427acbb05173d9c5ee0698832c7e5d80002937595326ce3915b9d37a30d2f"),
            ConfidentialNoteTags.deriveAssetTag("rose#wonderland"),
        )
        assertContentEquals(
            hex("17870127066ce27fda568817c7a8705c878f18abb56e7653dd30f6157de7a237"),
            ConfidentialNoteTags.deriveChainTag("confidential-sdk-chain"),
        )

        val diversifier = ConfidentialOwnerTag.deriveDiversifier("recipient".encodeToByteArray())
        assertContentEquals(hex("0e200699218253a789fd3cd2c5bc5fe7ec4ad663ca35804554fd60cd89cd2525"), diversifier)
        assertContentEquals(
            hex("5c7dd75a2bb565931e3cc4badba834e976e251e63bc9dbb911b884a27250b53a"),
            ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(spendKey, diversifier),
        )
        assertContentEquals(
            ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(spendKey, diversifier),
            ConfidentialNoteOpening.fromSpendKeyWithDiversifier(
                rho,
                spendKey,
                diversifier,
                "rose#wonderland",
                "confidential-sdk-chain",
                "7",
            ).ownerTag,
        )
    }

    @Test
    fun constructorsAndAccessorsAreDefensive() {
        val spendKey = repeated(0x11, 32)
        val rho = repeated(0x22, 32)
        val ownerTag = ConfidentialOwnerTag.deriveFromSpendKey(spendKey)
        val opening = ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", "chain", "1")

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
        val opening = ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", "chain", "1")

        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(ByteArray(31), spendKey, ownerTag, "rose#wonderland", "chain", "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, ByteArray(0), ownerTag, "rose#wonderland", "chain", "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, spendKey, ByteArray(32) { 0xff.toByte() }, "rose#wonderland", "chain", "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, spendKey, ownerTag, " rose#wonderland", "chain", "1")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", "chain", "01")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", "chain", U128_OVERFLOW)
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
            "chain-a",
            "7",
        )
        val second = ConfidentialNoteOpening.fromSpendKey(
            repeated(0x23, 32),
            repeated(0x11, 32),
            "rose#wonderland",
            "chain-b",
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
            "confidential-sdk-chain",
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
            "confidential-sdk-chain",
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
        assertContentEquals(
            hex(
                "86c7d4b51314553a9f72fa2207969a7bec6626e3c75943c5c7794a660ed54e76" +
                    "371555e888bde13b513f434beef43f5558f1d8fdcd63ac6f40a42c6c90bf26e07d0" +
                    "26dd8a3c632afae83d0aea120fa2886dc97f1dc8a91c6b78de3a57e22da75d217e" +
                    "4924da954b2b2a758df8cacb2ea153d70a756b7f1b8921e",
            ),
            payload.ciphertext,
        )
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
            ConfidentialNoteDecryption.decryptNote(tamperedPayload, recipientPrivateKey, spendKey)
        }
        assertFailsWith<SecurityException> {
            ConfidentialNoteDecryption.decryptNote(payload, repeated(0x56, 32), spendKey)
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteDecryption.decryptNote(payload, recipientPrivateKey, spendKey, "other-chain")
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteDecryption.decryptNote(payload, ByteArray(32), spendKey)
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteDecryption.decryptNote(
                payload,
                recipientPrivateKey,
                repeated(0x12, 32),
                "confidential-sdk-chain",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ConfidentialNoteDecryption.decryptNoteWithOwnerTag(
                payload,
                recipientPrivateKey,
                spendKey,
                ConfidentialOwnerTag.deriveFromSpendKey(repeated(0x12, 32)),
                "confidential-sdk-chain",
            )
        }

        val diversifier = ConfidentialOwnerTag.deriveDiversifier("invoice-1".encodeToByteArray())
        val diversifiedOpening = ConfidentialNoteOpening.fromSpendKeyWithDiversifier(
            repeated(0x24, 32),
            spendKey,
            diversifier,
            "rose#wonderland",
            "confidential-sdk-chain",
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
                "confidential-sdk-chain",
            )
        }
        assertOpeningEquals(
            diversifiedOpening,
            ConfidentialNoteDecryption.decryptNoteWithOwnerTag(
                diversifiedPayload,
                recipientPrivateKey,
                spendKey,
                diversifiedOpening.ownerTag,
                "confidential-sdk-chain",
            ),
        )
    }

    private fun assertOpeningEquals(expected: ConfidentialNoteOpening, actual: ConfidentialNoteOpening) {
        assertContentEquals(expected.rho, actual.rho)
        assertContentEquals(expected.spendKey, actual.spendKey)
        assertContentEquals(expected.ownerTag, actual.ownerTag)
        assertEquals(expected.asset, actual.asset)
        assertEquals(expected.chainId, actual.chainId)
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
