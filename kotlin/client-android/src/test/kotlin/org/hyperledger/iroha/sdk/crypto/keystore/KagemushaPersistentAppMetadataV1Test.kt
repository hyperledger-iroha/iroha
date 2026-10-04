// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto.keystore

import android.security.keystore.KeyProperties
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Pure metadata correlation fixtures, not Android physical hardware evidence. */
class KagemushaPersistentAppMetadataV1Test {
    private val tee = KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT
    private val strong = KeyProperties.SECURITY_LEVEL_STRONGBOX
    private val unlimited = KeyProperties.UNRESTRICTED_USAGE_COUNT

    @Test fun `keystore1 refuses even exact hardware metadata because it cannot prove absence`() {
        for (api in listOf(24, 26, 28, 30)) for (level in listOf(tee, strong)) {
            assertFailsWith<IllegalStateException> { requirePersistentAppHardwareMetadataV1(api, true, level, level, unlimited) }
        }
    }

    @Test fun `keystore2 requires actual residency and a KeyInfo level equal to the signed level`() {
        for (api in listOf(31, 35)) for (level in listOf(tee, strong)) {
            assertEquals(level, requirePersistentAppHardwareMetadataV1(api, true, level, level, unlimited))
            assertFailsWith<IllegalStateException> { requirePersistentAppHardwareMetadataV1(api, false, level, level, unlimited) }
        }
        for ((reported, remaining) in listOf(strong to unlimited, tee to 1, tee to 0)) {
            assertFailsWith<IllegalStateException> { requirePersistentAppHardwareMetadataV1(31, true, tee, reported, remaining) }
        }
        for (level in listOf(KeyProperties.SECURITY_LEVEL_SOFTWARE, -1)) {
            assertFailsWith<IllegalStateException> { requirePersistentAppHardwareMetadataV1(31, true, level, level, unlimited) }
        }
    }

    @Test fun `modern metadata does not broaden the TEE or StrongBox policy`() {
        fun check(level: Int, policy: KagemushaAndroidAppKeyHardwarePolicyV1) = requirePersistentHardwareAppKeyV1(level,
            KeyProperties.ORIGIN_GENERATED, KeyProperties.PURPOSE_SIGN, setOf(KeyProperties.DIGEST_SHA256), unlimited, false, policy)
        check(tee, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX)
        check(strong, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX)
        assertFailsWith<IllegalStateException> { check(strong, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY) }
        assertFailsWith<IllegalStateException> { check(tee, KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY) }
    }
}
