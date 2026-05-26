package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Context
import android.content.pm.PackageManager
import org.hyperledger.iroha.sdk.offline.OfflineNoteTransferCapabilities

/** Android framework helper for choosing Offline Note local transfer modalities. */
object AndroidOfflineNoteTransferCapabilities {
    @JvmStatic
    @JvmOverloads
    fun current(context: Context, nearbyAvailable: Boolean = true): OfflineNoteTransferCapabilities {
        val hceSupported =
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_NFC_HOST_CARD_EMULATION)
        return OfflineNoteTransferCapabilities.current(
            androidHceSupported = hceSupported,
            nearbyAvailable = nearbyAvailable,
        )
    }
}
