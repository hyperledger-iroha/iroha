package org.hyperledger.iroha.android.offline;

import android.content.Context;
import android.content.pm.PackageManager;
import java.util.Objects;
import org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoff.OfflineNoteTransferCapabilities;

/** Android framework helper for choosing Offline Note local transfer modalities. */
public final class AndroidOfflineNoteTransferCapabilities {
  private AndroidOfflineNoteTransferCapabilities() {}

  public static OfflineNoteTransferCapabilities current(final Context context) {
    return current(context, true);
  }

  public static OfflineNoteTransferCapabilities current(
      final Context context, final boolean nearbyAvailable) {
    final PackageManager packageManager =
        Objects.requireNonNull(context, "context").getPackageManager();
    final boolean hceSupported =
        packageManager.hasSystemFeature(PackageManager.FEATURE_NFC_HOST_CARD_EMULATION);
    return OfflineNoteTransferCapabilities.current(hceSupported, nearbyAvailable);
  }
}
