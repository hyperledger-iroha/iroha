package org.hyperledger.iroha.sdk.privacy

import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.Signature
import java.security.spec.ECGenParameterSpec
import org.bouncycastle.jce.provider.BouncyCastleProvider
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Same-source JNI is mandatory; these tests do not skip unavailable native artifacts. */
class ZkX509SigningPreparationNativeTests {
    private fun rows(): List<PrivacyExact12TypedFixtureRowV1> {
        assertTrue(PrivacyNativeBridge.isNativeAvailable(), "rebuilt native privacy bridge is required")
        return PrivacyExact12FixtureCodecV1.decodeCanonical(PrivacyNativeBridge.exact12FixtureBundleV1()).rows
    }

    @Test fun nativePreparationMatchesRawJniAndRealMessageAndPrehashSigning() {
        val row = rows().single { it.protocolId == PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1 }
        val prepared = ZkX509SigningPreparationV1.prepareOwnershipSigningV1(row.statementNorito)
        val raw = requireNotNull(ZkX509SigningPreparationNative.prepare(row.statementNorito))
        assertEquals(2, raw.size)
        assertArrayEquals(raw[0], prepared.message)
        assertArrayEquals(raw[1], prepared.digest)
        assertArrayEquals(MessageDigest.getInstance("SHA-256").digest(prepared.message), prepared.digest)
        val provider = BouncyCastleProvider()
        val generator = KeyPairGenerator.getInstance("EC", provider)
        generator.initialize(ECGenParameterSpec("secp256r1"))
        val key = generator.generateKeyPair() // This test key never enters the bridge.
        fun sign(algorithm: String, bytes: ByteArray): ByteArray = Signature.getInstance(algorithm, provider).run {
            initSign(key.private); update(bytes); sign()
        }
        fun verify(algorithm: String, bytes: ByteArray, signature: ByteArray): Boolean = Signature.getInstance(algorithm, provider).run {
            initVerify(key.public); update(bytes); verify(signature)
        }
        val messageSignature = sign("SHA256withECDSA", prepared.message)
        val prehashSignature = sign("NONEwithECDSA", prepared.digest)
        assertTrue(verify("NONEwithECDSA", prepared.digest, messageSignature))
        assertTrue(verify("SHA256withECDSA", prepared.message, prehashSignature))
        val doubleHash = sign("SHA256withECDSA", prepared.digest)
        assertFalse(verify("SHA256withECDSA", prepared.message, doubleHash))
        assertFalse(verify("NONEwithECDSA", prepared.digest, doubleHash))
    }

    @Test fun actualJniRejectsWrongProtocolAndNoncanonicalInput() {
        val rows = rows()
        for (row in rows.filter { it.protocolId != PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1 }) {
            val failure = assertThrows(ZkX509SigningPreparationException::class.java) {
                ZkX509SigningPreparationNative.prepare(row.statementNorito)
            }
            assertEquals(-5, failure.code)
        }
        val original = rows.single { it.protocolId == PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1 }.statementNorito
        for ((input, code) in listOf(
            ByteArray(0) to -2,
            ByteArray(ZkX509SigningPreparationV1.MAX_STATEMENT_BYTES + 1) to -3,
            original.copyOf(original.size - 1) to -4,
            (original + byteArrayOf(0)) to -4,
            original.clone().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() } to -4,
        )) {
            val failure = assertThrows(ZkX509SigningPreparationException::class.java) {
                ZkX509SigningPreparationNative.prepare(input)
            }
            assertEquals(code, failure.code)
        }
    }
}
