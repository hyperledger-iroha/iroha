package org.hyperledger.iroha.samples.preview

import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.TransactionBuilder
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

/** Offline sample previews use a synthetic network identity and are never submitted to Torii. */
object PreviewTransactions {
    private val network = NetworkId.fromBytes(ByteArray(32).also { it[31] = 1 })
    private val publicKeyPrefix = byteArrayOf(0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00)

    /** Bind the preview authority to the actual software signer and retain the explicit fee intent. */
    fun sign(signer: Signer, alias: String, message: String): SignedTransaction {
        require(signer.algorithm() == "Ed25519") { "sample preview requires Ed25519" }
        val publicKey = signer.publicKey()
        require(publicKey.size == 44 && publicKey.copyOfRange(0, 12).contentEquals(publicKeyPrefix)) {
            "sample signer must provide canonical Ed25519 SPKI"
        }
        val authority = AccountAddress.fromAccount(publicKey.copyOfRange(12, 44), "ed25519").toI105Default()
        val payload = TransactionPayload(
            networkId = network,
            authority = authority,
            executable = Executable.instructions(listOf(NoritoJavaCodecAdapter.logInstruction(2L, message))),
            feePayment = FeePaymentIntent.authority(emptyList()),
            metadata = mapOf("sample" to JsonValue.string(alias))
        )
        return TransactionBuilder(NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT))
            .encodeAndSign(payload, signer, alias)
    }
}
