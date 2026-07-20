package org.hyperledger.iroha.sdk.crypto

import java.security.Signature
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.crypto.export.DeterministicKeyExporter

class ReflectionFreeCryptoTest {
    @Test
    fun bouncyCastleGeneratorIsLinkedDirectly() {
        val generator = assertNotNull(SoftwareKeyProvider.tryBouncyCastleGenerator())
        val keyPair = SoftwareKeyProvider(SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED)
            .generateEphemeral()

        assertEquals("BC", generator.provider.name)
        assertTrue(keyPair.private.encoded.isNotEmpty())
        assertTrue(keyPair.public.encoded.isNotEmpty())
    }

    @Test
    fun directArgon2ExporterRoundTripsEd25519Key() {
        val keyPair = SoftwareKeyProvider(SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED)
            .generateEphemeral()
        val passphrase = "reflection-free-export".toCharArray()
        try {
            val bundle = DeterministicKeyExporter.exportKeyPair(
                keyPair.private,
                keyPair.public,
                "release-test",
                passphrase,
            )
            val restored = DeterministicKeyExporter.importKeyPair(bundle, passphrase)

            assertContentEquals(keyPair.public.encoded, restored.publicKey.encoded)
            assertEquals(SigningAlgorithm.ED25519, restored.signingAlgorithm)
            val payload = "argon2-round-trip".toByteArray()
            val signer = Signature.getInstance("Ed25519", "BC")
            signer.initSign(restored.privateKey)
            signer.update(payload)
            val signature = signer.sign()
            val verifier = Signature.getInstance("Ed25519", "BC")
            verifier.initVerify(keyPair.public)
            verifier.update(payload)
            assertTrue(verifier.verify(signature))
        } finally {
            passphrase.fill('\u0000')
        }
    }
}
