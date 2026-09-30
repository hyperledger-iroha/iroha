package org.hyperledger.iroha.samples.wallet

import java.security.KeyPairGenerator
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.junit.Test
import org.hyperledger.iroha.samples.preview.PreviewTransactions
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.crypto.Ed25519Signer
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

class PreviewTransactionsTest {
    @Test
    fun `preview signs the actual authority and explicit synthetic network`() {
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val signer = Ed25519Signer(keyPair.private, keyPair.public)
        val signed = PreviewTransactions.sign(signer, "example", "offline preview")
        val decoded = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
            .decodeTransaction(signed.encodedPayload())
        assertEquals(AccountAddress.fromAccount(signer.publicKey().copyOfRange(12, 44), "ed25519")
            .toI105Default(), decoded.authority)
        assertTrue(decoded.networkId.bytes().contentEquals(ByteArray(32).also { it[31] = 1 }))
        assertTrue(decoded.executable is Executable.Instructions)
        assertEquals(1, (decoded.executable as Executable.Instructions).instructions.size)
        assertTrue(decoded.feePayment.chargeLimits.isEmpty())
        assertEquals(64, signed.signature().size)
    }

    @Test
    fun `preview refuses an ambiguous signer key encoding`() {
        val signer = object : Signer {
            override fun algorithm() = "Ed25519"
            override fun publicKey() = ByteArray(32)
            override fun sign(message: ByteArray): ByteArray = error("invalid signer must not sign")
        }
        assertFailsWith<IllegalArgumentException> { PreviewTransactions.sign(signer, "example", "preview") }
    }
}
