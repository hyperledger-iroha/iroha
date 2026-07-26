package org.hyperledger.iroha.android.explorer;

import java.util.Objects;

/** Immutable view over `/v1/explorer/accounts/{account_id}/qr` responses. */
public final class ExplorerAccountQrSnapshot {

  private final String canonicalId;
  private final String literal;
  private final String addressFormat;
  private final int networkPrefix;
  private final int modules;
  private final int qrVersion;
  private final String errorCorrection;
  private final String svg;

  public ExplorerAccountQrSnapshot(
      final String canonicalId,
      final String literal,
      final String addressFormat,
      final int networkPrefix,
      final int modules,
      final int qrVersion,
      final String errorCorrection,
      final String svg) {
    this.canonicalId = requireNonEmpty(canonicalId, "canonicalId");
    this.literal = requireNonEmpty(literal, "literal");
    this.addressFormat = requireNonEmpty(addressFormat, "addressFormat");
    this.networkPrefix = requirePositive(networkPrefix, "networkPrefix");
    this.modules = requirePositive(modules, "modules");
    this.qrVersion = requirePositive(qrVersion, "qrVersion");
    this.errorCorrection = requireNonEmpty(errorCorrection, "errorCorrection");
    this.svg = requireNonEmpty(svg, "svg");
  }

  public String canonicalId() {
    return canonicalId;
  }

  public String literal() {
    return literal;
  }

  public String addressFormat() {
    return addressFormat;
  }

  public int networkPrefix() {
    return networkPrefix;
  }

  public int modules() {
    return modules;
  }

  public int qrVersion() {
    return qrVersion;
  }

  public String errorCorrection() {
    return errorCorrection;
  }

  public String svg() {
    return svg;
  }

  private static String requireNonEmpty(final String value, final String label) {
    final String trimmed = Objects.requireNonNull(value, label).trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(label + " must not be blank");
    }
    return trimmed;
  }

  private static int requirePositive(final int value, final String label) {
    if (value <= 0) {
      throw new IllegalArgumentException(label + " must be positive");
    }
    return value;
  }
}
