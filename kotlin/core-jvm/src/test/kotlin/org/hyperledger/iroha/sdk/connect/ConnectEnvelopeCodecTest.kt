package org.hyperledger.iroha.sdk.connect

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull

class ConnectEnvelopeCodecTest {
    @Test
    fun signResultOkAcceptsTrimmedEd25519Alias() {
        val signature = ByteArray(64) { it.toByte() }

        for (algorithm in listOf("ed25519", "ED25519", " Ed25519 ")) {
            val encoded = ConnectEnvelopeCodec.encodeSignResultOkEnvelope(7L, signature, algorithm)
            val decoded = ConnectEnvelopeCodec.decodeEnvelope(encoded)

            assertEquals(7L, decoded.sequence)
            assertEquals(ConnectEnvelopeCodec.PayloadKind.SIGN_RESULT_OK, decoded.payload.kind())
            val payload = decoded.payload as ConnectEnvelopeCodec.SignResultOkPayload
            assertEquals("ed25519", payload.algorithm)
            assertEquals(signature.toList(), payload.signature().toList())
        }
    }

    @Test
    fun signResultOkRejectsControlAndUnicodeConfusableAlgorithms() {
        val signature = ByteArray(64) { 0x55 }

        for (algorithm in listOf(
            "secp256k1",
            "ed\t25519",
            "ed\u200B25519",
            "\u0435d25519",
            "ed\uFF0D25519",
        )) {
            assertFailsWith<ConnectProtocolException> {
                ConnectEnvelopeCodec.encodeSignResultOkEnvelope(7L, signature, algorithm)
            }
        }
    }

    @Test
    fun ciphertextFrameRoundTripsAeadBytesWithCanonicalFlags() {
        val sessionId = ByteArray(32) { (0xA0 + it).toByte() }
        val key = ByteArray(32) { (0x11 + it).toByte() }
        val envelope = ConnectEnvelopeCodec.encodeSignResultErrEnvelope(
            2L,
            "USER_DENIED",
            "Rejected by test",
        )
        val ciphertext = ConnectCrypto.encryptEnvelope(
            envelope,
            key,
            sessionId,
            ConnectDirection.WALLET_TO_APP,
            2L,
        )
        val frame = ConnectFrameCodec.encodeCiphertextFrame(
            sessionId,
            ConnectDirection.WALLET_TO_APP,
            2L,
            ciphertext,
        )
        val decodedFrame = ConnectFrameCodec.decode(frame)

        assertEquals(FrameType.CIPHERTEXT, decodedFrame.type)
        val decodedCiphertext = assertNotNull(decodedFrame.ciphertext)
        val plaintext = ConnectCrypto.decryptCiphertext(
            decodedCiphertext.aead(),
            key,
            sessionId,
            ConnectDirection.WALLET_TO_APP,
            2L,
        )
        val decodedEnvelope = ConnectEnvelopeCodec.decodeEnvelope(plaintext)

        assertEquals(ConnectEnvelopeCodec.PayloadKind.SIGN_RESULT_ERR, decodedEnvelope.payload.kind())
    }
}
