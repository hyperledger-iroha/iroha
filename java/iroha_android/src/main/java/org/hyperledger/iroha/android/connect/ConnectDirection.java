package org.hyperledger.iroha.android.connect;

import java.util.Locale;

/** Connect transport direction (app → wallet or wallet → app). */
public enum ConnectDirection {
  APP_TO_WALLET((byte) 0, "app_to_wallet"),
  WALLET_TO_APP((byte) 1, "wallet_to_app");

  private final byte tag;
  private final String label;

  ConnectDirection(final byte tag, final String label) {
    this.tag = tag;
    this.label = label;
  }

  public byte tag() {
    return tag;
  }

  public String label() {
    return label;
  }

  public static ConnectDirection fromLabel(final String value) {
    if (value == null || value.isEmpty()) {
      throw new IllegalArgumentException("direction label must not be null or empty");
    }
    final String normalized = value.trim().toLowerCase(Locale.ROOT);
    for (final ConnectDirection direction : values()) {
      if (direction.label.equals(normalized)) {
        return direction;
      }
    }
    throw new IllegalArgumentException("unknown Connect direction label: " + value);
  }

  public static ConnectDirection fromTag(final int tag) {
    for (final ConnectDirection direction : values()) {
      if (direction.tag == (byte) tag) {
        return direction;
      }
    }
    throw new IllegalArgumentException("unknown Connect direction tag: " + tag);
  }
}
