// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.offline.OfflineCashDeviceLifecycleBridgeV1
import org.junit.jupiter.api.Test

class OfflineCashAndroidWalletV1Test {
    @Test
    fun `online only bridge never invokes an OEM provider or falls back`() {
        var factoryCalls = 0
        val error = assertFailsWith<IllegalStateException> {
            OfflineCashAndroidWalletV1.openBridge(
                OfflineCashDeviceLifecycleBridgeV1.onlineOnly(),
            ) {
                factoryCalls += 1
                error("factory must not be invoked")
            }
        }
        assertEquals(0, factoryCalls)
        assertEquals(true, error.message!!.contains("online-only"))
    }
}
