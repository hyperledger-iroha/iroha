package org.hyperledger.iroha.android.offline;

import android.content.Context;
import android.content.pm.PackageManager;
import java.util.Objects;
import org.hyperledger.iroha.android.offline.OfflineNoteV2TransferHandoff.OfflineNoteV2TransferCapabilities;

/** Android framework helper for choosing Offline Note V2 local transfer modalities. */
public final class AndroidOfflineNoteV2TransferCapabilities {
  private AndroidOfflineNoteV2TransferCapabilities() {}

  public static OfflineNoteV2TransferCapabilities current(final Context context) {
    return current(context, true);
  }

  public static OfflineNoteV2TransferCapabilities current(
      final Context context, final boolean nearbyAvailable) {
    final PackageManager packageManager =
        Objects.requireNonNull(context, "context").getPackageManager();
    final boolean hceSupported =
        packageManager.hasSystemFeature(PackageManager.FEATURE_NFC_HOST_CARD_EMULATION);
    return OfflineNoteV2TransferCapabilities.current(hceSupported, nearbyAvailable);
  }
}
