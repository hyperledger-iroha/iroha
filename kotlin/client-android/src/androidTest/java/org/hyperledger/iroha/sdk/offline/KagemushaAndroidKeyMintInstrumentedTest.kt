// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import android.content.pm.PackageManager
import android.os.Build
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.security.GeneralSecurityException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith

/** Physical-device smoke coverage for the exact single-use KeyMint generation profile. */
@RunWith(AndroidJUnit4::class)
class KagemushaAndroidKeyMintInstrumentedTest {
    @Test
    fun platformFailsClosedOrGeneratesHardwareSingleUseMaterial() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val keyMint = KagemushaAndroidKeyMint(context)
        val challenge = ByteArray(32) { 0x45 }.also { it[31] = 0x45 }
        val alias = "iroha-kagemusha-instrumented-${System.nanoTime()}"
        val supported =
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
                context.packageManager.hasSystemFeature(
                    PackageManager.FEATURE_KEYSTORE_SINGLE_USE_KEY,
                )

        if (!supported) {
            try {
                keyMint.generateRegistrationMaterial(
                    alias,
                    challenge,
                    KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
                )
                fail("unsupported device generated Kagemusha KeyMint material")
            } catch (expected: GeneralSecurityException) {
                assertTrue(expected.message.orEmpty().isNotEmpty())
            }
            return
        }

        var material: KagemushaAndroidKeyMint.RegistrationMaterial? = null
        try {
            material = keyMint.generateRegistrationMaterial(
                alias,
                challenge,
                KagemushaAndroidKeyMint.StrongBoxPolicy.NOT_REQUESTED,
            )
            assertEquals(alias, material.alias())
            assertEquals(65, material.assertionPublicKeySec1().size)
            assertEquals(64, material.keyId().length)
            assertFalse(material.certificateChainDer().isEmpty())
            assertFalse(material.isConsumed())
        } finally {
            if (material != null && !material.isConsumed()) {
                keyMint.delete(material)
                assertTrue(material.isConsumed())
            }
        }
    }
}
