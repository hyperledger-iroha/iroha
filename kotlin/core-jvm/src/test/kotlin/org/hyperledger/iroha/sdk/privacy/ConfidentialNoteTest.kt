package org.hyperledger.iroha.sdk.privacy

import kotlin.test.Test
import kotlin.test.assertContentEquals
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
    fun decryptionFailsClosedUntilPlaintextContractExists() {
        val payload = ConfidentialEncryptedPayload(
            ephemeralPublicKey = repeated(0x11, 32),
            nonce = repeated(0x22, 24),
            ciphertext = byteArrayOf(0x33),
        )

        assertFailsWith<UnsupportedOperationException> {
            ConfidentialNoteDecryption.decryptNote(payload, repeated(0x44, 32))
        }
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
