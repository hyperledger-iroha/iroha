package org.hyperledger.iroha.samples.wallet

import android.util.Log
import java.util.concurrent.ConcurrentHashMap

/**
 * Records address copy events so UI instrumentation can feed dashboards mirroring the ADDR-6b gates.
 */
object AddressCopyTelemetry {
    private const val TAG = "AddressCopyTelemetry"

    enum class CopyMode(val analyticsLabel: String) {
        I105("i105")
    }

    private val counters = ConcurrentHashMap<CopyMode, Int>()

    fun record(mode: CopyMode) {
        val next = counters.merge(mode, 1) { current, increment -> current + increment } ?: 1
        Log.i(TAG, "address_copy_mode=${mode.analyticsLabel} count=$next")
    }
}
