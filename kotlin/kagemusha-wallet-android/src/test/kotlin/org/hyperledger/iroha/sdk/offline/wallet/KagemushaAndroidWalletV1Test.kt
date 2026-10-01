// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareProviderV1
import org.junit.jupiter.api.Test

class KagemushaAndroidWalletV1Test {
    @Test
    fun `production uses the factory admitted lifecycle bridge`() {
        var bridgeCalls = 0
        var providerCalls = 0
        val factory = object : KagemushaAndroidHardwareProviderFactoryV1 {
            override fun deviceLifecycleBridge(): KagemushaDeviceLifecycleBridgeV1 {
                bridgeCalls += 1
                throw IllegalStateException("native capability qualification failed")
            }

            override fun open(bridge: KagemushaDeviceLifecycleBridgeV1, intentStore: org.hyperledger.iroha.sdk.offline.KagemushaOperationIntentStoreV1, authorizeBootstrap: () -> Unit): KagemushaHardwareProviderV1 {
                providerCalls += 1
                error("provider must not be opened for a rejected capability frame")
            }
        }
        assertFailsWith<IllegalStateException> {
            KagemushaAndroidWalletV1.openProduction(factory, org.hyperledger.iroha.sdk.offline.TestOperationIntentStoreV1(), {})
        }
        assertEquals(1, bridgeCalls)
        assertEquals(0, providerCalls)
    }
}
