package org.hyperledger.iroha.android.model.zk;

import java.util.Locale;

/**
 * Lifecycle status for verifying keys as tracked in the registry.
 *
 * <p>Matches {@code iroha_data_model::confidential::ConfidentialStatus}.
 */
public enum VerifyingKeyStatus {
  PROPOSED("Proposed"),
  ACTIVE("Active"),
  WITHDRAWN("Withdrawn");

  private final String wireName;

  VerifyingKeyStatus(final String wireName) {
    this.wireName = wireName;
  }

  /** Returns the Norito string representation. */
  public String wireName() {
    return wireName;
  }

  /** Parses the wire representation into an enum value. */
  public static VerifyingKeyStatus parse(final String value) {
    if (value == null) {
      throw new IllegalArgumentException("status must not be null");
    }
    final String normalized = value.trim();
    for (final VerifyingKeyStatus status : values()) {
      if (status.wireName.equalsIgnoreCase(normalized)) {
        return status;
      }
    }
    throw new IllegalArgumentException("unsupported verifying key status: " + value);
  }
}
