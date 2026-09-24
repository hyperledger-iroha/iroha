// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith

/** Physical JNI ABI check only; no qualified coordinator backend is installed by this test. */
@RunWith(AndroidJUnit4::class)
class KagemushaCoreCoordinatorExportDeviceTest {
    @Test
    fun method14IsInTheNativeInventoryAndRejectsAnInvalidSelector() {
        System.loadLibrary("connect_norito_bridge")
        val contract = KagemushaCoreCoordinatorJniV1.contract()
        assertNotNull(contract)
        assertEquals(12, contract!!.size)
        assertEquals(14, contract[11])
        assertNull(KagemushaCoreCoordinatorJniV1.invoke(
            1L,
            KagemushaCoreCoordinatorMethodV1.EXPORT_OUTGOING_STATE_PROOF.code,
            arrayOf(ByteArray(32)),
        ))
    }
}
