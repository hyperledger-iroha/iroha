// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Intent
import android.os.BatteryManager
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.runner.lifecycle.Stage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Platform-value mapping controls only; no lifecycle/battery delivery or Native claim. */
@RunWith(AndroidJUnit4::class)
class KagemushaProbeActivityObservationTest {
    @Test fun onlyStartedResumedAndPausedStagesCountAsVisible() {
        val visible = setOf(Stage.STARTED, Stage.RESUMED, Stage.PAUSED)
        for (stage in Stage.values()) assertEquals(stage.name, stage in visible, kagemushaProbeVisibleStage(stage))
    }

    @Test fun chargingUsesObservedStatusAndNeverInfersFromPlugged() {
        assertFalse(kagemushaProbeChargingStatus(null))
        assertFalse(kagemushaProbeChargingStatus(Intent(Intent.ACTION_BATTERY_CHANGED)))
        assertFalse(kagemushaProbeChargingStatus(Intent("unrelated").putExtra(
            BatteryManager.EXTRA_STATUS, BatteryManager.BATTERY_STATUS_CHARGING)))
        for (status in listOf(BatteryManager.BATTERY_STATUS_CHARGING, BatteryManager.BATTERY_STATUS_FULL)) {
            assertTrue(kagemushaProbeChargingStatus(Intent(Intent.ACTION_BATTERY_CHANGED)
                .putExtra(BatteryManager.EXTRA_STATUS, status).putExtra(BatteryManager.EXTRA_PLUGGED, 0)))
        }
        for (status in listOf(BatteryManager.BATTERY_STATUS_UNKNOWN, BatteryManager.BATTERY_STATUS_DISCHARGING,
            BatteryManager.BATTERY_STATUS_NOT_CHARGING, -1, 0, 100)) {
            assertFalse(kagemushaProbeChargingStatus(Intent(Intent.ACTION_BATTERY_CHANGED)
                .putExtra(BatteryManager.EXTRA_STATUS, status)
                .putExtra(BatteryManager.EXTRA_PLUGGED, BatteryManager.BATTERY_PLUGGED_AC)))
        }
    }
}
