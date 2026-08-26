package org.hyperledger.iroha.sdk.crypto.export

import kotlin.test.Test
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.crypto.SoftwareKeyProvider

class DeterministicKeyExporterTest {
    @Test
    fun `import rejects a substituted Ed25519 public key`() {
        val provider = SoftwareKeyProvider()
        val original = provider.generateEphemeral()
        val attacker = provider.generateEphemeral()
        val passphrase = "authenticated-key-pair".toCharArray()
        try {
            val bundle = DeterministicKeyExporter.exportKeyPair(
                original.private,
                original.public,
                "restored-account",
                passphrase,
            )
            val tampered = KeyExportBundle(
                alias = bundle.alias,
                algorithmCode = bundle.algorithmCode,
                publicKey = attacker.public.encoded,
                nonce = bundle.nonce,
                ciphertext = bundle.ciphertext,
                salt = bundle.salt,
                kdfKind = bundle.kdfKind,
                kdfWorkFactor = bundle.kdfWorkFactor,
                version = bundle.version,
            )

            assertFailsWith<KeyExportException> {
                DeterministicKeyExporter.importKeyPair(tampered, passphrase)
            }
        } finally {
            passphrase.fill('\u0000')
        }
    }

    @Test
    fun `export rejects an inconsistent Ed25519 key pair`() {
        val provider = SoftwareKeyProvider()
        val privateKey = provider.generateEphemeral().private
        val unrelatedPublicKey = provider.generateEphemeral().public
        val passphrase = "authenticated-key-pair".toCharArray()
        try {
            assertFailsWith<KeyExportException> {
                DeterministicKeyExporter.exportKeyPair(
                    privateKey,
                    unrelatedPublicKey,
                    "inconsistent-account",
                    passphrase,
                )
            }
        } finally {
            passphrase.fill('\u0000')
        }
    }
}
