package org.hyperledger.iroha.sdk.telemetry

import java.util.Optional

/** Provider that supplies the device-profile bucket for telemetry emissions. */
fun interface DeviceProfileProvider {

    /** Returns the current device profile when available. */
    fun snapshot(): Optional<DeviceProfile>

    companion object {
        /** Returns a no-op provider. */
        @JvmStatic
        fun disabled(): DeviceProfileProvider = DeviceProfileProvider { Optional.empty() }
    }
}
