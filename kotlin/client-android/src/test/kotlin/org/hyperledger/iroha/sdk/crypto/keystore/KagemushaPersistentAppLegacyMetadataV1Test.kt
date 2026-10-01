// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto.keystore

import android.security.keystore.KeyProperties
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Pure metadata correlation fixtures, not Android physical hardware evidence. */
class KagemushaPersistentAppLegacyMetadataV1Test {
    @Test fun `API28 to30 consumes actual hardware residency and signed level without fabricated usage metadata`() {
        for (api in 28..30) for (level in listOf(KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT, KeyProperties.SECURITY_LEVEL_STRONGBOX)) {
            assertEquals(level, requirePersistentAppHardwareMetadataV1(api, true, level, null, null))
            requirePersistentHardwareAppKeyWithoutUsageMetadataV1(level, KeyProperties.ORIGIN_GENERATED, KeyProperties.PURPOSE_SIGN,
                setOf(KeyProperties.DIGEST_SHA256), false, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX)
            assertFailsWith<IllegalStateException> { requirePersistentAppHardwareMetadataV1(api, false, level, null, null) }
            assertFailsWith<IllegalStateException> { requirePersistentAppHardwareMetadataV1(api, true, level, level, -1) }
        }
        assertFailsWith<IllegalStateException> { requirePersistentAppHardwareMetadataV1(28, true, KeyProperties.SECURITY_LEVEL_SOFTWARE, null, null) }
        assertFailsWith<IllegalStateException> { requirePersistentAppHardwareMetadataV1(27, true, KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT, null, null) }
        assertFailsWith<IllegalStateException> { requirePersistentAppHardwareMetadataV1(28, true, -1, null, null) }
    }
    @Test fun `API31 adds exact modern level and persistent usage readback without broadening TEE policy`() {
        val tee = KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT; val strong = KeyProperties.SECURITY_LEVEL_STRONGBOX
        assertEquals(tee, requirePersistentAppHardwareMetadataV1(31, true, tee, tee, KeyProperties.UNRESTRICTED_USAGE_COUNT))
        for ((level, remaining) in listOf(strong to -1, tee to 1, tee to 0, null to null)) {
            assertFailsWith<IllegalStateException> { requirePersistentAppHardwareMetadataV1(31, true, tee, level, remaining) }
        }
        assertFailsWith<IllegalStateException> { requirePersistentHardwareAppKeyWithoutUsageMetadataV1(strong, KeyProperties.ORIGIN_GENERATED,
            KeyProperties.PURPOSE_SIGN, setOf(KeyProperties.DIGEST_SHA256), false, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY) }
    }
}
