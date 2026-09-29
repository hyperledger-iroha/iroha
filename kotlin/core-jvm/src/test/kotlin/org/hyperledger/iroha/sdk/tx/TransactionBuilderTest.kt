package org.hyperledger.iroha.sdk.tx

import org.hyperledger.iroha.sdk.testing.RetiredTransactionWire
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.crypto.SignatureAdmission
import org.hyperledger.iroha.sdk.crypto.SigningException
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.testing.TestNetworkIds
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoException
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class TransactionBuilderTest {

    @Test
    fun `public builder signs the caller's canonical nine-field payload unchanged`() {
        val codec = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val payload = payload(metadata = mapOf("channel" to JsonValue.string("sdk-test")))
        val directBytes = codec.encodeTransaction(payload)
        assertEquals(payload, codec.decodeTransaction(directBytes))
        val signer = CapturingSigner()
        val signed = TransactionBuilder(codec).encodeAndSign(payload, signer)
        val decoded = codec.decodeTransaction(signed.encodedPayload())
        NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(
            signed.encodedPayload(),
        )

        assertEquals(JsonValue.string("sdk-test"), decoded.metadata["channel"])
        assertContentEquals(directBytes, signed.encodedPayload())
        assertContentEquals(signed.encodedPayload(), signer.lastMessage)
    }

    @Test
    fun `canonical payload rejects every retired admission slot`() {
        val codec = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val payload = payload(metadata = mapOf("channel" to JsonValue.string("sdk-test")))
        val canonical = codec.encodeTransaction(payload)
        NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(canonical)
        assertEquals(payload, codec.decodeTransaction(canonical))
        for (tag in listOf(0, 1, 2)) {
            val retired = RetiredTransactionWire.insertAdmissionSlot(canonical, tag)
            assertFailsWith<NoritoException>("retired slot $tag") {
                NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(retired)
            }
            assertFailsWith<NoritoException>("retired codec slot $tag") {
                codec.decodeTransaction(retired)
            }
        }
    }

    @Test
    fun `public builder rejects malformed fixed-shape signer output`() {
        val codec = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val builder = TransactionBuilder(codec)
        val payload = payload(metadata = emptyMap())
        val invalidMlDsaSignatures = listOf(
            "1 byte" to nonzeroBytes(1),
            "64 bytes" to nonzeroBytes(64),
            "3308 bytes" to nonzeroBytes(SignatureAdmission.ML_DSA_65_SIGNATURE_LENGTH - 1),
            "3310 bytes" to nonzeroBytes(SignatureAdmission.ML_DSA_65_SIGNATURE_LENGTH + 1),
            "all-zero" to ByteArray(SignatureAdmission.ML_DSA_65_SIGNATURE_LENGTH),
        )
        for ((name, signature) in invalidMlDsaSignatures) {
            assertFailsWith<SigningException>(name) {
                builder.encodeAndSign(payload, CapturingSigner(signature, "ML-DSA-65"))
            }
        }

        for ((name, signature) in listOf(
            "short Ed25519" to nonzeroBytes(SignatureAdmission.ED25519_SIGNATURE_LENGTH - 1),
            "all-zero Ed25519" to ByteArray(SignatureAdmission.ED25519_SIGNATURE_LENGTH),
        )) {
            assertFailsWith<SigningException>(name) {
                builder.encodeAndSign(payload, CapturingSigner(signature, "Ed25519"))
            }
        }

        val validMlDsaSignature = nonzeroBytes(SignatureAdmission.ML_DSA_65_SIGNATURE_LENGTH)
        val signed = builder.encodeAndSign(
            payload,
            CapturingSigner(validMlDsaSignature, "ML-DSA-65"),
        )
        assertContentEquals(validMlDsaSignature, signed.signature())
    }

    private fun payload(
        metadata: Map<String, JsonValue>,

    ): TransactionPayload = TransactionPayload(
        networkId = TestNetworkIds.canonical(),
        authority = AccountAddress
            .fromAccount(TestEd25519Keys.publicKey(0x41), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT),
        creationTimeMs = 1_736_000_000_000,
        executable = Executable.instructions(emptyList()),
        feePayment = FeePaymentIntent.authority(emptyList()),

        metadata = metadata,
    )

    private class CapturingSigner(
        private val signature: ByteArray = nonzeroBytes(SignatureAdmission.ED25519_SIGNATURE_LENGTH),
        private val algorithmName: String = "Ed25519",
    ) : Signer {
        var lastMessage: ByteArray = byteArrayOf()
            private set

        override fun sign(message: ByteArray): ByteArray {
            lastMessage = message.copyOf()
            return signature.copyOf()
        }

        override fun publicKey(): ByteArray = byteArrayOf(0x52)

        override fun algorithm(): String = algorithmName
    }

    companion object {
        private fun nonzeroBytes(length: Int): ByteArray =
            ByteArray(length) { ((it % 251) + 1).toByte() }
    }
}
