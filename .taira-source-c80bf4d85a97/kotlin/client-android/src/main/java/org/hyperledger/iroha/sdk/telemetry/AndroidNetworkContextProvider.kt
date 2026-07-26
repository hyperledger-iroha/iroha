@file:Suppress("DEPRECATION")

package org.hyperledger.iroha.sdk.telemetry

import android.content.Context
import android.net.ConnectivityManager
import java.util.Optional

/** `NetworkContextProvider` that reads Android connectivity APIs directly. */
class AndroidNetworkContextProvider private constructor(
    private val context: Context,
) : NetworkContextProvider {

    override fun snapshot(): Optional<NetworkContext> {
        val connectivityManager = context.getSystemService(Context.CONNECTIVITY_SERVICE)
            as? ConnectivityManager ?: return Optional.empty()

        val networkInfo = connectivityManager.activeNetworkInfo ?: return Optional.empty()
        if (!networkInfo.isConnected) return Optional.empty()

        val networkType = normalizeNetworkType(networkInfo.typeName)
        val roaming = networkInfo.isRoaming
        return Optional.of(NetworkContext.of(networkType, roaming))
    }

    companion object {
        @JvmStatic
        fun fromContext(context: Context): NetworkContextProvider =
            AndroidNetworkContextProvider(context)

        private fun normalizeNetworkType(value: String?): String {
            if (value == null) return "unknown"
            return when (value.trim().lowercase()) {
                "wifi", "wi-fi" -> "wifi"
                "mobile", "cellular" -> "cellular"
                "ethernet" -> "ethernet"
                "bluetooth" -> "bluetooth"
                "vpn" -> "vpn"
                else -> "other"
            }
        }
    }
}
