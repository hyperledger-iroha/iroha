package org.hyperledger.iroha.android.crypto;

/** Supported transaction and offline signing algorithms exposed by the Java/Android SDKs. */
public enum SigningAlgorithm {
  ED25519(0, "Ed25519", "ed25519"),
  ML_DSA(4, "MlDsa", "ml-dsa");

  private final int bridgeCode;
  private final String providerName;
  private final String wireName;

  SigningAlgorithm(final int bridgeCode, final String providerName, final String wireName) {
    this.bridgeCode = bridgeCode;
    this.providerName = providerName;
    this.wireName = wireName;
  }

  public int bridgeCode() {
    return bridgeCode;
  }

  public String providerName() {
    return providerName;
  }

  public String wireName() {
    return wireName;
  }

  public boolean supportsHardwareBackedKeys() {
    return this == ED25519;
  }

  public static SigningAlgorithm fromAlgorithmName(final String name) {
    final String normalized = normalize(name);
    if ("ed25519".equals(normalized) || "eddsa".equals(normalized)) {
      return ED25519;
    }
    if ("mldsa".equals(normalized)
        || "mldsa65".equals(normalized)
        || "mldsa44".equals(normalized)
        || "mldsa87".equals(normalized)) {
      return ML_DSA;
    }
    return ED25519;
  }

  public static SigningAlgorithm fromBridgeCode(final int code) {
    for (final SigningAlgorithm algorithm : values()) {
      if (algorithm.bridgeCode == code) {
        return algorithm;
      }
    }
    throw new IllegalArgumentException("Unsupported signing algorithm code: " + code);
  }

  private static String normalize(final String name) {
    if (name == null || name.isBlank()) {
      return ED25519.wireName;
    }
    final StringBuilder builder = new StringBuilder(name.length());
    for (int i = 0; i < name.length(); i++) {
      final char ch = Character.toLowerCase(name.charAt(i));
      if (Character.isLetterOrDigit(ch)) {
        builder.append(ch);
      }
    }
    return builder.toString();
  }
}
