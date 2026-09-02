package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.util.Objects;
import org.hyperledger.iroha.android.model.NetworkId;

/** Stable, non-secret public metadata for the SORA Taira testnet. */
public final class TairaTestnetProfile {
  /** Public Torii origin. */
  public static final URI TORII_BASE_URI = URI.create("https://taira.sora.org");

  /** Stable semantic chain UUID; this is not a transaction-signing {@link NetworkId}. */
  public static final String CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0";

  /** Canonical I105 address discriminant for Taira. */
  public static final int I105_DISCRIMINANT = 369;

  /** Canonical Digital Shekel asset-definition ID used by Kagemusha V1 on Taira. */
  public static final String KAGEMUSHA_ASSET_DEFINITION_ID = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv";

  /** Canonical Digital Shekel alias used by Kagemusha V1 on Taira. */
  public static final String KAGEMUSHA_ASSET_ALIAS = "ds#boi.is";

  /** Canonical Digital Shekel fixed-point scale used by Kagemusha V1 on Taira. */
  public static final int KAGEMUSHA_ASSET_SCALE = 2;

  /** Public Taira XOR asset-definition ID used for transaction fees. */
  public static final String XOR_ASSET_DEFINITION_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";

  /** Public Taira XOR alias used for transaction fees. */
  public static final String XOR_ASSET_ALIAS = "xor#universal";

  /** Public Taira XOR fee-asset fixed-point scale. */
  public static final int XOR_ASSET_SCALE = 9;

  private TairaTestnetProfile() {}

  /**
   * Creates a Taira client config bound to the caller-supplied deployed genesis identity.
   *
   * <p>Taira resets can change {@link NetworkId}, so callers must obtain this value from the
   * current deployment config or genesis material. The profile never guesses or downloads signing
   * identity from an unauthenticated server response.
   */
  public static ClientConfig clientConfig(final NetworkId deployedNetworkId) {
    return ClientConfig.builder()
        .setBaseUri(TORII_BASE_URI)
        .setLocalSigningContext(
            new LocalSigningContext(Objects.requireNonNull(deployedNetworkId, "deployedNetworkId")))
        .build();
  }
}
