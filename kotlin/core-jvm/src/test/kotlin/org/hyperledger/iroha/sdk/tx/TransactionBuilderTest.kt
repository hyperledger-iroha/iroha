package org.hyperledger.iroha.sdk.tx

import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.testing.TestNetworkIds
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse

class TransactionBuilderTest {

    @Test
    fun `public builder binds QueuePlan admission while direct codec stays ordinary`() {
        val codec = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val payload = payload(metadata = mapOf("channel" to JsonValue.string("sdk-test")))
        val direct = codec.decodeTransaction(codec.encodeTransaction(payload))
        assertFalse(
            direct.metadata.containsKey(TransactionBuilder.QUEUE_PLAN_SYNCED_ADMISSION_METADATA_KEY),
        )

        val signer = CapturingSigner()
        val signed = TransactionBuilder(codec).encodeAndSign(payload, signer)
        val decoded = codec.decodeTransaction(signed.encodedPayload())

        assertEquals(
            JsonValue.bool(true),
            decoded.metadata[TransactionBuilder.QUEUE_PLAN_SYNCED_ADMISSION_METADATA_KEY],
        )
        assertEquals(JsonValue.string("sdk-test"), decoded.metadata["channel"])
        assertFalse(
            payload.metadata.containsKey(TransactionBuilder.QUEUE_PLAN_SYNCED_ADMISSION_METADATA_KEY),
        )
        assertContentEquals(signed.encodedPayload(), signer.lastMessage)
    }

    @Test
    fun `public builder canonicalizes a caller supplied QueuePlan marker`() {
        val codec = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val payload = payload(
            metadata = mapOf(
                TransactionBuilder.QUEUE_PLAN_SYNCED_ADMISSION_METADATA_KEY to JsonValue.bool(false),
            ),
        )

        val signed = TransactionBuilder(codec).encodeAndSign(payload, CapturingSigner())
        val decoded = codec.decodeTransaction(signed.encodedPayload())

        assertEquals(
            JsonValue.bool(true),
            decoded.metadata[TransactionBuilder.QUEUE_PLAN_SYNCED_ADMISSION_METADATA_KEY],
        )
        assertEquals(
            JsonValue.bool(false),
            payload.metadata[TransactionBuilder.QUEUE_PLAN_SYNCED_ADMISSION_METADATA_KEY],
        )
    }

    private fun payload(metadata: Map<String, JsonValue>): TransactionPayload = TransactionPayload(
        networkId = TestNetworkIds.canonical(),
        authority = AccountAddress
            .fromAccount(TestEd25519Keys.publicKey(0x41), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT),
        creationTimeMs = 1_736_000_000_000,
        executable = Executable.instructions(emptyList()),
        feePayment = FeePaymentIntent.authority(emptyList()),
        metadata = metadata,
    )

    private class CapturingSigner : Signer {
        var lastMessage: ByteArray = byteArrayOf()
            private set

        override fun sign(message: ByteArray): ByteArray {
            lastMessage = message.copyOf()
            return byteArrayOf(0x51)
        }

        override fun publicKey(): ByteArray = byteArrayOf(0x52)

        override fun algorithm(): String = "Ed25519"
    }
}
