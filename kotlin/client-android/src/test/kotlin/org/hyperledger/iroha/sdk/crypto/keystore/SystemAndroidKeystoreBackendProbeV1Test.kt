// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto.keystore

import java.security.Key
import java.security.KeyStoreException
import java.security.cert.Certificate
import javax.crypto.spec.SecretKeySpec
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertSame
import org.hyperledger.iroha.sdk.IrohaKeyManager
import org.hyperledger.iroha.sdk.crypto.KeyManagementException
import org.hyperledger.iroha.sdk.crypto.KeyProviderMetadata
import org.junit.jupiter.api.Test

/** Tri-state alias decisions of the generic platform backend over a keystore2 model. */
class SystemAndroidKeystoreBackendProbeV1Test {
    private val metadata = KeyProviderMetadata(
        name = "android-keystore",
        hardwareBacked = true,
        supportsAttestationCertificates = true,
        securityLevel = KeyProviderMetadata.HardwareSecurityLevel.TRUSTED_ENVIRONMENT,
    )
    private val keyStore = TestAndroidKeystoreV1()
    private val backend = SystemAndroidKeystoreBackend(metadata, keyStore)

    @Test fun `load returns null only for a definitive absence`() {
        assertNull(backend.load("alias"))
        val entry = keyStore.seed("alias")
        val pair = assertNotNull(backend.load("alias"))
        assertSame(entry.key, pair.private)
        assertContentEquals(entry.pair.public.encoded, pair.public.encoded)
    }

    @Test fun `a Keystore error is reported, never read as absence`() {
        keyStore.seed("alias")
        keyStore.getKeyFailure = KeyStoreException("keystore2 binder failure")
        assertFailsWith<KeyManagementException> { backend.load("alias") }
        assertFailsWith<KeyManagementException> { backend.generateAttestation("alias", ByteArray(0)) }
    }

    @Test fun `generateOrLoad never regenerates over a key it could not see`() {
        val original = keyStore.seed("alias")
        keyStore.getKeyFailure = KeyStoreException("keystore2 binder failure")
        var generations = 0
        val counting = object : AndroidKeystoreBackend by backend {
            override fun generate(alias: String, parameters: KeyGenParameters): KeyGenerationResult {
                generations += 1
                return backend.generate(alias, parameters)
            }
        }
        val manager = IrohaKeyManager.fromProviders(listOf(KeystoreKeyProvider(counting, KeyGenParameters())))
        assertFailsWith<KeyManagementException> {
            manager.generateOrLoad("alias", KeySecurityPreference.HARDWARE_PREFERRED)
        }
        assertEquals(0, generations)
        assertSame(original, keyStore.entries.getValue("alias"))
    }

    @Test fun `a non-private entry is an error rather than an absence to overwrite`() {
        val secretOnly = object : AndroidKeystoreEntriesV1 {
            override val apiLevel: Int = 31
            override fun getKey(alias: String): Key = SecretKeySpec(ByteArray(32), "AES")
            override fun getCertificateChain(alias: String): List<Certificate>? = null
        }
        assertFailsWith<KeyManagementException> { SystemAndroidKeystoreBackend(metadata, secretOnly).load("alias") }
    }

    @Test fun `a present key without a certificate is an error`() {
        val withoutChain = object : AndroidKeystoreEntriesV1 {
            override val apiLevel: Int = 31
            private val entry = keyStore.seed("alias")
            override fun getKey(alias: String): Key = entry.key
            override fun getCertificateChain(alias: String): List<Certificate>? = null
        }
        assertFailsWith<KeyManagementException> { SystemAndroidKeystoreBackend(metadata, withoutChain).load("alias") }
    }

    @Test fun `keystore1 never reports absence`() {
        val legacy = SystemAndroidKeystoreBackend(metadata, TestAndroidKeystoreV1(apiLevel = 30))
        assertFailsWith<KeyManagementException> { legacy.load("alias") }
        assertFailsWith<KeyManagementException> { legacy.generateAttestation("alias", ByteArray(0)) }
    }

    @Test fun `generateAttestation distinguishes absence, presence and re-attestation`() {
        assertNull(backend.generateAttestation("alias", ByteArray(0)))
        keyStore.seed("alias")
        assertEquals(2, assertNotNull(backend.generateAttestation("alias", ByteArray(0))).certificateChain().size)
        assertFailsWith<KeyManagementException> { backend.generateAttestation("alias", byteArrayOf(1)) }
    }

    @Test fun `the platform backend is not offered without keystore2`() {
        // The JVM reports API 0 through the mockable android.jar.
        assertNull(SystemAndroidKeystoreBackend.create())
    }
}
