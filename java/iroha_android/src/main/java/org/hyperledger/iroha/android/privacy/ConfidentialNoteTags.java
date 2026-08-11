package org.hyperledger.iroha.android.privacy;

import org.hyperledger.iroha.android.model.NetworkId;

/** Rust-owned asset and exact-network tags used by confidential V3 derivation. */
public final class ConfidentialNoteTags {
  private ConfidentialNoteTags() {}

  public static byte[] deriveAssetTag(final String asset) {
    return PrivacyNativeBridge.deriveConfidentialAssetTagV3(asset);
  }

  public static byte[] deriveNetworkTag(final NetworkId networkId) {
    return PrivacyNativeBridge.deriveConfidentialNetworkTagV3(networkId);
  }
}
