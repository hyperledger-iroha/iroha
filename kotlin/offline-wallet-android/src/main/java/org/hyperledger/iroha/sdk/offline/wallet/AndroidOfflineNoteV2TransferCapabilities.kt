package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Context
import android.content.pm.PackageManager
import org.hyperledger.iroha.sdk.offline.OfflineNoteV2TransferCapabilities

/** Android framework helper for choosing Offline Note V2 local transfer modalities. */
object AndroidOfflineNoteV2TransferCapabilities {
    @JvmStatic
    @JvmOverloads
    fun current(context: Context, nearbyAvailable: Boolean = true): OfflineNoteV2TransferCapabilities {
        val hceSupported =
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_NFC_HOST_CARD_EMULATION)
        return OfflineNoteV2TransferCapabilities.current(
            androidHceSupported = hceSupported,
            nearbyAvailable = nearbyAvailable,
        )
    }
}
